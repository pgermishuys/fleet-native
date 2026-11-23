//! The shell driven with no window: a runtime built as a headless render builds it, then
//! clicked and typed at by element key. New behaviour gets a test here (or in a `tests.rs`
//! beside its module) before it gets a snapshot.
use crate::mock::Status;
use crate::view::conversation::{Part, Role};
use crate::{Rail, State, send, spec, submit};
use rune::{App, Env, Key, KeyEvent, Mods, Runtime, TextEdit};

pub(crate) const FRAME: f64 = 1.0 / 60.0;

pub(crate) fn app() -> Runtime<State> {
    let mut rt = App::new(spec()).headless_runtime((1280.0, 760.0), Env::new(true, 1.0));
    // Two frames: the sheet measures itself on the first and lays out to it on the second.
    rt.frame(0.0);
    rt.frame(FRAME);
    rt
}

/// A window tall enough to hold a whole Settings page, for the tests that press things far down it.
pub(crate) fn app_tall() -> Runtime<State> {
    let mut rt = App::new(spec()).headless_runtime((1280.0, 1900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(FRAME);
    rt
}

pub(crate) fn click(rt: &mut Runtime<State>, key: &str) {
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
        // Fleet's login page is a page of its own, with no rail or status bar.
        if name.starts_with("sign-in") {
            assert!(rt.find("sign-in-page").is_some(), "scene {name} lost the page");
            continue;
        }
        // So are the splash and the error page (`start-*`).
        if let Some(page) = name.strip_prefix("start-") {
            let key = if page.starts_with("splash") {
                "start-splash"
            } else if page.starts_with("running-fleet") {
                "start-running-page"
            } else {
                "start-error"
            };
            assert!(rt.find(key).is_some(), "scene {name} lost the page");
            continue;
        }
        assert!(rt.find("rail-sessions").is_some(), "scene {name} lost the rail");
        assert!(rt.find("status-bar").is_some(), "scene {name} lost the status bar");
    }
}

#[test]
fn clicking_a_session_opens_it() {
    let mut rt = app();
    assert_eq!(rt.state.open_at(), (0, 0));
    click(&mut rt, "session-0-1");
    assert_eq!(rt.state.open_at(), (0, 1));
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
    assert_eq!(rt.state.open_at(), (last, rt.state.projects[last].sessions.len() - 1));
    command(&mut rt, ']');
    assert_eq!(rt.state.open_at(), (0, 0));
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
        s.set_open((0, 1));
        s.draft = TextEdit::new("  Ship it  ");
        send(s, tx);
    });
    let session = &rt.state.projects[0].sessions[1];
    let last = session.messages.last().unwrap();
    assert_eq!(last.role, Role::User);
    assert!(matches!(&last.parts[..], [Part::Text { text, .. }] if text == "  Ship it"));
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
    // The rows measure themselves one frame after they appear; the second press is aimed at where the header settled.
    rt.frame(FRAME);
    rt.frame(FRAME);
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
fn a_call_that_failed_starts_open_with_what_it_printed_and_keeps_its_preview_line_closed_or_open() {
    let mut rt = output_app(&["t2"]);
    assert!(rt.find("tool-t2-body").is_some(), "errors start open");
    assert!(rt.find("tool-t2-preview").is_some(), "Fleet's `v-if=\"preview && status === 'Error'\"` shows it open too");
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
    assert_eq!(rt.state.open_at(), (0, 1));
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
pub(crate) fn press_key(rt: &mut Runtime<State>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

/// Types into the open session's text area.
pub(crate) fn typed(rt: &mut Runtime<State>, text: &str) {
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
    // An idle session: while a turn runs, Enter queues (piece I).
    rt.update(|s, _| s.set_open((0, 2)));
    rt.frame(FRAME);
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
    rt.update(|s, _| s.set_open((0, 2)));
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
    rt.update(|s, _| s.set_open((0, 2)));
    rt.frame(FRAME);
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
        s.set_open((0, 1));
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

// Composer.test.ts: "enables composer for a stopped session when capabilities canPrompt is true"
#[test]
fn a_stopped_session_whose_row_can_prompt_takes_a_prompt() {
    let mut rt = app();
    rt.update(|s, _| {
        s.projects[0].sessions[0].status = Status::Idle;
        s.projects[0].sessions[0].row.can_prompt = Some(true);
    });
    assert!(!rt.state.composer_disabled());
    rt.update(|s, _| s.projects[0].sessions[0].row.can_prompt = Some(false));
    assert!(rt.state.composer_disabled());
}

#[test]
fn interrupt_is_live_while_a_turn_runs_and_while_the_session_waits_and_off_when_idle() {
    let mut rt = app();
    assert!(rt.state.can_interrupt(), "the first session is working");
    rt.update(|s, _| s.set_open((0, 1)));
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
    rt.update(|s, _| s.set_open((0, 2)));
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
    // With no server the side conversation is made at once (piece A of component 15), so the draft goes to it.
    assert_eq!(rt.state.draft.text, "");
    assert!(rt.state.sides.values().any(|s| s.side.is_some()));
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

/// `TurnFold.test.ts` ("says what the turn did on one line and toggles on click") as the app runs it: a finished turn is
/// one row, a click opens it to its own calls, another click folds it again, and the running turn stays open.
#[test]
fn a_finished_turn_folds_to_one_row_and_opens_on_click() {
    let mut rt = app();
    rt.update(|s, _| s.show_folded(false, true));
    rt.frame(FRAME);
    assert!(rt.find("turn-fold-u1").is_some(), "the finished turn has its fold row");
    assert!(rt.find("tool-t1").is_none(), "its command is folded away");
    assert!(rt.find("agent-t5").is_some(), "the sub-agent call stays, as its own row");
    assert!(rt.find("tool-t6").is_some(), "the failed call stays");
    assert!(rt.find("tool-t7").is_some(), "the running turn stays open");
    assert!(rt.find("turn-fold-u2").is_none(), "the running turn has no fold row");
    click(&mut rt, "turn-fold-u1");
    assert!(rt.state.turns_open.contains("u1"));
    assert!(rt.find("tool-t1").is_some(), "a click opens the turn to its calls");
    assert!(rt.find("turn-fold-u1").is_some(), "the fold row stays above them");
    // The conversation is pinned to its end, so the opened rows push the fold row up under the header: scroll back to it.
    rt.wheel(640.0, 400.0, 200.0);
    rt.frame(FRAME);
    click(&mut rt, "turn-fold-u1");
    assert!(rt.find("tool-t1").is_none(), "another click folds it again");
}

#[test]
fn when_the_turn_is_over_it_stays_as_watched_and_folds_once_the_next_prompt_goes() {
    let mut rt = app();
    rt.update(|s, _| s.show_folded(false, true));
    rt.frame(FRAME);
    // The session stopped working while you looked at it: the turn you watched stays open.
    rt.update(|s, _| {
        if let Some(stream) = s.stream.as_mut() {
            stream.session_status = crate::fleet::model::StreamStatus::Idle;
            // Its last call finished with the turn.
            for part in stream.messages.iter_mut().flat_map(|m| m.parts.iter_mut()) {
                if let crate::fleet::model::PartData::Tool { state, .. } = part {
                    state["status"] = serde_json::json!("completed");
                }
            }
        }
        s.refold();
    });
    rt.frame(FRAME);
    assert!(rt.find("turn-fold-u2").is_none(), "the turn you watched is as you saw it");
    assert!(rt.find("tool-t7").is_some());
    // Sending the next prompt makes it a finished turn: it folds.
    rt.update(|s, _| {
        s.prompts.add("", "Go on.", 50_000, 0.0);
        s.sync_open();
    });
    rt.frame(FRAME);
    assert!(rt.find("turn-fold-u2").is_some(), "the next prompt folds the turn you watched");
}

#[test]
fn a_session_opened_after_its_turns_ended_folds_all_of_them() {
    let mut rt = app();
    rt.update(|s, _| s.show_folded(false, false));
    rt.frame(FRAME);
    assert!(rt.find("turn-fold-u1").is_some());
    assert!(rt.find("turn-fold-u2").is_some(), "no turn was watched, so the last one folds too");
    assert!(rt.find("tool-t7").is_none());
}

// ── Turn failure card and retry bar (component 3, piece H) ──────────────────────────────────────────────────────────

fn failure_app(case: crate::mock::failure::Case) -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| s.show_failure(case, false));
    rt.frame(FRAME);
    rt
}

#[test]
fn a_failed_turn_says_so_with_its_stack_behind_details() {
    let mut rt = failure_app(crate::mock::failure::Case::Stopped);
    assert!(rt.find("turn-failure-a1").is_some());
    assert!(rt.find("turn-failure-a1-stack").is_none());
    click(&mut rt, "turn-failure-a1-details");
    assert!(rt.find("turn-failure-a1-stack").is_some());
    // The newest message, with a prompt before it: Try again is there, and no retry bar.
    assert!(rt.find("turn-failure-a1-retry").is_some());
    assert!(rt.find("turn-retry").is_none());
}

#[test]
fn a_limit_shows_the_bar_and_hides_try_again_until_it_is_taken_away() {
    let mut rt = failure_app(crate::mock::failure::Case::Usage);
    assert!(rt.find("turn-retry").is_some());
    assert!(rt.find("turn-failure-a1-retry").is_none());
    click(&mut rt, "turn-retry-cancel");
    assert!(rt.find("turn-retry").is_none());
    assert!(rt.find("turn-failure-a1-retry").is_some());
}

#[test]
fn an_earlier_failure_cannot_be_tried_again() {
    let rt = failure_app(crate::mock::failure::Case::Earlier);
    assert!(rt.find("turn-failure-a1").is_some());
    assert!(rt.find("turn-failure-a1-retry").is_none());
    assert!(rt.find("turn-failure-a1-report").is_some());
}

#[test]
fn try_again_sends_the_last_prompt_again() {
    let mut rt = failure_app(crate::mock::failure::Case::Stopped);
    let before = rt.state.session().messages.len();
    click(&mut rt, "turn-failure-a1-retry");
    let messages = &rt.state.session().messages;
    assert_eq!(messages.len(), before + 1);
    assert_eq!(messages.last().unwrap().text(), "Add cursor paging to the order list.");
}

#[test]
fn a_failed_action_says_why_and_a_second_answer_clears_it() {
    let mut rt = failure_app(crate::mock::failure::Case::ActionFailed);
    assert!(rt.find("turn-retry-why").is_some());
    rt.update(|s, _| {
        s.on_retry_acted("mock.0.0", true, Ok((500, serde_json::Value::Null)));
        assert_eq!(s.retry_act().error.as_deref(), Some("Fleet couldn't try again now (HTTP 500)."));
        s.on_retry_acted("mock.0.0", true, Ok((404, serde_json::Value::Null)));
        assert_eq!(s.retry_act().error, None);
    });
}

#[test]
fn the_countdown_redraws_by_the_second_in_the_last_minute() {
    let mut rt = failure_app(crate::mock::failure::Case::RateLimited);
    let first = rt.state.retry_bucket();
    rt.update(|s, _| s.now_ms = s.now_ms.map(|n| n + 1_000));
    assert_ne!(rt.state.retry_bucket(), first);
}

// ── Working line and interrupted card (component 3, piece I) ────────────────────────────────────────────────────────

fn working_app(case: crate::mock::working::Case) -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| s.show_working(case));
    rt.frame(FRAME);
    rt
}

fn interrupted_app(cut: crate::mock::working::Cut) -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| s.show_interrupted(cut));
    rt.frame(FRAME);
    rt
}

#[test]
fn the_working_line_names_the_call_and_the_turns_time() {
    use crate::conv::working::Working;
    let rt = working_app(crate::mock::working::Case::Tool);
    assert!(rt.find("working-indicator").is_some());
    assert!(rt.find("working-word").is_some());
    assert_eq!(rt.state.working_text(), Working::Busy { elapsed: Some("12 min".into()), tool: Some("Bash running 10 min".into()) });
    assert!(rt.find("working-elapsed").is_some() && rt.find("working-tool").is_some());
}

#[test]
fn a_held_message_is_said_on_the_line() {
    use crate::conv::working::Working;
    let rt = working_app(crate::mock::working::Case::Held);
    let Working::Busy { tool: Some(tool), .. } = rt.state.working_text() else { panic!("busy") };
    assert_eq!(tool, "Bash running 10 min +2 more. Your message goes to the agent when it finishes.");
}

#[test]
fn a_turn_with_no_call_shows_only_the_time() {
    let rt = working_app(crate::mock::working::Case::Fresh);
    assert!(rt.find("working-elapsed").is_some());
    assert!(rt.find("working-tool").is_none());
}

#[test]
fn a_question_holding_the_turn_says_needs_input_not_working() {
    let rt = working_app(crate::mock::working::Case::Waiting);
    assert!(rt.find("working-waiting").is_some());
    assert!(rt.find("working-word").is_none());
}

#[test]
fn a_retry_counts_down_in_the_line() {
    let mut rt = working_app(crate::mock::working::Case::Retry);
    assert!(rt.find("working-retry").is_some());
    assert!(rt.find("working-word").is_none());
    rt.update(|s, _| s.now_ms = s.now_ms.map(|n| n + 5_000));
    assert!(matches!(rt.state.working_text(), crate::conv::working::Working::Retry(t) if t.contains("in 7 s")));
}

#[test]
fn a_machine_that_stopped_answering_replaces_the_working_line() {
    let rt = working_app(crate::mock::working::Case::Quiet);
    assert!(rt.find("working-not-answering").is_some());
    assert!(rt.find("working-word").is_none());
    assert!(rt.find("working-elapsed").is_none());
}

#[test]
fn an_idle_session_has_no_working_line() {
    let mut rt = working_app(crate::mock::working::Case::Tool);
    rt.update(|s, _| {
        if let Some(session) = s.session_mut() {
            session.status = crate::mock::Status::Idle;
        }
    });
    rt.frame(FRAME);
    assert!(rt.find("working-indicator").is_none());
}

#[test]
fn the_interrupted_card_offers_continue_and_dismiss_and_goes_when_dismissed() {
    let mut rt = interrupted_app(crate::mock::working::Cut::Working);
    assert!(rt.find("interrupted-card").is_some());
    assert!(rt.find("interrupted-last-call").is_some() && rt.find("interrupted-files").is_some());
    assert!(rt.find("interrupted-continue").is_some());
    click(&mut rt, "interrupted-dismiss");
    assert!(rt.find("interrupted-card").is_none());
}

#[test]
fn a_lost_question_offers_ask_again_and_no_last_call() {
    let rt = interrupted_app(crate::mock::working::Cut::Waiting);
    assert!(rt.find("interrupted-text").is_some());
    assert!(rt.find("interrupted-last-call").is_none());
    assert_eq!(rt.state.interruption().map(|i| i.waiting), Some(true));
}

#[test]
fn a_session_fleet_continues_by_itself_hides_continue() {
    let rt = interrupted_app(crate::mock::working::Cut::Auto);
    assert!(rt.find("interrupted-auto").is_some());
    assert!(rt.find("interrupted-continue").is_none());
    assert!(rt.find("interrupted-dismiss").is_some());
}

#[test]
fn a_failed_continue_says_why_and_the_card_stays() {
    let mut rt = interrupted_app(crate::mock::working::Cut::Working);
    rt.update(|s, _| {
        s.on_interruption_acted("mock.0.0", true, Ok((500, serde_json::Value::Null)));
        assert_eq!(s.interruption_act().error.as_deref(), Some("Fleet couldn't continue the session (HTTP 500)."));
        assert!(s.interruption().is_some());
    });
    rt.frame(FRAME);
    assert!(rt.find("interrupted-error").is_some());
    // A 404 means Fleet has already dropped it: nothing to say. (The invented session has no id for the patch to hit.)
    rt.update(|s, _| {
        s.on_interruption_acted("mock.0.0", true, Ok((404, serde_json::Value::Null)));
        assert_eq!(s.interruption_act().error, None);
    });
}

#[test]
fn both_buttons_wait_while_a_request_is_on_its_way() {
    let mut rt = interrupted_app(crate::mock::working::Cut::Working);
    rt.update(|s, _| {
        s.interrupt_acts.insert("mock.0.0".into(), crate::fleet::retry::Act { busy: true, error: None });
    });
    rt.frame(FRAME);
    click(&mut rt, "interrupted-dismiss");
    assert!(rt.state.interruption().is_some(), "a disabled button takes no press");
}

// ── Background strip (component 3, piece K) ─────────────────────────────────────────────────────────────────────────

fn work_app(case: crate::mock::work::Case) -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| s.show_work(case));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

#[test]
fn the_strip_is_absent_when_nothing_runs() {
    let mut rt = work_app(crate::mock::work::Case::Mixed);
    assert!(rt.find("background-strip").is_some());
    rt.update(|s, _| {
        if let Some(stream) = &mut s.stream {
            stream.running_work.clear();
        }
    });
    rt.frame(FRAME);
    assert!(rt.find("background-strip").is_none());
}

#[test]
fn the_strip_lists_each_kind_and_offers_only_what_the_work_allows() {
    let rt = work_app(crate::mock::work::Case::Mixed);
    assert!(rt.find("background-strip-summary").is_some());
    for id in ["s1", "a1", "m1", "p1"] {
        assert!(rt.find(&format!("work-row-{id}")).is_some(), "row {id}");
    }
    // Shell: Output and Stop. Sub-agent with a session: Open and Stop. Monitor: Output and Events, no Stop. Pi's: Details.
    assert!(rt.find("work-output-s1").is_some() && rt.find("work-stop-s1").is_some() && rt.find("work-open-s1").is_none());
    assert!(rt.find("work-open-a1").is_some() && rt.find("work-stop-a1").is_some() && rt.find("work-output-a1").is_none());
    assert!(rt.find("work-events-m1").is_some() && rt.find("work-output-m1").is_some() && rt.find("work-stop-m1").is_none());
    assert!(rt.find("work-details-p1").is_some() && rt.find("work-stop-p1").is_none());
    assert_eq!(rt.state.work.summary(rt.state.running_work(), crate::view::message::now_ms(&rt.state)), "4 running");
}

#[test]
fn finished_work_shows_its_result_and_no_stop_and_then_drops() {
    let mut rt = work_app(crate::mock::work::Case::Finished);
    assert!(rt.find("work-meta-s2").is_some() && rt.find("work-stop-s2").is_none());
    assert!(rt.find("work-stop-s1").is_some());
    rt.update(|s, _| s.now_ms = s.now_ms.map(|n| n + 25_000));
    rt.frame(FRAME);
    assert!(rt.find("work-row-s2").is_none(), "ended 8 s before the scene's clock, 33 s on");
    assert!(rt.find("work-row-s1").is_some());
}

#[test]
fn the_header_collapses_the_list_and_keeps_the_summary() {
    let mut rt = work_app(crate::mock::work::Case::Mixed);
    click(&mut rt, "background-strip-toggle");
    assert!(rt.find("work-row-s1").is_none());
    assert!(rt.find("background-strip-summary").is_some());
    // The flag is the app's, not the session's, and the settings file keeps it (row 4.4).
    assert!(rt.state.sidebar.strip_collapsed());
    click(&mut rt, "background-strip-toggle");
    assert!(rt.find("work-row-s1").is_some());
}

#[test]
fn a_stop_in_flight_disables_its_button() {
    let mut rt = work_app(crate::mock::work::Case::Mixed);
    rt.update(|s, _| {
        s.work.stopping.insert("s1".into());
    });
    rt.frame(FRAME);
    assert!(rt.find("work-stop-s1").is_some());
}

#[test]
fn the_clock_redraws_when_a_time_changes_and_rests_with_nothing_shown() {
    use crate::conv::work_clock::WorkStrip;
    let mut rt = work_app(crate::mock::work::Case::Running);
    let items = rt.state.running_work().to_vec();
    rt.state.work.changed(&items, crate::view::message::now_ms(&rt.state));
    rt.update(|s, _| s.now_ms = s.now_ms.map(|n| n + 1_000));
    let now = crate::view::message::now_ms(&rt.state);
    assert!(rt.state.work.changed(&items, now), "a second on, the first minute's counter reads differently");
    assert_eq!(WorkStrip::default().next_change_ms(&[], now), None);
}

// ContextRing.test.ts: "shows nothing until the session's harness has reported a call"
#[test]
fn the_ring_shows_nothing_until_the_harness_has_reported_a_call() {
    let mut rt = app();
    rt.update(|s, _| s.set_open((0, 2)));
    rt.frame(FRAME);
    assert!(rt.find("composer-frame").is_some());
    assert!(rt.find("context-ring").is_none());
}

// ContextRing.test.ts: "is a quiet ring under three-quarters, and says how full on hover"
#[test]
fn the_ring_is_quiet_under_three_quarters_and_says_how_full_on_hover() {
    let mut rt = app();
    rt.update(|s, _| s.show_context(crate::mock::ring::Case::Quiet));
    rt.frame(FRAME);
    assert!(rt.find("context-ring").is_some());
    let ctx = rt.state.context().unwrap();
    assert_eq!(crate::composer::ring::title(ctx, false), "Context: 38% of 200k tokens");
    assert_eq!(crate::composer::ring::tone(crate::composer::ring::percent(ctx)), crate::composer::ring::Tone::Ok);
}

// ContextRing.test.ts: "shows the percentage once it's getting full, in the tone that says so"
#[test]
fn the_ring_shows_the_percentage_once_its_getting_full_in_the_tone_that_says_so() {
    use crate::composer::ring::{Tone, amber, percent, tone};
    for (case, p, t, a) in [(crate::mock::ring::Case::Warm, 78, Tone::Warn, false), (crate::mock::ring::Case::Amber, 84, Tone::Warn, true)]
    {
        let mut rt = app();
        rt.update(|s, _| s.show_context(case));
        rt.frame(FRAME);
        let ctx = rt.state.context().unwrap();
        assert_eq!((percent(ctx), tone(percent(ctx)), amber(percent(ctx))), (Some(p), t, a));
        assert!(rt.find("context-ring").is_some());
    }
}

// ContextRing.test.ts: "after a compaction, says so until the next call measures it"
#[test]
fn the_ring_after_a_compaction_says_so_until_the_next_call_measures_it() {
    use crate::composer::ring::{label, state_label};
    let mut rt = app();
    rt.update(|s, _| s.show_context(crate::mock::ring::Case::Compacted));
    rt.frame(FRAME);
    let ctx = rt.state.context().unwrap().clone();
    assert_eq!(state_label(&ctx, false), "Compacted");
    assert_eq!(label(&ctx, false), "Context compacted. Its size shows after the next reply.. Details");
    assert!(rt.find("context-ring").is_some());
    // The next call measures it.
    rt.update(|s, _| s.show_context(crate::mock::ring::Case::Quiet));
    rt.frame(FRAME);
    assert_eq!(crate::composer::ring::state_label(rt.state.context().unwrap(), false), "Plenty of room");
}

// Native: the ring sits by Send, after the buttons before it, and the toolbar still fits at the narrow rule.
#[test]
fn the_ring_sits_between_interrupt_and_send() {
    let mut rt = app();
    rt.update(|s, _| s.show_context(crate::mock::ring::Case::Quiet));
    rt.frame(FRAME);
    let x = |k: &str| rt.find(k).unwrap_or_else(|| panic!("no {k}")).0;
    assert!(x("interrupt") < x("context-ring") && x("context-ring") < x("prompt-send-button"));
}

// ── Work actions (component 3, piece L) ─────────────────────────────────────────────────────────────────────────────

fn a_server() -> rune::Runtime<State> {
    let mut rt = work_app(crate::mock::work::Case::Mixed);
    rt.update(|s, _| s.link = Some(crate::fleet::link::Link::start("http://127.0.0.1:9".into(), crate::fleet::link::Post::new(|_| {}))));
    rt
}

#[test]
fn output_opens_under_its_row_says_it_is_reading_and_closes_again() {
    let mut rt = work_app(crate::mock::work::Case::Mixed);
    assert!(rt.find("work-output-panel-s1").is_none());
    click(&mut rt, "work-output-s1");
    assert!(rt.find("work-output-loading-s1").is_some());
    click(&mut rt, "work-output-s1");
    assert!(rt.find("work-output-panel-s1").is_none());
}

#[test]
fn the_output_box_shows_what_was_read_with_a_line_when_its_start_was_left_out() {
    let mut rt = work_app(crate::mock::work::Case::Output);
    assert!(rt.find("work-output-text-s1").is_some() && rt.find("work-output-clipped-s1").is_some());
    assert!(rt.find("work-output-loading-s1").is_none());
    rt.update(|s, _| s.work.outputs.get_mut("s1").unwrap().clipped = false);
    rt.frame(FRAME);
    assert!(rt.find("work-output-clipped-s1").is_none());
}

#[test]
fn no_output_yet_and_a_read_error_are_notes_and_the_error_gives_way_to_text() {
    use crate::conv::work_output::Reader;
    let mut rt = work_app(crate::mock::work::Case::Mixed);
    let mut quiet = Reader::new(1, true);
    quiet.loading = false;
    rt.update(move |s, _| {
        s.work.outputs.insert("s1".into(), quiet);
    });
    rt.frame(FRAME);
    assert!(rt.find("work-output-empty-s1").is_some());
    rt.update(|s, _| s.work.outputs.get_mut("s1").unwrap().error = Some("The output couldn't be read (HTTP 404).".into()));
    rt.frame(FRAME);
    assert!(rt.find("work-output-error-s1").is_some() && rt.find("work-output-empty-s1").is_none());
}

#[test]
fn an_answer_for_a_panel_closed_since_is_dropped_and_one_for_the_open_panel_is_read() {
    use serde_json::json;
    let mut rt = a_server();
    rt.update(|s, _| s.toggle_work_output("s1"));
    let epoch = rt.state.work.outputs["s1"].epoch;
    let page = json!({"output": "✓ 12 tests passed\n", "nextOffset": 19, "size": 19, "truncated": false});
    let stale = page.clone();
    rt.update(move |s, tx| s.on_work_page("s1", epoch + 1, Ok((200, stale)), tx));
    assert!(rt.state.work.outputs["s1"].text.is_empty());
    rt.update(move |s, tx| s.on_work_page("s1", epoch, Ok((200, page)), tx));
    assert_eq!(rt.state.work.outputs["s1"].text, "✓ 12 tests passed\n");
    rt.frame(FRAME);
    assert!(rt.find("work-output-text-s1").is_some());
    // Closed: its answer finds nothing.
    rt.update(|s, _| s.toggle_work_output("s1"));
    rt.update(move |s, tx| s.on_work_page("s1", epoch, Ok((200, json!({"output": "x", "nextOffset": 1, "size": 1}))), tx));
    assert!(rt.state.work.outputs.is_empty());
}

#[test]
fn a_refused_read_says_why_and_a_page_without_its_offsets_is_an_error() {
    use serde_json::json;
    let mut rt = a_server();
    rt.update(|s, _| s.toggle_work_output("s1"));
    let epoch = rt.state.work.outputs["s1"].epoch;
    rt.update(move |s, tx| s.on_work_page("s1", epoch, Ok((404, json!({"error": "That output is gone."}))), tx));
    assert_eq!(rt.state.work.outputs["s1"].error.as_deref(), Some("That output is gone."));
    rt.update(move |s, tx| s.on_work_page("s1", epoch, Ok((200, json!({"output": "x"}))), tx));
    assert_eq!(rt.state.work.outputs["s1"].error.as_deref(), Some("The output couldn't be read (HTTP 200)."));
}

#[test]
fn a_running_item_is_read_again_every_two_seconds_and_once_more_when_it_ends() {
    use serde_json::json;
    let mut rt = a_server();
    rt.update(|s, _| s.toggle_work_output("s1"));
    let epoch = rt.state.work.outputs["s1"].epoch;
    let page = json!({"output": "a\n", "nextOffset": 2, "size": 2});
    rt.update(move |s, tx| s.on_work_page("s1", epoch, Ok((200, page)), tx));
    let now = crate::view::message::now_ms(&rt.state);
    let reader = &rt.state.work.outputs["s1"];
    assert!(!reader.in_flight);
    assert_eq!(reader.next_at, Some(now + 2_000));
    // Not due yet: nothing starts. Due: a read starts.
    rt.update(|s, tx| s.poll_work_outputs(tx));
    assert!(!rt.state.work.outputs["s1"].in_flight);
    rt.update(|s, _| s.now_ms = s.now_ms.map(|n| n + 2_000));
    rt.update(|s, tx| s.poll_work_outputs(tx));
    assert!(rt.state.work.outputs["s1"].in_flight);
    // It ends while a read is out: that read's answer comes, and then one last read starts at once.
    let page = json!({"output": "b\n", "nextOffset": 4, "size": 4});
    rt.update(move |s, tx| s.on_work_page("s1", epoch, Ok((200, page)), tx));
    rt.update(|s, _| {
        s.stream.as_mut().unwrap().running_work[0].status = crate::fleet::work::WorkStatus::Completed;
        s.stream.as_mut().unwrap().running_work[0].ended_reason = Some(crate::fleet::work::EndedReason::Completed);
    });
    rt.update(|s, tx| s.poll_work_outputs(tx));
    assert!(rt.state.work.outputs["s1"].in_flight, "the final read");
    let page = json!({"output": "done\n", "nextOffset": 9, "size": 9});
    rt.update(move |s, tx| s.on_work_page("s1", epoch, Ok((200, page)), tx));
    rt.update(|s, tx| s.poll_work_outputs(tx));
    assert!(!rt.state.work.outputs["s1"].in_flight, "an ended item is not read on");
    assert_eq!(rt.state.work.outputs["s1"].text, "a\nb\ndone\n");
}

#[test]
fn a_refused_stop_says_why_under_the_row_and_the_next_press_clears_it() {
    use serde_json::json;
    let mut rt = a_server();
    rt.update(|s, _| s.work.stopping.insert("s1".into()).then_some(()).unwrap());
    rt.update(|s, _| s.on_work_stopped("s1", Ok((409, json!({"error": "The session isn't running."})))));
    rt.frame(FRAME);
    assert!(!rt.state.work.stopping.contains("s1"));
    assert!(rt.find("work-stop-error-s1").is_some());
    click(&mut rt, "work-stop-s1");
    assert!(rt.find("work-stop-error-s1").is_none(), "cleared on the next press");
    assert!(rt.state.work.stopping.contains("s1"), "and a stop is on its way");
    // No message in the body: the status says it.
    rt.update(|s, _| s.on_work_stopped("s1", Ok((500, serde_json::Value::Null))));
    assert_eq!(rt.state.work.stop_errors["s1"], "It couldn't be stopped (HTTP 500).");
    rt.update(|s, _| s.on_work_stopped("s1", Err("It couldn't be stopped.".into())));
    assert_eq!(rt.state.work.stop_errors["s1"], "It couldn't be stopped.");
}

#[test]
fn a_second_stop_while_one_is_out_says_so_and_sends_nothing() {
    let mut rt = a_server();
    rt.update(|s, _| {
        s.work.stopping.insert("s1".into());
        s.stop_work("s1");
    });
    assert_eq!(rt.state.work.stop_errors["s1"], "Already stopping.");
}

#[test]
fn a_stopped_item_is_applied_at_once_and_shows_stopped_without_stop() {
    use serde_json::json;
    let mut rt = a_server();
    let ended = json!({"id": "s1", "sessionId": "scene", "workId": "w-s1", "kind": "shell", "title": "shell", "label": "bun run test:e2e",
        "status": "cancelled", "background": true, "startedAt": "2026-10-04T10:00:00Z", "endedAt": "2026-10-04T10:03:12Z",
        "endedReason": "cancelled", "detail": "stopped", "canReadOutput": true});
    rt.update(move |s, _| s.on_work_stopped("s1", Ok((200, ended))));
    rt.frame(FRAME);
    assert!(rt.find("work-stop-s1").is_none());
    assert!(rt.find("work-meta-s1").is_some());
}

#[test]
fn details_open_and_close_and_say_what_the_agent_was_asked() {
    let mut rt = work_app(crate::mock::work::Case::Mixed);
    assert!(rt.find("work-details-panel-p1").is_none() && rt.find("work-details-p1").is_some());
    click(&mut rt, "work-details-p1");
    assert!(rt.state.work.details.contains("p1"));
    assert!(rt.find("work-details-asked-p1").is_some() && rt.find("work-details-now-p1").is_some());
    assert!(rt.find("work-details-note-p1").is_some(), "it can't be stopped on its own: the panel says why");
    click(&mut rt, "work-details-p1");
    assert!(rt.find("work-details-asked-p1").is_none());
}

#[test]
fn open_shows_the_child_session_and_remembers_the_parent_but_only_one_the_list_holds() {
    let mut rt = work_app(crate::mock::work::Case::Mixed);
    click(&mut rt, "work-open-a1");
    assert!(rt.state.work.opened_from.is_none(), "child-1 isn't in the list (component 6): nothing to show");
    rt.update(|s, tx| {
        s.set_session_id((0, 1), "child-1");
        assert!(s.open_child("scene", "child-1", tx));
    });
    assert_eq!(rt.state.work.opened_from.as_deref(), Some("scene"));
    assert_eq!(rt.state.session().id, "child-1");
}

#[test]
fn events_press_is_taken_by_a_monitor_with_a_call() {
    let mut rt = work_app(crate::mock::work::Case::Mixed);
    click(&mut rt, "work-events-m1");
    assert!(rt.find("work-row-m1").is_some());
}

/// Ends a fake server's answer the way a real server does: stop sending, then wait for the client to close, so
/// Windows doesn't reset the connection under an answer the client hasn't read yet.
pub(crate) fn finish(stream: &mut std::net::TcpStream) {
    use std::io::Read;
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let _ = stream.set_read_timeout(Some(crate::test_wait::BOUND));
    let mut sink = [0u8; 256];
    while matches!(stream.read(&mut sink), Ok(n) if n > 0) {}
}

#[test]
fn stop_and_output_calls_go_to_fleets_item_id_and_read_the_answer_as_the_server_writes_it() {
    use std::io::Write;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let request = crate::test_http::read(&mut stream).raw();
            let line = request.lines().next().unwrap_or("").to_string();
            let (status, body) = if line.contains("/stop") {
                ("409 Conflict", r#"{"error":"The session isn't running."}"#)
            } else {
                ("200 OK", r#"{"output":"hi\n","nextOffset":3,"size":3,"truncated":false}"#)
            };
            log.lock().unwrap().push(line);
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            finish(&mut stream);
        }
    });
    let (code, body) = crate::fleet::http::work_stop(&base, "sess", "a91d").unwrap();
    assert_eq!((code, body["error"].as_str()), (409, Some("The session isn't running.")));
    let (code, body) = crate::fleet::http::work_output(&base, "sess", "a91d", 12).unwrap();
    assert_eq!((code, body["nextOffset"].as_u64()), (200, Some(3)));
    let seen = seen.lock().unwrap();
    assert!(seen[0].starts_with("POST /api/sessions/sess/work/a91d/stop "), "{}", seen[0]);
    assert!(seen[1].starts_with("GET /api/sessions/sess/work/a91d/output?offset=12 "), "{}", seen[1]);
}

// ContextRing.test.ts: "opens the details: the size, each turn, the last call and what the session cost"
#[test]
fn clicking_the_ring_opens_the_details_above_it_and_a_press_outside_closes_them() {
    let mut rt = app();
    rt.update(|s, _| s.show_context(crate::mock::ring::Case::Quiet));
    rt.frame(FRAME);
    assert!(rt.find("context-popover").is_none());
    click(&mut rt, "context-ring");
    rt.frame(FRAME);
    rt.frame(FRAME);
    let (_, ring_y) = rt.find("context-ring").unwrap();
    let (_, card_y) = rt.find("context-popover").expect("the details open");
    assert!(card_y < ring_y, "above the ring");
    assert!(rt.find("context-bar0").is_some() && rt.find("context-bar7").is_some());
    assert!(rt.find("context-compact").is_some());
    click(&mut rt, "context-scrim");
    rt.frame(FRAME);
    assert!(rt.find("context-popover").is_none());
}

// ContextRing.test.ts: "compacts now, and says why when Fleet refuses"
#[test]
fn compact_now_waits_for_fleets_answer_and_shows_why_it_refused() {
    let mut rt = app();
    rt.update(|s, _| {
        s.set_session_id((0, 2), "s1");
        s.show_context_popover(crate::mock::ring::Case::Quiet, "");
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("context-error").is_none());
    click(&mut rt, "context-compact");
    rt.frame(FRAME);
    assert!(rt.state.compact.requesting.contains("s1"), "in flight");
    rt.update(|s, _| s.compact.finish("s1", Err("Wait for the agent to finish its turn, then compact.".into())));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("context-error").is_some());
}

// ContextRing.test.ts: "can't compact when the session says it can't, with its reason"
#[test]
fn compact_now_is_off_when_the_session_cant_compact() {
    let mut rt = app();
    rt.update(|s, _| s.show_context_popover(crate::mock::ring::Case::Quiet, "disabled"));
    rt.frame(FRAME);
    rt.frame(FRAME);
    click(&mut rt, "context-compact");
    rt.frame(FRAME);
    assert!(rt.state.compact.requesting.is_empty());
}

// ContextRing.test.ts: "shows the harness's usage limits under the context, when it reports them"
#[test]
fn the_popover_draws_the_harness_limits_only_when_it_reports_some() {
    let mut rt = app();
    rt.update(|s, _| s.show_context_popover(crate::mock::ring::Case::Quiet, ""));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("context-limits").is_none());
    rt.update(|s, _| s.show_context_popover(crate::mock::ring::Case::Quiet, "limits"));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("context-limits").is_some() && rt.find("context-limit2").is_some());
}

// ── Sub-agent rows and background notices (component 3, piece M) ────────────────────────────────────────────────────

fn agents_app(case: crate::mock::agents::Case) -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| s.show_agents(case));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn notices(rt: &Runtime<State>) -> Vec<crate::conv::background::BackgroundNotice> {
    rt.state.session().messages.iter().filter_map(|m| m.meta.background.clone()).collect()
}

#[test]
fn a_sub_agent_call_with_a_session_is_one_row_of_its_own_and_one_without_stays_a_tool_row() {
    let mut rt = agents_app(crate::mock::agents::Case::Done);
    assert!(rt.find("agent-c1").is_some() && rt.find("tool-c1").is_none());
    // The harness hasn't made the child session yet: the ordinary row, with the call's own words.
    rt.update(|s, _| {
        s.stream.as_mut().unwrap().delegations[0].child_session_id = None;
        // The row's memo reads the message, not the delegation list: a changed message redraws it.
        s.session_mut().unwrap().messages[1].meta.created_at = Some(1);
    });
    rt.frame(FRAME);
    assert!(rt.find("agent-c1").is_none() && rt.find("tool-c1").is_some());
}

#[test]
fn the_row_redraws_when_the_delegation_changes_with_no_change_to_the_message() {
    use crate::fleet::delegations::DelegationStatus;
    let mut rt = agents_app(crate::mock::agents::Case::Background);
    let key = |rt: &Runtime<State>| crate::view::agent_row::key(&rt.state, &rt.state.session().messages[1]);
    let before = key(&rt);
    rt.update(|s, _| s.stream.as_mut().unwrap().delegations[0].status = DelegationStatus::Completed);
    assert_ne!(before, key(&rt));
    rt.frame(FRAME);
    assert!(rt.find("agent-c1").is_some());
}

#[test]
fn every_state_of_the_row_draws_and_a_press_opens_the_child() {
    let mut rt = agents_app(crate::mock::agents::Case::Every);
    for id in ["c1", "c2", "c3", "c4", "c5", "c6"] {
        assert!(rt.find(&format!("agent-{id}")).is_some(), "row {id}");
        assert!(rt.find(&format!("agent-{id}-status")).is_some());
    }
    // Pressing opens the child as `open_child` does; one the list doesn't hold shows nothing yet (component 6).
    click(&mut rt, "agent-c2");
    assert!(rt.state.work.opened_from.is_none());
    rt.update(|s, tx| {
        s.set_session_id((0, 1), "child-1");
        assert!(s.open_child("scene", "child-1", tx));
    });
    assert_eq!(rt.state.session().id, "child-1");
}

#[test]
fn a_notice_is_a_chip_with_its_own_state_and_not_a_user_bubble() {
    use crate::conv::background::{BackgroundKind, BackgroundState};
    let mut rt = agents_app(crate::mock::agents::Case::Notices);
    assert!(rt.find("background-note").is_some());
    let all = notices(&rt);
    let states: Vec<_> = all.iter().map(|n| (n.kind, n.state)).collect();
    assert_eq!(
        states,
        vec![
            (BackgroundKind::Shell, BackgroundState::Completed),
            (BackgroundKind::Subagent, BackgroundState::Completed),
            (BackgroundKind::Subagent, BackgroundState::Error),
            (BackgroundKind::Subagent, BackgroundState::Cancelled),
        ]
    );
    // The words are the notice's text, not the raw tags.
    let m = &rt.state.session().messages[2];
    assert_eq!(m.text(), "12 passed\n\nCommand exited with code 0.");
    // And the call that started the shell stopped saying Background once its notice arrived.
    let call = rt.state.session().messages[1].parts.iter().find_map(|p| match p {
        Part::Tool { status, .. } => Some(*status),
        _ => None,
    });
    assert_eq!(call, Some(crate::conv::tools::ToolStatus::Completed));
    rt.frame(FRAME);
}

// ActivityStream.background-work.test.ts
fn stopped_shell(sh: &str) -> (serde_json::Value, serde_json::Value) {
    use serde_json::json;
    let messages = json!([
        {"info": {"id": "m1", "role": "assistant", "sessionID": "s1", "time": {"created": 1}}, "parts": [
            {"type": "tool", "id": "p1", "tool": "shell", "callID": "call_dev", "state": {"status": "running", "background": true,
                "input": {"command": "sh scripts/dev.sh"}, "metadata": {"shellID": sh}}}]},
        {"info": {"id": "m2", "role": "user", "sessionID": "s1", "time": {"created": 2}}, "parts": [
            {"type": "text", "id": "p2", "text": format!("<shell id=\"{sh}\" state=\"error\" command=\"sh scripts/dev.sh\">\nShell.NotFoundError\n</shell>")}]},
    ]);
    let work = json!({"id": "w1", "sessionId": "s1", "workId": sh, "kind": "shell", "title": "shell", "label": "sh scripts/dev.sh",
        "status": "cancelled", "background": true, "toolCallId": "call_dev", "canStop": true, "startedAt": "2026-10-04T10:00:00Z",
        "endedAt": "2026-10-04T10:05:00Z", "endedReason": "cancelled", "detail": "stopped"});
    (messages, work)
}

fn open_stream(messages: serde_json::Value, work: Vec<serde_json::Value>) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| {
        let mut stream = crate::fleet::reducer::create_state(
            &serde_json::json!({"session": {}, "messages": messages, "delegations": [], "activityStatus": "idle"}),
        );
        stream.running_work = crate::fleet::work::to_items(Some(&serde_json::Value::Array(work)));
        s.stream = Some(stream);
        s.stream_of = None;
        s.sync_open();
    });
    rt.frame(FRAME);
    rt
}

#[test]
fn activity_stream_and_work_fleet_stopped_says_stopped_not_error_for_a_shell_fleet_stopped() {
    use crate::conv::background::BackgroundState;
    let (messages, work) = stopped_shell("sh_dev");
    let rt = open_stream(messages, vec![work]);
    let note = notices(&rt).remove(0);
    assert_eq!((note.state, note.state.word()), (BackgroundState::Cancelled, "stopped"));
    // The raw error body is not shown.
    assert!(!rt.state.session().messages.iter().any(|m| m.text().contains("Shell.NotFoundError")));
    assert!(rt.find("background-note").is_some());
}

#[test]
fn activity_stream_and_work_fleet_stopped_asks_fleet_for_older_work_when_the_stream_no_longer_has_it() {
    use crate::conv::background::BackgroundState;
    use std::io::Write;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let (log, stopped) = (seen.clone(), stopped_shell("sh_dev").1);
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let request = crate::test_http::read(&mut stream).raw();
            log.lock().unwrap().push(request.lines().next().unwrap_or("").to_string());
            let body = serde_json::json!([stopped]).to_string();
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            finish(&mut stream);
        }
    });
    let (messages, _) = stopped_shell("sh_dev");
    let mut rt = app();
    rt.update(move |s, _| {
        s.set_session_id((0, 0), "s1");
        s.link = Some(crate::fleet::link::Link::start(base, crate::fleet::link::Post::new(|_| {})));
        let stream = crate::fleet::reducer::create_state(
            &serde_json::json!({"session": {}, "messages": messages, "delegations": [], "activityStatus": "idle"}),
        );
        s.stream = Some(stream);
        s.stream_of = None;
        s.sync_open();
    });
    // While the answer is out the notice says what the harness said: a failure.
    assert_eq!(notices(&rt).remove(0).state, BackgroundState::Error);
    let id = rt.state.open_id().unwrap();
    assert_eq!(rt.state.work.older_for.as_deref(), Some(id.as_str()));
    // The link's own hub negotiation arrives too: look for the one call this test is about.
    let wanted = format!("GET /api/sessions/{id}/work?all=true ");
    crate::test_wait::until(
        &mut (),
        &format!("the call {wanted:?}"),
        |_| seen.lock().unwrap().iter().any(|l| l.starts_with(&wanted)),
        |_| format!("calls seen: {:?}", seen.lock().unwrap()),
    );
    // Its answer (delivered here by hand: the test's `Post` drops the app's own delivery) turns the notice to stopped.
    let (_, work) = stopped_shell("sh_dev");
    rt.update(move |s, _| s.on_older_work(&id, Ok((200, serde_json::json!([work])))));
    assert_eq!(notices(&rt).remove(0).state, BackgroundState::Cancelled);
}

#[test]
fn activity_stream_and_work_fleet_stopped_keeps_a_real_failure_a_failure() {
    use crate::conv::background::BackgroundState;
    let (messages, mut work) = stopped_shell("sh_dev");
    work["status"] = serde_json::json!("error");
    work["endedReason"] = serde_json::json!("error");
    let rt = open_stream(messages, vec![work]);
    assert_eq!(notices(&rt).remove(0).state, BackgroundState::Error);
    assert!(rt.state.session().messages.iter().any(|m| m.text().contains("Shell.NotFoundError")));
}

// Component 6, piece A: the list's groups (rows 1.31, 1.32, 1.39).
#[test]
fn a_project_with_no_sessions_still_shows_in_its_place() {
    let rt = scene_app(|s| s.show_groups(""));
    let names: Vec<&str> = rt.state.projects.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Ledger", "Orchard", "Lighthouse", "Field notes", "Benchmarks", "Docs site"]);
    assert!(rt.find("project-0").is_some(), "Ledger has no sessions and still has its header");
    assert!(rt.find("project-5").is_some());
}

#[test]
fn a_filter_that_matches_a_projects_name_keeps_all_its_sessions_and_drops_the_others() {
    let rt = scene_app(|s| s.show_groups("light"));
    assert!(rt.find("project-1").is_none(), "Orchard has no match and another name");
    assert!(rt.find("project-0").is_none());
    for i in 0..3 {
        assert!(rt.find(&format!("session-2-{i}")).is_some(), "Lighthouse's session {i} stays");
    }
}

#[test]
fn a_filter_matches_a_sessions_status_word_not_only_its_title() {
    let rt = scene_app(|s| s.show_groups("waiting_input"));
    assert!(rt.find("session-1-1").is_some(), "the session that waits for input");
    assert!(rt.find("session-1-0").is_none());
}

// MessageBubble.attached-files.test.ts: "leaves the agent's replies alone"
#[test]
fn an_attached_files_note_in_an_agents_reply_is_left_as_text() {
    let mut rt = frame_app();
    let chip = "attached-.weave/attachments/3f9a2c1d/build-output.log";
    assert!(rt.find(chip).is_some(), "the user's message draws it");
    rt.update(|s, _| {
        let m = &mut s.session_mut().unwrap().messages[0];
        m.role = crate::view::conversation::Role::Assistant;
        m.meta.created_at = Some(1);
    });
    rt.frame(FRAME);
    run(&mut rt, 0.3);
    assert!(rt.find(chip).is_none(), "an agent's reply never reads as a note of attached files");
}

#[test]
fn markdown_markers_take_the_accent_tint_and_code_takes_fleets_syntax_colours() {
    for dark in [true, false] {
        let t = crate::theme::Theme::of(dark);
        let marker = t.md_marker();
        // `--md-marker` lies between `--muted` and `--accent`, nearer the accent.
        assert!((marker.b - t.muted.b).abs() < (t.accent.b - t.muted.b).abs() + 1e-6);
        assert_ne!(marker, t.muted);
        assert_ne!(marker, t.accent);
    }
    let keyword = crate::theme::Theme::of(true).syntax()[0];
    assert_eq!(keyword, rune::color::hex("#c678dd").unwrap(), "One Dark's keyword colour, Fleet's default");
    assert_eq!(crate::theme::Theme::of(false).syntax()[0], rune::color::hex("#a626a4").unwrap());
}

// ime-enter-guard.test.ts: the Enter that confirms an input method's composition is the composition's, never "send".
#[test]
fn an_enter_that_confirms_an_input_method_composition_does_not_send() {
    let mut rt = app();
    rt.update(|s, _| s.set_open((0, 2)));
    rt.frame(FRAME);
    let before = user_messages(&rt);
    click(&mut rt, "composer");
    rt.ime(rune::Ime::Preedit("かん".into(), Some((0, 6)), Vec::new()));
    rt.frame(FRAME);
    assert!(rt.state.draft.is_composing());
    press_key(&mut rt, Key::Enter, Mods::default());
    assert_eq!(user_messages(&rt), before, "the Enter went to the composition");
    // A plain Enter afterwards still sends.
    typed(&mut rt, "Ship it");
    press_key(&mut rt, Key::Enter, Mods::default());
    assert_eq!(user_messages(&rt), before + 1);
}

// prompt-text.test.ts: a prompt that starts with an indented line is sent with that indent.
#[test]
fn the_composers_enter_sends_the_first_lines_indent() {
    let mut rt = app();
    rt.update(|s, _| s.set_open((0, 2)));
    rt.frame(FRAME);
    typed(&mut rt, "\n    run-tests  \n");
    press_key(&mut rt, Key::Enter, Mods::default());
    let sent = rt.state.session().messages.iter().rfind(|m| m.role == Role::User).unwrap();
    let text = sent.parts.iter().find_map(|p| if let Part::Text { text, .. } = p { Some(text.clone()) } else { None });
    assert_eq!(text.as_deref(), Some("    run-tests"));
}
