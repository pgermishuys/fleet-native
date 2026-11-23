//! The page canvases (component 4, piece F): `page-choice.test.ts`'s `isChoiceLive` under its name, the canvas reader and
//! store, `use-waiting-page`'s rule, and a load over a fake server.
use super::super::events::{Event, Kind};
use super::super::link::{Job, Link, Post};
use super::super::pages::*;
use super::super::sessions;
use super::super::state::Topic;
use super::reconnect::app;
use super::*;
use crate::State;
use std::io::Write;
use std::sync::Mutex;
use std::sync::mpsc::channel;

fn choice(status: &str) -> Value {
    json!({"callId": "c1", "status": status, "at": "2026-10-16T12:00:00Z"})
}

fn canvas(id: &str, version: i64, choice: Value) -> Value {
    json!({"canvasId": id, "kind": "page", "title": "Layouts", "version": version, "state": {"pageId": "p", "entry": "index.html", "choice": choice}})
}

fn waiting() -> Option<PageChoice> {
    read_page_choice(&choice("waiting"))
}

// page-choice.test.ts: "isChoiceLive is live while the session is in a turn, not when idle or unknown"
#[test]
fn is_live_while_the_session_is_in_a_turn_not_when_idle_or_unknown() {
    let waiting = waiting();
    for status in ["busy", "retry", "waiting_input", "delegating"] {
        assert!(is_choice_live(waiting.as_ref(), Some(status)), "{status}");
    }
    assert!(!is_choice_live(waiting.as_ref(), Some("idle")));
    assert!(!is_choice_live(waiting.as_ref(), None));
    assert!(!is_choice_live(read_page_choice(&choice("answered")).as_ref(), Some("busy")));
}

#[test]
fn reads_a_choice_and_nothing_malformed() {
    let read = read_page_choice(&json!({"callId": "c1", "status": "not-answered", "reason": "skipped", "at": "t"})).unwrap();
    assert_eq!((read.status, read.reason.as_deref()), (ChoiceStatus::NotAnswered, Some("skipped")));
    for bad in [
        json!(null),
        json!({"status": "waiting", "at": "t"}),
        json!({"callId": "c", "status": "later", "at": "t"}),
        json!({"callId": "c", "status": "waiting"}),
    ] {
        assert!(read_page_choice(&bad).is_none(), "{bad}");
    }
}

#[test]
fn only_a_page_canvas_is_a_page() {
    assert!(read_page_canvas(&canvas("c", 1, choice("waiting"))).is_some());
    assert!(read_page_canvas(&json!({"canvasId": "c", "kind": "browser", "title": "x", "version": 1, "state": {}})).is_none());
    assert!(read_page_canvas(&canvas("", 1, choice("waiting"))).is_none());
    assert!(read_page_canvas(&json!({"canvasId": "c", "kind": "page", "title": "x", "version": 1, "state": {}})).unwrap().choice.is_none());
}

fn updated(session: &str, canvas: Value) -> Event {
    let mut payload = canvas;
    payload["sessionId"] = json!(session);
    Event::Other(Kind::CanvasUpdated, payload)
}

fn closed(session: &str, id: &str) -> Event {
    Event::Other(Kind::CanvasClosed, json!({"sessionId": session, "canvasId": id}))
}

fn status_of(s: &State, session: &str) -> Vec<(String, i64, Option<ChoiceStatus>)> {
    s.pages.of(session).iter().map(|c| (c.id.clone(), c.version, c.choice.as_ref().map(|x| x.status))).collect()
}

#[test]
fn canvas_events_add_redraw_and_remove_never_going_back_a_version() {
    let mut rt = app();
    rt.update(|s, _| {
        let topic = Topic::Session("s-pages".into());
        s.dispatch(&topic, &[updated("s-pages", canvas("c1", 2, choice("waiting")))]);
        assert_eq!(status_of(s, "s-pages"), [("c1".to_string(), 2, Some(ChoiceStatus::Waiting))]);
        s.dispatch(&topic, &[updated("s-pages", canvas("c1", 1, choice("answered")))]);
        assert_eq!(status_of(s, "s-pages")[0].2, Some(ChoiceStatus::Waiting), "an older version is dropped");
        s.dispatch(&topic, &[updated("s-pages", canvas("c1", 3, choice("answered")))]);
        assert_eq!(status_of(s, "s-pages")[0].2, Some(ChoiceStatus::Answered));
        s.dispatch(&topic, &[closed("s-pages", "c1")]);
        assert!(s.pages.of("s-pages").is_empty());
    });
}

#[test]
fn a_load_does_not_undo_what_an_event_made_newer_or_a_close_that_came_after_it() {
    let mut pages = Pages::default();
    let g = pages.begin_load("s");
    pages.updated("s", read_page_canvas(&canvas("c1", 5, choice("answered"))).unwrap());
    pages.loaded("s", g, vec![read_page_canvas(&canvas("c1", 4, choice("waiting"))).unwrap()]);
    assert_eq!(pages.of("s")[0].choice.as_ref().unwrap().status, ChoiceStatus::Answered);
    let g = pages.begin_load("s");
    pages.closed("s", "c1");
    pages.loaded("s", g, vec![read_page_canvas(&canvas("c1", 6, choice("waiting"))).unwrap()]);
    assert!(pages.of("s").is_empty(), "a load asked for before the close is older than it");
}

// use-waiting-page.ts: the session is stopped on you and a page waits.
#[test]
fn a_page_waits_only_while_the_session_is_stopped_on_you() {
    use crate::mock::Status;
    let mut rt = app();
    rt.update(|s, _| {
        s.set_open((0, 0));
        s.pages.set("mock.0.0", read_page_canvas(&canvas("c1", 1, choice("waiting"))).into_iter().collect());
        s.projects[0].sessions[0].status = Status::NeedsInput;
        assert_eq!(s.waiting_page().map(|c| c.id.as_str()), Some("c1"));
        s.projects[0].sessions[0].status = Status::Idle;
        assert!(s.waiting_page().is_none(), "a restart leaves 'waiting' behind, idle");
        s.projects[0].sessions[0].status = Status::NeedsInput;
        s.pages.set("mock.0.0", read_page_canvas(&canvas("c1", 1, choice("answered"))).into_iter().collect());
        assert!(s.waiting_page().is_none());
    });
}

#[test]
fn loads_the_canvases_when_the_session_opens() {
    let listener = crate::test_http::bind().unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let heard = crate::test_http::read(&mut stream);
                if heard.method.is_empty() {
                    return;
                }
                let request = heard.head.lines().next().unwrap_or("").to_string();
                let path = request.split(' ').nth(1).unwrap_or("");
                let (status, body) = if path.starts_with("/hubs/session-events/negotiate") {
                    ("200 OK", "{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}".to_string())
                } else if path.starts_with("/api/sessions?") || path == "/api/sessions" {
                    ("200 OK", fixture("api-sessions.json"))
                } else if path.ends_with("/canvases") {
                    ("200 OK", json!([canvas("c1", 2, choice("waiting")), {"canvasId": "b", "kind": "browser", "title": "x", "version": 1, "state": {}}]).to_string())
                } else {
                    ("404 Not Found", "{}".to_string())
                };
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                );
            });
        }
    });
    let (tx, jobs) = channel::<Job>();
    let tx = Mutex::new(tx);
    let post = Post::new(move |job| {
        let _ = tx.lock().unwrap().send(job);
    });
    let mut rt = app();
    let link = Link::start(base, post);
    rt.update(move |s, tx| {
        s.link = Some(link);
        s.on_list(sessions::parse_list(&serde_json::from_str(&fixture("api-sessions.json")).unwrap(), 0), tx);
    });
    let id = rt.state.open_id().unwrap();
    crate::test_wait::drive(&mut rt, &jobs, "the canvases to load", |s| !s.pages.of(&id).is_empty(), crate::test_wait::app_state);
    assert_eq!(status_of(&rt.state, &id), [("c1".to_string(), 2, Some(ChoiceStatus::Waiting))], "the browser canvas is component 14's");
}
