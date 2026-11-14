//! The state, driven as the connection drives it.
use super::*;
use crate::{State, spec};
use rune::{App, Env, Runtime};

fn app() -> Runtime<State> {
    let mut rt = App::new(spec()).headless_runtime((1280.0, 760.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(1.0 / 60.0);
    rt
}

fn list() -> Vec<crate::mock::Project> {
    let rows: Value = serde_json::from_str(&fixture("api-sessions.json")).unwrap();
    sessions::parse_list(&rows, 1_794_674_800_000)
}

#[test]
fn a_list_from_the_server_replaces_the_invented_one_and_opens_the_first_session() {
    let mut rt = app();
    rt.update(|s, tx| s.on_list(list(), tx));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.projects.len(), 1);
    assert_eq!(rt.state.session().title, "Fix pagination");
    assert!(rt.find("session-0-0").is_some());
    assert!(rt.find("session-1-0").is_none(), "the invented Lighthouse project is gone");
}

#[test]
fn a_snapshot_then_events_fill_the_open_conversation_and_the_status() {
    let mut rt = app();
    rt.update(|s, tx| s.on_list(list(), tx));
    let id = rt.state.open_id().unwrap();
    let live = frames_of("hub-turn-run-tests.jsonl");
    rt.update({
        let (id, snap) = (id.clone(), snapshot_of(&live));
        move |s, tx| s.on_snapshot(&id, create_state(&snap), tx)
    });
    let before = rt.state.session().messages.len();
    assert!(before >= 2, "the snapshot's messages are drawn");
    let events: Vec<Event> = live
        .iter()
        .filter_map(|f| match f {
            signalr::Frame::Event { topic, data } if topic.starts_with("session:") => Event::from_wire(data),
            _ => None,
        })
        .collect();
    // Up to the first tool call: busy, with a new prompt on screen.
    let cut = events.iter().position(|e| matches!(e, Event::PartUpdated(p) if p["type"] == "tool")).unwrap();
    rt.update({
        let (id, head) = (id.clone(), events[..cut].to_vec());
        move |s, tx| s.on_events(&id, head, tx)
    });
    assert_eq!(rt.state.session().status, Status::Working);
    assert!(rt.state.session().messages.len() > before);
    rt.update(move |s, tx| s.on_events(&id, events[cut..].to_vec(), tx));
    assert_eq!(rt.state.session().status, Status::Idle);
}

#[test]
fn events_for_a_session_that_is_no_longer_open_are_dropped() {
    let mut rt = app();
    rt.update(|s, tx| s.on_list(list(), tx));
    let before = rt.state.session().messages.len();
    rt.update(|s, tx| {
        s.on_snapshot("another-session", new_state(), tx);
        s.on_events("another-session", vec![Event::TurnStarted], tx);
    });
    assert_eq!(rt.state.session().messages.len(), before);
    assert_eq!(rt.state.session().status, Status::Idle);
}

#[test]
fn a_session_activity_event_updates_its_row() {
    let mut rt = app();
    rt.update(|s, tx| s.on_list(list(), tx));
    let id = rt.state.open_id().unwrap();
    rt.update(move |s, _| s.on_activity(&id, Status::NeedsInput));
    assert_eq!(rt.state.session().status, Status::NeedsInput);
}

#[test]
fn a_refreshed_list_keeps_the_open_session_and_its_conversation() {
    let mut rt = app();
    rt.update(|s, tx| s.on_list(list(), tx));
    let id = rt.state.open_id().unwrap();
    let snap = snapshot_of(&frames_of("hub-snapshot-after-turn.jsonl"));
    rt.update({
        let id = id.clone();
        move |s, tx| s.on_snapshot(&id, create_state(&snap), tx)
    });
    let shown = rt.state.session().messages.len();
    rt.update(|s, tx| s.on_list(list(), tx));
    assert_eq!(rt.state.open_id(), Some(id));
    assert_eq!(rt.state.session().messages.len(), shown);
}

#[test]
fn an_empty_list_leaves_a_window_that_still_draws() {
    let mut rt = app();
    rt.update(|s, tx| s.on_list(Vec::new(), tx));
    rt.frame(1.0 / 60.0);
    assert!(rt.find("rail-sessions").is_some());
    assert_eq!(rt.state.open_id(), None);
}

#[test]
fn the_server_argument_comes_out_before_rune_reads_the_rest() {
    let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>().into_iter();
    let (server, rest) = crate::split_args(args(&["--snap", "a.png", "--server", "http://127.0.0.1:6411", "--at", "1"]));
    assert_eq!(server.as_deref(), Some("http://127.0.0.1:6411"));
    assert_eq!(rest, ["--snap", "a.png", "--at", "1"]);
    assert_eq!(crate::split_args(args(&["--server=http://x:1"])).0.as_deref(), Some("http://x:1"));
    assert_eq!(crate::split_args(args(&["--snap", "a.png"])).0, None);
}
