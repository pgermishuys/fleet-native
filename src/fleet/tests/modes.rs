//! The composer's other modes (docs/parity/05-composer.md, piece M): message history, shell mode and the side question.
//! Ports `Composer.test.ts` "Composer shell commands", "Composer side questions (/btw)" and "Composer with a minimized side
//! conversation" under their names, and adds the history ones Fleet's tests lack.
use super::queue::{Live, click, mock_app, press, typed};
const FRAME: f64 = 1.0 / 60.0;
use crate::State;
use crate::composer::drafts::{side_draft_key, side_main_draft_key};
use crate::fleet::modes::SidePanel;
use crate::mock::Status;
use rune::{Key, Mods, Runtime};
use serde_json::json;

fn plain() -> Mods {
    Mods::default()
}

fn owner(rt: &Runtime<State>) -> String {
    rt.state.draft_owner().unwrap()
}

fn history_of(rt: &Runtime<State>) -> Vec<String> {
    rt.state.drafts.history(&owner(rt)).map(|h| h.entries.clone()).unwrap_or_default()
}

fn with_shell(rt: &mut Runtime<State>) {
    rt.update(|s, _| s.pickers.harnesses[0].supports_shell_commands = true);
    rt.frame(FRAME);
}

fn sent(rt: &mut Runtime<State>, text: &str) {
    typed(rt, text);
    press(rt, Key::Enter, plain());
}

fn rows_on_screen(rt: &Runtime<State>) -> usize {
    (0..20).filter(|i| rt.find(&format!("history-row{i}")).is_some()).count()
}

// ─── message history (UIH, `handleKeydown`) ─────────────────────────────────

// new: "opens the history on ArrowUp in an empty draft, newest selected, and Enter fills the draft without sending"
#[test]
fn history_opens_on_arrow_up_in_an_empty_draft_and_enter_fills_without_sending() {
    let mut rt = mock_app(false, "opencode", false, false);
    sent(&mut rt, "first question");
    sent(&mut rt, "second question");
    let before = rt.state.session().messages.len();
    click(&mut rt, "composer");
    press(&mut rt, Key::Up, plain());
    assert_eq!(rows_on_screen(&rt), 2);
    assert_eq!(rt.state.history_view().map(|(_, i)| i), Some(0), "the newest is selected");
    press(&mut rt, Key::Enter, plain());
    assert_eq!(rt.state.draft.text, "second question");
    assert!(rt.state.history_view().is_none());
    assert_eq!(rt.state.session().messages.len(), before, "Enter fills the text area and does not send");
}

// new: "does nothing on ArrowUp with no entries, or in a draft that has text"
#[test]
fn arrow_up_does_nothing_with_no_entries_or_a_draft_that_has_text() {
    let mut rt = mock_app(false, "opencode", false, false);
    click(&mut rt, "composer");
    press(&mut rt, Key::Up, plain());
    assert!(rt.state.history_view().is_none());
    sent(&mut rt, "remembered");
    typed(&mut rt, "typing");
    press(&mut rt, Key::Up, plain());
    assert!(rt.state.history_view().is_none());
}

// new: "moves to older entries with ArrowUp and newer with ArrowDown, clamped, and closes on Escape or any other key"
#[test]
fn history_moves_clamped_and_closes_on_escape_or_any_other_key() {
    let mut rt = mock_app(false, "opencode", false, false);
    for text in ["one", "two", "three"] {
        sent(&mut rt, text);
    }
    click(&mut rt, "composer");
    press(&mut rt, Key::Up, plain());
    for _ in 0..5 {
        press(&mut rt, Key::Up, plain());
    }
    assert_eq!(rt.state.history_view().map(|(_, i)| i), Some(2), "the oldest, no wrap");
    for _ in 0..5 {
        press(&mut rt, Key::Down, plain());
    }
    assert_eq!(rt.state.history_view().map(|(_, i)| i), Some(0), "the newest, no wrap");
    press(&mut rt, Key::Escape, plain());
    assert!(rt.state.history_view().is_none());
    assert_eq!(rt.state.draft.text, "");
    press(&mut rt, Key::Up, plain());
    assert!(rt.state.history_view().is_some());
    press(&mut rt, Key::Text("x".into()), plain());
    assert!(rt.state.history_view().is_none(), "any other key closes it");
    assert_eq!(rt.state.draft.text, "x", "and goes on to be typed");
}

// new: "keeps ten entries, the repeat of one moving to the front"
#[test]
fn history_keeps_ten_entries_and_a_repeat_moves_to_the_front() {
    let mut rt = mock_app(false, "opencode", false, false);
    for n in 1..=12 {
        sent(&mut rt, &format!("message {n}"));
    }
    assert_eq!(history_of(&rt).len(), 10);
    sent(&mut rt, "message 5");
    assert_eq!(history_of(&rt)[0], "message 5");
    assert_eq!(history_of(&rt).len(), 10);
    click(&mut rt, "composer");
    press(&mut rt, Key::Up, plain());
    assert_eq!(rows_on_screen(&rt), 10);
}

// new: "picks an entry with a click and highlights the one under the pointer"
#[test]
fn history_picks_with_a_click_and_the_text_area_keeps_focus() {
    let mut rt = mock_app(false, "opencode", false, false);
    sent(&mut rt, "older one");
    sent(&mut rt, "newer one");
    click(&mut rt, "composer");
    press(&mut rt, Key::Up, plain());
    let (x, y) = rt.find("history-row1").unwrap();
    rt.pointer_move(x, y);
    rt.frame(FRAME);
    assert_eq!(rt.state.history_view().map(|(_, i)| i), Some(1), "mouseenter selects");
    click(&mut rt, "history-row1");
    assert_eq!(rt.state.draft.text, "older one");
    assert!(rt.state.history_view().is_none());
}

// UIH: "records what a shell command and a side question send, too"
#[test]
fn history_records_shell_and_side_sends() {
    let mut rt = mock_app(false, "opencode", false, false);
    with_shell(&mut rt);
    sent(&mut rt, "!ls");
    sent(&mut rt, "/btw why?");
    assert_eq!(history_of(&rt), ["/btw why?", "!ls"]);
}

// ─── Composer shell commands ────────────────────────────────────────────────

// "runs a draft that starts with ! as a shell command, not a prompt"
#[test]
fn runs_a_draft_that_starts_with_bang_as_a_shell_command_not_a_prompt() {
    let mut live = Live::start(json!([]), None, "opencode", false);
    live.rt.update(|s, _| {
        s.pickers.harnesses[0].supports_shell_commands = true;
        s.pickers.harnesses_loaded = true;
        s.projects[0].sessions[0].status = Status::Idle;
    });
    typed(&mut live.rt, "!git status");
    assert!(live.rt.find("composer-shell-mode").is_some());
    press(&mut live.rt, Key::Enter, plain());
    live.wait(&|l| l.calls().len() == 1, "the shell request");
    let calls = live.calls();
    assert_eq!((calls[0].0.as_str(), calls[0].1.as_str()), ("POST", &*format!("/api/sessions/{}/shell", live.id)));
    assert_eq!(calls[0].2, json!({"command": "git status"}));
    assert_eq!(live.rt.state.draft.text, "", "the draft clears at once");
    assert!(!live.calls().iter().any(|c| c.1.ends_with("/prompt")));
}

// "leaves ! as plain text for a harness that can't run shell commands"
#[test]
fn leaves_bang_as_plain_text_for_a_harness_that_cant_run_shell_commands() {
    let mut rt = mock_app(false, "claude-code", false, false);
    let before = rt.state.session().messages.len();
    typed(&mut rt, "!ls");
    assert!(rt.find("composer-shell-mode").is_none());
    press(&mut rt, Key::Enter, plain());
    assert_eq!(rt.state.session().messages.len(), before + 1, "it went as a prompt");
}

// "doesn't send a ! draft as a prompt before it knows whether the harness runs commands"
#[test]
fn doesnt_send_a_bang_draft_as_a_prompt_before_it_knows_whether_the_harness_runs_commands() {
    let mut live = Live::start(json!([]), None, "opencode", false);
    live.rt.update(|s, _| s.projects[0].sessions[0].status = Status::Idle);
    typed(&mut live.rt, "!git status");
    press(&mut live.rt, Key::Enter, plain());
    assert!(live.calls().is_empty() && live.rt.state.draft.text == "!git status", "held, not sent");
    assert!(live.rt.state.shell_support_pending());
    live.rt.update(|s, _| {
        s.pickers.harnesses[0].supports_shell_commands = true;
        s.pickers.harnesses_loaded = true;
    });
    live.rt.frame(FRAME);
    assert!(live.rt.find("composer-shell-mode").is_some());
    press(&mut live.rt, Key::Enter, plain());
    live.wait(&|l| l.calls().len() == 1, "the shell request");
    assert!(live.calls()[0].1.ends_with("/shell"));
}

// "goes back to a prompt on Escape, keeping what was typed"
#[test]
fn goes_back_to_a_prompt_on_escape_keeping_what_was_typed() {
    let mut rt = mock_app(false, "opencode", false, false);
    with_shell(&mut rt);
    typed(&mut rt, "!npm test");
    assert!(rt.find("composer-shell-mode").is_some());
    press(&mut rt, Key::Escape, plain());
    assert_eq!(rt.state.draft.text, "npm test");
    assert!(rt.find("composer-shell-mode").is_none());
}

// "offers no @ references or / commands in a shell command"
#[test]
fn offers_no_references_in_a_shell_command() {
    let mut rt = mock_app(false, "opencode", false, false);
    with_shell(&mut rt);
    typed(&mut rt, "!cat @src");
    rt.frame(FRAME);
    assert!(rt.state.auto.popup.is_none());
    assert!(rt.state.draft_references().is_empty());
}

// "queues a command typed during a turn with Fleet, which runs it when the turn ends"
#[test]
fn queues_a_shell_command_typed_during_a_turn() {
    let mut rt = mock_app(true, "opencode", true, false);
    with_shell(&mut rt);
    typed(&mut rt, "!git status");
    assert!(rt.find("prompt-send-now-button").is_none(), "nothing offers to send it into the turn");
    press(&mut rt, Key::Enter, plain());
    let queued = rt.state.queued().to_vec();
    assert_eq!(
        queued.iter().map(|i| (i.text.as_str(), i.kind)).collect::<Vec<_>>(),
        [("!git status", crate::fleet::queue::ItemKind::Shell)]
    );
}

// "puts a refused command back and says why"
#[test]
fn puts_a_refused_command_back_and_says_why() {
    let refusal = json!({"error": "The agent is working. Run the command when its turn ends."});
    let mut live = Live::start(json!([]), Some((409, refusal)), "opencode", false);
    live.rt.update(|s, _| {
        s.pickers.harnesses[0].supports_shell_commands = true;
        s.pickers.harnesses_loaded = true;
        s.projects[0].sessions[0].status = Status::Idle;
    });
    typed(&mut live.rt, "!git status");
    press(&mut live.rt, Key::Enter, plain());
    live.wait(&|l| l.rt.state.composer_error.is_some(), "the refusal");
    assert_eq!(live.rt.state.draft.text, "!git status");
    assert_eq!(live.rt.state.composer_error.as_deref(), Some("The agent is working. Run the command when its turn ends."));
}

// new: "a command refused doesn't overwrite what was typed since"
#[test]
fn a_refused_command_doesnt_overwrite_what_was_typed_since() {
    let mut rt = mock_app(false, "opencode", false, false);
    rt.update(|s, _| {
        let id = s.draft_owner().unwrap();
        s.draft = rune::TextEdit::new("something new");
        s.on_shell_answer(&id, "!ls".into(), Ok((409, json!({"error": "no"}))));
    });
    assert_eq!(rt.state.draft.text, "something new");
    assert_eq!(rt.state.composer_error.as_deref(), Some("no"));
}

// new: "an empty command does nothing" (row 9.3)
#[test]
fn an_empty_shell_command_does_nothing() {
    let mut rt = mock_app(false, "opencode", false, false);
    with_shell(&mut rt);
    typed(&mut rt, "!  ");
    press(&mut rt, Key::Enter, plain());
    assert!(history_of(&rt).is_empty());
    assert_eq!(rt.state.draft.text, "!  ");
}

// ─── Composer side questions (/btw) ─────────────────────────────────────────

// "asks /btw <question> in the side conversation, not the session, even while the session works"
#[test]
fn asks_btw_in_the_side_conversation_not_the_session_even_while_the_session_works() {
    let mut rt = mock_app(true, "opencode", true, false);
    rt.update(|s, _| s.side_panel = SidePanel::Accepts);
    let before = rt.state.session().messages.len();
    typed(&mut rt, "/btw what changed?");
    press(&mut rt, Key::Enter, plain());
    assert_eq!(rt.state.side_asked.iter().map(|a| a.question.as_str()).collect::<Vec<_>>(), ["what changed?"]);
    assert_eq!(rt.state.session().messages.len(), before);
    assert!(rt.state.queued().is_empty());
    assert_eq!(rt.state.draft.text, "");
}

// "sends what's typed to the open side conversation"
#[test]
fn sends_whats_typed_to_the_open_side_conversation() {
    let mut rt = mock_app(false, "opencode", false, false);
    rt.update(|s, _| {
        s.side_panel = SidePanel::Accepts;
        s.side_open = true;
    });
    rt.frame(FRAME);
    assert!(rt.find("composer-side-mode").is_some());
    typed(&mut rt, "and the tests?");
    press(&mut rt, Key::Enter, plain());
    assert_eq!(rt.state.side_asked.iter().map(|a| a.question.as_str()).collect::<Vec<_>>(), ["and the tests?"]);
}

// "asks for a question after a bare /btw"
#[test]
fn asks_for_a_question_after_a_bare_btw() {
    let mut rt = mock_app(false, "opencode", false, false);
    rt.update(|s, _| s.side_panel = SidePanel::Accepts);
    typed(&mut rt, "/btw");
    click(&mut rt, "prompt-send-button");
    assert!(rt.state.side_asked.is_empty());
    assert_eq!(rt.state.composer_error.as_deref(), Some("Type a question after /btw."));
    assert_eq!(rt.state.draft.text, "/btw");
}

// "puts a refused question back and says why" (the panel is component 15's: a stand-in refuses)
#[test]
fn puts_a_refused_question_back_and_says_why() {
    let mut rt = mock_app(false, "opencode", false, false);
    rt.update(|s, _| s.side_panel = SidePanel::Refuses);
    typed(&mut rt, "/btw what changed?");
    press(&mut rt, Key::Enter, plain());
    assert_eq!(rt.state.draft.text, "/btw what changed?");
    assert!(rt.state.composer_error.is_some());
}

// new: "says the panel isn't built yet and keeps the draft" (until component 15)
#[test]
fn says_the_side_conversation_isnt_built_yet_and_keeps_the_draft() {
    let mut rt = mock_app(false, "opencode", false, false);
    typed(&mut rt, "/btw what changed?");
    press(&mut rt, Key::Enter, plain());
    assert_eq!(rt.state.draft.text, "/btw what changed?");
    assert!(rt.state.ui.toast.is_some());
}

// "the question carries the agent, model and effort picked" (row 9.5)
#[test]
fn the_question_carries_the_choice_picked() {
    let mut rt = mock_app(false, "opencode", false, false);
    rt.update(|s, _| {
        s.side_panel = SidePanel::Accepts;
        let owner = s.draft_owner().unwrap();
        s.drafts.set_choice(&owner, crate::composer::drafts::Choice { effort: "high".into(), ..Default::default() });
    });
    typed(&mut rt, "/btw why?");
    press(&mut rt, Key::Enter, plain());
    assert_eq!(rt.state.side_asked.len(), 1);
    assert_eq!(rt.state.side_asked[0].pick.effort.as_deref(), Some("high"));
}

// ─── Composer with a minimized side conversation ────────────────────────────

// "keeps each side's draft when the side conversation opens and folds"
#[test]
fn keeps_each_sides_draft_when_the_side_conversation_opens_and_folds() {
    let mut rt = mock_app(false, "opencode", false, false);
    rt.update(|s, _| s.side_id = Some("side-1".into()));
    typed(&mut rt, "for the session");
    rt.update(|s, _| s.side_open = true);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("composer-side-mode").is_some());
    assert_eq!(rt.state.draft.text, "");
    typed(&mut rt, "for the side");
    rt.update(|s, _| s.side_open = false);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.draft.text, "for the session");
    rt.update(|s, _| s.side_open = true);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.draft.text, "for the side");
}

// "keeps the put-away draft through a reload": the put-away text is in the drafts file under Fleet's keys
#[test]
fn keeps_the_put_away_draft_in_the_drafts_file_under_fleets_keys() {
    let mut rt = mock_app(false, "opencode", false, false);
    rt.update(|s, _| s.side_id = Some("side-1".into()));
    typed(&mut rt, "for the session");
    rt.update(|s, _| s.side_open = true);
    rt.frame(FRAME);
    rt.frame(FRAME);
    let owner = owner(&rt);
    assert_eq!(rt.state.drafts.read(&side_main_draft_key(&owner)), Some("for the session"));
    typed(&mut rt, "for the side");
    rt.update(|s, _| s.side_open = false);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.drafts.read(&side_draft_key(&owner, "side-1")), Some("for the side"));
    assert_eq!(rt.state.drafts.read(&side_main_draft_key(&owner)).unwrap_or(""), "");
}

// new: "a side conversation that is another one doesn't swap" (only one side changing mode counts)
#[test]
fn a_different_side_conversation_doesnt_swap_the_drafts() {
    let mut rt = mock_app(false, "opencode", false, false);
    rt.update(|s, _| s.side_id = Some("side-1".into()));
    typed(&mut rt, "kept");
    rt.update(|s, _| {
        s.side_id = Some("side-2".into());
        s.side_open = true;
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.draft.text, "kept");
}
