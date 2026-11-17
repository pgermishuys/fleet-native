//! Improve a skill, piece U of docs/parity/09-app-frame.md: `ImproveSkillDialog.test.ts` (4 cases) under its names, run
//! against a fake Fleet (`tests_skills::fake`), and the rows Fleet's tests don't name (the stages, Esc, the token lines,
//! Undo failing). The fake's skill is `tidy-docs`: its own text is "a", the model's proposal "b".
use crate::fleet::improve::Stage;
use crate::fleet::link::Job;
use crate::mock::improve::Case;
use crate::tests::{FRAME, click};
use crate::tests_skills::{Heard, calls, fake, rig};
use crate::{State, test_wait};
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::json;
use std::sync::mpsc::Receiver;

type Rt = Runtime<State>;

fn all_text(rt: &Rt) -> String {
    rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

fn has(rt: &Rt, key: &str) -> bool {
    rt.find(key).is_some()
}

fn type_into(rt: &mut Rt, key: &str, words: &str) {
    click(rt, key);
    rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn wait(rt: &mut Rt, queue: &Receiver<Job>, what: &str, done: impl FnMut(&State) -> bool) {
    test_wait::drive(rt, queue, what, done, |s| format!("stage {:?}, error {:?}", s.skills.dialog.stage, s.skills.dialog.error));
}

/// The dialog's `dialog()`: the open session is a server's, a tool call of `tidy-docs` finished in it, Improve pressed, the
/// skill's text read.
fn opened() -> (Rt, Receiver<Job>, Heard) {
    let (base, heard) = fake();
    let (mut rt, queue) = rig(base);
    rt.update(|s, _| {
        s.show_conversation(crate::mock::tools::of_skills());
        s.projects[0].sessions[0].id = "s-1".into();
        s.route = crate::fleet::store::Route::Session("s-1".into());
        s.skills.list = Some(crate::fleet::skills::mock_skills());
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    click(&mut rt, "tool-k1-improve");
    wait(&mut rt, &queue, "the skill's text", |s| s.skills.dialog.detail.is_some());
    rt.frame(FRAME);
    rt.frame(FRAME);
    (rt, queue, heard)
}

const EXCHANGE: &str = "The user asked:\nBring the docs up to date with the new totals code.";

// ImproveSkillDialog.test.ts: "sends the note and only the parts of the session that are ticked"
#[test]
fn sends_the_note_and_only_the_parts_of_the_session_that_are_ticked() {
    let (mut rt, queue, heard) = opened();
    assert!(all_text(&rt).contains("Fleet's"));
    type_into(&mut rt, "improve-skill-note", "Don't report style.");
    click(&mut rt, "improve-skill-ask");
    wait(&mut rt, &queue, "the proposal", |s| s.skills.dialog.stage == Stage::Proposal);
    rt.frame(FRAME);
    rt.frame(FRAME);

    let sent = calls(&heard, "POST", "/improve");
    assert_eq!(sent[0].2, json!({"sessionId": "s-1", "note": "Don't report style.", "context": EXCHANGE}));
    assert!(has(&rt, "improve-skill-diff") && all_text(&rt).contains("b"), "the diff shows the proposal");
    assert!(all_text(&rt).contains("The question used 10 tokens (4 from the cache)."));
}

// ImproveSkillDialog.test.ts: "sends no parts when it reads the whole conversation"
#[test]
fn sends_no_parts_when_it_reads_the_whole_conversation() {
    let (mut rt, queue, heard) = opened();
    click(&mut rt, "improve-skill-include-tools");
    click(&mut rt, "improve-skill-whole-conversation");
    // The parts can't be picked while the whole conversation is: the press does nothing.
    click(&mut rt, "improve-skill-include-exchange");
    assert!(rt.state.skills.dialog.exchange, "its own tick is untouched");
    assert!(all_text(&rt).contains("+ the session"));
    type_into(&mut rt, "improve-skill-note", "No style.");
    click(&mut rt, "improve-skill-ask");
    wait(&mut rt, &queue, "the proposal", |s| s.skills.dialog.stage == Stage::Proposal);

    let sent = &calls(&heard, "POST", "/improve")[0].2;
    assert_eq!(sent["wholeConversation"], json!(true));
    assert!(sent.get("context").is_none(), "no context key: {sent}");
}

// ImproveSkillDialog.test.ts: "keeps the change, edited by hand, as the next version and offers Undo"
#[test]
fn keeps_the_change_edited_by_hand_as_the_next_version_and_offers_undo() {
    let (mut rt, queue, heard) = opened();
    type_into(&mut rt, "improve-skill-note", "No style.");
    click(&mut rt, "improve-skill-ask");
    wait(&mut rt, &queue, "the proposal", |s| s.skills.dialog.stage == Stage::Proposal);
    rt.frame(FRAME);
    rt.frame(FRAME);

    click(&mut rt, "improve-skill-edit");
    rt.frame(FRAME);
    click(&mut rt, "improve-skill-editor");
    rt.key(KeyEvent { key: Key::End, mods: Mods { ctrl: true, ..Mods::default() } });
    rt.key(KeyEvent { key: Key::Text("Mine too.".into()), mods: Mods::default() });
    rt.frame(FRAME);
    assert_eq!(rt.state.skills.dialog.draft.text, "b\nMine too.");
    click(&mut rt, "improve-skill-keep");
    wait(&mut rt, &queue, "the notice", |s| s.notices.has("skill-version-tidy-docs-1"));

    let kept = &calls(&heard, "POST", "/versions")[0].2;
    assert_eq!(*kept, json!({"content": "b\nMine too.", "note": "No style.", "sessionId": "s-1"}));
    assert!(!rt.state.skills.dialog.open, "the dialog closes");
    rt.frame(FRAME);
    assert!(!has(&rt, "improve-skill-dialog"));
    let notice = rt.state.notices.get("skill-version-tidy-docs-1").unwrap().clone();
    assert_eq!(notice.title, "tidy-docs is now your version 1");
    assert_eq!(notice.body.as_deref(), Some("Sessions you start from now on use it."));
    assert!(notice.countdown && notice.chip.is_none());
    assert_eq!(rt.state.skills.list.as_ref().unwrap().iter().find(|s| s.name == "tidy-docs").unwrap().version, Some(1));

    // Undo puts back what sessions got before: Fleet's own.
    let run = notice.actions[0].run.clone();
    rt.update(move |s, _| s.notices.tell(&run));
    rt.update(|s, _| s.keep_skill_undo());
    wait(&mut rt, &queue, "the notice to close", |s| !s.notices.has("skill-version-tidy-docs-1"));
    assert_eq!(calls(&heard, "PUT", "/active")[0].2, json!({"version": null}));
}

// ImproveSkillDialog.test.ts: "shows why the model couldn't propose a change and lets the note be changed"
#[test]
fn shows_why_the_model_couldnt_propose_a_change_and_lets_the_note_be_changed() {
    let (mut rt, queue, _) = opened();
    type_into(&mut rt, "improve-skill-note", "refuse");
    click(&mut rt, "improve-skill-ask");
    wait(&mut rt, &queue, "the refusal", |s| s.skills.dialog.error.is_some());
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(all_text(&rt).contains("The model is busy."));
    assert!(has(&rt, "improve-skill-error"));
    assert_eq!(rt.state.skills.dialog.stage, Stage::Compose);
    // The note is writable again, and the button asks again.
    type_into(&mut rt, "improve-skill-note", " Now say more.");
    assert!(rt.state.skills.dialog.note.text.ends_with(" Now say more."));
    assert!(rt.state.skills.dialog.can_ask());
}

// ---- the rows Fleet's tests don't name ------------------------------------------------------------------------------

fn scene(case: Case) -> Rt {
    let mut rt = crate::tests::app();
    rt.update(move |s, _| s.show_improve(case, true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

#[test]
fn the_form_names_the_skill_its_copy_and_what_the_model_reads_with_what_each_costs() {
    let rt = scene(Case::Fresh);
    let text = all_text(&rt);
    for want in [
        "tidy-docs",
        "Fleet's",
        "Say what it should do differently. The model proposes a change to the skill, and nothing is saved until you keep it.",
        "What should it do differently?",
        "What the model reads",
        "Your note",
        "What you asked, and the reply",
        "The tool calls in that turn",
        "The whole conversation, from a copy of this session",
        "whole session",
        "Asked once, on this session's model",
        "Propose a change",
        "Cancel",
    ] {
        assert!(text.contains(want), "missing {want:?} in {text}");
    }
    // 250 for the rules, the skill's 5 lines, no note, the exchange.
    let d = &rt.state.skills.dialog;
    assert_eq!(d.total_tokens(), 250 + d.skill_tokens() + d.exchange_tokens());
    assert!(d.exchange && !d.tools, "the exchange starts ticked, the tool calls not");
    assert!(!d.can_ask(), "Propose a change waits for a note");
}

#[test]
fn the_total_follows_the_ticks_and_the_note() {
    let mut rt = scene(Case::Fresh);
    let before = rt.state.skills.dialog.total_tokens();
    click(&mut rt, "improve-skill-include-tools");
    let with_tools = rt.state.skills.dialog.total_tokens();
    assert_eq!(with_tools, before + rt.state.skills.dialog.tool_tokens());
    click(&mut rt, "improve-skill-include-exchange");
    assert_eq!(rt.state.skills.dialog.total_tokens(), with_tools - rt.state.skills.dialog.exchange_tokens());
    click(&mut rt, "improve-skill-whole-conversation");
    assert_eq!(rt.state.skills.dialog.total_tokens(), 250 + rt.state.skills.dialog.skill_tokens());
    assert!(all_text(&rt).contains("tokens + the session"));
}

#[test]
fn control_enter_in_the_note_asks_and_enter_alone_does_not() {
    let mut rt = scene(Case::Filled);
    click(&mut rt, "improve-skill-note");
    rt.key(KeyEvent { key: Key::Enter, mods: Mods::default() });
    rt.frame(FRAME);
    assert_eq!(rt.state.skills.dialog.stage, Stage::Compose);
    assert!(rt.state.skills.dialog.error.is_none());
    rt.key(KeyEvent { key: Key::Enter, mods: Mods { ctrl: true, cmd: true, ..Mods::default() } });
    rt.frame(FRAME);
    // No server in a scene: the ask comes straight back with Fleet's own words for it.
    assert_eq!(rt.state.skills.dialog.error.as_deref(), Some("Couldn't ask for a better tidy-docs."));
}

#[test]
fn asking_shows_the_wait_and_locks_the_form() {
    let mut rt = scene(Case::Asking);
    assert!(all_text(&rt).contains("Asking the model\u{2026}"));
    click(&mut rt, "improve-skill-whole-conversation");
    assert!(!rt.state.skills.dialog.whole, "the ticks are locked while the model is asked");
    click(&mut rt, "improve-skill-ask");
    assert_eq!(rt.state.skills.dialog.stage, Stage::Asking);
}

#[test]
fn the_proposal_quotes_the_note_and_says_which_version_keeping_makes() {
    let rt = scene(Case::Proposal);
    let text = all_text(&rt);
    assert!(text.contains("Keep it to make it your version 1: sessions you start from now on use it in place of Fleet's."));
    assert!(text.contains("\u{201c}End with the pages you changed, and say so when nothing needed changing.\u{201d}"));
    assert!(text.contains("The question used 2,400 tokens (1,800 from the cache)."));
    assert!(has(&rt, "improve-skill-diff") && has(&rt, "improve-skill-keep") && has(&rt, "improve-skill-change-note"));
    assert!(rt.state.skills.dialog.can_keep());
}

#[test]
fn keep_is_off_while_the_text_is_what_the_change_was_made_to() {
    let mut rt = scene(Case::Proposal);
    rt.update(|s, _| s.skills.dialog.draft = rune::TextEdit::new(crate::view::skill_diff::BEFORE));
    assert!(!rt.state.skills.dialog.can_keep());
}

#[test]
fn edit_by_hand_swaps_the_diff_for_the_text_and_back() {
    let mut rt = scene(Case::Proposal);
    click(&mut rt, "improve-skill-edit");
    assert!(has(&rt, "improve-skill-editor") && !has(&rt, "improve-skill-diff"));
    assert!(all_text(&rt).contains("Show the diff"));
    click(&mut rt, "improve-skill-edit");
    assert!(has(&rt, "improve-skill-diff") && !has(&rt, "improve-skill-editor"));
}

#[test]
fn change_the_note_goes_back_to_the_form_with_the_note_kept() {
    let mut rt = scene(Case::Proposal);
    click(&mut rt, "improve-skill-change-note");
    assert!(has(&rt, "improve-skill-note") && !has(&rt, "improve-skill-keep"));
    assert!(rt.state.skills.dialog.note.text.starts_with("End with the pages"));
}

#[test]
fn escape_and_cancel_close_it() {
    let mut rt = scene(Case::Filled);
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.frame(FRAME);
    assert!(!rt.state.skills.dialog.open && rt.state.skills.improving.is_none());
    let mut rt = scene(Case::Filled);
    click(&mut rt, "improve-skill-cancel");
    assert!(!rt.state.skills.dialog.open);
}

#[test]
fn an_answer_for_a_dialog_that_was_closed_changes_nothing() {
    let (mut rt, queue, _) = opened();
    type_into(&mut rt, "improve-skill-note", "No style.");
    click(&mut rt, "improve-skill-ask");
    click(&mut rt, "improve-skill-cancel");
    // The reply still comes; it must not bring the dialog back or fill it.
    test_wait::drive(&mut rt, &queue, "the answer", |s| !s.skills.asking, test_wait::app_state);
    rt.frame(FRAME);
    assert!(!rt.state.skills.dialog.open && !has(&rt, "improve-skill-dialog"));
    assert!(rt.state.skills.dialog.proposal.is_none());
}

#[test]
fn a_failed_undo_says_so_and_loses_its_button() {
    let mut rt = scene(Case::Kept);
    assert!(rt.state.notices.has("skill-version-tidy-docs-1"));
    rt.update(|s, _| s.skill_undone("skill-version-tidy-docs-1", Err("Couldn't switch tidy-docs.".into())));
    let n = rt.state.notices.get("skill-version-tidy-docs-1").unwrap();
    assert_eq!((n.title.as_str(), n.body.as_deref(), n.actions.len()), ("Couldn't undo it", Some("Couldn't switch tidy-docs."), 0));
}

#[test]
fn the_notice_scene_has_the_card_and_no_dialog() {
    let rt = scene(Case::Kept);
    assert!(has(&rt, "notice-card") || rt.state.notices.open().is_some());
    assert!(!has(&rt, "improve-skill-dialog"));
}
