//! The shell driven with no window: a runtime built as a headless render builds it, then
//! clicked and typed at by element key. New behaviour gets a test here (or in a `tests.rs`
//! beside its module) before it gets a snapshot.
use crate::mock::Status;
use crate::view::conversation::{Part, Role};
use crate::{Rail, State, send, spec};
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
