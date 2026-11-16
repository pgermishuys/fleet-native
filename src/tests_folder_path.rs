//! Piece G of the New session page: the folder menu's path view and the making of folders (`NSD/FolderPicker.vue`, rows
//! B2.110-B2.145), driven with no window against a fake machine that keeps a small tree of folders. The tests have the
//! names of Fleet's own ("the folder box" and "making a folder" in `NewSessionComposer.test.ts`).
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::new_session::feed_state::Feeds;
use crate::fleet::new_session::folder_menu::Make;
use crate::fleet::new_session::path::PathAction;
use crate::fleet::new_session::request::Folder;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::tests::{FRAME, app, click};
use crate::{State, test_wait};
use rune::{Key, KeyEvent, Mods, Runtime, TextEdit};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

const HOME: &str = "/home/me";

/// What the fake machine says that a test may change.
pub(crate) struct Config {
    /// The home folder `GET /api/directories/defaults` reports (the machine's own is always `HOME`).
    pub(crate) home: String,
    pub(crate) roots: Vec<String>,
    pub(crate) inspect: Value,
    pub(crate) create: (u16, Value),
    /// Folders on the machine, and whether each is a git checkout.
    pub(crate) folders: Vec<(String, bool)>,
    pub(crate) seen: Vec<String>,
    /// The answer to a clone: its status and what the stream does (piece H).
    pub(crate) clone: (u16, Vec<Step>),
    /// A `Step::Wait` waits for one message here; the test sends it when it wants the stream to go on.
    pub(crate) gate: Option<Arc<Mutex<Receiver<()>>>>,
}

/// One thing the fake machine's clone stream does.
#[derive(Clone)]
pub(crate) enum Step {
    /// Writes these bytes and flushes (a line may be cut in two).
    Write(String),
    Wait,
}

pub(crate) type Shared = Arc<Mutex<Config>>;

fn config() -> Shared {
    let dir = |p: &str, git: bool| (p.to_string(), git);
    Arc::new(Mutex::new(Config {
        home: HOME.into(),
        roots: vec!["/home/me/src".into()],
        inspect: json!({ "path": "/home/me/src/notes", "exists": true, "isGitRepo": false, "isWithinRoots": true }),
        create: (200, json!({ "path": "/home/me/src/recipe-box", "isGitRepo": true, "addedToFleet": false, "warning": null })),
        folders: vec![
            dir("/home/me", false),
            dir("/home/me/src", false),
            dir("/home/me/src/clients", false),
            dir("/home/me/src/clients/northwind-api", false),
            dir("/home/me/src/comet", true),
            dir("/home/me/src/rocket", true),
            dir("/home/me/deep", false),
            dir("/home/me/deep/agent-playbook", true),
            dir("/opt", false),
            dir("/opt/agent-playbook", true),
            dir("/srv", false),
        ],
        seen: Vec::new(),
        clone: (200, Vec::new()),
        gate: None,
    }))
}

fn decode(value: &str) -> String {
    let b = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(n) = u8::from_str_radix(&value[i + 1..i + 3], 16)
        {
            out.push(n);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn listing(cfg: &Config, asked: &str) -> Value {
    let expanded = if let Some(rest) = asked.strip_prefix('~') { format!("{HOME}{rest}") } else { asked.to_string() };
    let path = if expanded.len() > 1 { expanded.trim_end_matches('/').to_string() } else { expanded };
    let has = |p: &str| cfg.folders.iter().any(|(f, _)| f == p);
    if has(&path) {
        let entries: Vec<Value> = cfg
            .folders
            .iter()
            .filter(|(f, _)| f.rsplit_once('/').is_some_and(|(parent, _)| parent == path))
            .map(|(f, git)| json!({ "name": f.rsplit('/').next(), "path": f, "isGitRepo": git }))
            .collect();
        return json!({ "currentPath": path, "exists": true, "nearestExisting": null, "entries": entries });
    }
    let mut up = path.clone();
    let nearest = loop {
        match up.rsplit_once('/') {
            Some((parent, _)) if !parent.is_empty() => {
                up = parent.to_string();
                if has(&up) {
                    break Some(up.clone());
                }
            }
            _ => break None,
        }
    };
    json!({ "currentPath": path, "exists": false, "nearestExisting": nearest, "entries": [] })
}

fn serve(cfg: Shared) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let cfg = cfg.clone();
            std::thread::spawn(move || {
                let mut buf = vec![0u8; 16384];
                let n = stream.read(&mut buf).unwrap_or(0);
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                let mut first = text.lines().next().unwrap_or("").split(' ');
                let (method, target) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
                let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
                let (path, query) = target.split_once('?').unwrap_or((&target, ""));
                let asked = query.split('&').find_map(|p| p.strip_prefix("path=")).map(decode).unwrap_or_default();
                if method == "POST" && path == "/api/directories/clone" {
                    let (status, steps, gate) = {
                        let mut c = cfg.lock().unwrap();
                        c.seen.push(format!("{method} {path} {asked} {body}").trim().to_string());
                        (c.clone.0, c.clone.1.clone(), c.gate.clone())
                    };
                    let _ = write!(stream, "HTTP/1.1 {status} X\r\ncontent-type: application/x-ndjson\r\nconnection: close\r\n\r\n");
                    for step in steps {
                        match step {
                            Step::Write(bytes) => {
                                let _ = stream.write_all(bytes.as_bytes());
                                let _ = stream.flush();
                            }
                            Step::Wait => {
                                if let Some(gate) = &gate {
                                    let _ = gate.lock().unwrap().recv_timeout(test_wait::BOUND);
                                }
                            }
                        }
                    }
                    return;
                }
                let mut cfg = cfg.lock().unwrap();
                cfg.seen.push(format!("{method} {path} {asked} {body}").trim().to_string());
                let (status, reply) = match (method.as_str(), path) {
                    ("GET", "/api/workspace-roots") => {
                        (200, json!({ "roots": cfg.roots.iter().map(|r| json!({ "path": r, "exists": true })).collect::<Vec<_>>() }))
                    }
                    ("POST", "/api/workspace-roots") => (200, json!({})),
                    ("GET", "/api/directories/defaults") => (200, json!({ "firstBranch": "main", "home": cfg.home })),
                    ("GET", "/api/directories") => (200, listing(&cfg, &asked)),
                    ("GET", "/api/directories/inspect") => (200, cfg.inspect.clone()),
                    ("POST", "/api/directories") => cfg.create.clone(),
                    ("GET", "/api/repositories") | ("POST", "/api/repositories/refresh") => (
                        200,
                        json!({ "repositories": [
                            { "name": "comet", "path": "/home/me/src/comet", "parentRoot": "/home/me/src" },
                            { "name": "rocket", "path": "/home/me/src/rocket", "parentRoot": "/home/me/src" },
                        ], "scannedAt": 1 }),
                    ),
                    _ => (404, json!({})),
                };
                let reply = reply.to_string();
                let out = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                    reply.len()
                );
                let _ = stream.write_all(out.as_bytes());
            });
        }
    });
    base
}

pub(crate) struct Rig {
    pub(crate) rt: Runtime<State>,
    queue: Receiver<Job>,
    pub(crate) cfg: Shared,
}

pub(crate) fn rig() -> Rig {
    rig_with(|_| {})
}

pub(crate) fn rig_with(change: impl FnOnce(&mut Config)) -> Rig {
    let cfg = config();
    change(&mut cfg.lock().unwrap());
    let base = serve(cfg.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    let link = Link::start(base, post);
    rt.update(move |s, _| {
        s.link = Some(link);
        s.feeds = Feeds::default();
    });
    click(&mut rt, "new-session");
    rt.frame(FRAME);
    rt.update(|s, _| s.read_repositories(HOME_MACHINE_KEY, false));
    let mut r = Rig { rt, queue, cfg };
    r.wait("the repositories", |s| !s.feeds.repos(HOME_MACHINE_KEY).list.is_empty());
    r
}

impl Rig {
    pub(crate) fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| {
            format!(
                "listing {:?}, error {:?}, make {:?}, view {:?}",
                s.feeds.listing.path, s.folder_menu.listing_error, s.folder_menu.make, s.folder_menu.view
            )
        });
    }

    pub(crate) fn state(&self) -> &State {
        &self.rt.state
    }

    /// Opens the menu and waits for the machine's locations and defaults.
    pub(crate) fn open_menu(&mut self) {
        click(&mut self.rt, "new-session-folder-chip");
        self.wait("the locations and defaults", |s| !s.feeds.roots.is_empty() && s.feeds.defaults.is_some());
    }

    pub(crate) fn open_box(&mut self) {
        self.open_menu();
        click(&mut self.rt, "new-session-folder-path");
        // The pointer was on the row that opened the box: rows that arrive under it would take the highlight.
        self.rt.pointer_move(0.0, 0.0);
        self.wait("the first listing", |s| s.current_listing().is_some());
        self.rt.update(|s, _| s.folder_menu.path_highlight = 0);
    }

    pub(crate) fn type_text(&mut self, words: &str) {
        self.rt.frame(FRAME);
        self.rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    pub(crate) fn type_in_box(&mut self, words: &str) {
        self.type_text(words);
        self.wait("the listing of what was typed", |s| s.current_listing().is_some());
    }

    pub(crate) fn press(&mut self, key: Key) {
        self.rt.frame(FRAME);
        self.rt.key(KeyEvent { key, mods: Mods::default() });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    pub(crate) fn set_path(&mut self, text: &str) {
        let text = text.to_string();
        self.rt.update(move |s, _| {
            s.folder_menu.path.replace_text(text);
            s.path_changed();
        });
        self.wait("the listing", |s| s.current_listing().is_some());
    }

    pub(crate) fn asked(&self) -> Vec<String> {
        self.cfg.lock().unwrap().seen.clone()
    }

    pub(crate) fn last_listing_request(&self) -> String {
        let seen = self.asked();
        seen.iter().rev().find_map(|l| l.strip_prefix("GET /api/directories ").map(str::to_string)).unwrap_or_default()
    }

    pub(crate) fn created(&self) -> Vec<Value> {
        self.asked().iter().filter_map(|l| l.strip_prefix("POST /api/directories ")).filter_map(|b| serde_json::from_str(b).ok()).collect()
    }

    pub(crate) fn row_titles(&self) -> Vec<String> {
        self.state()
            .path_rows()
            .into_iter()
            .map(|r| format!("{} {}", r.title, r.title_name.unwrap_or_default()).trim().to_string())
            .collect()
    }

    pub(crate) fn chip(&self) -> String {
        let folder = self.state().new_draft.draft().and_then(|d| d.folder.clone());
        self.state().folder_chip_label(folder.as_ref())
    }

    pub(crate) fn folder(&self) -> Option<Folder> {
        self.state().new_draft.draft().and_then(|d| d.folder.clone())
    }

    /// Search the list view: opens the menu and types.
    pub(crate) fn search(&mut self, words: &str) {
        self.open_menu();
        self.type_text(words);
    }
}

// ─── the folder box ─────────────────────────────────────────────────────────

#[test]
fn the_folder_box_starts_in_the_location_and_lists_whats_there() {
    let mut r = rig();
    r.open_box();
    assert_eq!(r.state().folder_menu.path.text, "~/src/");
    assert_eq!(r.last_listing_request(), "~/src");
    let rows = r.state().path_rows();
    assert_eq!((rows[0].title.as_str(), rows[0].title_name.as_deref()), ("Open", Some("src")));
    assert_eq!(rows[1..].iter().map(|r| r.title.as_str()).collect::<Vec<_>>(), ["clients", "comet", "rocket"]);
    assert_eq!(rows[2].detail, "Repository");
    assert_eq!(rows[1].detail, "Folder");
}

#[test]
fn the_folder_box_writes_tilde_only_for_the_machines_own_home_folder_which_the_server_expands_the_same_way() {
    let mut r = rig_with(|c| c.home = "/home/someone-else".into());
    r.open_box();
    assert_eq!(r.state().folder_menu.path.text, "/home/me/src/");
    assert_eq!(r.last_listing_request(), "/home/me/src");
}

#[test]
fn the_folder_box_puts_whats_there_before_create_so_tab_and_enter_finish_a_name_instead_of_making_it() {
    let mut r = rig();
    r.open_box();
    r.type_text("cl");
    let rows = r.state().path_rows();
    assert_eq!(rows[0].title, "clients");
    assert_eq!(rows.last().map(|x| (x.action, x.title_name.clone())), Some((PathAction::Create, Some("cl".into()))));
    r.press(Key::Tab);
    r.wait("what is in clients", |s| s.current_listing().is_some_and(|l| l.entries.iter().any(|e| e.name == "northwind-api")));
    assert_eq!(r.state().folder_menu.path.text, "~/src/clients/");
    let titles = r.row_titles();
    assert_eq!(titles[0], "Open clients");
    assert_eq!(titles[1], "northwind-api");
}

#[test]
fn the_folder_box_creates_a_folder_inside_a_folder_typed_with_backslash_or_slash_on_the_branch_asked_for() {
    let mut r = rig_with(|c| c.create.1 = json!({ "path": "/home/me/src/clients/acme-portal", "isGitRepo": true, "addedToFleet": false }));
    r.open_box();
    r.type_in_box("clients\\acme-portal");
    assert_eq!(r.last_listing_request(), "~/src/clients");
    let create = r.state().path_rows().pop().unwrap();
    assert_eq!((create.title.as_str(), create.title_name.as_deref()), ("Create", Some("acme-portal")));
    assert_eq!(create.detail, "in ~/src/clients");
    assert!(r.rt.find("new-session-path-preview").is_some());
    assert_eq!(r.state().path_create().unwrap().folders, ["acme-portal"]);
    r.rt.update(|s, _| s.folder_menu.branch = TextEdit::new("trunk"));
    r.press(Key::Enter);
    r.wait("the create", |s| s.folder_menu.make == Make::Idle && !s.folder_menu_open());
    assert_eq!(r.created(), [json!({ "path": "/home/me/src/clients/acme-portal", "git": true, "branch": "trunk" })]);
    assert!(r.chip().contains("acme-portal"));
    assert!(r.state().folder_is_new(r.folder().as_ref()));
}

#[test]
fn the_folder_box_makes_missing_folders_above_it_too_and_can_make_an_empty_folder() {
    let mut r = rig_with(|c| c.create.1 = json!({ "path": "/home/me/src/clients/acme/site", "isGitRepo": false, "addedToFleet": false }));
    r.open_box();
    r.type_in_box("clients/acme/site");
    let create = r.state().path_rows().pop().unwrap();
    assert_eq!(create.title_name.as_deref(), Some("acme/site"));
    assert_eq!(r.state().path_create().unwrap().folders, ["acme", "site"]);
    r.rt.frame(FRAME);
    click(&mut r.rt, "new-session-path-kind-empty");
    click(&mut r.rt, "new-session-path-create-submit");
    r.wait("the create", |s| s.folder_menu.make == Make::Idle && !s.folder_menu_open());
    assert_eq!(r.created(), [json!({ "path": "/home/me/src/clients/acme/site", "git": false, "branch": null })]);
    assert_eq!(r.chip(), "~/src/clients/acme/site");
}

#[test]
fn the_folder_box_says_which_character_a_folder_name_cant_have_and_offers_no_create() {
    let mut r = rig();
    r.open_box();
    r.type_in_box("notes?");
    assert_eq!(r.state().path_note().as_deref(), Some("“?” can't be in a folder name."));
    assert!(r.state().path_rows().iter().all(|x| x.action != PathAction::Create));
    r.rt.frame(FRAME);
    assert!(r.rt.find("new-session-path-note").is_some());
    assert!(r.rt.find("new-session-path-create").is_none());
}

#[test]
fn the_folder_box_uses_a_git_folder_as_a_repository_so_it_can_get_a_new_worktree() {
    let mut r = rig_with(|c| {
        c.inspect = json!({ "path": "/home/me/deep/agent-playbook", "exists": true, "isGitRepo": true, "isWithinRoots": true })
    });
    r.open_box();
    r.set_path("/home/me/deep/agent-playbook");
    r.press(Key::Enter);
    r.wait("the folder is chosen", |s| !s.folder_menu_open());
    assert_eq!(r.folder(), Some(Folder::Repository("/home/me/deep/agent-playbook".into())));
    assert!(r.chip().contains("agent-playbook"));
    assert!(!r.asked().iter().any(|l| l.starts_with("POST /api/workspace-roots")), "it was in the locations already");
}

#[test]
fn the_folder_box_says_a_folder_outside_the_locations_is_added_to_them_and_adds_it_when_its_opened() {
    let mut r =
        rig_with(|c| c.inspect = json!({ "path": "/opt/agent-playbook", "exists": true, "isGitRepo": true, "isWithinRoots": false }));
    r.open_box();
    r.set_path("/opt/agent-playbook");
    assert!(
        r.state().path_adds_location(),
        "{:?} {:?} {:?}",
        (r.state().path_rows(), r.state().folder_menu.path_highlight),
        r.state().feeds.roots,
        r.state().current_listing()
    );
    r.rt.frame(FRAME);
    assert!(r.rt.find("new-session-path-adds-location").is_some());
    assert!(!r.asked().iter().any(|l| l.starts_with("POST /api/workspace-roots")), "nothing is added by looking");
    r.press(Key::Enter);
    r.wait("the folder is chosen", |s| !s.folder_menu_open());
    assert!(r.asked().contains(&r#"POST /api/workspace-roots  {"path":"/opt/agent-playbook"}"#.to_string()), "{:?}", r.asked());
    let cfg = r.cfg.clone();
    r.wait("the scan again", move |_| cfg.lock().unwrap().seen.iter().any(|l| l.starts_with("POST /api/repositories/refresh")));
    assert!(r.asked().iter().any(|l| l.starts_with("POST /api/repositories/refresh")));
    assert!(r.chip().contains("agent-playbook"));
}

#[test]
fn the_folder_box_goes_into_a_folder_and_up_one_and_up_from_the_root_stays_there() {
    let mut r = rig();
    r.open_box();
    click(&mut r.rt, "new-session-path-up");
    r.wait("the folder above", |s| s.current_listing().is_some_and(|l| l.path == "/home/me"));
    assert_eq!(r.state().folder_menu.path.text, "~/");
    r.set_path("/");
    click(&mut r.rt, "new-session-path-up");
    assert_eq!(r.state().folder_menu.path.text, "/", "a root has no parent");
}

#[test]
fn the_folder_box_arrow_keys_wrap_and_back_returns_to_the_list() {
    let mut r = rig();
    r.open_box();
    let n = r.state().path_rows().len();
    r.press(Key::Up);
    assert_eq!(r.state().folder_menu.path_highlight, n - 1);
    r.press(Key::Down);
    assert_eq!(r.state().folder_menu.path_highlight, 0);
    click(&mut r.rt, "new-session-folder-back");
    assert!(r.rt.find("new-session-folder-search").is_some());
}

#[test]
fn the_folder_box_says_when_a_folder_cant_be_listed_until_a_listing_works() {
    let mut r = rig();
    r.open_box();
    r.rt.update(|s, _| s.folder_menu.listing_error = Some("Couldn't list that folder.".into()));
    assert_eq!(r.state().path_note().as_deref(), Some("Couldn't list that folder."));
}

// ─── making a folder ────────────────────────────────────────────────────────

#[test]
fn making_a_folder_a_name_no_repository_has_offers_to_create_it_and_enter_does() {
    let mut r = rig();
    r.search("recipe box");
    let rows = r.state().folder_rows();
    assert_eq!(rows[0].id, "create");
    assert_eq!(rows[0].title_name.as_deref(), Some("recipe-box"));
    assert_eq!(rows[0].detail, "~/src/recipe-box · git repository on main");
    assert_eq!(r.state().folder_menu.highlight, 0);
    r.press(Key::Enter);
    r.wait("the create", |s| !s.folder_menu_open());
    assert_eq!(r.created(), [json!({ "path": "/home/me/src/recipe-box", "git": true, "branch": null })]);
    assert!(r.chip().contains("recipe-box"));
    assert!(r.state().folder_is_new(r.folder().as_ref()));
    assert!(matches!(r.folder(), Some(Folder::Repository(_))));
}

#[test]
fn making_a_folder_a_path_makes_folders_inside_folders_in_the_location_typed_with_slash_or_backslash() {
    let mut r = rig();
    r.search("clients\\acme portal");
    let create = r.state().folder_rows().into_iter().find(|x| x.id == "create").unwrap();
    assert_eq!(create.title_name.as_deref(), Some("clients/acme-portal"));
    assert!(create.detail.contains("~/src/clients/acme-portal"));
    r.press(Key::Enter);
    r.wait("the create", |s| !s.folder_menu_open());
    assert_eq!(r.created()[0]["path"], "/home/me/src/clients/acme-portal");
}

#[test]
fn making_a_folder_says_which_character_a_folder_name_cant_have_instead_of_dropping_it() {
    let mut r = rig();
    r.search("notes: draft");
    r.rt.frame(FRAME);
    assert_eq!(r.state().folder_invalid_char(), Some(':'));
    assert!(r.rt.find("new-session-folder-invalid").is_some());
    assert!(r.rt.find("new-session-folder-create").is_none());
}

#[test]
fn making_a_folder_a_path_of_its_own_goes_to_the_folder_box() {
    let mut r = rig();
    r.search("/srv/");
    r.rt.frame(FRAME);
    click(&mut r.rt, "new-session-folder-go");
    r.wait("the listing", |s| s.current_listing().is_some());
    assert_eq!(r.state().folder_menu.path.text, "/srv/");
    assert_eq!(r.last_listing_request(), "/srv");
}

#[test]
fn making_a_folder_never_offers_to_create_a_repository_thats_already_there() {
    let mut r = rig();
    r.search("Rocket");
    r.rt.frame(FRAME);
    assert!(r.rt.find("new-session-folder-create").is_none());
    assert!(r.state().folder_rows().iter().any(|x| x.title == "rocket"));
}

#[test]
fn making_a_folder_puts_new_folders_in_the_root_used_last_and_remembers_a_different_pick() {
    let mut r = rig_with(|c| c.roots = vec!["/home/me/src".into(), "/home/me/work".into()]);
    r.rt.update(|s, _| s.sidebar.update_new_session_defaults(HOME_MACHINE_KEY, |d| d.remember_new_folder_root("/home/me/work")));
    r.search("invoices");
    let create = r.state().folder_rows().into_iter().find(|x| x.id == "create").unwrap();
    assert!(create.detail.contains("~/work/invoices"), "{}", create.detail);
    r.rt.update(|s, _| s.pick_folder_root("/home/me/src".into()));
    r.rt.frame(FRAME);
    click(&mut r.rt, "new-session-folder-create");
    r.wait("the create", |s| !s.folder_menu_open());
    assert_eq!(r.created()[0]["path"], "/home/me/src/invoices");
    let remembered = r.state().sidebar.new_session_defaults(HOME_MACHINE_KEY).last_new_folder_root().map(str::to_string);
    assert_eq!(remembered.as_deref(), Some("/home/me/src"));
}

#[test]
fn making_a_folder_the_first_time_a_new_folder_goes_in_the_root_of_the_folder_in_use() {
    let mut r = rig_with(|c| c.roots = vec!["/home/me/src".into(), "/home/me/work".into()]);
    r.rt.update(|s, _| s.set_folder(Folder::Repository("/home/me/work/billing".into()), true));
    r.search("invoices");
    let create = r.state().folder_rows().into_iter().find(|x| x.id == "create").unwrap();
    assert!(create.detail.contains("~/work/invoices"), "{}", create.detail);
}

#[test]
fn making_a_folder_an_existing_folder_says_so_and_can_be_used_instead() {
    let mut r = rig_with(|c| c.create = (409, json!({ "error": "/home/me/src/notes already exists." })));
    r.search("notes");
    click(&mut r.rt, "new-session-folder-create");
    r.wait("the refusal", |s| s.folder_menu.error.is_some());
    assert_eq!(r.state().folder_menu.error.as_deref(), Some("/home/me/src/notes already exists."));
    r.rt.frame(FRAME);
    click(&mut r.rt, "new-session-folder-use-existing");
    r.wait("the folder is chosen", |s| !s.folder_menu_open());
    assert_eq!(r.folder(), Some(Folder::Directory("/home/me/src/notes".into())));
    assert_eq!(r.chip(), "~/src/notes");
}

#[test]
fn making_a_folder_says_so_when_the_machine_could_not_make_it() {
    let mut r = rig_with(|c| c.create = (500, json!({ "error": "Permission denied." })));
    r.search("notes2");
    r.press(Key::Enter);
    r.wait("the refusal", |s| s.folder_menu.error.is_some());
    assert_eq!(r.state().folder_menu.error.as_deref(), Some("Permission denied."));
    assert!(r.state().folder_menu.existing.is_none(), "only a 409 offers the folder that is there");
    assert!(r.state().folder_menu_open());
}

#[test]
fn making_a_folder_keeps_the_menu_open_to_say_the_first_commit_couldnt_be_made() {
    let warning = "Git couldn't make the first commit: empty ident name not allowed";
    let mut r = rig_with(|c| {
        c.create.1 = json!({ "path": "/home/me/src/recipe-box", "isGitRepo": true, "addedToFleet": false, "warning": warning })
    });
    r.search("recipe-box");
    r.press(Key::Enter);
    r.wait("the warning", |s| s.folder_menu.warning.is_some());
    r.rt.frame(FRAME);
    assert!(r.state().folder_menu_open());
    assert!(r.rt.find("new-session-folder-warning").is_some());
    assert_eq!(r.state().folder_menu.warning.as_ref().map(|w| w.text.as_str()), Some(warning));
    assert!(r.chip().contains("recipe-box"));
    click(&mut r.rt, "new-session-folder-warning-ok");
    assert!(!r.state().folder_menu_open());
}

#[test]
fn making_a_folder_one_create_at_a_time() {
    let mut r = rig();
    r.search("recipe-box");
    r.rt.update(|s, _| s.folder_menu.make = Make::Creating);
    r.press(Key::Enter);
    assert!(r.created().is_empty(), "a create that runs blocks the next");
}
