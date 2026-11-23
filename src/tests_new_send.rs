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
use std::io::Write;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

pub(crate) const ROCKET: &str = "/home/me/src/rocket";

#[derive(Default)]
pub(crate) struct Fake {
    /// The bodies `POST /api/sessions` got.
    posted: Vec<Value>,
    /// What it answers instead of a new session: a status and a body.
    refuse: Option<(u16, Value)>,
    /// While set, the answer waits for a message here.
    gate: Option<Arc<Mutex<Receiver<()>>>>,
    /// What `GET /api/directories/find` lists by folder, and the (folder, query) pairs it was asked.
    pub(crate) folder_files: std::collections::HashMap<String, Vec<String>>,
    pub(crate) finds: Vec<(String, String)>,
    /// What `GET /api/sessions` answers, and the paths it was asked (the `@` list reads `retentionStatus=all`).
    pub(crate) sessions: Vec<Value>,
    pub(crate) session_reads: Vec<String>,
    /// The (session, body) pairs `POST /api/sessions/{id}/prompt` got.
    pub(crate) prompts: Vec<(String, Value)>,
    /// The (key, value) pairs `PUT /api/preferences/{key}` got.
    pub(crate) preferences: Vec<(String, String)>,
    /// What `GET /api/machines` lists.
    machines: Vec<Value>,
    /// What `GET /api/repositories/detail` answers (a 404 while none is set), and, for the GitHub routes (the `#` picker's), what
    /// each path with its query answers, every one asked in order, and the paths whose next request waits for a message.
    pub(crate) detail: Option<Value>,
    pub(crate) github: std::collections::HashMap<String, (u16, Value)>,
    pub(crate) github_log: Vec<String>,
    pub(crate) github_gates: std::collections::HashMap<String, Arc<Mutex<Receiver<()>>>>,
}

pub(crate) type Shared = Arc<Mutex<Fake>>;

fn serve(fake: Shared) -> String {
    let listener = crate::test_http::bind().expect("a port");
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
    let req = crate::test_http::read(&mut stream);
    if req.method.is_empty() {
        return;
    }
    let (head, body) = (req.head, req.body);
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
    } else if method == "GET" && path.starts_with("/api/directories/find") {
        let value = |name: &str| {
            path.split_once('?')
                .map_or("", |(_, q)| q)
                .split('&')
                .find_map(|p| p.strip_prefix(&format!("{name}=")))
                .unwrap_or("")
                .to_string()
        };
        let (dir, q) = (value("path").replace("%2F", "/"), value("q").replace("%2F", "/"));
        let mut f = fake.lock().unwrap();
        f.finds.push((dir.clone(), q.clone()));
        let files: Vec<String> =
            f.folder_files.get(&dir).map(|l| l.iter().filter(|p| p.contains(&q)).cloned().collect()).unwrap_or_default();
        (200, json!({ "directory": dir, "files": files }))
    } else if method == "GET" && path.split('?').next() == Some("/api/repositories/detail") {
        fake.lock().unwrap().detail.clone().map_or((404, json!({})), |d| (200, d))
    } else if path.starts_with("/api/integrations/github/") {
        let (hit, gate) = {
            let mut f = fake.lock().unwrap();
            f.github_log.push(path.to_string());
            (f.github.get(path).cloned(), f.github_gates.remove(path))
        };
        if let Some(gate) = gate {
            let _ = gate.lock().unwrap().recv_timeout(test_wait::BOUND);
        }
        hit.unwrap_or((200, json!({})))
    } else if method == "GET" && path.split('?').next() == Some("/api/machines") {
        (200, json!({ "machines": fake.lock().unwrap().machines.clone() }))
    } else if method == "PUT" && path.starts_with("/api/preferences/") {
        let sent: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let value = sent["value"].as_str().unwrap_or("").to_string();
        fake.lock().unwrap().preferences.push((path.trim_start_matches("/api/preferences/").to_string(), value));
        (200, json!({}))
    } else if method == "POST" && path.starts_with("/api/sessions/") && path.ends_with("/prompt") {
        let sent: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let id = path.trim_start_matches("/api/sessions/").trim_end_matches("/prompt").to_string();
        fake.lock().unwrap().prompts.push((id, sent));
        (202, json!({}))
    } else if method == "GET" && path.starts_with("/api/sessions") {
        let mut f = fake.lock().unwrap();
        f.session_reads.push(path.to_string());
        (200, Value::Array(f.sessions.clone()))
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

pub(crate) struct Rig {
    pub(crate) rt: Runtime<State>,
    pub(crate) queue: Receiver<Job>,
    pub(crate) fake: Shared,
}

pub(crate) fn rig() -> Rig {
    rig_with(Default::default())
}

pub(crate) fn rig_with(fake: Shared) -> Rig {
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
    pub(crate) fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("route {:?}, starting {}", s.route, s.new_session_starting()));
    }

    pub(crate) fn text(&mut self, words: &str) {
        click(&mut self.rt, "new-session-message");
        self.rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    pub(crate) fn press(&mut self, key: Key, mods: Mods) {
        self.rt.key(KeyEvent { key, mods });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    pub(crate) fn enter(&mut self) {
        self.press(Key::Enter, Mods::default());
    }

    pub(crate) fn posted(&self) -> Vec<Value> {
        self.fake.lock().unwrap().posted.clone()
    }

    pub(crate) fn message(&self) -> String {
        self.rt.state.new_draft.draft().map(|d| d.message.clone()).unwrap_or_default()
    }

    /// Enter, and the wait for the answer to have been applied.
    pub(crate) fn send_and_wait(&mut self) {
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

// ─── another machine (A21.101, A21.104) ─────────────────────────────────────────

/// The rig, plus a second Fleet that home lists as a machine, and the page moved onto it.
pub(crate) fn rig_on_another_machine() -> (Rig, Shared) {
    let elsewhere: Shared = Default::default();
    let base = serve(elsewhere.clone());
    let falcon = json!({ "id": "m-falcon", "name": "falcon", "baseUrl": base, "os": "linux", "status": "online",
        "addedAt": "2026-09-26T00:00:00Z", "token": "falcon-token", "addresses": [{ "url": base, "kind": "lan", "learned": false }] });
    // Home lists it from its first answer on, as it does while the page is open.
    let home: Shared = Default::default();
    home.lock().unwrap().machines = vec![falcon];
    let mut r = rig_with(home);
    r.wait("home's machine list", |s| s.machines.list.len() == 1);
    // A draft with something in it is kept while the box is rebuilt for the machine (A3.12).
    r.text("Fix the login redirect");
    r.rt.update(|s, _| {
        s.pick_machine("m-falcon");
        s.set_folder(Folder::Repository("/srv/falcon/rocket".into()), true);
    });
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    (r, elsewhere)
}

#[test]
fn machine_on_the_new_session_page_starts_the_session_on_the_machine_picked_and_opens_it() {
    let (mut r, elsewhere) = rig_on_another_machine();
    assert_eq!(r.rt.state.target_key(), "m-falcon");
    r.send_and_wait();
    assert_eq!(elsewhere.lock().unwrap().posted.len(), 1, "the machine picked got the request");
    assert!(r.posted().is_empty(), "this machine got none");
    assert!(!r.rt.state.on_new_session_page(), "the session opened: {:?}", r.rt.state.new_box.create_error);
    assert_eq!(r.rt.state.machines.current_key(), "m-falcon");
}

#[test]
fn machine_on_another_machine_starts_the_session_there_and_opens_it_in_place_its_row_kept_out_of_the_live_machines_list() {
    let (mut r, _) = rig_on_another_machine();
    r.send_and_wait();
    let live = r.rt.state.projects.iter().flat_map(|p| p.sessions.iter()).any(|s| s.id == "ses-new");
    assert!(!live, "the new row is not in this machine's own list");
    assert_eq!(r.rt.state.machine_key_of("ses-new"), "m-falcon");
}
