//! The conversation's scroll behaviour, driven through a headless runtime: Jump to latest, following the end, the
//! session commands, showing a message, and asking for older messages near the top (rows 1.43-1.51). The decisions
//! themselves are tested under Fleet's names in `conv/tests/pin.rs`.
use super::super::link::{Link, Post};
use super::reconnect::app;
use crate::State;
use crate::fleet::reducer::create_state;
use crate::view::conversation::{Older, Part};
use rune::{Runtime, TextEdit};

const FRAME: f64 = 1.0 / 60.0;

fn long() -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| s.open_long());
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn run(rt: &mut Runtime<State>, seconds: f64) {
    for _ in 0..(seconds / FRAME).ceil() as usize {
        rt.frame(FRAME);
    }
}

/// A person wheels up by `points` over a few frames.
fn wheel_up(rt: &mut Runtime<State>, points: f32) {
    let steps = (points / 100.0).ceil().max(1.0) as usize;
    for _ in 0..steps {
        rt.wheel(640.0, 400.0, points / steps as f32);
        rt.frame(FRAME);
    }
    rt.frame(FRAME);
}

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(FRAME);
}

#[test]
fn starts_at_the_bottom_without_the_jump_button() {
    let rt = long();
    assert_eq!(rt.scroll_at_bottom("stream"), Some(true));
    assert!(rt.find("jump-to-latest").is_none());
}

#[test]
fn offers_jump_to_latest_once_scrolled_up_past_the_threshold_and_scrolls_to_the_bottom_when_clicked() {
    let mut rt = long();
    wheel_up(&mut rt, 600.0);
    assert!(rt.find("jump-to-latest").is_some(), "no pill after scrolling up");
    assert_eq!(rt.scroll_at_bottom("stream"), Some(false));

    click(&mut rt, "jump-to-latest");
    run(&mut rt, 0.3);
    assert_eq!(rt.scroll_at_bottom("stream"), Some(true));
    assert!(rt.find("jump-to-latest").is_none(), "the pill stayed after the jump");
}

#[test]
fn a_small_scroll_up_by_wheel_stays_put_while_text_streams_in() {
    let mut rt = app();
    rt.update(|s, _| s.open_long_live());
    run(&mut rt, 0.1);
    wheel_up(&mut rt, 30.0);
    assert!(rt.find("jump-to-latest").is_some(), "a nudge up with the wheel is leaving the end");
    run(&mut rt, 1.5);
    assert_eq!(rt.scroll_at_bottom("stream"), Some(false), "the streaming reply pulled the view back");
    assert!(rt.find("jump-to-latest").is_some());
}

#[test]
fn keeps_the_view_pinned_to_the_bottom_as_content_grows() {
    let mut rt = app();
    rt.update(|s, _| s.open_long_live());
    run(&mut rt, 1.5);
    assert_eq!(rt.scroll_at_bottom("stream"), Some(true));
    assert!(rt.find("jump-to-latest").is_none(), "growth showed the pill");
}

#[test]
fn sending_a_prompt_pins_again_after_a_nudge_up_so_the_reply_is_followed() {
    let mut rt = long();
    wheel_up(&mut rt, 600.0);
    assert!(rt.find("jump-to-latest").is_some());
    rt.update(|s, tx| {
        s.draft = TextEdit::new("hello");
        crate::send(s, tx);
    });
    run(&mut rt, 0.5);
    assert!(rt.find("jump-to-latest").is_none(), "the pill stayed after sending");
    assert_eq!(rt.scroll_at_bottom("stream"), Some(true));
}

#[test]
fn pins_again_for_a_session_switch() {
    let mut rt = long();
    wheel_up(&mut rt, 600.0);
    assert!(rt.find("jump-to-latest").is_some());
    rt.update(|s, tx| s.open_session(0, 1, tx));
    run(&mut rt, 0.5);
    assert!(rt.find("jump-to-latest").is_none());
    assert_eq!(rt.scroll_at_bottom("stream"), Some(true));
}

#[test]
fn scrolls_to_the_top_and_the_bottom_on_the_session_commands() {
    let mut rt = long();
    rt.update(|s, tx| s.scroll_top(tx));
    run(&mut rt, 2.0);
    assert!(rt.find("a0-p0").is_some(), "scroll-top didn't reach the first message");
    assert!(rt.find("jump-to-latest").is_some());

    rt.update(|s, tx| s.follow_end(tx));
    run(&mut rt, 2.0);
    assert_eq!(rt.scroll_at_bottom("stream"), Some(true));
    assert!(rt.find("jump-to-latest").is_none());
}

#[test]
fn show_message_brings_a_message_to_the_middle_rings_it_for_1600_ms_and_stops_following_the_end() {
    let mut rt = long();
    let shown = rt.state.session().messages[300].id.clone();
    let found = std::rc::Rc::new(std::cell::Cell::new(false));
    rt.update({
        let (id, found) = (shown.clone(), found.clone());
        move |s, tx| found.set(s.show_message(Some(&id), None, tx))
    });
    assert!(found.get());
    run(&mut rt, 1.0);
    assert!(rt.state.is_shown(&shown));
    assert!(rt.find("jump-to-latest").is_some(), "showing a message left the view following the end");

    // The glide is a long one (1,700 messages), and the ring ends on its own clock.
    run(&mut rt, 3.0);
    let (_, y) = rt.find(&shown).unwrap_or_else(|| panic!("{shown} wasn't brought in"));
    assert!((y - 330.0).abs() < 120.0, "the message sits at {y}, not near the middle");
    assert!(!rt.state.is_shown(&shown), "the ring outlived 1.6 s");
}

#[test]
fn show_message_finds_a_message_by_the_tool_call_it_holds_and_ignores_an_unknown_one() {
    let mut rt = app();
    let call = rt
        .state
        .session()
        .messages
        .iter()
        .find_map(|m| m.parts.iter().find_map(|p| if let Part::Tool { call_id, .. } = p { Some(call_id.clone()) } else { None }));
    let call = call.expect("the invented session has a tool call");
    assert!(rt.state.message_for(None, Some(&call)).is_some());
    assert!(rt.state.message_for(None, Some("call-unknown")).is_none());
    assert!(rt.state.message_for(Some("no-such-message"), None).is_none());
    let found = std::rc::Rc::new(std::cell::Cell::new(true));
    let seen = found.clone();
    rt.update(move |s, tx| seen.set(s.show_message(None, Some("call-unknown"), tx)));
    assert!(!found.get());
}

/// A session on a server whose conversation has older messages; the server is a port nobody listens on, so the ask is
/// seen as `Loading` and goes no further.
fn with_older() -> Runtime<State> {
    let mut rt = long();
    let link = Link::start("http://127.0.0.1:9".into(), Post::new(|_| {}));
    rt.update(move |s, _| {
        s.link = Some(link);
        let (p, i) = s.open_at();
        s.set_session_id((p, i), "older-session");
        let mut stream = create_state(&serde_json::json!({"messages": []}));
        stream.has_more = true;
        stream.cursor = Some("cursor-1".into());
        s.stream = Some(stream);
        s.stream_of = Some("older-session".into());
        s.loading = false;
    });
    rt.frame(FRAME);
    rt
}

#[test]
fn asks_for_older_messages_when_scrolled_near_the_top_and_not_before() {
    let mut rt = with_older();
    assert_eq!(rt.state.older(), Some(Older::Ask));
    wheel_up(&mut rt, 600.0);
    assert_eq!(rt.state.older(), Some(Older::Ask), "asked for them far from the top");
    rt.update(|s, tx| s.scroll_top(tx));
    run(&mut rt, 2.0);
    assert_eq!(rt.state.older(), Some(Older::Loading), "didn't ask at the top");
}

#[test]
fn does_not_ask_again_while_they_are_loading_or_when_there_are_none() {
    let mut rt = with_older();
    rt.update(|s, _| s.stream.as_mut().unwrap().has_more = false);
    rt.update(|s, tx| s.scroll_top(tx));
    run(&mut rt, 2.0);
    assert_eq!(rt.state.older(), None);
    assert!(!rt.state.stream.as_ref().unwrap().is_loading_older, "asked with nothing to ask for");
}

#[test]
fn keeps_the_reading_position_when_they_arrive_above() {
    let mut rt = long();
    wheel_up(&mut rt, 3000.0);
    run(&mut rt, 0.5);
    // The first message that is on screen, and where.
    let (id, y) = {
        let s = &rt.state;
        let m = s.session().messages.iter().find(|m| rt.find(&m.id).is_some_and(|(_, y)| y > 150.0 && y < 500.0)).expect("a row on screen");
        (m.id.clone(), rt.find(&m.id).unwrap().1)
    };
    // Older messages are prepended: ten short questions, as a page of history joins.
    rt.update(|s, _| {
        let (p, i) = s.open_at();
        for n in 0..10 {
            let older = crate::view::conversation::Message::user(format!("older-{n}"), "An older question that took a few words.");
            s.projects[p].sessions[i].messages.insert(0, older);
        }
        s.sync_open();
    });
    run(&mut rt, 0.5);
    let after = rt.find(&id).unwrap_or_else(|| panic!("{id} left the screen")).1;
    assert!((after - y).abs() < 2.0, "the row moved from {y} to {after}");
}

#[test]
fn progress_reports_the_newest_assistant_message_with_a_body_once_the_snapshot_has_loaded() {
    let mut rt = long();
    let (id, text) = rt.state.latest_answer().map(|(i, t)| (i.to_string(), t)).expect("an answer");
    let last = rt
        .state
        .session()
        .messages
        .iter()
        .rev()
        .find(|m| m.role == crate::view::conversation::Role::Assistant && !m.text().trim().is_empty());
    assert_eq!(Some(id.as_str()), last.map(|m| m.id.as_str()));
    assert!(!text.trim().is_empty());
    rt.update(|s, _| s.loading = true);
    assert!(rt.state.latest_answer().is_none(), "reported before the snapshot loaded");
}
