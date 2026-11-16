//! Archive with Undo, pins and the row's hover buttons (piece I of docs/parity/06-session-list.md): Fleet's
//! `archive-queue.test.ts` ("archive queue") and `session-pins.test.ts` ("session pins") under their names, the buttons and the
//! toast from `SessionItem.vue` and `ArchiveUndoToast.vue`, and the same calls on another machine, which must reach that
//! machine with its own token. The invented list takes a change at once; the tests that read a request use a fake server.
use crate::fleet::archive_queue::{UNDO_SECS, archived_message, failed_message, retention_body, retention_path, toast_title};
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::machines::parse_machine_list;
use crate::fleet::pins::{pin_body, pin_path};
use crate::mock::machines::Case;
use crate::tests::{FRAME, app, click};
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

/// One request a fake heard: method, path, body, bearer token.
pub(crate) type Call = (String, String, Value, Option<String>);
pub(crate) type Log = Arc<Mutex<Vec<Call>>>;
type Answer = Arc<dyn Fn(&str, &str) -> (u16, String) + Send + Sync>;

/// A fake Fleet: what it heard (method, path, body, bearer token), what it answers, and a gate that holds the retention
/// answer back while `hold` is set.
pub(crate) struct Fake {
    pub(crate) base: String,
    pub(crate) heard: Log,
    hold: Arc<AtomicBool>,
}

impl Fake {
    pub(crate) fn start(answer: impl Fn(&str, &str) -> (u16, String) + Send + Sync + 'static) -> Fake {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let heard: Log = Default::default();
        let hold = Arc::new(AtomicBool::new(false));
        let answer: Answer = Arc::new(answer);
        let (log, gate) = (heard.clone(), hold.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (log, gate, answer) = (log.clone(), gate.clone(), answer.clone());
                std::thread::spawn(move || serve(stream, log, gate, answer));
            }
        });
        Fake { base, heard, hold }
    }

    /// Answers each retention call with `status`, a pin with `{"pinOrder": order}`, and an unpin with 200.
    fn ok() -> Fake {
        Fake::start(|_, _| (200, "{}".into()))
    }

    pub(crate) fn calls(&self, needle: &str) -> Vec<Call> {
        self.heard.lock().unwrap().iter().filter(|c| c.1.contains(needle)).cloned().collect()
    }
}

fn serve(stream: std::net::TcpStream, log: Log, gate: Arc<AtomicBool>, answer: Answer) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let (mut head, mut length) = (String::new(), 0usize);
    let mut token = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        if line == "\r\n" {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(n) = lower.strip_prefix("content-length:") {
            length = n.trim().parse().unwrap_or(0);
        }
        if let Some(t) = lower.strip_prefix("authorization: bearer ") {
            token = Some(t.trim().to_string());
        }
        head.push_str(&line);
    }
    let mut raw = vec![0; length];
    let _ = reader.read_exact(&mut raw);
    let body: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
    let mut parts = head.lines().next().unwrap_or("").split(' ');
    let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
    let (status, text) = if (path.starts_with("/api/sessions/") && !path.contains("negotiate")) || path.starts_with("/api/projects/") {
        log.lock().unwrap().push((method.clone(), path.clone(), body, token));
        while path.ends_with("/retention") && gate.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        answer(&method, &path)
    } else if path.starts_with("/api/sessions?") || path.starts_with("/api/fleet/") || path.starts_with("/api/analytics/") {
        log.lock().unwrap().push((method.clone(), path.clone(), body, token));
        answer(&method, &path)
    } else if path.starts_with("/api/machines") {
        answer(&method, &path)
    } else {
        (404, "{}".into())
    };
    let _ = stream.try_clone().unwrap().write_all(
        format!(
            "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
            text.len()
        )
        .as_bytes(),
    );
}

/// The app with its home at `fake` and the jobs its threads hand back.
pub(crate) struct Rig {
    pub(crate) rt: Runtime<crate::State>,
    jobs: Receiver<Job>,
}

impl Rig {
    pub(crate) fn home(fake: &Fake) -> Rig {
        let mut rt = app();
        let (tx, jobs) = channel::<Job>();
        let tx = Mutex::new(tx);
        let link = Link::start(fake.base.clone(), Post::new(move |job| drop(tx.lock().unwrap().send(job))));
        rt.update(move |s, _| s.link = Some(link));
        rt.frame(FRAME);
        Rig { rt, jobs }
    }

    pub(crate) fn wait(&mut self, what: &str, done: impl FnMut(&crate::State) -> bool) {
        crate::test_wait::drive(&mut self.rt, &self.jobs, what, done, crate::test_wait::app_state);
    }

    pub(crate) fn state(&self) -> &crate::State {
        &self.rt.state
    }
}

fn id(rt: &Runtime<crate::State>, p: usize, i: usize) -> String {
    rt.state.projects[p].sessions[i].id.clone()
}

fn hidden(rt: &Runtime<crate::State>, p: usize, i: usize) -> bool {
    rt.find(&format!("session-{p}-{i}")).is_none()
}

fn width(rt: &Runtime<crate::State>, key: &str) -> f32 {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).map_or(0.0, |e| e.rect[2])
}

fn pin_order(rt: &Runtime<crate::State>, p: usize, i: usize) -> Option<f64> {
    rt.state.projects[p].sessions[i].listing.pin_order
}

fn archive(rt: &mut Runtime<crate::State>, ids: &[&str]) {
    let ids: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
    rt.update(move |s, tx| s.archive_sessions(&ids, None, tx));
    rt.frame(FRAME);
}

// ---- the requests and their words -------------------------------------------------------------------------------------------

#[test]
fn the_requests_and_the_toasts_words_are_fleets() {
    assert_eq!(retention_path("s1"), "/api/sessions/s1/retention");
    assert_eq!(retention_body(true), json!({ "retentionStatus": "archived" }));
    assert_eq!(retention_body(false), json!({ "retentionStatus": "active" }));
    assert_eq!(pin_path("s1"), "/api/sessions/s1/pin");
    assert_eq!(pin_body(Some("s2")), json!({ "beforeSessionId": "s2" }));
    assert_eq!(pin_body(None), json!({ "beforeSessionId": null }));
    assert_eq!(archived_message(&["Docs".into()]), "Archived \"Docs\"");
    assert_eq!(archived_message(&["A".into(), "B".into()]), "Archived 2 sessions");
    assert_eq!(failed_message(&["Docs".into()]), "Couldn't archive \"Docs\".");
    assert_eq!(failed_message(&["A".into(), "B".into(), "C".into()]), "Couldn't archive 3 sessions.");
    assert_eq!(toast_title("  Docs  "), "Docs");
    assert_eq!(toast_title("   "), "Untitled session");
}

// ---- archive-queue.test.ts, "archive queue" ---------------------------------------------------------------------------------

#[test]
fn archive_queue_holds_an_archive_back_until_the_undo_window_closes() {
    let fake = Fake::ok();
    let mut r = Rig::home(&fake);
    let (a, title) = (id(&r.rt, 0, 1), r.rt.state.projects[0].sessions[1].title.clone());
    archive(&mut r.rt, &[&a]);
    assert!(hidden(&r.rt, 0, 1), "the row goes at once");
    assert_eq!(r.state().archive.pending.as_ref().map(|p| p.message.clone()), Some(format!("Archived \"{title}\"")));
    assert!(fake.calls("/retention").is_empty(), "nothing is sent while Undo can still be pressed");
    r.rt.frame(UNDO_SECS - 1.0);
    assert!(fake.calls("/retention").is_empty(), "not before the six seconds are up");
    r.rt.frame(1.2);
    r.wait("the archive request", |_| !fake.calls("/retention").is_empty());
    let call = &fake.calls("/retention")[0];
    assert_eq!(
        (call.0.as_str(), call.1.as_str(), call.2.clone()),
        ("PATCH", format!("/api/sessions/{a}/retention").as_str(), json!({ "retentionStatus": "archived" }))
    );
    r.wait("the row to say archived", |s| s.projects[0].sessions.iter().any(|x| x.id == a && x.row.archived));
    assert!(r.state().archive.pending.is_none());
}

#[test]
fn archive_queue_keeps_the_row_hidden_while_the_archive_request_is_on_its_way() {
    let fake = Fake::ok();
    fake.hold.store(true, Ordering::SeqCst);
    let mut r = Rig::home(&fake);
    let a = id(&r.rt, 0, 1);
    archive(&mut r.rt, &[&a]);
    r.rt.frame(UNDO_SECS + 0.1);
    r.wait("the request to be sent", |_| !fake.calls("/retention").is_empty());
    assert!(r.state().archive.pending.is_none(), "the window is over");
    r.rt.frame(FRAME);
    assert!(hidden(&r.rt, 0, 1), "the list says active until the server has answered, and the row stays hidden");
    fake.hold.store(false, Ordering::SeqCst);
    r.wait("the answer", |s| s.projects[0].sessions.iter().any(|x| x.id == a && x.row.archived));
    r.rt.frame(FRAME);
    assert!(hidden(&r.rt, 0, 1), "and stays hidden once archived");
}

#[test]
fn archive_queue_shows_the_row_again_when_the_archive_request_fails() {
    let fake = Fake::start(|_, _| (409, r#"{"error":"busy"}"#.into()));
    let mut r = Rig::home(&fake);
    let (a, title) = (id(&r.rt, 0, 1), r.rt.state.projects[0].sessions[1].title.clone());
    archive(&mut r.rt, &[&a]);
    r.rt.frame(UNDO_SECS + 0.1);
    r.wait("the failure", |s| s.archive.error.is_some());
    r.rt.frame(FRAME);
    assert!(!hidden(&r.rt, 0, 1), "the row is back");
    assert_eq!(r.state().archive.error, Some(format!("Couldn't archive \"{title}\".")));
    assert!(!r.state().projects[0].sessions[1].row.archived);
}

#[test]
fn archive_queue_never_reaches_the_server_when_undone() {
    let fake = Fake::ok();
    let mut r = Rig::home(&fake);
    let a = id(&r.rt, 0, 1);
    archive(&mut r.rt, &[&a]);
    assert!(hidden(&r.rt, 0, 1));
    click(&mut r.rt, "toast-action");
    r.rt.frame(FRAME);
    assert!(!hidden(&r.rt, 0, 1), "Undo brings the row back");
    assert!(r.state().archive.pending.is_none() && r.rt.find("toast").is_none());
    r.rt.frame(UNDO_SECS + 1.0);
    // The clock ran well past the window: still nothing.
    assert!(fake.calls("/retention").is_empty());
}

#[test]
fn archive_queue_sends_the_waiting_archive_when_another_one_starts() {
    let fake = Fake::ok();
    let mut r = Rig::home(&fake);
    let (a, b) = (id(&r.rt, 0, 1), id(&r.rt, 0, 2));
    archive(&mut r.rt, &[&a]);
    r.rt.frame(1.0);
    archive(&mut r.rt, &[&b]);
    r.wait("the first to be sent", |_| fake.calls(&format!("{a}/retention")).len() == 1);
    assert!(fake.calls(&format!("{b}/retention")).is_empty(), "the second still waits");
    assert_eq!(r.state().archive.pending.as_ref().map(|p| p.ids.clone()), Some(vec![b]));
}

#[test]
fn archive_queue_names_the_sessions_it_couldnt_archive() {
    let fake = Fake::start(|_, _| (500, "{}".into()));
    let mut r = Rig::home(&fake);
    let (a, b) = (id(&r.rt, 0, 1), id(&r.rt, 0, 2));
    // One at a time, then two together.
    archive(&mut r.rt, &[&a, &a]);
    assert_eq!(r.state().archive.pending.as_ref().map(|p| p.ids.len()), Some(1), "an id twice is one session");
    r.rt.frame(UNDO_SECS + 0.1);
    r.wait("the failure", |s| s.archive.error.is_some());
    assert!(r.state().archive.error.as_deref().is_some_and(|e| e.starts_with("Couldn't archive \"")));
    archive(&mut r.rt, &[&a, &b]);
    assert_eq!(r.state().archive.error, None, "a new archive clears the old error");
    assert_eq!(r.state().archive.pending.as_ref().map(|p| p.message.clone()), Some("Archived 2 sessions".into()));
    r.rt.frame(UNDO_SECS + 0.1);
    r.wait("both failures", |s| s.archive.error.is_some());
    assert_eq!(r.state().archive.error.as_deref(), Some("Couldn't archive 2 sessions."));
}

#[test]
fn archive_queue_restores_an_archived_session() {
    let fake = Fake::ok();
    let mut r = Rig::home(&fake);
    let a = id(&r.rt, 0, 1);
    r.rt.update(|s, _| s.projects[0].sessions[1].row.archived = true);
    r.rt.update({
        let a = a.clone();
        move |s, _| s.restore_session(&a, None)
    });
    r.wait("the session to be active", |s| !s.projects[0].sessions[1].row.archived);
    let call = &fake.calls("/retention")[0];
    assert_eq!((call.0.as_str(), call.2.clone()), ("PATCH", json!({ "retentionStatus": "active" })));
    assert!(!r.state().is_restoring(&a));
    // A refusal says what the server said.
    let fake = Fake::start(|_, _| (409, r#"{"error":"The checkout is gone."}"#.into()));
    let mut r = Rig::home(&fake);
    r.rt.update(|s, _| s.projects[0].sessions[1].row.archived = true);
    let a = id(&r.rt, 0, 1);
    r.rt.update(move |s, _| s.restore_session(&a, None));
    r.wait("the refusal", |s| s.archive.error.is_some());
    assert_eq!(r.state().archive.error.as_deref(), Some("The checkout is gone."));
    assert!(r.state().projects[0].sessions[1].row.archived);
}

#[test]
fn closing_the_app_inside_the_wait_leaves_the_sessions_active() {
    let fake = Fake::ok();
    let mut r = Rig::home(&fake);
    let a = id(&r.rt, 0, 1);
    archive(&mut r.rt, &[&a]);
    r.rt.frame(UNDO_SECS - 0.5);
    // The app goes here: nothing was sent, so the server still has it active.
    assert!(fake.calls("/retention").is_empty());
    assert!(!r.state().projects[0].sessions[1].row.archived);
}

// ---- session-pins.test.ts, "session pins" -----------------------------------------------------------------------------------

/// Three sessions of the first project pinned the way the pins test starts: a at 1, b at 2, c not.
fn pinned_rig(answer: impl Fn(&str, &str) -> (u16, String) + Send + Sync + 'static) -> (Rig, Fake) {
    let fake = Fake::start(answer);
    let mut r = Rig::home(&fake);
    r.rt.update(|s, _| {
        s.projects[0].sessions[0].listing.pin_order = Some(1.0);
        s.projects[0].sessions[1].listing.pin_order = Some(2.0);
        s.projects[0].sessions[2].listing.pin_order = None;
    });
    (r, fake)
}

#[test]
fn session_pins_pins_at_once_where_it_will_go_tells_the_server_and_keeps_the_servers_answer() {
    let (mut r, fake) = pinned_rig(|_, _| (200, r#"{"pinOrder":1.5}"#.into()));
    let (b, c) = (id(&r.rt, 0, 1), id(&r.rt, 0, 2));
    r.rt.update(move |s, _| s.pin_session(&c, Some(&b)));
    assert_eq!(pin_order(&r.rt, 0, 2), Some(1.5), "halfway between a and b, before the server has answered");
    r.wait("the pin request", |_| !fake.calls("/pin").is_empty());
    let call = &fake.calls("/pin")[0];
    assert_eq!((call.0.as_str(), call.2.clone()), ("PUT", json!({ "beforeSessionId": id(&r.rt, 0, 1) })));
    r.wait("the answer", |_| true);
    r.rt.frame(FRAME);
    assert_eq!(pin_order(&r.rt, 0, 2), Some(1.5));
    assert!(fake.calls("/api/sessions?").is_empty(), "the order is as expected, so no list is read");
}

#[test]
fn session_pins_puts_it_back_and_says_so_when_the_server_refuses() {
    let (mut r, _fake) = pinned_rig(|_, _| (409, r#"{"error":"Archived sessions can't be pinned; restore it first."}"#.into()));
    let c = id(&r.rt, 0, 2);
    r.rt.update(move |s, _| s.pin_session(&c, None));
    assert_eq!(pin_order(&r.rt, 0, 2), Some(3.0), "at once, at the end");
    r.wait("the refusal", |s| s.pins.error.is_some());
    assert_eq!(pin_order(&r.rt, 0, 2), None);
    assert_eq!(r.state().pins.error.as_deref(), Some("Archived sessions can't be pinned; restore it first."));
    r.rt.frame(FRAME);
    assert!(r.rt.find("toast").is_some(), "the one toast says it");
}

#[test]
fn session_pins_reads_every_pins_order_again_when_the_server_numbered_them_again() {
    let (mut r, fake) = pinned_rig(|method, path| {
        if method == "PUT" {
            (200, r#"{"pinOrder":2}"#.into())
        } else {
            assert!(path.starts_with("/api/sessions?limit="), "{path}");
            let row = |id: &str, order: u32| json!({"session": {"id": id}, "pinOrder": order});
            (200, json!([row("mock.0.0", 1), row("mock.0.2", 2), row("mock.0.1", 3)]).to_string())
        }
    });
    let (b, c) = (id(&r.rt, 0, 1), id(&r.rt, 0, 2));
    assert_eq!((id(&r.rt, 0, 0).as_str(), b.as_str(), c.as_str()), ("mock.0.0", "mock.0.1", "mock.0.2"));
    r.rt.update(move |s, _| s.pin_session(&c, Some(&b)));
    r.wait("every order read again", |s| s.projects[0].sessions[1].listing.pin_order == Some(3.0));
    assert_eq!([pin_order(&r.rt, 0, 0), pin_order(&r.rt, 0, 2), pin_order(&r.rt, 0, 1)], [Some(1.0), Some(2.0), Some(3.0)]);
    assert!(!fake.calls("/api/sessions?limit=").is_empty());
}

#[test]
fn session_pins_unpins_at_once_and_puts_the_pin_back_when_the_server_refuses() {
    let (mut r, fake) = pinned_rig(|method, path| {
        if method == "DELETE" && path.contains("mock.0.1") { (404, r#"{"error":"Session not found"}"#.into()) } else { (200, "{}".into()) }
    });
    let (a, b) = (id(&r.rt, 0, 0), id(&r.rt, 0, 1));
    r.rt.update(move |s, _| s.unpin_session(&a));
    assert_eq!(pin_order(&r.rt, 0, 0), None);
    r.wait("the unpin request", |_| !fake.calls("/pin").is_empty());
    assert_eq!(fake.calls("/pin")[0].0, "DELETE");
    r.rt.update(move |s, _| s.unpin_session(&b));
    r.wait("the refusal", |s| s.pins.error.is_some());
    assert_eq!(pin_order(&r.rt, 0, 1), Some(2.0));
    assert_eq!(r.state().pins.error.as_deref(), Some("Session not found"));
    assert_eq!(pin_order(&r.rt, 0, 0), None, "the first stays unpinned");
}

#[test]
fn session_pins_moves_a_pin_up_or_down_past_its_neighbour_and_not_past_the_ends() {
    let (mut r, fake) = pinned_rig(|_, _| (200, r#"{"pinOrder":0.5}"#.into()));
    let (a, b) = (id(&r.rt, 0, 0), id(&r.rt, 0, 1));
    r.rt.update({
        let a = a.clone();
        move |s, _| s.move_pin(&a, -1)
    });
    assert!(fake.calls("/pin").is_empty(), "the first can't go up");
    r.rt.update({
        let b = b.clone();
        move |s, _| s.move_pin(&b, 1)
    });
    assert!(fake.calls("/pin").is_empty(), "the last can't go down");
    let c = id(&r.rt, 0, 2);
    r.rt.update(move |s, _| s.move_pin(&c, -1));
    assert!(fake.calls("/pin").is_empty(), "one that isn't pinned can't move");
    r.rt.update({
        let b = b.clone();
        move |s, _| s.move_pin(&b, -1)
    });
    r.wait("the move", |_| !fake.calls("/pin").is_empty());
    assert_eq!(fake.calls("/pin")[0].2, json!({ "beforeSessionId": a }));
    assert_eq!(pin_order(&r.rt, 0, 1), Some(0.5));
}

#[test]
fn pinning_a_session_before_itself_does_nothing() {
    let (mut r, fake) = pinned_rig(|_, _| (200, r#"{"pinOrder":9}"#.into()));
    let a = id(&r.rt, 0, 0);
    r.rt.update(move |s, _| s.pin_session(&a, Some(&a.clone())));
    r.rt.frame(FRAME);
    assert!(fake.calls("/pin").is_empty());
    assert_eq!(pin_order(&r.rt, 0, 0), Some(1.0));
}

// ---- the row's buttons, the toast ---------------------------------------------------------------------------------------------

#[test]
fn a_row_has_pin_and_archive_buttons_with_fleets_labels() {
    let mut rt = app();
    rt.frame(FRAME);
    let title = rt.state.projects[0].sessions[1].title.clone();
    for key in ["session-pin-0-1", "session-archive-0-1"] {
        assert!(rt.find(key).is_some(), "{key}");
    }
    let labels: Vec<String> = rt
        .elements()
        .into_iter()
        .filter(|e| e.key.as_deref() == Some("session-pin-0-1") || e.key.as_deref() == Some("session-archive-0-1"))
        .filter_map(|e| e.label)
        .collect();
    assert!(labels.contains(&format!("Pin {title}")) && labels.contains(&format!("Archive {title}")), "{labels:?}");
    assert!(rt.find("session-restore-0-1").is_none(), "Restore is for an archived row");
    // Pinned: Unpin.
    rt.update(|s, _| s.projects[0].sessions[1].listing.pin_order = Some(1.0));
    rt.frame(FRAME);
    let label = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("session-pin-0-1")).and_then(|e| e.label);
    assert_eq!(label, Some(format!("Unpin {title}")));
}

#[test]
fn the_archive_button_hides_the_row_and_the_toast_offers_undo_with_a_drain() {
    let mut rt = app();
    let title = rt.state.projects[0].sessions[1].title.clone();
    click(&mut rt, "session-archive-0-1");
    rt.frame(FRAME);
    assert!(hidden(&rt, 0, 1), "the row goes at once");
    assert!(rt.find("toast").is_some() && rt.find("toast-action").is_some());
    let text = rt.elements().into_iter().filter_map(|e| e.label.or(e.text)).any(|t| t == format!("Archived \"{title}\""));
    assert!(text, "the toast says what went");
    let full = width(&rt, "toast-drain");
    rt.frame(3.0);
    let half = width(&rt, "toast-drain");
    assert!(full > 300.0 && half < full * 0.6 && half > full * 0.4, "the bar drains with the window: {full} then {half}");
    rt.frame(3.5);
    assert!(rt.find("toast").is_none(), "the toast goes with the window");
    assert!(hidden(&rt, 0, 1) && rt.state.projects[0].sessions[1].row.archived, "with no server the archive just happens");
}

#[test]
fn the_error_toast_has_a_dismiss_and_no_drain() {
    let mut rt = app();
    rt.update(|s, _| s.archive.error = Some("Couldn't archive \"Docs\".".into()));
    rt.frame(FRAME);
    assert!(rt.find("toast").is_some());
    assert!(rt.find("toast-drain").is_none(), "no drain on an error");
    click(&mut rt, "toast-action");
    rt.frame(FRAME);
    assert!(rt.find("toast").is_none() && rt.state.archive.error.is_none());
    // A pin error takes the same toast, and the dismiss clears it.
    rt.update(|s, _| s.pins.error = Some("Couldn't pin the session.".into()));
    rt.frame(FRAME);
    assert!(rt.find("toast").is_some());
    click(&mut rt, "toast-action");
    assert!(rt.state.pins.error.is_none());
}

#[test]
fn the_archive_error_comes_before_the_pin_error() {
    let mut rt = app();
    rt.update(|s, _| {
        s.pins.error = Some("pin".into());
        s.archive.error = Some("archive".into());
    });
    assert_eq!(rt.state.queue_toast().map(|t| t.message), Some("archive".into()));
}

#[test]
fn an_archived_row_offers_restore_in_place_of_archive_and_no_pin() {
    let mut rt = app();
    rt.update(|s, _| {
        s.store.retention = crate::fleet::store::Retention::All;
        s.projects[0].sessions[1].row.archived = true;
    });
    rt.frame(FRAME);
    assert!(rt.find("session-restore-0-1").is_some());
    assert!(rt.find("session-archive-0-1").is_none() && rt.find("session-pin-0-1").is_none());
    click(&mut rt, "session-restore-0-1");
    rt.frame(FRAME);
    assert!(!rt.state.projects[0].sessions[1].row.archived, "with no server it is restored at once");
    // The Active list leaves an archived row out.
    rt.update(|s, _| {
        s.store.retention = crate::fleet::store::Retention::Active;
        s.projects[0].sessions[1].row.archived = true;
    });
    rt.frame(FRAME);
    assert!(hidden(&rt, 0, 1));
}

#[test]
fn a_session_that_cannot_archive_has_no_archive_button() {
    let mut rt = app();
    rt.update(|s, _| s.projects[0].sessions[1].row.capabilities.can_archive = Some(false));
    rt.frame(FRAME);
    assert!(rt.find("session-archive-0-1").is_none() && rt.find("session-pin-0-1").is_some());
}

#[test]
fn alt_up_and_alt_down_move_a_pinned_rows_pin() {
    let fake = Fake::start(|_, _| (200, r#"{"pinOrder":0.5}"#.into()));
    let mut r = Rig::home(&fake);
    r.rt.update(|s, _| {
        s.projects[0].sessions[0].listing.pin_order = Some(1.0);
        s.projects[0].sessions[1].listing.pin_order = Some(2.0);
    });
    r.rt.frame(FRAME);
    r.rt.update(|_, tx| tx.focus("session-0-1", true));
    r.rt.frame(FRAME);
    r.rt.key(KeyEvent { key: Key::Up, mods: Mods { alt: true, ..Mods::default() } });
    r.rt.frame(FRAME);
    r.wait("the move", |_| !fake.calls("/pin").is_empty());
    assert_eq!(fake.calls("/pin")[0].2, json!({ "beforeSessionId": "mock.0.0" }));
    // A row that isn't pinned does nothing.
    r.rt.update(|_, tx| tx.focus("session-0-2", true));
    r.rt.frame(FRAME);
    r.rt.key(KeyEvent { key: Key::Down, mods: Mods { alt: true, ..Mods::default() } });
    r.rt.frame(FRAME);
    assert_eq!(fake.calls("/pin").len(), 1);
}

// ---- another machine ---------------------------------------------------------------------------------------------------------

const MINI: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// Home lists the machine too, as it does for the poll that runs while the test waits, so the poll agrees with what is set up.
fn on_machine(mini: &Fake) -> (Rig, Fake) {
    let body = json!({ "machines": [{"id": MINI, "name": "mini", "baseUrl": mini.base, "token": "token-of-mini"}] }).to_string();
    let listed = body.clone();
    let home = Fake::start(move |_, path| if path == "/api/machines" { (200, listed.clone()) } else { (200, "{}".into()) });
    let mut r = Rig::home(&home);
    r.rt.update(move |s, _| {
        s.machines.apply_list(parse_machine_list(200, &body));
        // Home's own tree is folded so the machine's rows are on screen.
        s.toggle_machine(crate::fleet::settings::HOME_MACHINE_KEY);
        let mut one = crate::mock::machines::listed("mini-1", "Water the tomatoes", crate::mock::Status::Idle, "now", None, |_| {});
        one.listing.pin_order = None;
        s.machines.upsert_session(MINI, one);
        let mut two =
            crate::mock::machines::listed("mini-2", "Paint the fence", crate::mock::Status::Idle, "1h", None, |l| l.pin_order = Some(4.0));
        two.title = "Paint the fence".into();
        s.machines.upsert_session(MINI, two);
        if let Some(m) = s.machines.others.get_mut(MINI) {
            m.loaded_at = Some(1);
        }
    });
    r.rt.frame(FRAME);
    (r, home)
}

#[test]
fn archive_and_pin_on_another_machine_reach_that_machine_with_its_token_and_nothing_reaches_home() {
    // The machine keeps what it was told, so the list home reads of it again while the test waits agrees with the calls.
    let state = Arc::new(Mutex::new((None::<f64>, false)));
    let kept = state.clone();
    let mini = Fake::start(move |method, path| {
        let mut state = kept.lock().unwrap();
        if method == "PUT" {
            state.0 = Some(5.0);
            return (200, r#"{"pinOrder":5}"#.into());
        }
        if method == "PATCH" {
            state.1 = true;
        }
        if path.starts_with("/api/sessions?") {
            let row = |id: &str, title: &str, pin: Option<f64>, archived: bool| {
                json!({"session": {"id": id, "title": title, "time": {"created": 1, "updated": 2}}, "pinOrder": pin,
                       "retentionStatus": if archived { "archived" } else { "active" }})
            };
            return (
                200,
                json!([row("mini-1", "Water the tomatoes", state.0, state.1), row("mini-2", "Paint the fence", Some(4.0), false)])
                    .to_string(),
            );
        }
        (200, "{}".into())
    });
    let (mut r, home) = on_machine(&mini);
    let (a, p) = (format!("session-archive-{MINI}-mini-1"), format!("session-pin-{MINI}-mini-1"));
    click(&mut r.rt, &p);
    r.wait("the pin on its machine", |_| !mini.calls("/pin").is_empty());
    assert_eq!(mini.calls("/pin")[0].3.as_deref(), Some("token-of-mini"));
    assert_eq!(mini.calls("/pin")[0].2, json!({ "beforeSessionId": null }));
    r.wait("its row to carry the server's number", |s| {
        s.machines.others.get(MINI).is_some_and(|m| m.sessions.iter().any(|x| x.id == "mini-1" && x.listing.pin_order == Some(5.0)))
    });
    // The pinned one of the same machine has an Unpin.
    assert!(r.rt.find(&format!("session-pin-{MINI}-mini-2")).is_some());
    click(&mut r.rt, &a);
    r.rt.frame(FRAME);
    assert!(r.rt.find(&format!("machine-session-{MINI}-mini-1")).is_none(), "its row is hidden at once");
    assert_eq!(r.state().archive.pending.as_ref().map(|p| p.message.clone()), Some("Archived \"Water the tomatoes\"".into()));
    r.rt.frame(UNDO_SECS + 0.1);
    r.wait("the archive on its machine", |_| !mini.calls("/retention").is_empty());
    let call = &mini.calls("/retention")[0];
    assert_eq!((call.0.as_str(), call.1.as_str(), call.3.as_deref()), ("PATCH", "/api/sessions/mini-1/retention", Some("token-of-mini")));
    r.wait("the answer", |s| !s.archive.any_hidden());
    assert_eq!(r.state().archive.error, None);
    assert!(home.calls("mini-").is_empty(), "nothing reaches home");
}

#[test]
fn a_machine_that_isnt_answering_has_no_row_buttons_and_a_failed_archive_there_shows_its_row_again() {
    let mut rt = app();
    rt.update(|s, _| s.show_machines(Case::Down, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    assert!(
        rt.elements()
            .iter()
            .all(|e| !e.key.as_deref().is_some_and(|k| k.starts_with("session-archive-m-") || k.starts_with("session-pin-m-")))
    );
    let mut rt = app();
    rt.update(|s, _| s.show_machines(Case::Several, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    // An answering machine's rows have them; with no server the archive happens at once.
    click(&mut rt, "session-archive-m-falcon-f-fence");
    rt.frame(FRAME);
    assert!(rt.find("machine-session-m-falcon-f-fence").is_none());
    rt.frame(UNDO_SECS + 0.1);
    assert!(rt.state.machines.others["m-falcon"].sessions.iter().any(|x| x.id == "f-fence" && x.row.archived));
    assert!(rt.find("machine-session-m-falcon-f-fence").is_none(), "an archived row stays out of the Active list");
}

#[test]
fn a_pinned_session_of_another_machine_unpins_from_its_own_button() {
    let mut rt = app();
    rt.update(|s, _| s.show_machines(Case::Several, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    assert_eq!(rt.state.machines.others["m-falcon"].sessions.iter().find(|x| x.id == "f-plan").unwrap().listing.pin_order, Some(1.0));
    click(&mut rt, "session-pin-m-falcon-f-plan");
    rt.frame(FRAME);
    assert_eq!(rt.state.machines.others["m-falcon"].sessions.iter().find(|x| x.id == "f-plan").unwrap().listing.pin_order, None);
}
