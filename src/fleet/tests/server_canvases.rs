//! Component 10, piece C: the server's canvases as tabs. Fleet's `stores/__tests__/canvases.test.ts` ("useCanvasesStore server
//! canvases"), `lib/__tests__/server-canvas.test.ts` (the kinds) and `composables/__tests__/use-server-canvases.test.ts`, under
//! their names; the load, the close and the focus run against a fake server that records what it was asked.
use super::super::canvases::*;
use super::super::events::{Event, Kind};
use super::super::link::{Job, Link, Post};
use super::super::sessions;
use super::super::state::Topic;
use super::reconnect::app;
use super::*;
use crate::State;
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

fn browser(id: &str, version: i64, label: &str) -> Value {
    json!({"canvasId": id, "kind": "browser", "title": format!("Browser {id}"), "version": version, "state": {"url": format!("http://localhost:5173/{label}")}})
}

fn page(id: &str, version: i64) -> Value {
    json!({"canvasId": id, "kind": "page", "title": "Layouts", "version": version, "state": {"pageId": "p", "entry": "index.html"}})
}

fn retired(kind: &str) -> Value {
    json!({"canvasId": format!("cv_{kind}"), "kind": kind, "title": "Old", "version": 1, "state": {}})
}

fn list(items: &[Value]) -> Vec<ServerCanvas> {
    items.iter().filter_map(read_server_canvas).collect()
}

fn server_tabs(c: &Canvases, session: &str) -> Vec<(String, i64)> {
    c.session(session).canvases.iter().filter_map(|t| t.server.as_ref().map(|k| (t.id.clone(), k.version))).collect()
}

fn updated(c: &mut Canvases, session: &str, canvas: &Value) {
    if let Some(canvas) = read_server_canvas(canvas) {
        c.apply_canvas_event(session, CanvasEvent::Updated(canvas));
    }
}

// ─── lib/server-canvas.test.ts ──────────────────────────────────────────────

#[test]
fn knows_browser_and_page_canvases_and_not_the_retired_diagram_and_sequence_kinds() {
    assert!(read_server_canvas(&browser("a", 1, "x")).is_some());
    assert!(read_server_canvas(&page("a", 1)).is_some());
    for kind in ["diagram", "sequence", "terminal"] {
        assert!(read_server_canvas(&retired(kind)).is_none(), "{kind}");
    }
}

// ─── stores/canvases.test.ts, "useCanvasesStore server canvases" ────────────

#[test]
fn adds_a_tab_for_canvas_updated_and_brings_it_forward_on_canvas_focused() {
    let mut c = Canvases::default();
    updated(&mut c, "s1", &browser("cv_1", 1, "A"));
    let tabs = c.session("s1");
    assert_eq!(tabs.ids(), ["changes", "files", "canvas:cv_1"]);
    assert_eq!(tabs.active_id, "changes", "an update doesn't take focus");
    c.apply_canvas_event("s1", CanvasEvent::Focused("cv_1".into()));
    assert_eq!(c.session("s1").active_id, "canvas:cv_1");
}

#[test]
fn redraws_the_same_tab_in_place_on_a_later_version() {
    let mut c = Canvases::default();
    updated(&mut c, "s1", &browser("cv_1", 1, "A"));
    updated(&mut c, "s1", &browser("cv_2", 1, "B"));
    updated(&mut c, "s1", &browser("cv_1", 2, "A2"));
    assert_eq!(server_tabs(&c, "s1"), [("canvas:cv_1".to_string(), 2), ("canvas:cv_2".to_string(), 1)]);
    let state = c.session("s1").canvases[2].server.clone().unwrap().state;
    assert_eq!(state["url"], "http://localhost:5173/A2");
}

#[test]
fn ignores_an_update_older_than_the_tab_it_would_replace() {
    let mut c = Canvases::default();
    updated(&mut c, "s1", &browser("cv_1", 3, "new"));
    updated(&mut c, "s1", &browser("cv_1", 2, "old"));
    assert_eq!(server_tabs(&c, "s1"), [("canvas:cv_1".to_string(), 3)]);
}

#[test]
fn removes_the_tab_on_canvas_closed_and_a_second_close_is_a_no_op() {
    let mut c = Canvases::default();
    updated(&mut c, "s1", &browser("cv_1", 1, "A"));
    c.apply_canvas_event("s1", CanvasEvent::Focused("cv_1".into()));
    c.apply_canvas_event("s1", CanvasEvent::Closed("cv_1".into()));
    assert_eq!(c.session("s1").ids(), ["changes", "files"]);
    assert_eq!(c.session("s1").active_id, "files", "the left neighbour takes over");
    c.apply_canvas_event("s1", CanvasEvent::Closed("cv_1".into()));
    assert_eq!(c.session("s1").ids(), ["changes", "files"]);
}

#[test]
fn ignores_focus_for_a_canvas_that_isnt_open() {
    let mut c = Canvases::default();
    c.apply_canvas_event("s1", CanvasEvent::Focused("cv_none".into()));
    assert_eq!(c.session("s1").active_id, "changes");
}

#[test]
fn skips_the_retired_diagram_and_sequence_kinds_which_an_older_server_may_still_send() {
    let mut c = Canvases::default();
    updated(&mut c, "s1", &retired("diagram"));
    updated(&mut c, "s1", &retired("sequence"));
    assert_eq!(c.session("s1").ids(), ["changes", "files"]);
}

#[test]
fn skips_a_canvas_kind_it_cant_show() {
    let mut c = Canvases::default();
    updated(&mut c, "s1", &retired("terminal"));
    c.set_server_canvases("s1", list(&[retired("terminal"), browser("cv_1", 1, "A")]));
    assert_eq!(c.session("s1").ids(), ["changes", "files", "canvas:cv_1"]);
}

#[test]
fn replaces_only_the_server_canvases_when_a_list_loads() {
    let mut c = Canvases::default();
    c.set_server_canvases("s1", list(&[browser("cv_1", 1, "A"), browser("cv_2", 1, "B")]));
    c.open("s1", CanvasKind::Context);
    // cv_1 is redrawn in place, cv_2 is gone, cv_3 is new at the end; Context keeps its place.
    c.set_server_canvases("s1", list(&[browser("cv_3", 1, "C"), browser("cv_1", 2, "A2")]));
    let tabs = c.session("s1");
    assert_eq!(tabs.ids(), ["changes", "files", "canvas:cv_1", "context", "canvas:cv_3"]);
    assert_eq!(server_tabs(&c, "s1")[0].1, 2);
    c.apply_canvas_event("s1", CanvasEvent::Focused("cv_3".into()));
    c.set_server_canvases("s1", list(&[]));
    assert_eq!(c.session("s1").active_id, "changes", "the active tab went, so the first takes over");
}

#[test]
fn shows_a_browser_canvas_as_a_browser_tab_with_its_page_app_and_title() {
    let mut c = Canvases::default();
    updated(
        &mut c,
        "s1",
        &json!({"canvasId": "cv_1", "kind": "browser", "title": "Shop", "version": 1, "state": {"url": "http://localhost:5173/", "appId": "app_1"}}),
    );
    let tab = c.session("s1").canvases[2].clone();
    assert_eq!((tab.kind, tab.title().as_str()), (CanvasKind::Browser, "Shop"));
    assert_eq!(tab.server.unwrap().state["appId"], "app_1");
}

#[test]
fn shows_a_page_canvas_as_a_page_tab_with_fleets_copy_and_its_title() {
    let mut c = Canvases::default();
    updated(&mut c, "s1", &page("cv_2", 1));
    let tab = c.session("s1").canvases[2].clone();
    assert_eq!((tab.kind, tab.id.as_str(), tab.title().as_str()), (CanvasKind::Page, "canvas:cv_2", "Layouts"));
    assert_eq!(tab.server.unwrap().state["pageId"], "p");
}

#[test]
fn titles_a_page_canvass_tab_and_frame_without_them() {
    let mut c = Canvases::default();
    updated(&mut c, "s1", &json!({"canvasId": "cv_2", "kind": "page", "title": "\u{1b}[32mLayouts\u{1b}[0m", "version": 1, "state": {}}));
    assert_eq!(c.session("s1").canvases[2].title(), "Layouts");
}

// canvas-registry.test.ts: "a server canvas of a kind the client does not know › is left out of the session's canvases"
#[test]
fn a_server_canvas_of_a_kind_the_client_does_not_know_is_left_out_of_the_sessions_canvases() {
    let mut c = Canvases::default();
    c.set_server_canvases("s1", list(&[json!({"canvasId": "cv_x", "kind": "hologram", "title": "x", "version": 1, "state": {}})]));
    assert_eq!(c.session("s1").ids(), ["changes", "files"]);
}

// ─── composables/use-server-canvases.test.ts, over a fake server ────────────

type Answer = Box<dyn Fn(&str, &str) -> (u16, String) + Send + Sync>;

/// A server that records `(method, path)` of each request before answering it, and the app talking to it.
struct Fake {
    rt: rune::Runtime<State>,
    jobs: Receiver<Job>,
    asked: Receiver<(String, String)>,
    id: String,
}

fn start(answer: Answer) -> Fake {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (asked_tx, asked) = channel::<(String, String)>();
    let (asked_tx, answer) = (Arc::new(Mutex::new(asked_tx)), Arc::new(answer));
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let (asked_tx, answer) = (asked_tx.clone(), answer.clone());
            std::thread::spawn(move || {
                let heard = crate::test_http::read(&mut stream);
                if heard.method.is_empty() {
                    return;
                }
                let request = heard.head.lines().next().unwrap_or("").to_string();
                let mut words = request.split(' ');
                let (method, path) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
                let (status, body) = if path.starts_with("/hubs/session-events/negotiate") {
                    (200, "{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}".to_string())
                } else if path.starts_with("/api/sessions?") || path == "/api/sessions" {
                    (200, fixture("api-sessions.json"))
                } else {
                    if path.contains("/canvases") {
                        let _ = asked_tx.lock().unwrap().send((method.to_string(), path.to_string()));
                    }
                    answer(method, path)
                };
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
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
    Fake { rt, jobs, asked, id }
}

impl Fake {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        crate::test_wait::drive(&mut self.rt, &self.jobs, what, done, crate::test_wait::app_state);
    }

    /// Waits for the next request about canvases.
    fn request(&self, what: &str) -> (String, String) {
        crate::test_wait::recv(what, &self.asked)
    }

    fn tabs(&self) -> Vec<(String, i64)> {
        server_tabs(&self.rt.state.canvases, &self.id)
    }

    fn event(&mut self, event: Event) {
        let topic = Topic::Session(self.id.clone());
        self.rt.update(move |s, _| s.dispatch(&topic, &[event]));
    }
}

fn updated_event(session: &str, canvas: &Value) -> Event {
    let mut payload = canvas.clone();
    payload["sessionId"] = json!(session);
    Event::Other(Kind::CanvasUpdated, payload)
}

fn serving(items: Vec<Value>) -> Answer {
    Box::new(move |_, path| if path.ends_with("/canvases") { (200, Value::Array(items.clone()).to_string()) } else { (204, String::new()) })
}

#[test]
fn loads_the_sessions_canvases_and_applies_its_canvas_events() {
    let mut f = start(serving(vec![browser("cv_1", 1, "A")]));
    let id = f.id.clone();
    assert_eq!(f.request("the load").0, "GET");
    f.wait("the tab", |s| !server_tabs(&s.canvases, &id).is_empty());
    assert_eq!(f.tabs(), [("canvas:cv_1".to_string(), 1)]);
    f.event(updated_event(&id, &browser("cv_1", 2, "A2")));
    assert_eq!(f.tabs(), [("canvas:cv_1".to_string(), 2)]);
}

#[test]
fn replays_events_that_arrive_while_the_list_loads_so_an_older_list_cant_undo_them() {
    let (gate, held) = channel::<()>();
    let held = Mutex::new(held);
    let mut f = start(Box::new(move |_, path| {
        if path.ends_with("/canvases") {
            let _ = held.lock().unwrap().recv_timeout(crate::test_wait::BOUND);
            (200, json!([browser("cv_1", 1, "A")]).to_string())
        } else {
            (204, String::new())
        }
    }));
    let id = f.id.clone();
    f.request("the load, still held");
    f.event(updated_event(&id, &browser("cv_1", 2, "A2")));
    f.event(updated_event(&id, &browser("cv_new", 1, "N")));
    gate.send(()).unwrap();
    f.wait("the list to land", |s| s.server_loads_idle());
    assert_eq!(f.tabs(), [("canvas:cv_1".to_string(), 2), ("canvas:cv_new".to_string(), 1)]);
}

#[test]
fn ignores_canvases_of_a_retired_kind_from_the_list_and_from_live_events() {
    let mut f = start(serving(vec![retired("diagram"), browser("cv_1", 1, "A"), retired("sequence")]));
    let id = f.id.clone();
    f.wait("the tab", |s| !server_tabs(&s.canvases, &id).is_empty());
    f.event(updated_event(&id, &retired("diagram")));
    f.event(updated_event(&id, &retired("sequence")));
    assert_eq!(f.tabs(), [("canvas:cv_1".to_string(), 1)]);
}

#[test]
fn loads_again_on_reconnect_since_canvas_events_arent_replayed() {
    let mut f = start(serving(vec![browser("cv_1", 1, "A")]));
    let id = f.id.clone();
    f.request("the first load");
    f.wait("the tab", |s| !server_tabs(&s.canvases, &id).is_empty());
    f.rt.update(|s, _| s.on_reconnected());
    assert_eq!(f.request("the load after the reconnect").0, "GET");
}

#[test]
fn loads_again_when_the_session_changes() {
    let mut f = start(serving(vec![]));
    f.request("the first load");
    f.rt.update(|s, _| s.load_server_canvases("s2"));
    assert_eq!(f.request("the second session's load").1, "/api/sessions/s2/canvases");
}

#[test]
fn a_load_that_was_overtaken_by_a_newer_one_is_dropped() {
    let mut f = start(serving(vec![browser("cv_1", 1, "A")]));
    let id = f.id.clone();
    f.rt.update(|s, _| {
        let id = s.open_id().unwrap();
        s.load_server_canvases(&id);
        s.load_server_canvases(&id);
    });
    f.wait("the tab", |s| !server_tabs(&s.canvases, &id).is_empty());
    assert_eq!(f.tabs(), [("canvas:cv_1".to_string(), 1)]);
}

#[test]
fn a_load_the_server_refuses_changes_nothing() {
    let mut f = start(Box::new(|_, _| (500, "{}".to_string())));
    let id = f.id.clone();
    f.rt.update(|s, _| s.canvases.set_server_canvases(&id, list(&[browser("cv_1", 1, "A")])));
    f.request("the load");
    f.wait("the load to end", |s| s.server_loads_idle());
    assert_eq!(f.tabs(), [("canvas:cv_1".to_string(), 1)]);
}

// ─── closeServerCanvas, focusServerCanvas ───────────────────────────────────

#[test]
fn removes_the_tab_at_once_and_asks_the_server_to_close_it() {
    let mut f = start(serving(vec![browser("cv_1", 1, "A")]));
    let id = f.id.clone();
    f.wait("the tab", |s| !server_tabs(&s.canvases, &id).is_empty());
    f.rt.update(|s, _| s.close_server_canvas("cv_1"));
    assert!(f.tabs().is_empty(), "the tab goes before the server answers");
    loop {
        let (method, path) = f.request("the DELETE");
        if method == "DELETE" {
            assert_eq!(path, format!("/api/sessions/{id}/canvases/cv_1"));
            break;
        }
    }
}

#[test]
fn brings_the_tab_back_when_the_server_refuses() {
    let mut f = start(Box::new(|method, path| match (method, path.ends_with("/canvases")) {
        ("DELETE", _) => (500, json!({"error": "nope"}).to_string()),
        (_, true) => (200, json!([browser("cv_1", 1, "A")]).to_string()),
        _ => (204, String::new()),
    }));
    let id = f.id.clone();
    f.wait("the tab", |s| !server_tabs(&s.canvases, &id).is_empty());
    f.rt.update(|s, _| s.close_server_canvas("cv_1"));
    assert!(f.tabs().is_empty());
    f.wait("the tab to come back", |s| !server_tabs(&s.canvases, &id).is_empty());
    assert_eq!(f.tabs(), [("canvas:cv_1".to_string(), 1)]);
}

#[test]
fn brings_a_canvas_forward_here_and_through_fleet_which_reopens_a_closed_one() {
    let mut f = start(serving(vec![browser("cv_1", 1, "A"), browser("cv_2", 1, "B")]));
    let id = f.id.clone();
    f.wait("both tabs", |s| server_tabs(&s.canvases, &id).len() == 2);
    f.rt.update(|s, _| s.focus_server_canvas("cv_1"));
    assert_eq!(f.rt.state.canvases.session(&id).active_id, "canvas:cv_1", "at once");
    loop {
        let (method, path) = f.request("the POST");
        if method == "POST" {
            assert_eq!(path, format!("/api/sessions/{id}/canvases/cv_1/focus"));
            break;
        }
    }
}

#[test]
fn focusing_a_canvas_whose_tab_is_closed_waits_for_fleet_to_reopen_it() {
    let mut f = start(serving(vec![]));
    let id = f.id.clone();
    f.rt.update(|s, _| s.focus_server_canvas("cv_1"));
    assert_eq!(f.rt.state.canvases.session(&id).active_id, "changes", "no tab yet");
    f.event(updated_event(&id, &browser("cv_1", 2, "A")));
    f.event(Event::Other(Kind::CanvasFocused, json!({"sessionId": id, "canvasId": "cv_1"})));
    assert_eq!(f.rt.state.canvases.session(&id).active_id, "canvas:cv_1");
}
