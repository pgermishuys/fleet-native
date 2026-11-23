//! Presence and focus (rows 6.5 and 6.6). Fleet tests these in `use-desk-presence.test.ts` and
//! `use-session-recap.test.ts`; the cases are the same ideas, under those names where they match.
//! The rule is tested with a clock of its own; a fake hub on loopback records the calls it receives.
use super::super::link::{Link, Post};
use super::super::presence::{Call, FOCUS_SETTLE, PRESENCE_INTERVAL, Presence};
use super::*;
use std::io::Write;
use std::net::TcpStream;
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant};
use tungstenite::Message;

fn focus(id: &str, focused: bool) -> Call {
    Call::Focus(id.to_string(), focused)
}

/// A presence that is connected, with `id` open and its snapshot in.
fn watching(id: &str, t0: Instant) -> Presence {
    let mut p = Presence::default();
    p.connected(t0);
    assert_eq!(p.snapshot(id), [focus(id, true)]);
    p
}

// ─── presence ───────────────────────────────────────────────────────────────

#[test]
fn reports_presence_on_connect_and_again_every_thirty_seconds() {
    let t0 = Instant::now();
    let mut p = Presence::default();
    assert_eq!(p.connected(t0), [Call::Presence(true)]);
    assert_eq!(p.tick(t0 + PRESENCE_INTERVAL - Duration::from_millis(1)), []);
    assert_eq!(p.tick(t0 + PRESENCE_INTERVAL), [Call::Presence(true)]);
    assert_eq!(p.tick(t0 + PRESENCE_INTERVAL * 2 - Duration::from_millis(1)), []);
    assert_eq!(p.tick(t0 + PRESENCE_INTERVAL * 2), [Call::Presence(true)]);
}

#[test]
fn reports_presence_again_after_a_reconnect() {
    let t0 = Instant::now();
    let mut p = Presence::default();
    p.connected(t0);
    p.disconnected();
    assert_eq!(p.tick(t0 + PRESENCE_INTERVAL * 5), [], "nothing is sent while the connection is down");
    assert_eq!(p.connected(t0 + PRESENCE_INTERVAL * 6), [Call::Presence(true)]);
}

#[test]
fn reports_presence_the_moment_visibility_changes() {
    let t0 = Instant::now();
    let mut p = Presence::default();
    p.connected(t0);
    assert_eq!(p.visibility_changed(false, t0), [Call::Presence(false)]);
    assert_eq!(p.visibility_changed(false, t0), [], "a repeat isn't a change");
    assert_eq!(p.tick(t0 + PRESENCE_INTERVAL), [Call::Presence(false)], "the next tick says what is true now");
    assert_eq!(p.visibility_changed(true, t0 + PRESENCE_INTERVAL), [Call::Presence(true)]);
}

#[test]
fn says_the_window_is_hidden_when_it_is_hidden_while_the_connection_is_down() {
    let t0 = Instant::now();
    let mut p = Presence::default();
    assert_eq!(p.visibility_changed(false, t0), [], "no connection to tell");
    assert_eq!(p.connected(t0), [Call::Presence(false)]);
}

// ─── focus ──────────────────────────────────────────────────────────────────

#[test]
fn reports_focus_after_the_session_snapshot_arrives() {
    let mut p = Presence::default();
    p.connected(Instant::now());
    assert_eq!(p.snapshot("s1"), [focus("s1", true)]);
}

#[test]
fn reports_focus_again_once_the_snapshot_is_back_after_a_reconnect() {
    let t0 = Instant::now();
    let mut p = watching("s1", t0);
    p.disconnected();
    assert_eq!(p.connected(t0), [Call::Presence(true)]);
    assert_eq!(p.tick(t0 + FOCUS_SETTLE), [], "no snapshot yet, so nothing to say about a session");
    assert_eq!(p.snapshot("s1"), [focus("s1", true)], "the server forgot this window");
}

#[test]
fn reports_a_focus_change_once_it_has_settled() {
    let t0 = Instant::now();
    let mut p = watching("s1", t0);
    p.focus_changed(false, t0);
    assert_eq!(p.tick(t0 + FOCUS_SETTLE - Duration::from_millis(1)), []);
    assert_eq!(p.tick(t0 + FOCUS_SETTLE), [focus("s1", false)]);
    p.focus_changed(true, t0 + Duration::from_secs(1));
    assert_eq!(p.tick(t0 + Duration::from_secs(1) + FOCUS_SETTLE), [focus("s1", true)]);
}

#[test]
fn a_focus_flicker_that_comes_back_reports_nothing() {
    let t0 = Instant::now();
    let mut p = watching("s1", t0);
    p.focus_changed(false, t0);
    p.focus_changed(true, t0 + Duration::from_millis(100));
    assert_eq!(p.tick(t0 + Duration::from_millis(100) + FOCUS_SETTLE), [], "it ended where it began: not repeated");
}

#[test]
fn a_window_that_goes_off_screen_is_not_looking() {
    let t0 = Instant::now();
    let mut p = watching("s1", t0);
    p.visibility_changed(false, t0);
    assert_eq!(p.tick(t0 + FOCUS_SETTLE), [focus("s1", false)]);
    p.visibility_changed(true, t0 + Duration::from_secs(2));
    assert_eq!(p.tick(t0 + Duration::from_secs(2) + FOCUS_SETTLE), [focus("s1", true)]);
}

#[test]
fn focus_is_not_repeated_when_unchanged() {
    let t0 = Instant::now();
    let mut p = watching("s1", t0);
    p.focus_changed(true, t0);
    assert_eq!(p.tick(t0 + FOCUS_SETTLE), []);
    p.focus_changed(false, t0);
    assert_eq!(p.tick(t0 + FOCUS_SETTLE), [focus("s1", false)]);
    // A visibility change while already unfocused leaves the answer the same.
    p.visibility_changed(false, t0 + Duration::from_secs(1));
    assert_eq!(p.tick(t0 + Duration::from_secs(1) + FOCUS_SETTLE), []);
}

#[test]
fn reports_no_focus_while_recaps_and_notifications_are_both_off() {
    let t0 = Instant::now();
    let mut p = watching("s1", t0);
    assert_eq!(p.set_enabled(false), [], "turning it off says nothing");
    p.focus_changed(false, t0);
    assert_eq!(p.tick(t0 + FOCUS_SETTLE), [], "a change while off is not reported");
    assert_eq!(p.snapshot("s2"), [], "nor is a new session's");
    assert_eq!(p.set_enabled(false), [], "a repeat isn't a change");
    assert_eq!(p.set_enabled(true), [focus("s2", false)], "turned on, the window says where it is");
    assert_eq!(p.set_enabled(true), []);
}

#[test]
fn says_false_on_leaving_a_session_that_was_looked_at() {
    let t0 = Instant::now();
    let mut p = watching("s1", t0);
    assert_eq!(p.left(), [focus("s1", false)]);
    assert_eq!(p.left(), [], "once");
    p.focus_changed(false, t0);
    assert_eq!(p.tick(t0 + FOCUS_SETTLE), [], "no open session to report on until its snapshot");
    assert_eq!(p.snapshot("s2"), [focus("s2", false)]);
}

#[test]
fn says_nothing_on_leaving_a_session_that_was_not_looked_at() {
    let t0 = Instant::now();
    let mut p = watching("s1", t0);
    p.focus_changed(false, t0);
    p.tick(t0 + FOCUS_SETTLE);
    assert_eq!(p.left(), []);
}

#[test]
fn the_next_due_time_is_the_nearer_of_the_tick_and_a_settling_change() {
    let t0 = Instant::now();
    let mut p = Presence::default();
    assert_eq!(p.next_due(), None);
    p.connected(t0);
    assert_eq!(p.next_due(), Some(t0 + PRESENCE_INTERVAL));
    p.focus_changed(false, t0);
    assert_eq!(p.next_due(), Some(t0 + FOCUS_SETTLE));
}

// ─── a hub that records what it is sent ─────────────────────────────────────

fn serve(mut stream: TcpStream, calls: Sender<(usize, String, Value)>, hubs: Arc<AtomicUsize>, drop_after_focus: bool) {
    let mut peek = [0u8; 16];
    let n = stream.peek(&mut peek).unwrap_or(0);
    if peek[..n].starts_with(b"GET /hubs") {
        let Ok(mut ws) = tungstenite::accept(stream) else { return };
        let nth = hubs.fetch_add(1, Ordering::SeqCst);
        let snapshot = snapshot_of(&frames_of("hub-snapshot-after-turn.jsonl"));
        while let Ok(Message::Text(text)) = ws.read() {
            for frame in text.split('\u{1e}').filter(|f| !f.is_empty()) {
                let call: Value = serde_json::from_str(frame).unwrap();
                let send = |ws: &mut tungstenite::WebSocket<TcpStream>, v: Value| {
                    let _ = ws.send(Message::text(format!("{v}\u{1e}")));
                };
                let Some(target) = call["target"].as_str() else {
                    if call["protocol"] == "json" {
                        send(&mut ws, json!({}));
                    }
                    continue;
                };
                let result = match target {
                    "SubscribeToSessionAsync" => snapshot.clone(),
                    // An older Fleet has no such method: the answer is an error, which must change nothing.
                    "SetPresenceAsync" | "SetSessionFocusAsync" => Value::Null,
                    _ => Value::Null,
                };
                if target == "SetPresenceAsync" && nth == 0 && !drop_after_focus {
                    send(&mut ws, json!({"type": 3, "invocationId": call["invocationId"], "error": "Method does not exist"}));
                } else {
                    send(&mut ws, json!({"type": 3, "invocationId": call["invocationId"], "result": result}));
                }
                let _ = calls.send((nth, target.to_string(), call["arguments"].clone()));
                if drop_after_focus && nth == 0 && target == "SetSessionFocusAsync" {
                    return;
                }
            }
        }
        return;
    }
    let heard = crate::test_http::read(&mut stream);
    if heard.method.is_empty() {
        return;
    }
    let (head, _body) = (heard.head, heard.body);
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

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Hub {
    base: String,
    calls: std::sync::mpsc::Receiver<(usize, String, Value)>,
}

fn hub(drop_after_focus: bool) -> Hub {
    let listener = crate::test_http::bind().unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (tx, calls) = channel();
    let hubs = Arc::new(AtomicUsize::new(0));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (tx, hubs) = (tx.clone(), hubs.clone());
            std::thread::spawn(move || serve(stream, tx, hubs, drop_after_focus));
        }
    });
    Hub { base, calls }
}

impl Hub {
    /// The next presence or focus call, with which hub connection it came on.
    fn next(&self) -> (usize, String, Value) {
        loop {
            let got = crate::test_wait::recv("what the hub should have heard", &self.calls);
            if got.1 == "SetPresenceAsync" || got.1 == "SetSessionFocusAsync" {
                return got;
            }
        }
    }
}

fn link(hub: &Hub) -> Link {
    let link = Link::start(hub.base.clone(), Post::new(|_| {}));
    link.open("s1");
    link
}

#[test]
fn the_hub_hears_presence_then_focus_as_the_window_gains_and_loses_focus() {
    let hub = hub(false);
    let link = link(&hub);
    assert_eq!(hub.next(), (0, "SetPresenceAsync".into(), json!([true, "desktop"])), "on connect; an error answer is swallowed");
    assert_eq!(hub.next(), (0, "SetSessionFocusAsync".into(), json!(["s1", true])), "after the snapshot");
    link.window_focus(false);
    assert_eq!(hub.next().2, json!(["s1", false]));
    link.window_focus(true);
    assert_eq!(hub.next().2, json!(["s1", true]));
    link.window_visible(false);
    assert_eq!(hub.next(), (0, "SetPresenceAsync".into(), json!([false, "desktop"])));
    assert_eq!(hub.next().2, json!(["s1", false]));
    link.open("s2");
    // Leaving a session that is already not looked at says nothing; the new one's snapshot reports.
    assert_eq!(hub.next().2, json!(["s2", false]));
}

#[test]
fn the_hub_hears_no_focus_while_the_report_is_switched_off() {
    let hub = hub(false);
    let link = link(&hub);
    assert_eq!(hub.next().1, "SetPresenceAsync");
    assert_eq!(hub.next().1, "SetSessionFocusAsync");
    link.report_focus(false);
    link.window_focus(false);
    link.window_visible(false);
    // The window leaving the screen is presence, which is always said; the focus call in between is not.
    assert_eq!(hub.next(), (0, "SetPresenceAsync".into(), json!([false, "desktop"])));
    link.report_focus(true);
    assert_eq!(hub.next(), (0, "SetSessionFocusAsync".into(), json!(["s1", false])));
}

#[test]
fn the_hub_hears_both_again_after_a_reconnect() {
    let hub = hub(true);
    let _link = link(&hub);
    assert_eq!(hub.next().1, "SetPresenceAsync");
    assert_eq!(hub.next().1, "SetSessionFocusAsync");
    // The hub hung up after that; the next connection is told everything again.
    assert_eq!(hub.next(), (1, "SetPresenceAsync".into(), json!([true, "desktop"])));
    assert_eq!(hub.next(), (1, "SetSessionFocusAsync".into(), json!(["s1", true])));
}
