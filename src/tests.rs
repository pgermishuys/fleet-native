//! The shell driven with no window: a runtime built as a headless render builds it, then
//! clicked and typed at by element key. New behaviour gets a test here (or in a `tests.rs`
//! beside its module) before it gets a snapshot.
use crate::mock::Status;
use crate::view::conversation::{Part, Role};
use crate::{Rail, State, send, spec, submit};
use rune::{App, Env, Key, KeyEvent, Mods, Runtime, TextEdit};

const FRAME: f64 = 1.0 / 60.0;

fn app() -> Runtime<State> {
    let mut rt = App::new(spec()).headless_runtime((1280.0, 760.0), Env::new(true, 1.0));
    // Two frames: the sheet measures itself on the first and lays out to it on the second.
    rt.frame(0.0);
    rt.frame(FRAME);
    rt
}

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(FRAME);
}

fn command(rt: &mut Runtime<State>, c: char) {
    rt.key(KeyEvent { key: Key::Command(c), mods: Mods::default() });
    rt.frame(FRAME);
}

fn run(rt: &mut Runtime<State>, seconds: f64) {
    for _ in 0..(seconds / FRAME).ceil() as usize {
        rt.frame(FRAME);
    }
}

#[test]
fn every_scene_draws_a_frame() {
    for (name, scene) in spec().scenes {
        let mut rt = app();
        rt.update(|s, _| scene(s));
        rt.frame(FRAME);
        assert!(rt.find("rail-sessions").is_some(), "scene {name} lost the rail");
        assert!(rt.find("status-bar").is_some(), "scene {name} lost the status bar");
    }
}

#[test]
fn clicking_a_session_opens_it() {
    let mut rt = app();
    assert_eq!(rt.state.open, (0, 0));
    click(&mut rt, "session-0-1");
    assert_eq!(rt.state.open, (0, 1));
}

#[test]
fn clicking_a_project_folds_it() {
    let mut rt = app();
    assert!(rt.find("session-1-0").is_some());
    click(&mut rt, "project-1");
    assert!(!rt.state.projects[1].open);
    assert!(rt.find("session-1-0").is_none());
}

#[test]
fn the_sidebar_key_hides_the_session_list() {
    let mut rt = app();
    assert!(rt.find("session-list").is_some());
    command(&mut rt, 'b');
    assert!(!rt.state.sidebar_open);
    // The list slides out, so give the exit time to finish.
    run(&mut rt, 1.0);
    assert!(rt.find("session-list").is_none());
}

#[test]
fn next_and_previous_wrap_around_the_list() {
    let mut rt = app();
    command(&mut rt, '[');
    let last = rt.state.projects.len() - 1;
    assert_eq!(rt.state.open, (last, rt.state.projects[last].sessions.len() - 1));
    command(&mut rt, ']');
    assert_eq!(rt.state.open, (0, 0));
}

#[test]
fn a_rail_entry_takes_the_whole_context_area() {
    let mut rt = app();
    click(&mut rt, "rail-automations");
    assert_eq!(rt.state.rail, Rail::Automations);
    // The rail rule: no view keeps another view's panel.
    assert!(rt.find("session-list").is_none());
}

#[test]
fn a_sent_message_gets_its_canned_reply() {
    let mut rt = app();
    rt.update(|s, tx| {
        s.open = (0, 1);
        s.draft = TextEdit::new("  Ship it  ");
        send(s, tx);
    });
    let session = &rt.state.projects[0].sessions[1];
    let last = session.messages.last().unwrap();
    assert_eq!(last.role, Role::User);
    assert!(matches!(&last.parts[..], [Part::Text { text, .. }] if text == "Ship it"));
    assert!(session.status == Status::Working);
    assert!(rt.state.draft.text.is_empty());

    run(&mut rt, 3.0);
    let session = &rt.state.projects[0].sessions[1];
    assert_eq!(session.messages.last().unwrap().role, Role::Assistant);
    assert!(session.status == Status::Idle);
}

#[test]
fn an_empty_draft_sends_nothing() {
    let mut rt = app();
    let before = rt.state.session().messages.len();
    rt.update(|s, tx| {
        s.draft = TextEdit::new("   ");
        send(s, tx);
    });
    assert_eq!(rt.state.session().messages.len(), before);
}

#[test]
fn escape_interrupts_the_working_session() {
    let mut rt = app();
    assert!(rt.state.session().status == Status::Working);
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.frame(FRAME);
    assert!(rt.state.session().status == Status::Idle);
}

/// Opens the long conversation with its scripted reply held back.
fn long() -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| s.open_long());
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

#[test]
fn the_long_conversation_has_two_thousand_messages() {
    let rt = long();
    assert_eq!(rt.state.session().title, "Long conversation");
    assert_eq!(rt.state.session().messages.len(), 2_000);
}

#[test]
fn only_the_rows_on_screen_are_built() {
    let mut rt = long();
    // Pinned to the end: the last message is drawn, the first never was.
    assert!(rt.find("a999-p0").is_some(), "the end of the conversation isn't on screen");
    assert!(rt.find("a0-p0").is_none(), "the first message was built");
    // Scroll a long way up: the end goes away and rows near the middle come in.
    for _ in 0..400 {
        rt.wheel(640.0, 400.0, 600.0);
        rt.frame(FRAME);
    }
    assert!(rt.find("a999-p0").is_none(), "the end stayed built after scrolling away");
    assert_eq!(rt.scroll_at_bottom("stream"), Some(false));
}

#[test]
fn a_streaming_reply_grows_and_the_view_stays_at_the_end() {
    let mut rt = app();
    rt.update(|s, _| {
        s.open_long();
        s.script_next = 0.0;
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    let len = |rt: &Runtime<State>| match rt.state.session().messages.last().and_then(|m| m.parts.last()) {
        Some(Part::Text { text, .. }) => text.len(),
        _ => panic!("the reply has no text part"),
    };
    let before = len(&rt);
    run(&mut rt, 1.0);
    let after = len(&rt);
    assert!(after > before + 200, "{before} then {after}: the reply didn't stream");
    assert_eq!(rt.scroll_at_bottom("stream"), Some(true), "the view left the end of the conversation");
    assert_eq!(rt.state.session().status, Status::Working);
    // The whole reply arrives, the turn ends, and a few seconds later the next one starts.
    run(&mut rt, 8.0);
    assert_eq!(rt.state.session().status, Status::Idle);
    let before = rt.state.session().messages.len();
    run(&mut rt, 3.0);
    assert!(rt.state.session().messages.len() > before, "the script didn't ask its next question");
}

#[test]
fn a_person_scrolled_up_is_left_alone_while_a_reply_streams() {
    let mut rt = app();
    rt.update(|s, _| {
        s.open_long();
        s.script_next = 0.0;
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    for _ in 0..30 {
        rt.wheel(640.0, 400.0, 300.0);
        rt.frame(FRAME);
    }
    assert_eq!(rt.scroll_at_bottom("stream"), Some(false));
    run(&mut rt, 1.0);
    assert_eq!(rt.scroll_at_bottom("stream"), Some(false), "streaming pulled the view back down");
}

#[test]
fn a_batch_of_deltas_rebuilds_only_the_last_message() {
    use crate::fleet::reducer::Event;
    let mut rt = app();
    let id = crate::bench::long_from_events(&mut rt);
    let text_at = |rt: &Runtime<State>, n: usize| match &rt.state.session().messages[n].parts[0] {
        Part::Text { text, .. } => (text.as_ptr(), text.len()),
        _ => panic!("not text"),
    };
    let (middle_ptr, _) = text_at(&rt, 1_000);
    let (_, tail_len) = text_at(&rt, 1_999);
    // What the hub sends in a frame: a few dozen deltas to the reply being written.
    let batch: Vec<Event> = (0..40)
        .map(|k| Event::TextDelta {
            session_id: "bench".into(),
            message_id: "msg_01999".into(),
            part_id: "prt_01999".into(),
            delta: format!("word{k} "),
            offset: None,
        })
        .collect();
    rt.update(move |s, tx| s.on_events(id, batch, tx));
    rt.frame(FRAME);
    let (ptr_after, _) = text_at(&rt, 1_000);
    assert_eq!(middle_ptr, ptr_after, "a message the batch didn't touch was copied again");
    assert!(text_at(&rt, 1_999).1 > tail_len, "the reply didn't grow");
    assert_eq!(rt.scroll_at_bottom("stream"), Some(true));
}

#[test]
fn events_leave_a_person_who_scrolled_up_where_they_are() {
    use crate::fleet::reducer::Event;
    let mut rt = app();
    let id = crate::bench::long_from_events(&mut rt);
    for _ in 0..30 {
        rt.wheel(640.0, 400.0, 300.0);
        rt.frame(FRAME);
    }
    assert_eq!(rt.scroll_at_bottom("stream"), Some(false));
    let delta = Event::TextDelta {
        session_id: "bench".into(),
        message_id: "msg_01999".into(),
        part_id: "prt_01999".into(),
        delta: "more ".into(),
        offset: None,
    };
    rt.update(move |s, tx| s.on_events(id, vec![delta], tx));
    rt.frame(FRAME);
    assert_eq!(rt.scroll_at_bottom("stream"), Some(false), "an event pulled the view back to the end");
}

#[test]
fn clicking_the_reads_row_opens_and_closes_the_folded_reads() {
    let mut rt = app();
    rt.update(|s, _| s.show_conversation(crate::mock::tools::of_each_status()));
    rt.frame(FRAME);
    assert!(rt.find("tool-r1").is_none(), "reads are folded until asked");
    click(&mut rt, "reads-reads-r1");
    assert!(rt.find("tool-r1").is_some());
    click(&mut rt, "reads-reads-r1");
    assert!(rt.find("tool-r1").is_none());
}

/// `ToolCard.rows.test.ts` and `ToolOutputBody.test.ts` as the app runs them: a click on a call's row opens its body.
fn output_app(only: &'static [&'static str]) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_conversation(crate::mock::tools::of_output_only(only)));
    rt.frame(FRAME);
    rt
}

#[test]
fn a_closed_card_draws_no_body_and_a_click_on_its_row_opens_and_closes_it() {
    let mut rt = output_app(&["t1"]);
    assert!(rt.find("tool-t1").is_some());
    assert!(rt.find("tool-t1-body").is_none(), "a closed card draws no body");
    click(&mut rt, "tool-t1");
    assert!(rt.find("tool-t1-body").is_some());
    assert!(rt.find("tool-t1-output").is_some());
    click(&mut rt, "tool-t1");
    assert!(rt.find("tool-t1-body").is_none());
}

#[test]
fn a_call_that_failed_starts_open_with_what_it_printed_and_closes_to_a_preview_line() {
    let mut rt = output_app(&["t2"]);
    assert!(rt.find("tool-t2-body").is_some(), "errors start open");
    assert!(rt.find("tool-t2-preview").is_none(), "an open card has no preview line");
    click(&mut rt, "tool-t2");
    assert!(rt.find("tool-t2-body").is_none());
    assert!(rt.find("tool-t2-preview").is_some(), "a closed failed card keeps a line");
}

#[test]
fn a_shell_card_has_one_header_and_one_copy_button() {
    let mut rt = output_app(&["t1"]);
    click(&mut rt, "tool-t1");
    assert!(rt.find("tool-t1-header").is_some());
    assert!(rt.find("tool-t1-copy").is_some());
}

#[test]
fn long_output_shows_the_last_12_lines_until_asked_then_toggles_back() {
    let mut rt = output_app(&["t3"]);
    // The row measures itself after it opens, so the toggles are clicked once it has settled.
    click(&mut rt, "tool-t3");
    run(&mut rt, 0.5);
    assert!(rt.find("tool-t3-expand").is_some());
    assert!(rt.find("tool-t3-collapse").is_none());
    click(&mut rt, "tool-t3-expand");
    run(&mut rt, 0.5);
    assert!(rt.state.tool_wide.contains("t3"));
    assert_eq!(rt.scroll_at_bottom("tool-t3-output"), Some(true), "the box opens on the last lines");
    assert!(rt.find("tool-t3-collapse").is_some());
    assert!(rt.find("tool-t3-expand").is_none());
    click(&mut rt, "tool-t3-collapse");
    run(&mut rt, 0.5);
    assert!(!rt.state.tool_wide.contains("t3"));
    assert!(rt.find("tool-t3-expand").is_some());
}

#[test]
fn a_call_that_printed_nothing_says_so() {
    let mut rt = output_app(&["t4"]);
    click(&mut rt, "tool-t4");
    assert!(rt.find("tool-t4-empty").is_some());
    assert!(rt.find("tool-t4-output").is_none());
}

#[test]
fn a_summary_shows_above_the_output() {
    let mut rt = output_app(&["t5"]);
    click(&mut rt, "tool-t5");
    assert!(rt.find("tool-t5-summary").is_some());
    assert!(rt.find("tool-t5-output").is_some());
    assert!(rt.find("tool-t5-empty").is_none());
}

/// `ToolCard.rows.test.ts` "a write opens on what it wrote even with inline diffs off, twelve lines then Show all", and
/// `DiffView.test.ts` as the app runs it.
fn diff_app(only: &'static [&'static str], inline: bool) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| {
        s.show_conversation(crate::mock::tools::of_diffs(only));
        s.inline_diffs = inline;
    });
    rt.frame(FRAME);
    run(&mut rt, 0.3);
    rt
}

#[test]
fn a_write_opens_on_its_diff_twelve_lines_then_show_all() {
    let mut rt = diff_app(&["w1"], false);
    assert!(rt.find("tool-w1-body").is_none(), "a write starts closed");
    click(&mut rt, "tool-w1");
    run(&mut rt, 0.3);
    assert!(rt.find("tool-w1-diff").is_some(), "and opens on what it wrote, not what it printed");
    assert!(rt.find("tool-w1-diff-line-11").is_some());
    assert!(rt.find("tool-w1-diff-line-12").is_none());
    // The card measures itself after it opens.
    run(&mut rt, 1.0);
    click(&mut rt, "tool-w1-diff-more");
    run(&mut rt, 0.3);
    assert!(rt.state.diff_all.contains("w1"));
    // The longer card runs past the bottom of the view; the rows of a conversation are built as they come into it.
    rt.wheel(640.0, 400.0, 300.0);
    run(&mut rt, 0.3);
    assert!(rt.find("tool-w1-diff-line-12").is_some(), "the rest shows");
    assert!(rt.find("tool-w1-diff-more").is_none());
}

#[test]
fn an_edit_is_a_write_too_and_expands_a_collapsed_run_once_all_its_lines_show() {
    let mut rt = diff_app(&["e1"], false);
    assert!(rt.find("tool-e1-body").is_none());
    click(&mut rt, "tool-e1");
    run(&mut rt, 0.3);
    assert!(rt.find("tool-e1-diff").is_some());
    // Twelve lines first: three kept lines, the change, then the run is cut by the preview, not collapsed.
    assert!(rt.find("tool-e1-diff-expand-6-24").is_none());
    run(&mut rt, 1.0);
    click(&mut rt, "tool-e1-diff-more");
    run(&mut rt, 0.3);
    // 3 context lines, the change (3 lines), then 18 unchanged lines: 3 stay at each edge and 12 hide.
    assert!(rt.find("tool-e1-diff-expand-6-24").is_some());
    click(&mut rt, "tool-e1-diff-expand-6-24");
    run(&mut rt, 0.3);
    assert!(rt.find("tool-e1-diff-expand-6-24").is_none(), "the run is open");
}

#[test]
fn a_row_that_is_not_a_write_shows_a_diff_only_with_inline_diffs_on() {
    let rt = diff_app(&["x1"], false);
    assert!(rt.find("tool-x1-body").is_none());
    let rt = diff_app(&["x1"], true);
    assert!(rt.find("tool-x1-diff").is_some(), "inline diffs open it by themselves");
    assert!(rt.find("tool-x1-diff-more").is_none(), "only a write is cut to twelve lines");
}

// Component 5, piece A: a draft per session.
#[test]
fn the_draft_follows_the_session_when_another_is_opened_and_comes_back_on_return() {
    let mut rt = app();
    rt.update(|s, _| s.draft = TextEdit::new("first idea"));
    rt.frame(FRAME);
    click(&mut rt, "session-0-1");
    assert_eq!(rt.state.open, (0, 1));
    assert_eq!(rt.state.draft.text, "", "the other session starts empty");
    rt.update(|s, _| s.draft = TextEdit::new("second idea"));
    rt.frame(FRAME);
    click(&mut rt, "session-0-0");
    assert_eq!(rt.state.draft.text, "first idea");
    click(&mut rt, "session-0-1");
    assert_eq!(rt.state.draft.text, "second idea");
}

#[test]
fn a_sent_text_is_remembered_for_the_history_and_the_draft_is_gone() {
    let mut rt = app();
    rt.update(|s, tx| {
        s.draft = TextEdit::new("  run the checks  ");
        send(s, tx);
    });
    rt.frame(FRAME);
    assert_eq!(rt.state.draft.text, "");
    let history = rt.state.drafts.history("mock.0.0").expect("a history for the open session");
    assert_eq!(history.entries, ["run the checks"]);
}

#[test]
fn a_restart_brings_the_text_back_and_a_typed_one_wins_over_the_file() {
    use crate::composer::drafts::{Drafts, Stored, session_draft_key};
    let mut before = Drafts::default();
    before.write(&session_draft_key("mock.0.0"), "kept over a restart", 0.0);
    let json = before.take_save();
    assert!(json.is_none(), "with no folder nothing is written");

    let stored = Stored { drafts: [("session.mock.0.0".to_string(), "kept over a restart".to_string())].into(), ..Default::default() };
    let mut rt = app();
    rt.update(move |s, _| s.adopt_drafts(std::env::temp_dir().join("fleet-native-never-written"), stored));
    rt.frame(FRAME);
    assert_eq!(rt.state.draft.text, "kept over a restart");

    let stored = Stored { drafts: [("session.mock.0.0".to_string(), "from the file".to_string())].into(), ..Default::default() };
    let mut rt = app();
    rt.update(|s, _| s.draft = TextEdit::new("typed first"));
    rt.update(move |s, _| s.adopt_drafts(std::env::temp_dir().join("fleet-native-never-written"), stored));
    rt.frame(FRAME);
    assert_eq!(rt.state.draft.text, "typed first");
}

/// Piece 3D: `ReasoningBlock.test.ts`, `ShellCommandBlock.test.ts`, `CompactionDivider.test.ts`, `RecapLine.test.ts`.
fn scene_app(setup: impl FnOnce(&mut State) + 'static) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| setup(s));
    rt.frame(FRAME);
    run(&mut rt, 0.3);
    rt
}

#[test]
fn reasoning_is_folded_to_one_line_by_default_and_opens_on_click() {
    let mut rt = scene_app(|s| s.show_conversation(crate::mock::parts::of_thinking()));
    assert!(rt.find("reasoning-th1-gist").is_some());
    assert!(rt.find("reasoning-th1-label").is_none(), "a closed finished block has no label");
    assert!(rt.find("reasoning-th1-body").is_none());
    click(&mut rt, "reasoning-th1-fold");
    run(&mut rt, 0.3);
    assert!(rt.find("reasoning-th1-body").is_some());
    assert!(rt.find("reasoning-th1-label").is_some());
    assert!(rt.find("reasoning-th1-gist").is_none(), "the gist gives way to the whole block");
    click(&mut rt, "reasoning-th1-fold");
    run(&mut rt, 0.3);
    assert!(rt.find("reasoning-th1-body").is_none());
}

#[test]
fn reasoning_follows_the_newest_sentence_while_live() {
    let rt = scene_app(|s| s.show_live_thinking());
    assert!(rt.find("reasoning-th1-label").is_some(), "a live block says Thinking…");
    assert!(rt.find("reasoning-th1-gist").is_some());
}

#[test]
fn reasoning_shows_every_block_in_full_when_set_to_open_and_nothing_when_hidden() {
    use crate::conv::gist::Thinking;
    let rt = scene_app(|s| {
        s.show_conversation(crate::mock::parts::of_thinking());
        s.thinking = Thinking::Open;
    });
    assert!(rt.find("reasoning-th1-fold").is_none());
    assert!(rt.find("reasoning-th1-body").is_some());
    assert!(rt.find("reasoning-th2-summary").is_some());
    let rt = scene_app(|s| {
        s.show_conversation(crate::mock::parts::of_thinking());
        s.thinking = Thinking::Hidden;
    });
    assert!(rt.find("reasoning-th1").is_none());
    assert!(rt.find("reasoning-th2").is_none());
}

#[test]
fn a_shell_command_is_the_users_block_with_its_state() {
    let rt = scene_app(|s| s.show_conversation(crate::mock::parts::of_shell(&["s1", "s2", "s3", "s5"])));
    for id in ["s1", "s2", "s3", "s5"] {
        assert!(rt.find(&format!("shell-{id}-tool-command")).is_some(), "{id}");
    }
    assert!(rt.find("shell-s1-tool-state").is_some(), "exit 0");
    assert!(rt.find("shell-s2-tool-state").is_some(), "Failed · exit 3");
    assert!(rt.find("shell-s3-tool-state").is_some(), "Running…");
    assert!(rt.find("shell-s3-tool-none").is_none(), "a running command hasn't printed nothing yet");
    assert!(rt.find("shell-s3-tool-output").is_none());
    assert!(rt.find("shell-s5-tool-state").is_none(), "no exit code to say");
    assert!(rt.find("shell-s5-tool-none").is_some(), "a finished command that printed nothing says so");
    assert!(rt.find("shell-s1-tool-output").is_some());
}

#[test]
fn a_shell_commands_long_output_collapses_to_its_last_lines_until_asked() {
    let mut rt = scene_app(|s| s.show_conversation(crate::mock::parts::of_shell(&["s4"])));
    assert!(rt.find("shell-s4-tool-expand").is_some());
    assert!(rt.find("shell-s4-tool-collapse").is_none());
    click(&mut rt, "shell-s4-tool-expand");
    run(&mut rt, 0.3);
    assert!(rt.state.shell_wide.contains("s4-tool"));
    assert!(rt.find("shell-s4-tool-expand").is_none());
    assert!(rt.find("shell-s4-tool-collapse").is_some());
    rt.update(|_, tx| tx.scroll_to_bottom_instant("stream"));
    run(&mut rt, 0.3);
    click(&mut rt, "shell-s4-tool-collapse");
    run(&mut rt, 1.0);
    assert!(rt.find("shell-s4-tool-expand").is_some());
    // A timed out command that the harness cut short says so.
    let rt = scene_app(|s| s.show_conversation(crate::mock::parts::of_shell(&["s6"])));
    assert!(rt.find("shell-s6-tool-truncated").is_some());
}

#[test]
fn a_compaction_divider_keeps_its_summary_behind_a_toggle() {
    let mut rt = scene_app(|s| s.show_conversation(crate::mock::parts::of_compaction()));
    assert!(rt.find("compaction-c1-label").is_some());
    assert!(rt.find("compaction-c1-summary").is_none());
    assert!(rt.find("compaction-c2-toggle").is_none(), "no summary, no toggle");
    click(&mut rt, "compaction-c1-toggle");
    run(&mut rt, 0.3);
    assert!(rt.find("compaction-c1-summary").is_some());
    click(&mut rt, "compaction-c1-toggle");
    run(&mut rt, 0.3);
    assert!(rt.find("compaction-c1-summary").is_none());
}

#[test]
fn the_recap_sits_under_the_conversation_only_when_fleet_wrote_one() {
    let rt = scene_app(|s| s.show_recap());
    assert!(rt.find("recap-line").is_some());
    let rt = scene_app(|s| s.show_conversation(crate::mock::parts::of_thinking()));
    assert!(rt.find("recap-line").is_none());
}

// Component 5, piece B: the frame and the toolbar (`Composer.test.ts`'s core).
fn press_key(rt: &mut Runtime<State>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

/// Types into the open session's text area.
fn typed(rt: &mut Runtime<State>, text: &str) {
    click(rt, "composer");
    press_key(rt, Key::Text(text.into()), Mods::default());
}

fn user_messages(rt: &Runtime<State>) -> usize {
    rt.state.session().messages.iter().filter(|m| m.role == Role::User).count()
}

// "sends on Enter when the popup is closed"
#[test]
fn enter_sends_the_draft_and_the_text_area_keeps_focus() {
    let mut rt = app();
    let before = user_messages(&rt);
    typed(&mut rt, "Ship it");
    assert_eq!(rt.state.draft.text, "Ship it");
    press_key(&mut rt, Key::Enter, Mods::default());
    assert_eq!(user_messages(&rt), before + 1);
    assert_eq!(rt.state.draft.text, "");
    // The caret stayed in the text area: what is typed next lands in it with no click.
    press_key(&mut rt, Key::Text("more".into()), Mods::default());
    assert_eq!(rt.state.draft.text, "more");
}

// "does not intercept Shift+Enter ..."
#[test]
fn shift_enter_adds_a_line_and_sends_nothing() {
    let mut rt = app();
    let before = user_messages(&rt);
    typed(&mut rt, "one");
    press_key(&mut rt, Key::Enter, Mods { shift: true, ..Mods::default() });
    press_key(&mut rt, Key::Text("two".into()), Mods::default());
    assert_eq!(rt.state.draft.text, "one\ntwo");
    assert_eq!(user_messages(&rt), before);
}

#[test]
fn ctrl_enter_is_the_other_choice_and_on_an_idle_harness_it_sends() {
    let mut rt = app();
    rt.update(|s, _| s.open = (0, 2));
    rt.frame(FRAME);
    let before = user_messages(&rt);
    typed(&mut rt, "Ship it");
    press_key(&mut rt, Key::Enter, Mods { cmd: true, ctrl: true, ..Mods::default() });
    assert_eq!(user_messages(&rt), before + 1);
    assert_eq!(rt.state.draft.text, "");
}

#[test]
fn send_is_off_with_nothing_to_send_and_on_with_text() {
    let mut rt = app();
    assert!(rt.state.draft.text.is_empty());
    let before = user_messages(&rt);
    click(&mut rt, "prompt-send-button");
    assert_eq!(user_messages(&rt), before, "a click on a dimmed Send sends nothing");
    typed(&mut rt, "Go");
    click(&mut rt, "prompt-send-button");
    assert_eq!(user_messages(&rt), before + 1);
}

#[test]
fn a_refused_message_puts_the_text_back_and_says_why_in_the_box_above_the_composer() {
    let mut rt = app();
    assert!(rt.find("send-prompt-error").is_none());
    rt.update(|s, _| s.composer_error = Some("HTTP 500".into()));
    rt.frame(FRAME);
    assert!(rt.find("send-prompt-error").is_some());
    // The next send clears it, as Fleet's does when a send begins.
    typed(&mut rt, "again");
    press_key(&mut rt, Key::Enter, Mods::default());
    assert!(rt.find("send-prompt-error").is_none());
}

// "is disabled for errored and archived sessions"
#[test]
fn an_archived_session_takes_no_prompt() {
    let mut rt = app();
    rt.update(|s, _| {
        s.open = (0, 1);
        s.projects[0].sessions[1].row.archived = true;
    });
    rt.frame(FRAME);
    assert!(rt.state.composer_disabled());
    assert!(!rt.state.can_interrupt());
    rt.update(|s, tx| {
        s.draft = TextEdit::new("hello");
        submit(s, tx, false);
    });
    assert_eq!(rt.state.draft.text, "hello", "nothing was sent");
    assert_eq!(rt.state.session().messages.last().unwrap().role, Role::Assistant);
}

#[test]
fn a_session_whose_row_says_it_cannot_prompt_is_disabled() {
    let mut rt = app();
    rt.update(|s, _| s.projects[0].sessions[0].row.can_prompt = Some(false));
    assert!(rt.state.composer_disabled());
}

#[test]
fn interrupt_is_live_while_a_turn_runs_and_while_the_session_waits_and_off_when_idle() {
    let mut rt = app();
    assert!(rt.state.can_interrupt(), "the first session is working");
    rt.update(|s, _| s.open = (0, 1));
    assert!(rt.state.can_interrupt(), "the second waits on a question");
    rt.update(|s, _| s.projects[0].sessions[1].status = Status::Idle);
    assert!(!rt.state.can_interrupt());
}

#[test]
fn an_abort_on_its_way_turns_interrupt_off_until_the_session_is_idle() {
    let mut rt = app();
    rt.update(|s, _| s.aborting = s.draft_owner());
    assert!(!rt.state.can_interrupt());
}

#[test]
fn a_sent_prompt_reads_busy_at_once_and_the_guess_ends_with_the_turn() {
    let mut rt = app();
    rt.update(|s, _| s.open = (0, 2));
    rt.frame(FRAME);
    assert_eq!(rt.state.composer_status(), crate::composer::status::ComposerStatus::Idle);
    rt.update(|s, tx| {
        s.draft = TextEdit::new("Go");
        send(s, tx);
    });
    assert_eq!(rt.state.composer_status(), crate::composer::status::ComposerStatus::Busy);
    run(&mut rt, 4.0);
    assert_eq!(rt.state.composer_status(), crate::composer::status::ComposerStatus::Idle);
    assert!(rt.state.optimistic.is_none());
}

#[test]
fn the_composer_reads_only_state_and_the_side_conversation_changes_the_frame_and_the_send_label() {
    let mut rt = app();
    rt.update(|s, _| s.side_open = true);
    rt.frame(FRAME);
    assert!(rt.find("prompt-send-button").is_some());
    rt.update(|s, tx| {
        s.draft = TextEdit::new("what is this?");
        submit(s, tx, false);
    });
    assert_eq!(rt.state.draft.text, "what is this?", "the draft stays until the side conversation exists");
}

/// Piece 3E: `MessageBubble.*.test.ts` (command, attached files, meta pill) as clicks and finds on the drawn frame.
fn frame_app() -> Runtime<State> {
    scene_app(|s| s.show_frame())
}

#[test]
fn a_command_shows_as_typed_and_the_prompt_opens_behind_show_prompt() {
    let mut rt = frame_app();
    assert!(rt.find("message-command").is_some());
    assert!(rt.find("message-command-prompt").is_none());
    click(&mut rt, "message-command-toggle");
    run(&mut rt, 0.3);
    assert!(rt.find("message-command-prompt").is_some());
    click(&mut rt, "message-command-toggle");
    run(&mut rt, 0.3);
    assert!(rt.find("message-command-prompt").is_none());
}

#[test]
fn a_message_with_a_note_shows_a_chip_per_file_and_the_note() {
    let rt = frame_app();
    assert!(rt.find("attached-.weave/attachments/3f9a2c1d/build-output.log").is_some());
    assert!(rt.find("attached-.weave/attachments/3f9a2c1d/incident-report.pdf").is_some());
}

#[test]
fn a_steered_prompt_and_one_a_mod_sent_wear_their_marks() {
    let rt = frame_app();
    assert!(rt.find("steered-mark").is_some());
    assert!(rt.find("mod-mark").is_some());
}

#[test]
fn the_pill_is_on_an_assistant_message_that_has_a_time() {
    let rt = frame_app();
    assert!(rt.find("meta-a1").is_some());
    assert!(rt.find("meta-u1").is_none(), "a user's time is plain text, not a pill");
}

#[test]
fn the_copy_button_says_copied_after_a_press() {
    let mut rt = frame_app();
    assert!(rt.state.copied.is_none());
    click(&mut rt, "copy-a1");
    assert_eq!(rt.state.copied.as_ref().map(|c| c.0.as_str()), Some("a1"));
    run(&mut rt, 2.0);
    assert!(rt.state.copied.is_none(), "it goes back to the icon by itself");
}
