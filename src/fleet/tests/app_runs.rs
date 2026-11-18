//! Component 14, piece G: the apps Fleet runs for browser tabs. Fleet's `stores/__tests__/app-runs.test.ts` under its names,
//! over a fake server that records what it was asked, and `lib/__tests__/preview-bridge.test.ts` and the browser case of
//! `canvases.test.ts` (the reader, `browserPage`).
use super::super::app_runs::{AppStatus, RESTART_MARKER, address_for_port, current_run_lines};
use super::super::browser_tab::browser_page;
use super::super::canvases::{CanvasEvent, CanvasKind, Canvases, read_server_canvas, server_tab_id};
use super::super::events::{Event, Kind};
use super::super::link::{Job, Link, Post};
use super::reconnect::app;
use super::*;
use crate::State;
use crate::web::preview_bridge::{BridgeMessage, NavAction, PreviewHmr, app_address, nav_message, read_bridge_message};
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

type Answer = Box<dyn Fn(&str, &str) -> (u16, String) + Send + Sync>;

struct Fake {
    rt: rune::Runtime<State>,
    jobs: Receiver<Job>,
    asked: Receiver<(String, String)>,
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
                if !path.starts_with("/hubs/") && !path.starts_with("/api/sessions?") {
                    let _ = asked_tx.lock().unwrap().send((method.to_string(), path.to_string()));
                }
                let (status, body) = answer(method, path);
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
    rt.update(move |s, _| s.link = Some(link));
    Fake { rt, jobs, asked }
}

impl Fake {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        crate::test_wait::drive(&mut self.rt, &self.jobs, what, done, crate::test_wait::app_state);
    }

    /// The next request the server heard about an app.
    fn request(&self, what: &str) -> (String, String) {
        loop {
            let heard = crate::test_wait::recv(what, &self.asked);
            if heard.1.contains("/apps") {
                return heard;
            }
        }
    }

    fn event(&mut self, status: &str, reason: &str) {
        let payload = app_payload(status, reason);
        self.rt.update(move |s, _| s.dispatch(&Topic::Session("s1".into()), &[Event::Other(Kind::AppUpdated, payload)]));
    }
}

fn app_payload(status: &str, reason: &str) -> Value {
    let url = if status == "running" { json!("http://localhost:5173/") } else { Value::Null };
    json!({"sessionId": "s1", "appId": "app_1", "command": "npm run dev", "status": status, "url": url, "ports": [], "exitCode": null, "reason": reason})
}

fn response() -> Value {
    json!({"id": "app_1", "command": "npm run dev", "status": "starting", "exitCode": null, "url": null, "ports": [], "logs": []})
}

fn serving(body: Value) -> Answer {
    Box::new(move |_, _| (200, body.to_string()))
}

use super::super::state::Topic;

// ─── stores/__tests__/app-runs.test.ts ──────────────────────────────────────

#[test]
fn takes_an_app_from_its_events() {
    let mut f = start(serving(json!({})));
    f.event("running", "ready");
    let run = f.rt.state.apps.app("app_1").unwrap();
    assert_eq!(
        (run.status, run.url.as_deref(), run.reason.as_deref(), run.session_id.as_str()),
        (AppStatus::Running, Some("http://localhost:5173/"), Some("ready"), "s1")
    );
}

#[test]
fn doesnt_let_a_load_that_was_read_before_an_event_undo_it() {
    let (gate, held) = channel::<()>();
    let held = Mutex::new(held);
    let mut f = start(Box::new(move |_, path| {
        if path.ends_with("/apps/app_1") {
            let _ = held.lock().unwrap().recv();
        }
        (200, response().to_string())
    }));
    let landed = Arc::new(Mutex::new(None));
    let sink = landed.clone();
    f.rt.update(move |s, _| s.load_app("s1", "app_1", move |_, run| *sink.lock().unwrap() = Some(run)));
    f.request("the load");
    f.event("running", "ready");
    gate.send(()).unwrap();
    let sink = landed.clone();
    f.wait("the load's answer", move |_| sink.lock().unwrap().is_some());
    assert_eq!(f.rt.state.apps.app("app_1").unwrap().status, AppStatus::Running);
    let seen = landed.lock().unwrap().take().unwrap().unwrap().unwrap();
    assert_eq!(seen.status, AppStatus::Running, "the load hands back what the store holds, not what it read");
}

#[test]
fn returns_why_fleet_refused_a_start() {
    let mut f = start(Box::new(|_, _| (409, json!({"error": "This session already runs 3 apps, its limit."}).to_string())));
    let why = Arc::new(Mutex::new(None));
    let sink = why.clone();
    f.rt.update(move |s, _| s.act_on_app("s1", "app_1", "restart", move |_, e| *sink.lock().unwrap() = Some(e)));
    assert_eq!(f.request("the restart"), ("POST".into(), "/api/sessions/s1/apps/app_1/restart".into()));
    let sink = why.clone();
    f.wait("the refusal", move |_| sink.lock().unwrap().is_some());
    assert_eq!(why.lock().unwrap().clone().unwrap().as_deref(), Some("This session already runs 3 apps, its limit."));
}

#[test]
fn asks_only_for_output_it_doesnt_have_and_starts_over_when_fleet_lost_it() {
    let answers = Mutex::new(
        vec![
            json!({"lines": ["a", "b"], "next": 2}),
            json!({"lines": ["c"], "next": 3}),
            json!({"lines": [], "next": 1}),
            json!({"lines": ["x"], "next": 1}),
        ]
        .into_iter(),
    );
    let mut f = start(Box::new(move |_, path| {
        // The link's own calls (the hub, the list) must not use up the answers.
        if path.contains("/output") {
            (200, answers.lock().unwrap().next().unwrap_or(Value::Null).to_string())
        } else {
            (200, "{}".to_string())
        }
    }));
    for (after, want) in [(0, ["a", "b"].as_slice()), (2, ["a", "b", "c"].as_slice())] {
        f.rt.update(|s, _| s.fetch_app_output("s1", "app_1"));
        assert_eq!(f.request("the output").1, format!("/api/sessions/s1/apps/app_1/output?after={after}"));
        f.wait("the lines", |s| s.apps.output("app_1").is_some_and(|o| o.lines.len() == want.len()));
    }
    assert_eq!(f.rt.state.apps.output("app_1").unwrap().next, 3);
    f.rt.update(|s, _| s.fetch_app_output("s1", "app_1"));
    assert_eq!(f.request("the output").1, "/api/sessions/s1/apps/app_1/output?after=3");
    // The refetch is started by the answer, which lands when the app is driven.
    f.wait("the new lines", |s| s.apps.output("app_1").is_some_and(|o| o.lines == ["x"]));
    assert_eq!(f.request("asked again").1, "/api/sessions/s1/apps/app_1/output?after=0");
    assert_eq!(f.rt.state.apps.output("app_1").unwrap().next, 1);
}

#[test]
fn starts_a_command_and_returns_the_canvas_that_shows_it() {
    let mut f = start(serving(json!({"app": response(), "canvasId": "cv_1"})));
    let canvas = Arc::new(Mutex::new(None));
    let sink = canvas.clone();
    f.rt.update(move |s, _| s.start_command("s1", "npm run dev", move |_, id| *sink.lock().unwrap() = Some(id)));
    assert_eq!(f.request("the start"), ("POST".into(), "/api/sessions/s1/apps".into()));
    let sink = canvas.clone();
    f.wait("the canvas", move |_| sink.lock().unwrap().is_some());
    assert_eq!(canvas.lock().unwrap().clone().unwrap(), Ok("cv_1".to_string()));
    assert_eq!(f.rt.state.apps.app("app_1").unwrap().status, AppStatus::Starting);
}

#[test]
fn keeps_only_the_current_runs_output_for_the_panel() {
    let lines = |all: &[&str]| all.iter().map(|l| l.to_string()).collect::<Vec<_>>();
    assert_eq!(current_run_lines(&lines(&["old", "crash", RESTART_MARKER, "$ npm run dev", "ready"])), ["$ npm run dev", "ready"]);
    assert_eq!(current_run_lines(&lines(&["first run"])), ["first run"]);
}

#[test]
fn keeps_the_printed_addresses_it_loaded_when_an_event_comes_and_drops_them_on_a_restart() {
    let mut body = response();
    body["printedUrls"] = json!(["https://localhost:17155/login?t=abc"]);
    let mut f = start(serving(body));
    f.rt.update(|s, _| s.load_app("s1", "app_1", |_, _| {}));
    f.wait("the app", |s| s.apps.app("app_1").is_some());
    f.event("running", "ready");
    assert_eq!(f.rt.state.apps.app("app_1").unwrap().printed_urls, ["https://localhost:17155/login?t=abc"]);
    f.event("starting", "restarted");
    assert!(f.rt.state.apps.app("app_1").unwrap().printed_urls.is_empty());
}

#[test]
fn opens_a_port_at_the_link_the_app_printed_for_it_a_sign_in_link_first() {
    let printed: Vec<String> =
        ["https://localhost:17155/", "https://localhost:17155/login?t=abc", "http://localhost:5173/app/"].map(String::from).to_vec();
    assert_eq!(address_for_port(17155, &printed), "https://localhost:17155/login?t=abc");
    assert_eq!(address_for_port(5173, &printed), "http://localhost:5173/app/");
    assert_eq!(address_for_port(5199, &printed), "http://localhost:5199/");
    assert_eq!(address_for_port(443, &["https://localhost/".to_string()]), "https://localhost/");
}

// ─── what Fleet's code does around the store (row 14.65), not in a test of its own there ────

#[test]
fn a_page_fleet_reloaded_pulses_every_browser_tab_showing_that_app() {
    let mut f = start(serving(json!({})));
    let browser = |id: &str, app: Option<&str>| {
        let state = app.map_or(json!({"url": "http://localhost:5173/"}), |a| json!({"url": "http://localhost:5173/", "appId": a}));
        json!({"canvasId": id, "kind": "browser", "title": "Shop", "version": 1, "state": state})
    };
    f.rt.update(move |s, _| {
        for canvas in [browser("cv_a", Some("app_1")), browser("cv_b", Some("app_2")), browser("cv_c", None)] {
            s.canvases.apply_canvas_event("s1", CanvasEvent::Updated(read_server_canvas(&canvas).unwrap()));
        }
        s.clock = 7.0;
    });
    f.event("running", "ready");
    assert_eq!(f.rt.state.canvases.updated_at("canvas:cv_a"), None, "only a reload pulses");
    f.event("running", "reloaded");
    assert_eq!(f.rt.state.canvases.updated_at("canvas:cv_a"), Some(7.0));
    assert_eq!(f.rt.state.canvases.updated_at("canvas:cv_b"), None, "another app's tab");
    assert_eq!(f.rt.state.canvases.updated_at("canvas:cv_c"), None, "a tab with no app");
}

#[test]
fn an_app_that_the_session_doesnt_have_is_none() {
    let mut f = start(Box::new(|_, _| (404, String::new())));
    let landed = Arc::new(Mutex::new(None));
    let sink = landed.clone();
    f.rt.update(move |s, _| s.load_app("s1", "gone", move |_, run| *sink.lock().unwrap() = Some(run)));
    f.request("the load");
    let sink = landed.clone();
    f.wait("the answer", move |_| sink.lock().unwrap().is_some());
    assert_eq!(landed.lock().unwrap().take().unwrap(), Ok(None));
    assert!(f.rt.state.apps.app("gone").is_none());
}

#[test]
fn a_reconnect_loads_the_apps_it_knows_again() {
    let mut f = start(serving(response()));
    f.event("running", "ready");
    f.rt.update(|s, _| s.reload_apps("s1"));
    assert_eq!(f.request("the load again"), ("GET".into(), "/api/sessions/s1/apps/app_1".into()));
}

// ─── canvases.test.ts ───────────────────────────────────────────────────────

#[test]
fn shows_a_browser_canvas_as_a_browser_tab_with_its_page_app_and_title() {
    let mut c = Canvases::default();
    let canvas = json!({"canvasId": "cv_b", "kind": "browser", "title": "Bun Shop", "version": 1, "state": {"url": "http://localhost:5173/", "appId": "app_1"}});
    c.apply_canvas_event("s1", CanvasEvent::Updated(read_server_canvas(&canvas).unwrap()));
    let tab = c.session("s1").canvases.last().unwrap().clone();
    assert_eq!((tab.id.as_str(), tab.kind), (server_tab_id("cv_b").as_str(), CanvasKind::Browser));
    let page = tab.browser().unwrap();
    assert_eq!((page.url.as_str(), page.app_id.as_deref(), page.title.as_str()), ("http://localhost:5173/", Some("app_1"), "Bun Shop"));
    assert_eq!(tab.server.unwrap().version, 1);
    assert!(c.session("s1").canvases[0].browser().is_none(), "another kind has none");
}

#[test]
fn reads_a_browser_canvass_state_and_fills_what_is_missing() {
    assert_eq!(browser_page(&json!({"url": 5, "appId": ""})).url, "");
    assert_eq!(browser_page(&json!({"url": 5, "appId": ""})).app_id, None);
    assert_eq!(browser_page(&json!(null)).url, "");
    assert_eq!(browser_page(&json!({"appId": "app_1"})).app_id.as_deref(), Some("app_1"));
}

// ─── lib/__tests__/preview-bridge.test.ts ───────────────────────────────────

#[test]
fn preview_bridge_v1_reads_the_pages_hello_location_and_update() {
    let hello = read_bridge_message(&json!({"fleet": 1, "type": "hello", "hmr": "vite", "href": "http://p:1/", "title": "Shop"}));
    assert_eq!(hello, Some(BridgeMessage::Hello { hmr: PreviewHmr::Vite, href: "http://p:1/".into(), title: "Shop".into() }));
    let location = read_bridge_message(&json!({"fleet": 1, "type": "location", "href": "http://p:1/cart"}));
    assert_eq!(location, Some(BridgeMessage::Location { href: "http://p:1/cart".into(), title: String::new() }));
    assert_eq!(read_bridge_message(&json!({"fleet": 1, "type": "update"})), Some(BridgeMessage::Update));
}

#[test]
fn preview_bridge_v1_reads_a_link_the_page_wants_opened_in_this_tab_unless_it_says_a_new_one() {
    let open = |new_tab| Some(BridgeMessage::Open { href: "http://localhost:5199/".into(), new_tab });
    assert_eq!(read_bridge_message(&json!({"fleet": 1, "type": "open", "href": "http://localhost:5199/", "newTab": true})), open(true));
    assert_eq!(read_bridge_message(&json!({"fleet": 1, "type": "open", "href": "http://localhost:5199/"})), open(false));
    assert_eq!(read_bridge_message(&json!({"fleet": 1, "type": "open", "newTab": true})), None);
}

#[test]
fn preview_bridge_v1_takes_a_hot_reload_client_it_doesnt_know_as_none() {
    let hello = read_bridge_message(&json!({"fleet": 1, "type": "hello", "hmr": "parcel", "href": "http://p:1/"}));
    assert!(matches!(hello, Some(BridgeMessage::Hello { hmr: PreviewHmr::None, .. })));
}

#[test]
fn preview_bridge_v1_ignores_other_versions_types_and_senders() {
    for data in [
        json!({"fleet": 2, "type": "update"}),
        json!({"fleet": 1, "type": "error", "message": "boom"}),
        json!({"type": "fleet-browser:location", "href": "http://p:1/"}),
        json!({"fleet": 1, "type": "location"}),
        json!("webpackHotUpdate"),
        json!(null),
    ] {
        assert_eq!(read_bridge_message(&data), None, "{data}");
    }
}

#[test]
fn preview_bridge_v1_sends_navigation_as_v1() {
    assert_eq!(nav_message(NavAction::Back), json!({"fleet": 1, "type": "nav", "action": "back"}));
}

#[test]
fn preview_bridge_v1_shows_the_apps_own_address_for_the_previews_page() {
    assert_eq!(
        app_address("http://p1.localhost:41234/cart?x=1#top", "http://p1.localhost:41234", "http://localhost:5173"),
        "http://localhost:5173/cart?x=1#top"
    );
    assert_eq!(app_address("about:blank", "http://p1.localhost:41234", "http://localhost:5173"), "about:blank");
}
