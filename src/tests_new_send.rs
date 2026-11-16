//! Piece L of the New session page: the message box and what Enter does (`NewSessionComposer.vue`; rows A4, A5, A13, A14),
//! against a fake Fleet that records what `POST /api/sessions` was sent and answers as a test says. The tests have the names
//! of Fleet's own cases in `NewSessionComposer.test.ts` ("keyboard", "quick chat", "feels instant").
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::new_session::request::Folder;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::store::Route;
use crate::tests::{FRAME, app, click};
use crate::{State, test_wait};
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

const ROCKET: &str = "/home/me/src/rocket";

#[derive(Default)]
struct Fake {
    /// The bodies `POST /api/sessions` got.
    posted: Vec<Value>,
    /// What it answers instead of a new session: a status and a body.
    refuse: Option<(u16, Value)>,
    /// While set, the answer waits for a message here.
    gate: Option<Arc<Mutex<Receiver<()>>>>,
}

type Shared = Arc<Mutex<Fake>>;

fn serve(fake: Shared) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let fake = fake.clone();
            std::thread::spawn(move || answer(stream, fake));
        }
    });
    base
}

fn answer(mut stream: std::net::TcpStream, fake: Shared) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let (head, mut body) = loop {
        let n = stream.read(&mut chunk).unwrap_or(0);
        buf.extend_from_slice(&chunk[..n]);
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break (String::from_utf8_lossy(&buf[..at]).to_string(), buf[at + 4..].to_vec());
        }
        if n == 0 {
            return;
        }
    };
    let wanted: usize =
        head.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse().ok())).unwrap_or(0);
    while body.len() < wanted {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    let mut first = head.lines().next().unwrap_or("").split(' ');
    let (method, path) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    let (status, reply) = if method == "POST" && path == "/api/sessions" {
        let posted: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let (gate, refuse) = {
            let mut f = fake.lock().unwrap();
            f.posted.push(posted.clone());
            (f.gate.clone(), f.refuse.clone())
        };
        if let Some(gate) = gate {
            let _ = gate.lock().unwrap().recv_timeout(test_wait::BOUND);
        }
        match refuse {
            Some(answer) => answer,
            None => {
                let title = posted.pointer("/title").and_then(Value::as_str).unwrap_or("").to_string();
                (
                    200,
                    json!({ "instanceId": "inst-1", "workspaceId": "ws-1", "harnessType": "opencode",
                    "session": { "id": "ses-new", "title": title, "time": { "created": 1, "updated": 1 } } }),
                )
            }
        }
    } else {
        (404, json!({}))
    };
    let reply = reply.to_string();
    let out = format!(
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(out.as_bytes());
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Shared,
}

fn rig() -> Rig {
    let fake: Shared = Default::default();
    let base = serve(fake.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    let link = Link::start(base, post);
    rt.update(move |s, _| s.link = Some(link));
    click(&mut rt, "new-session");
    rt.frame(FRAME);
    rt.update(|s, _| s.set_folder(Folder::Repository(ROCKET.into()), true));
    rt.frame(FRAME);
    Rig { rt, queue, fake }
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("route {:?}, starting {}", s.route, s.new_session_starting()));
    }

    fn text(&mut self, words: &str) {
        click(&mut self.rt, "new-session-message");
        self.rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    fn press(&mut self, key: Key, mods: Mods) {
        self.rt.key(KeyEvent { key, mods });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    fn enter(&mut self) {
        self.press(Key::Enter, Mods::default());
    }

    fn posted(&self) -> Vec<Value> {
        self.fake.lock().unwrap().posted.clone()
    }

    fn message(&self) -> String {
        self.rt.state.new_draft.draft().map(|d| d.message.clone()).unwrap_or_default()
    }

    /// Enter, and the wait for the answer to have been applied.
    fn send_and_wait(&mut self) {
        self.enter();
        self.wait("the session to open", |s| !s.on_new_session_page() || s.new_box.create_error.is_some());
    }

    fn hold_answer(&mut self) -> std::sync::mpsc::Sender<()> {
        let (tx, rx) = channel();
        self.fake.lock().unwrap().gate = Some(Arc::new(Mutex::new(rx)));
        tx
    }
}

fn exists(rig: &Rig, key: &str) -> bool {
    rig.rt.find(key).is_some()
}

// ─── keyboard ───────────────────────────────────────────────────────────────────

#[test]
fn keyboard_enter_creates_the_session_with_the_message_as_its_first_prompt() {
    let mut r = rig();
    r.text("Fix the login redirect");
    r.send_and_wait();
    let posted = r.posted();
    assert_eq!(posted.len(), 1);
    assert_eq!(posted[0]["directory"], ROCKET);
    assert_eq!(posted[0]["initialPrompt"], "Fix the login redirect");
    assert_eq!(r.rt.state.route, Route::Session("ses-new".into()));
}

#[test]
fn keyboard_shift_enter_is_a_new_line_not_a_send() {
    let mut r = rig();
    r.text("first line");
    r.press(Key::Enter, Mods { shift: true, ..Mods::default() });
    assert!(r.posted().is_empty());
    assert!(r.message().contains('\n'), "the box took a new line: {:?}", r.message());
    assert!(r.rt.state.on_new_session_page());
}

#[test]
fn keyboard_enter_with_an_empty_message_does_nothing() {
    let mut r = rig();
    click(&mut r.rt, "new-session-message");
    r.enter();
    assert!(r.posted().is_empty());
    assert!(!r.rt.state.new_session_starting());
}

#[test]
fn keyboard_says_why_no_harness_is_ready_and_wont_send_until_one_is() {
    let mut r = rig();
    r.rt.update(|s, _| {
        let mut off = crate::fleet::new_session::harness_choice::HarnessEntry::ready("opencode");
        off.available = false;
        off.reason = Some("OpenCode isn't installed.".into());
        s.feeds.harnesses.get_mut(HOME_MACHINE_KEY).expect("home's harnesses").harnesses.all = vec![off];
        s.settle_choices();
    });
    r.text("Anything");
    r.rt.frame(FRAME);
    assert!(exists(&r, "new-session-no-harness"));
    r.enter();
    assert!(r.posted().is_empty());
    assert!(!r.rt.state.new_session_starting());
    assert!(!r.rt.state.can_send_new_session());
}

#[test]
fn keyboard_esc_never_leaves_the_page() {
    let mut r = rig();
    r.text("Half a thought");
    r.press(Key::Escape, Mods::default());
    assert!(r.rt.state.on_new_session_page());
    assert_eq!(r.message(), "Half a thought");
}

#[test]
fn keyboard_the_first_time_enter_asks_where_to_run_instead_of_creating() {
    let mut r = rig();
    r.rt.update(|s, _| s.new_draft.draft_mut().expect("draft").folder = None);
    r.text("Fix it");
    r.enter();
    assert!(r.posted().is_empty());
    assert!(r.rt.state.folder_menu_open(), "the folder menu opened");
    assert_eq!(r.message(), "Fix it");
}

// ─── quick chat, and the other ways to start ────────────────────────────────────

#[test]
fn quick_chat_choosing_no_folder_creates_nothing_and_enter_creates_a_chat_with_the_message() {
    let mut r = rig();
    r.rt.update(|s, _| s.set_folder(Folder::None, true));
    r.rt.frame(FRAME);
    assert!(r.posted().is_empty(), "choosing No folder creates nothing");
    r.text("What is a monad?");
    r.send_and_wait();
    let sent = &r.posted()[0];
    assert_eq!(sent["source"]["key"]["providerId"], "builtin.quickchat");
    assert_eq!(sent["initialPrompt"], "What is a monad?");
    assert!(sent["directory"].is_null());
}

#[test]
fn starts_without_a_message() {
    let mut r = rig();
    assert!(exists(&r, "create-session-without-message"));
    click(&mut r.rt, "create-session-without-message");
    r.wait("the session to open", |s| !s.on_new_session_page());
    let sent = &r.posted()[0];
    assert!(sent.get("initialPrompt").is_none_or(Value::is_null));
}

#[test]
fn uses_a_typed_folder() {
    let mut r = rig();
    r.rt.update(|s, _| s.set_folder(Folder::Directory("/home/me/notes".into()), true));
    r.text("Tidy the notes");
    r.send_and_wait();
    assert_eq!(r.posted()[0]["directory"], "/home/me/notes");
}

// ─── what is remembered, and what the server says ───────────────────────────────

#[test]
fn remembers_the_choices_a_session_was_created_with() {
    let mut r = rig();
    r.text("Fix the login redirect");
    r.send_and_wait();
    let defaults = r.rt.state.sidebar.new_session_defaults(HOME_MACHINE_KEY);
    assert_eq!(defaults.initial_folder(&[ROCKET.to_string()]), Some(Folder::Repository(ROCKET.into())));
}

#[test]
fn shows_the_servers_error_above_the_box_and_keeps_the_message() {
    let mut r = rig();
    r.fake.lock().unwrap().refuse = Some((409, json!({ "error": "Couldn't create the worktree: 'x' is not a valid branch name" })));
    r.text("Fix the login redirect");
    r.send_and_wait();
    r.rt.frame(FRAME);
    assert!(r.rt.state.on_new_session_page());
    assert!(exists(&r, "new-session-error"));
    assert_eq!(r.rt.state.new_session_error(), Some("Couldn't create the worktree: 'x' is not a valid branch name"));
    assert_eq!(r.message(), "Fix the login redirect");
    // Nothing is remembered for a session that wasn't made.
    let defaults = r.rt.state.sidebar.new_session_defaults(HOME_MACHINE_KEY);
    assert_eq!(defaults.initial_folder(&[ROCKET.to_string()]), None);
}

// ─── feels instant ──────────────────────────────────────────────────────────────

#[test]
fn feels_instant_shows_the_message_as_the_conversations_first_right_after_enter_while_the_session_starts() {
    let mut r = rig();
    let release = r.hold_answer();
    r.text("Fix the login redirect");
    r.enter();
    assert!(r.rt.state.new_session_starting());
    assert!(exists(&r, "new-session-conversation"), "the sent message stands where the stage was");
    assert!(!exists(&r, "new-session-empty"));
    assert_eq!(r.rt.state.new_draft.row().map(|d| d.is_starting), Some(true), "the sidebar row says Starting…");
    // A second Enter while it starts sends nothing more.
    r.enter();
    release.send(()).unwrap();
    r.wait("the session to open", |s| !s.on_new_session_page());
    assert_eq!(r.posted().len(), 1);
}

#[test]
fn feels_instant_hands_the_session_page_its_first_message_and_its_sidebar_row_before_opening_it() {
    let mut r = rig();
    r.text("Fix the login redirect");
    r.send_and_wait();
    let s = &r.rt.state;
    assert_eq!(s.open_id().as_deref(), Some("ses-new"));
    let pending = s.prompts.of("ses-new");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].body, "Fix the login redirect");
    assert!(s.projects.iter().flat_map(|p| &p.sessions).any(|row| row.id == "ses-new"), "the row is in the list");
    assert_eq!(s.new_draft.row_key_of("ses-new").map(|k| !k.is_empty()), Some(true), "the row keeps the draft's place");
}

#[test]
fn feels_instant_titles_the_session_after_the_first_message() {
    let mut r = rig();
    r.text("Fix the login redirect. It loops.");
    r.send_and_wait();
    assert_eq!(r.posted()[0]["title"], "Fix the login redirect. It loops.");
}

#[test]
fn feels_instant_seeds_no_message_when_starting_without_one() {
    let mut r = rig();
    click(&mut r.rt, "create-session-without-message");
    r.wait("the session to open", |s| !s.on_new_session_page());
    assert!(r.rt.state.prompts.of("ses-new").is_empty());
}

#[test]
fn feels_instant_puts_the_message_back_in_the_box_when_the_session_cant_be_created() {
    let mut r = rig();
    r.fake.lock().unwrap().refuse = Some((500, json!({ "error": "The harness fell over" })));
    r.text("Fix the login redirect");
    r.send_and_wait();
    r.rt.frame(FRAME);
    assert!(!r.rt.state.new_session_starting());
    assert_eq!(r.rt.state.new_box.edit.text, "Fix the login redirect");
    assert!(r.rt.state.projects.iter().flat_map(|p| &p.sessions).all(|row| row.id != "ses-new"), "no row was added");
}

#[test]
fn feels_instant_a_session_started_after_the_page_was_left_stays_where_you_went() {
    let mut r = rig();
    let release = r.hold_answer();
    r.text("Fix the login redirect");
    r.enter();
    click(&mut r.rt, "new-session-draft-row");
    r.rt.update(|s, tx| {
        let at = s.projects.iter().position(|p| !p.sessions.is_empty()).expect("a project");
        s.open_session(at, 0, tx);
    });
    release.send(()).unwrap();
    r.wait("the row to be in the list", |s| s.projects.iter().flat_map(|p| &p.sessions).any(|row| row.id == "ses-new"));
    assert_ne!(r.rt.state.route, Route::Session("ses-new".into()), "it did not pull the window back");
}

#[test]
fn a_starter_fills_the_box_with_its_ellipsis_selected_and_sends_nothing() {
    let mut r = rig();
    r.rt.update(|s, _| s.fill_starter("explore"));
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    assert!(r.message().ends_with('…') || r.message().contains('…'), "the starter's words are in the box: {:?}", r.message());
    assert_eq!(r.rt.state.new_box.edit.selected(), "…");
    assert!(r.posted().is_empty());
}
