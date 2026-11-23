//! Component 26, piece Q: other machines (rows 2.70-2.77 of docs/parity/26-parity-audit.md). A machine that comes back has its hub
//! connect at once, a new address pick moves the hub and is kept between runs, another machine's tree draws the lines beside
//! its forks, a parent on another machine opens from Back to parent, another machine's `workflow_run` events reach its runs, and
//! the token fields are masked. Hubs here are a fake on loopback; headless, nothing real is reached.
use crate::fleet::events::Event;
use crate::fleet::link::{Connection, Job, Link, Post};
use crate::fleet::machine::{Address, Kind};
use crate::fleet::machine_cache;
use crate::fleet::machine_routes;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::mock::machines::Case;
use crate::tests::{FRAME, app, click};
use crate::{State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::io::Write;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use tungstenite::Message;

const FALCON: &str = "m-falcon";

/// A machine's hub on loopback. While `up` is false it takes every connection and drops it (an attempt is counted); once up it
/// negotiates, accepts the socket, answers each call, and says `workflow_run` of run `run` after a `sessions` subscription.
struct Hub {
    base: String,
    up: Arc<AtomicBool>,
    attempts: Arc<AtomicUsize>,
    subscribed: Arc<AtomicUsize>,
}

fn run_json(id: &str) -> Value {
    json!({
        "id": id, "workflowId": "builtin:fix-a-bug", "workflowName": "Fix a bug", "title": "The pager skips the last page",
        "request": "The pager skips the last page", "status": "running", "currentStepId": "diagnose", "result": null,
        "repositoryPath": "/work/repo", "branch": "fleet/pager", "baseBranch": "main", "worktreePath": "/work/repo-worktrees/pager",
        "harnessType": "opencode", "createdAt": "2026-09-23T10:00:00Z", "updatedAt": "2026-09-23T10:05:00Z", "endedAt": null,
        "steps": [{"id": "diagnose", "title": "Diagnose", "kind": "agent", "state": "running", "visits": 1, "enabled": true}],
        "sessions": [{"sessionId": format!("{id}-s1"), "stepId": "diagnose", "visit": 1}]
    })
}

fn serve(mut stream: TcpStream, hub: &Hub, run: Option<&str>) {
    hub.attempts.fetch_add(1, Ordering::SeqCst);
    if !hub.up.load(Ordering::SeqCst) {
        return;
    }
    let mut peek = [0u8; 16];
    let n = stream.peek(&mut peek).unwrap_or(0);
    if peek[..n].starts_with(b"GET /hubs") {
        let Ok(mut ws) = tungstenite::accept(stream) else { return };
        while let Ok(Message::Text(text)) = ws.read() {
            for frame in text.split('\u{1e}').filter(|f| !f.is_empty()) {
                let call: Value = serde_json::from_str(frame).unwrap_or_default();
                let send = |ws: &mut tungstenite::WebSocket<TcpStream>, v: Value| drop(ws.send(Message::text(format!("{v}\u{1e}"))));
                let Some(target) = call["target"].as_str() else {
                    if call["protocol"] == "json" {
                        send(&mut ws, json!({}));
                    }
                    continue;
                };
                send(&mut ws, json!({"type": 3, "invocationId": call["invocationId"], "result": null}));
                if target == "SubscribeToSessionsTopicAsync" {
                    hub.subscribed.fetch_add(1, Ordering::SeqCst);
                    if let Some(run) = run {
                        let event = json!({"type": "workflow_run", "properties": run_json(run)});
                        send(&mut ws, json!({"type": 1, "target": "Event", "arguments": ["sessions", null, event]}));
                    }
                }
            }
        }
        return;
    }
    let heard = crate::test_http::read(&mut stream);
    let body = if heard.head.starts_with("POST /hubs/session-events/negotiate") {
        "{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}"
    } else if heard.head.starts_with("GET /api/workflows/runs") {
        "[]"
    } else {
        "{}"
    };
    let reply =
        format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
    let _ = stream.write_all(reply.as_bytes());
}

fn hub(up: bool, run: Option<&'static str>) -> Hub {
    let listener = crate::test_http::bind().expect("a port");
    let hub = Hub {
        base: format!("http://{}", listener.local_addr().expect("an address")),
        up: Arc::new(AtomicBool::new(up)),
        attempts: Arc::default(),
        subscribed: Arc::default(),
    };
    let shared =
        Arc::new(Hub { base: hub.base.clone(), up: hub.up.clone(), attempts: hub.attempts.clone(), subscribed: hub.subscribed.clone() });
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let shared = shared.clone();
            std::thread::spawn(move || serve(stream, &shared, run));
        }
    });
    hub
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
}

/// The app with a link to home (nothing listens there) and falcon listed at `falcon_base`.
fn rig(falcon_base: &str) -> Rig {
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| drop(jobs.lock().unwrap().send(job)));
    let home = Link::start_machine(HOME_MACHINE_KEY.to_string(), "http://127.0.0.1:9".to_string(), post);
    let mut rt = app();
    let base = falcon_base.to_string();
    rt.update(move |s, _| {
        s.link = Some(home);
        s.show_machines_only(Case::Several);
        s.machines.list.iter_mut().find(|m| m.id == FALCON).expect("falcon").base_url = base;
    });
    rt.frame(FRAME);
    Rig { rt, queue }
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("hubs {:?}", s.machines.hubs));
    }
}

#[test]
fn when_a_machine_comes_back_its_hub_connects_at_once() {
    let hub = hub(false, None);
    let mut rig = rig(&hub.base);
    // Opening a session there starts its hub, which now fails and backs off (1 s, 2 s, then 5 s).
    rig.rt.update(|s, _| drop(s.link_for_machine(FALCON)));
    rig.wait("the hub to have failed", |s| matches!(s.machines.hubs.get(FALCON), Some(Connection::Down(_))));
    rig.wait("its third attempt", |_| hub.attempts.load(Ordering::SeqCst) >= 3);
    let before = hub.attempts.load(Ordering::SeqCst);
    // The machine answers again: the read that finds it back tells the hub, which must not wait out its 5 s delay.
    hub.up.store(true, Ordering::SeqCst);
    let started = std::time::Instant::now();
    rig.rt.update(|s, _| s.link_if_any(FALCON).expect("a link").retry_now());
    rig.wait("the hub to be online", |s| s.machines.hubs.get(FALCON) == Some(&Connection::Online));
    assert!(started.elapsed() < std::time::Duration::from_secs(4), "took {:?} after {before} attempts", started.elapsed());
}

#[test]
fn another_machines_workflow_runs_follow_its_own_hub() {
    let hub = hub(true, Some("run-7"));
    let mut rig = rig(&hub.base);
    rig.rt.update(|s, _| s.workflow_runs_ensure_loaded(FALCON));
    rig.wait("the run its hub heard", |s| s.workflow_runs(FALCON).get("run-7").is_some());
    assert!(hub.subscribed.load(Ordering::SeqCst) >= 1, "its hub listened to the sessions topic");
    assert!(rig.rt.state.workflow_runs(HOME_MACHINE_KEY).get("run-7").is_none(), "and home's runs don't have it");
}

#[test]
fn a_run_event_of_another_machine_is_kept_only_once_its_runs_are_followed() {
    let mut rt = app();
    rt.update(|s, _| s.show_machines_only(Case::Several));
    let event = Event::from_wire(&json!({"type": "workflow_run", "properties": run_json("run-9")})).expect("an event");
    let again = event.clone();
    rt.update(move |s, _| s.on_workflow_event_on(FALCON, &again));
    assert!(rt.state.workflow_runs(FALCON).get("run-9").is_none(), "nobody asked for them yet");
    rt.update(|s, _| s.workflow_runs_ensure_loaded(FALCON));
    rt.update(move |s, _| s.on_workflow_event_on(FALCON, &event));
    assert!(rt.state.workflow_runs(FALCON).get("run-9").is_some());
}

#[test]
fn an_open_hub_follows_a_new_address_pick() {
    let (first, second) = (hub(true, None), hub(true, None));
    let mut rig = rig(&first.base);
    let (a, b) = (first.base.clone(), second.base.clone());
    rig.rt.update(move |s, _| {
        let at = |url: &str| Address { url: url.into(), kind: Kind::Lan, learned: false, last_result: None, last_tried_at: None };
        s.machines.list.iter_mut().find(|m| m.id == FALCON).expect("falcon").addresses = vec![at(&a), at(&b)];
        drop(s.link_for_machine(FALCON));
        assert_eq!(s.links.get(FALCON).expect("a link").base, a);
        machine_routes::use_address(s.machines.machine(FALCON).expect("falcon"), &b);
        s.follow_address(FALCON);
        assert_eq!(s.links.get(FALCON).expect("a link").base, b, "the link moved with the pick");
    });
    rig.wait("the hub to be online on the new address", |s| s.machines.hubs.get(FALCON) == Some(&Connection::Online));
}

#[test]
fn the_address_pick_is_kept_between_runs() {
    let id = "pick-kept-between-runs-machine";
    let url = "http://pick.example.test:2113";
    let mut machines = crate::fleet::machines::Machines::default();
    machines.list.push(crate::mock::machines::machine(id, "pick"));
    machine_routes::adopt_picks([(id.to_string(), url.to_string())].into());
    let text = machine_cache::to_json(&machines);
    assert_eq!(machine_cache::parse(&text).picks.get(id).map(String::as_str), Some(url));
}

// ─── the tree, parents and the masked fields ────────────────────────────────

#[test]
fn another_machines_fork_hangs_under_its_parent_with_lines_beside_it() {
    let mut rt = app();
    rt.update(|s, _| {
        s.show_machines(Case::Several, true);
        // Pinned, the parent would stand in the Pinned group and its fork in the project's.
        s.machines.patch_session(FALCON, "f-plan", |p| p.listing.pin_order = None);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    for key in ["machine-children-m-falcon-f-fork", "lineage-line-m-falcon-f-fork", "lineage-stub-m-falcon-f-fork"] {
        assert!(rt.find(key).is_some(), "no element keyed {key}");
    }
}

#[test]
fn back_to_parent_opens_a_parent_that_lives_on_another_machine() {
    let mut rt = app();
    rt.update(|s, tx| {
        s.show_machines(Case::Several, true);
        // A read remembers where each listed session lives.
        let ids: Vec<String> = s.machines.others[FALCON].sessions.iter().map(|x| x.id.clone()).collect();
        s.machines.remember_sessions(FALCON, &ids);
        s.open_machine_session(FALCON, "f-fork", tx);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.state.parent_listed("f-plan"), "the parent is in falcon's list");
    assert_eq!(rt.state.parent_title("f-plan").as_deref(), Some("Plan the lamp rewiring"));
    click(&mut rt, "session-lineage-link");
    rt.frame(FRAME);
    assert_eq!(rt.state.open_id().as_deref(), Some("f-plan"));
    assert_eq!(rt.state.machine_key_of("f-plan"), FALCON);
}

#[test]
fn the_token_fields_are_masked() {
    let mut rt = app();
    rt.update(|s, tx| {
        s.show_machines_only(Case::Several);
        s.open_add_form(tx);
    });
    assert!(rt.state.machine_page.add.token.is_secure(), "the Advanced token field of Add a machine");
    rt.update(|s, tx| s.start_machine_token(FALCON, tx));
    assert!(rt.state.machine_page.token.is_secure(), "the new token field of a machine's card");
}
