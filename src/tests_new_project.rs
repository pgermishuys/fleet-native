//! The New project dialog (piece O of docs/parity/06-session-list.md): the four cases of `NewProjectDialog.test.ts` under
//! their names, and the rows Fleet's tests don't name (Enter, blank name, error and its clearing, reset, one at a time).
//! The ones that read a request use a fake server on a port of its own; the machines are the invented ones of `mock::machines`.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::new_project::{create_body, unreachable};
use crate::mock::machines::Case;
use crate::tests::{FRAME, app, click};
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::{Value, json};
use std::io::Write;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

type Rt = Runtime<crate::State>;
type Heard = Arc<Mutex<Vec<(String, Value)>>>;

/// A fake Fleet: records the body of each `POST /api/projects` and answers it with `status` and `text`.
struct Fake {
    base: String,
    heard: Heard,
}

impl Fake {
    fn start(status: u16, text: &'static str) -> Fake {
        let listener = crate::test_http::bind().unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let heard: Heard = Default::default();
        let log = heard.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let log = log.clone();
                std::thread::spawn(move || serve(stream, log, status, text));
            }
        });
        Fake { base, heard }
    }

    fn posts(&self) -> Vec<Value> {
        self.heard.lock().unwrap().iter().filter(|c| c.0 == "/api/projects").map(|c| c.1.clone()).collect()
    }
}

fn serve(mut stream: std::net::TcpStream, log: Heard, status: u16, text: &'static str) {
    let heard = crate::test_http::read(&mut stream);
    if heard.method.is_empty() {
        return;
    }
    let (head, raw) = (heard.head, heard.body);
    let mut parts = head.lines().next().unwrap_or("").split(' ');
    let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or("").to_string());
    let (status, text) = if method == "POST" && path == "/api/projects" {
        log.lock().unwrap().push((path, serde_json::from_slice(&raw).unwrap_or(Value::Null)));
        (status, text)
    } else {
        (404, "{}")
    };
    let _ = stream.try_clone().unwrap().write_all(
        format!(
            "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
            text.len()
        )
        .as_bytes(),
    );
}

const CREATED: &str = r#"{"id":"p-new","name":"Release pipeline","type":"user","position":3,"sessionCount":0}"#;

struct Rig {
    rt: Rt,
    jobs: Receiver<Job>,
}

impl Rig {
    /// The app with its home at `home`.
    fn new(home: &Fake) -> Rig {
        let mut rt = app();
        let (tx, jobs) = channel::<Job>();
        let tx = Mutex::new(tx);
        let link = Link::start(home.base.clone(), Post::new(move |job| drop(tx.lock().unwrap().send(job))));
        rt.update(move |s, _| s.link = Some(link));
        rt.frame(FRAME);
        Rig { rt, jobs }
    }

    fn wait(&mut self, what: &str, done: impl FnMut(&crate::State) -> bool) {
        crate::test_wait::drive(&mut self.rt, &self.jobs, what, done, crate::test_wait::app_state);
    }
}

fn press(rt: &mut Rt, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn type_name(rt: &mut Rt, name: &str) {
    press(rt, Key::Text(name.into()));
}

fn open(rt: &mut Rt) {
    click(rt, "add-folder");
    rt.frame(FRAME);
    assert!(rt.find("new-project-dialog").is_some(), "the dialog is open");
}

fn is_open(rt: &Rt) -> bool {
    rt.find("new-project-dialog").is_some()
}

fn words(rt: &Rt, key: &str) -> String {
    let all = rt.elements();
    let it = all.iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    all.iter().filter(|e| e.path.starts_with(&it.path)).filter_map(|e| e.text.clone()).collect::<String>()
}

fn project_names(rt: &Rt) -> Vec<String> {
    rt.state.projects.iter().map(|p| p.name.clone()).collect()
}

/// Falcon (an invented machine that answers) with its address at `base`.
fn with_falcon(rt: &mut Rt, base: &str) {
    let base = base.to_string();
    rt.update(move |s, _| {
        s.show_machines_only(Case::Down);
        s.machines.list.clear();
        s.machines.others.clear();
        let mut falcon = crate::mock::machines::machine("m-falcon", "falcon");
        falcon.base_url = base;
        s.machines.others.insert(falcon.id.clone(), Default::default());
        s.machines.list.push(falcon);
    });
    rt.frame(FRAME);
}

// ---- the request ------------------------------------------------------------------------------------------------------------

#[test]
fn the_request_is_the_one_fleet_sends() {
    assert_eq!(create_body("  Release pipeline ", ""), json!({ "name": "Release pipeline" }));
    assert_eq!(create_body("Docs", "   "), json!({ "name": "Docs" }));
    assert_eq!(create_body("Docs", " Notes and guides "), json!({ "name": "Docs", "description": "Notes and guides" }));
    assert_eq!(unreachable("falcon"), "Can't reach falcon. The project wasn't created.");
}

// ---- NewProjectDialog.test.ts -----------------------------------------------------------------------------------------------

#[test]
fn new_project_dialog_doesnt_ask_for_a_machine_when_there_is_only_one_and_creates_the_project_on_it() {
    let home = Fake::start(201, CREATED);
    let mut r = Rig::new(&home);
    open(&mut r.rt);
    assert!(r.rt.find("new-project-machine").is_none(), "no machine field with one machine");
    type_name(&mut r.rt, "Release pipeline");
    press(&mut r.rt, Key::Enter);
    r.wait("the project to be sent", |_| !home.posts().is_empty());
    assert_eq!(home.posts(), vec![json!({ "name": "Release pipeline" })]);
    r.wait("the dialog to close", |s| s.proj.new.is_none());
}

#[test]
fn new_project_dialog_creates_the_project_on_the_machine_picked_from_a_headings_new_project_on() {
    let (home, falcon) = (Fake::start(201, CREATED), Fake::start(201, CREATED));
    let mut r = Rig::new(&home);
    with_falcon(&mut r.rt, &falcon.base);
    r.rt.update(|s, tx| s.new_project_on("m-falcon", tx));
    r.rt.frame(FRAME);
    assert!(words(&r.rt, "new-project-machine").contains("falcon"));
    type_name(&mut r.rt, "Release pipeline");
    click(&mut r.rt, "new-project-submit");
    r.wait("the project to reach falcon", |_| !falcon.posts().is_empty());
    assert_eq!(falcon.posts(), vec![json!({ "name": "Release pipeline" })]);
    assert!(home.posts().is_empty(), "home heard nothing");
    r.wait("the dialog to close", |s| s.proj.new.is_none());
}

#[test]
fn new_project_dialog_starts_on_the_machine_youre_on_and_lets_you_pick_another() {
    let (home, falcon) = (Fake::start(201, CREATED), Fake::start(201, CREATED));
    let mut r = Rig::new(&home);
    with_falcon(&mut r.rt, &falcon.base);
    open(&mut r.rt);
    assert!(!words(&r.rt, "new-project-machine").contains("falcon"));
    click(&mut r.rt, "new-project-machine");
    click(&mut r.rt, "new-project-machine-m-falcon");
    assert!(words(&r.rt, "new-project-machine").contains("falcon"));
    type_name(&mut r.rt, "Release pipeline");
    click(&mut r.rt, "new-project-submit");
    r.wait("the project to reach falcon", |_| !falcon.posts().is_empty());
    assert!(home.posts().is_empty());
}

#[test]
fn new_project_dialog_keeps_the_dialog_open_and_names_the_machine_when_it_cant_be_reached() {
    let home = Fake::start(201, CREATED);
    // A port nobody listens on.
    let closed = crate::test_http::dead_address();
    let mut r = Rig::new(&home);
    with_falcon(&mut r.rt, &closed);
    r.rt.update(|s, tx| s.new_project_on("m-falcon", tx));
    r.rt.frame(FRAME);
    type_name(&mut r.rt, "Release pipeline");
    click(&mut r.rt, "new-project-submit");
    r.wait("the failure", |s| s.proj.new.as_ref().is_some_and(|d| d.error.is_some()));
    r.rt.frame(FRAME);
    assert_eq!(words(&r.rt, "new-project-error"), unreachable("falcon"));
    assert!(is_open(&r.rt));
    assert_eq!(r.rt.state.proj.new.as_ref().unwrap().name.text, "Release pipeline", "what was typed is kept");
}

// ---- rows the cases don't name ----------------------------------------------------------------------------------------------

#[test]
fn a_blank_name_does_nothing_and_the_button_is_off() {
    let home = Fake::start(201, CREATED);
    let mut r = Rig::new(&home);
    open(&mut r.rt);
    press(&mut r.rt, Key::Enter);
    click(&mut r.rt, "new-project-submit");
    type_name(&mut r.rt, "   ");
    press(&mut r.rt, Key::Enter);
    assert!(is_open(&r.rt));
    assert!(home.posts().is_empty());
    assert!(!r.rt.state.proj.new.as_ref().unwrap().creating);
}

#[test]
fn the_description_is_sent_trimmed() {
    let home = Fake::start(201, CREATED);
    let mut r = Rig::new(&home);
    open(&mut r.rt);
    type_name(&mut r.rt, "Docs");
    click(&mut r.rt, "new-project-description");
    type_name(&mut r.rt, " Notes and guides ");
    click(&mut r.rt, "new-project-submit");
    r.wait("the project to be sent", |_| !home.posts().is_empty());
    assert_eq!(home.posts(), vec![json!({ "name": "Docs", "description": "Notes and guides" })]);
}

#[test]
fn a_refusal_shows_the_servers_words_keeps_the_text_and_clears_when_you_type() {
    let home = Fake::start(409, r#"{"error":"A project with this name already exists."}"#);
    let mut r = Rig::new(&home);
    open(&mut r.rt);
    type_name(&mut r.rt, "Docs");
    press(&mut r.rt, Key::Enter);
    r.wait("the refusal", |s| s.proj.new.as_ref().is_some_and(|d| d.error.is_some()));
    r.rt.frame(FRAME);
    assert_eq!(words(&r.rt, "new-project-error"), "A project with this name already exists.");
    assert_eq!(r.rt.state.proj.new.as_ref().unwrap().name.text, "Docs");
    click(&mut r.rt, "new-project-name");
    type_name(&mut r.rt, "2");
    assert!(r.rt.find("new-project-error").is_none(), "the error goes when the name changes");
}

#[test]
fn closing_resets_the_form_and_escape_closes() {
    let mut rt = app();
    open(&mut rt);
    type_name(&mut rt, "Docs");
    press(&mut rt, Key::Escape);
    assert!(!is_open(&rt));
    open(&mut rt);
    assert_eq!(rt.state.proj.new.as_ref().unwrap().name.text, "");
    click(&mut rt, "new-project-cancel");
    assert!(!is_open(&rt));
}

#[test]
fn only_one_create_runs_at_a_time_and_the_fields_are_off_while_it_does() {
    let mut rt = app();
    open(&mut rt);
    rt.update(|s, _| s.show_new_project(crate::fleet::new_project::Scene::Creating));
    rt.frame(FRAME);
    assert!(words(&rt, "new-project-submit").contains("Creating\u{2026}"));
    rt.update(|s, tx| s.new_project_submit(tx));
    press(&mut rt, Key::Escape);
    assert!(is_open(&rt), "nothing closes it while it is being made");
}

#[test]
fn with_no_server_the_invented_list_takes_the_project() {
    let mut rt = app();
    let before = project_names(&rt).len();
    open(&mut rt);
    type_name(&mut rt, "Release pipeline");
    click(&mut rt, "new-project-submit");
    rt.frame(FRAME);
    assert!(!is_open(&rt));
    let names = project_names(&rt);
    assert_eq!(names.len(), before + 1);
    assert!(names.contains(&"Release pipeline".to_string()));
}

#[test]
fn a_machine_heading_that_isnt_answering_cant_open_the_dialog() {
    let mut rt = app();
    rt.update(|s, _| s.show_machines_only(Case::Down));
    rt.update(|s, tx| s.new_project_on("m-harbor", tx));
    assert!(rt.state.proj.new.is_none());
}
