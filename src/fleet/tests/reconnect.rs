//! Reconnecting, kept streams and the partial banner (rows 5.1-5.7). The reducer case is weave-fleet's
//! `domain-event-reducer.test.ts`, under its name; the rest are new, because Fleet tests its socket in a browser.
use super::super::link::{Connection, Job, Link, Post};
use super::super::topics::{KEPT_SESSIONS, Kept, Topics, reconnect_delay};
use super::*;
use crate::{State, spec};
use rune::{App, Env, Runtime};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};
use tungstenite::Message;

// ─── the reducer ────────────────────────────────────────────────────────────

#[test]
fn keeps_live_stream_and_reconnect_snapshot_plus_pending_events_equivalent_without_duplicate_messages() {
    let user = life("user-message-1", "user", 1000, Some("Explain reconnect parity."), Some("user-text-1"));
    let created = life("assistant-message-1", "assistant", 1000, Some("Reconnect "), Some("assistant-text-1"));
    let mut completed = life("assistant-message-1", "assistant", 1000, Some("Reconnect parity is preserved."), Some("assistant-text-1"));
    completed.info["time"]["completed"] = json!(2000);
    completed.info["cost"] = json!(0.042);
    completed.info["tokens"] = json!({"input": 17, "output": 23, "reasoning": 5});
    let pending = vec![delta("assistant-message-1", "assistant-text-1", "parity is preserved.", None), Event::MessageUpdated(completed)];

    let mut live = new_state();
    for e in [Event::MessageCreated(user.clone()), Event::MessageCreated(created.clone())].iter().chain(&pending) {
        apply(&mut live, e);
    }
    let mut snap = snapshot("idle", vec![user, created]);
    snap["lastEventId"] = json!(2);
    let reconnected = apply_all(create_state(&snap), pending);

    assert_eq!(reconnected.messages, live.messages);
    assert_eq!(reconnected.messages.iter().map(|m| m.role).collect::<Vec<_>>(), [Role::User, Role::Assistant]);
    assert_eq!(ids(&reconnected), ["user-message-1", "assistant-message-1"]);
}

#[test]
fn a_snapshot_says_whether_it_is_partial() {
    assert!(!new_state().is_partial);
    let mut snap = snapshot("idle", vec![]);
    snap["isPartial"] = json!(true);
    assert!(create_state(&snap).is_partial);
}

// ─── the schedule, the topic set, kept streams ──────────────────────────────

#[test]
fn the_reconnect_delays_end_on_the_thirty_second_tail() {
    let secs: Vec<u64> = (0..8).map(|n| reconnect_delay(n).as_secs()).collect();
    assert_eq!(secs, [1, 2, 5, 10, 30, 30, 30, 30]);
}

#[test]
fn every_topic_is_subscribed_again_after_a_connect() {
    let mut t = Topics::default();
    assert!(t.watch("b"), "a watched topic nobody holds yet must be subscribed");
    assert_eq!(t.set_open(Some("a".into())), None);
    assert!(!t.watch("a"), "the open topic is already subscribed");
    assert_eq!(t.all(), ["a", "b"]);
    // Leaving `a` while it is also watched keeps the subscription.
    assert!(!t.watch("a"), "already held by being open");
    assert_eq!(t.set_open(Some("c".into())), None);
    assert_eq!(t.all(), ["c", "b", "a"]);
    assert!(t.unwatch("a"), "the last listener leaving ends the subscription");
    assert!(t.unwatch("b"));
    assert_eq!(t.all(), ["c"]);
}

#[test]
fn a_second_listener_reuses_the_snapshot_only_if_no_event_arrived_since() {
    let mut t = Topics::default();
    t.set_open(Some("a".into()));
    t.record_snapshot("a", &json!({"n": 1}));
    assert_eq!(t.fresh_snapshot("a"), Some(&json!({"n": 1})));
    t.note_event("a");
    assert_eq!(t.fresh_snapshot("a"), None, "an event made it stale: ask again");
    t.record_snapshot("a", &json!({"n": 2}));
    assert_eq!(t.fresh_snapshot("a"), Some(&json!({"n": 2})));
    t.forget_snapshots();
    assert_eq!(t.fresh_snapshot("a"), None);
}

#[test]
fn the_ninth_session_left_evicts_the_first() {
    let mut kept = Kept::default();
    let full = || create_state(&snapshot("idle", vec![life("m", "user", 1, Some("hi"), None)]));
    for n in 0..=KEPT_SESSIONS {
        kept.keep(&format!("s{n}"), full());
    }
    assert_eq!(kept.ids().len(), KEPT_SESSIONS);
    assert!(kept.get("s0").is_none());
    assert!(kept.get("s1").is_some() && kept.get(&format!("s{KEPT_SESSIONS}")).is_some());
    // Keeping again moves it to the newest end; an empty conversation isn't kept.
    kept.keep("s1", full());
    kept.keep("empty", new_state());
    assert_eq!(kept.ids().last(), Some(&"s1"));
    assert!(kept.get("empty").is_none());
}

pub(super) fn two_sessions() -> Vec<crate::mock::Project> {
    let mut rows: Value = serde_json::from_str(&fixture("api-sessions.json")).unwrap();
    let mut second = rows[0].clone();
    second["session"]["id"] = json!("second-session");
    second["session"]["title"] = json!("Second one");
    rows.as_array_mut().unwrap().push(second);
    sessions::parse_list(&rows, 1_794_674_800_000)
}

pub(super) fn app() -> Runtime<State> {
    let mut rt = App::new(spec()).headless_runtime((1280.0, 760.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(1.0 / 60.0);
    rt
}

fn texts(s: &State) -> Vec<String> {
    s.session()
        .messages
        .iter()
        .flat_map(|m| m.parts.iter())
        .filter_map(|p| if let Part::Text { text, .. } = p { Some(text.clone()) } else { None })
        .collect()
}

#[test]
fn a_kept_stream_shows_before_its_snapshot() {
    let mut rt = app();
    // A link that goes nowhere: the commands pile up unread.
    let post = Post::new(|_| {});
    rt.update(move |s, tx| {
        s.link = Some(Link::start("http://127.0.0.1:9".into(), post));
        s.on_list(two_sessions(), tx);
    });
    let first = rt.state.open_id().unwrap();
    let snap = snapshot("idle", vec![life("m1", "user", 1, Some("Kept words"), Some("p1"))]);
    rt.update({
        let first = first.clone();
        move |s, tx| s.on_snapshot(&first, create_state(&snap), tx)
    });
    assert_eq!(texts(&rt.state), ["Kept words"]);

    // Away to the other session: its conversation is empty and loading.
    rt.update(|s, tx| s.open_session(0, 1, tx));
    assert!(rt.state.loading);
    assert!(texts(&rt.state).is_empty());

    // Back: the kept words are there at once, with no snapshot yet.
    let at = rt.state.projects[0].sessions.iter().position(|s| s.id == first).unwrap();
    rt.update(move |s, tx| s.open_session(0, at, tx));
    assert!(rt.state.loading, "the fresh snapshot is still on its way");
    assert_eq!(texts(&rt.state), ["Kept words"]);
    assert_eq!(rt.state.kept.ids().len(), 1, "the other, empty one wasn't kept");
}

#[test]
fn a_kept_stream_takes_the_status_the_list_has_now() {
    let mut rt = app();
    let post = Post::new(|_| {});
    rt.update(move |s, tx| {
        s.link = Some(Link::start("http://127.0.0.1:9".into(), post));
        s.on_list(two_sessions(), tx);
    });
    let first = rt.state.open_id().unwrap();
    let snap = snapshot("idle", vec![life("m1", "user", 1, Some("Kept"), None)]);
    rt.update({
        let first = first.clone();
        move |s, tx| s.on_snapshot(&first, create_state(&snap), tx)
    });
    rt.update(|s, tx| s.open_session(0, 1, tx));
    // While away, the sessions topic says the first one is working.
    rt.update({
        let first = first.clone();
        move |s, _| s.on_row_patch(super::rows::activity(&first, "busy"))
    });
    let at = rt.state.projects[0].sessions.iter().position(|s| s.id == first).unwrap();
    rt.update(move |s, tx| s.open_session(0, at, tx));
    assert!(rt.state.stream.as_ref().unwrap().session_status.working());
    assert_eq!(rt.state.session().status, Status::Working);
}

#[test]
fn a_partial_snapshot_shows_the_banner_and_a_whole_one_takes_it_away() {
    let mut rt = app();
    let post = Post::new(|_| {});
    rt.update(move |s, tx| {
        s.link = Some(Link::start("http://127.0.0.1:9".into(), post));
        s.on_list(two_sessions(), tx);
    });
    let id = rt.state.open_id().unwrap();
    let mut snap = snapshot("idle", vec![life("m1", "user", 1, Some("Cached"), None)]);
    snap["isPartial"] = json!(true);
    rt.update({
        let (id, snap) = (id.clone(), snap.clone());
        move |s, tx| s.on_snapshot(&id, create_state(&snap), tx)
    });
    rt.frame(1.0 / 60.0);
    assert!(rt.find("partial-banner").is_some());
    snap["isPartial"] = json!(false);
    rt.update(move |s, tx| s.on_snapshot(&id, create_state(&snap), tx));
    rt.frame(1.0 / 60.0);
    assert!(rt.find("partial-banner").is_none());
}

// ─── a hub that drops ───────────────────────────────────────────────────────

/// A stand-in for Fleet whose first hub connection answers the subscription with the older snapshot and
/// then hangs up; later ones answer with the newer snapshot and its events.
fn serve(mut stream: TcpStream, hubs: Arc<AtomicUsize>) {
    let mut peek = [0u8; 16];
    let n = stream.peek(&mut peek).unwrap_or(0);
    if peek[..n].starts_with(b"GET /hubs") {
        let Ok(mut ws) = tungstenite::accept(stream) else { return };
        let nth = hubs.fetch_add(1, Ordering::SeqCst);
        let older = snapshot_of(&frames_of("hub-snapshot-after-turn.jsonl"));
        let live = frames_of("hub-turn-run-tests.jsonl");
        while let Ok(Message::Text(text)) = ws.read() {
            for frame in text.split('\u{1e}').filter(|f| !f.is_empty()) {
                let call: Value = serde_json::from_str(frame).unwrap();
                let send = |ws: &mut tungstenite::WebSocket<TcpStream>, v: Value| {
                    let _ = ws.send(Message::text(format!("{v}\u{1e}")));
                };
                match call["target"].as_str() {
                    None if call["protocol"] == "json" => send(&mut ws, json!({})),
                    Some("SubscribeToSessionsTopicAsync") => {
                        send(&mut ws, json!({"type": 3, "invocationId": call["invocationId"], "result": null}))
                    }
                    Some("SubscribeToSessionAsync") if nth == 0 => {
                        send(&mut ws, json!({"type": 3, "invocationId": call["invocationId"], "result": older}));
                        // The server goes away mid-stream.
                        return;
                    }
                    Some("SubscribeToSessionAsync") => {
                        send(&mut ws, json!({"type": 3, "invocationId": call["invocationId"], "result": snapshot_of(&live)}));
                        for f in &live {
                            if let signalr::Frame::Event { topic, data, .. } = f {
                                send(&mut ws, json!({"type": 1, "target": "Event", "arguments": [topic, 0, data]}));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        return;
    }
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut head = String::new();
    let mut length = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
            break;
        }
        if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            length = n.trim().parse().unwrap_or(0);
        }
        head.push_str(&line);
    }
    let mut body = vec![0; length];
    let _ = reader.read_exact(&mut body);
    let reply = |stream: &mut TcpStream, body: &str| {
        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(head.as_bytes());
    };
    if head.starts_with("POST /hubs/session-events/negotiate") {
        reply(&mut stream, "{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}");
    } else if head.starts_with("GET /api/sessions") {
        reply(&mut stream, &fixture("api-sessions.json"));
    }
}

#[test]
fn a_dropped_connection_comes_back_with_the_conversation_a_fresh_open_shows() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let hubs = Arc::new(AtomicUsize::new(0));
    let counted = hubs.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let hubs = counted.clone();
            std::thread::spawn(move || serve(stream, hubs));
        }
    });
    let (jobs, queue) = channel::<Job>();
    let jobs = std::sync::Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    let link = Link::start(base, post);
    rt.update(move |s, _| s.link = Some(link));

    // What a fresh open shows: the newer snapshot, then its events.
    let live = frames_of("hub-turn-run-tests.jsonl");
    let mut fresh = create_state(&snapshot_of(&live));
    for f in &live {
        if let signalr::Frame::Event { data, .. } = f
            && let Some(e) = Event::from_wire(data)
        {
            apply(&mut fresh, &e);
        }
    }

    let mut saw_down = false;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        while let Ok(job) = queue.try_recv() {
            rt.update(job);
        }
        rt.frame(1.0 / 60.0);
        saw_down |= matches!(rt.state.connection, Connection::Down(_));
        let back = hubs.load(Ordering::SeqCst) >= 2
            && rt.state.connection == Connection::Online
            && rt.state.stream.as_ref().is_some_and(|s| s.messages == fresh.messages);
        if back {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "didn't come back: {:?}, {} hub connections, {:?}",
            rt.state.connection,
            hubs.load(Ordering::SeqCst),
            rt.state.stream.as_ref().map(|s| s.messages.len())
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(saw_down, "the status bar had its Reconnecting state");
    let shown = rt.state.stream.as_ref().unwrap();
    let mut seen = std::collections::HashSet::new();
    assert!(shown.messages.iter().all(|m| seen.insert(m.id.clone())), "no message shown twice");
    assert!(!rt.state.loading);
}

#[test]
fn coming_back_to_the_window_while_the_hub_is_down_reconnects_at_once() {
    // A port nothing listens on yet: the first attempt is refused, and the next one is a second away.
    let held = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = held.local_addr().unwrap();
    drop(held);
    let (jobs, queue) = channel::<Job>();
    let jobs = std::sync::Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    let link = Link::start(format!("http://{addr}"), post);
    let wake = |rt: &mut Runtime<State>, until: &dyn Fn(&State) -> bool, secs: f32| {
        let end = Instant::now() + Duration::from_secs_f32(secs);
        while Instant::now() < end {
            while let Ok(job) = queue.try_recv() {
                rt.update(job);
            }
            if until(&rt.state) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    };
    assert!(wake(&mut rt, &|s| matches!(s.connection, Connection::Down(_)), 5.0), "never saw it down");
    let listener = TcpListener::bind(addr).unwrap();
    let hubs = Arc::new(AtomicUsize::new(0));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let hubs = hubs.clone();
            std::thread::spawn(move || serve(stream, hubs));
        }
    });
    link.window_focus(true);
    // Well inside the one-second delay the schedule would have waited out.
    assert!(wake(&mut rt, &|s| s.connection == Connection::Online, 0.7), "didn't reconnect when the window came back");
}
