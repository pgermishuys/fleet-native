//! Component 16, piece AA: Settings → Folders → Locations (`WorkspaceSection.vue`). Fleet has no test for it, so these are
//! native: the list and its chips, the empty and error states, Add and Remove in the order Fleet runs them, the Browse popover,
//! and that every call goes to the machine Settings shows.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::locations::LocationsPage;
use crate::tests::{FRAME, app, click};
use crate::{Rail, State, test_wait};
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fake {
    /// Every call heard: method, path and body.
    heard: Vec<(String, String, String)>,
    roots: Vec<Value>,
    /// What the list call answers instead of the roots.
    broken: Option<(u16, String)>,
    /// What an add or a remove answers instead of success.
    refuse: Option<(u16, String)>,
    machines: Vec<Value>,
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
    let body_at = loop {
        let n = stream.read(&mut chunk).unwrap_or(0);
        buf.extend_from_slice(&chunk[..n]);
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break at + 4;
        }
        if n == 0 {
            break buf.len();
        }
    };
    let head = String::from_utf8_lossy(&buf[..body_at]).to_string();
    let length = head
        .lines()
        .find_map(|l| l.to_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok()))
        .unwrap_or(0);
    while buf.len() < body_at + length {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = String::from_utf8_lossy(&buf[body_at..]).to_string();
    let mut first = head.lines().next().unwrap_or("").split(' ');
    let (method, path) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
    let mut f = fake.lock().unwrap();
    f.heard.push((method.clone(), path.clone(), body.clone()));
    let (status, reply) = match (method.as_str(), path.as_str()) {
        ("GET", "/api/workspace-roots") => match f.broken.clone() {
            Some((code, text)) => (code, text),
            None => (200, json!({ "roots": f.roots }).to_string()),
        },
        ("POST", "/api/workspace-roots") | ("DELETE", _) if f.refuse.is_some() && path.starts_with("/api/workspace-roots") => {
            f.refuse.clone().unwrap()
        }
        ("POST", "/api/workspace-roots") => {
            let path = serde_json::from_str::<Value>(&body).ok().and_then(|v| v["path"].as_str().map(str::to_string)).unwrap_or_default();
            let id = format!("loc-{}", f.roots.len() + 1);
            f.roots.push(json!({ "id": id, "path": path, "source": "user", "exists": true }));
            (200, "{}".to_string())
        }
        ("DELETE", p) if p.starts_with("/api/workspace-roots/") => {
            let id = p.trim_start_matches("/api/workspace-roots/").to_string();
            f.roots.retain(|r| r["id"] != id);
            (204, String::new())
        }
        ("GET", "/api/machines") => (200, json!({ "machines": f.machines }).to_string()),
        ("GET", p) if p.starts_with("/api/directories") => {
            let at = p.split("path=").nth(1).map(|s| s.split('&').next().unwrap_or("").replace("%2F", "/"));
            let entry = |parent: &str, name: &str, git: bool| json!({ "name": name, "path": format!("{parent}/{name}"), "isGitRepo": git });
            let reply = match at.as_deref() {
                None => {
                    json!({ "currentPath": null, "roots": ["/work"], "entries": [{ "name": "work", "path": "/work", "isGitRepo": false }] })
                }
                Some("/work") => {
                    json!({ "currentPath": "/work", "roots": ["/work"], "entries": [entry("/work", "lantern", true), entry("/work", "notes", false)] })
                }
                Some(other) => json!({ "currentPath": other, "roots": ["/work"], "entries": [] }),
            };
            (200, reply.to_string())
        }
        _ => (200, "{}".to_string()),
    };
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

fn link_to(base: String) -> (Link, Receiver<Job>) {
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    (Link::start(base, Post::new(move |job| drop(jobs.lock().unwrap().send(job)))), queue)
}

fn root(id: Option<&str>, path: &str, source: &str, exists: bool) -> Value {
    json!({ "id": id, "path": path, "source": source, "exists": exists })
}

/// A connected app on Settings → Folders, the list not yet read.
fn rig(setup: impl FnOnce(&mut Fake)) -> Rig {
    let fake: Shared = Default::default();
    setup(&mut fake.lock().unwrap());
    let (link, queue) = link_to(serve(fake.clone()));
    let mut rt = app();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.rail = Rail::Settings;
        s.open_settings("folders", None);
    });
    Rig { rt, queue, fake }
}

fn three(f: &mut Fake) {
    f.roots = vec![
        root(None, "/work", "env", true),
        root(Some("loc-1"), "/srv/projects", "user", true),
        root(Some("loc-2"), "/srv/gone", "user", false),
    ];
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("{:?}", s.settings.locations));
    }
    fn read(&mut self) {
        self.wait("the locations to be read", |s| !s.settings.locations.loading && s.settings.locations.loaded_for.is_some());
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }
    fn settle(&mut self) {
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }
    fn exists(&self, key: &str) -> bool {
        self.rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }
    fn text(&self, key: &str) -> String {
        let all = self.rt.elements();
        let it = all.iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element keyed {key:?}"));
        all.iter().filter(|e| e.path.starts_with(&it.path)).filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("")
    }
    /// Calls that change something. A press that "sends nothing" is about these: a read the page makes on its own schedule
    /// can still arrive after `read()` returns, and counting it made those tests flaky.
    fn writes(&self) -> usize {
        self.heard().iter().filter(|(m, _)| m != "GET").count()
    }
    fn heard(&self) -> Vec<(String, String)> {
        // The Worktrees and Storage cards share the page and read on their own schedule; these tests are about Locations.
        let other = |p: &str| p.starts_with("/api/storage/") || p.starts_with("/api/worktrees/");
        self.fake.lock().unwrap().heard.iter().filter(|(_, p, _)| !other(p)).map(|(m, p, _)| (m.clone(), p.clone())).collect()
    }
    fn key(&mut self, key: Key) {
        self.rt.key(KeyEvent { key, mods: Mods::default() });
        self.settle();
    }
    fn type_path(&mut self, words: &str) {
        click(&mut self.rt, "locations-path");
        self.key(Key::Text(words.into()));
    }
}

fn call(method: &str, path: &str) -> (String, String) {
    (method.to_string(), path.to_string())
}

// ─── The list ───────────────────────────────────────────────────────────────────

#[test]
fn says_loading_then_lists_each_location_with_where_it_came_from() {
    let mut r = rig(three);
    r.rt.frame(FRAME);
    assert!(r.exists("locations-loading") || !r.rt.state.settings.locations.loading, "loading shows while the read is on its way");
    r.read();
    assert!(!r.exists("locations-loading"));
    assert_eq!(r.text("location-path-0"), "/work");
    assert_eq!(r.text("location-source-0"), "From the environment");
    assert_eq!(r.text("location-source-1"), "Added");
    assert_eq!(r.text("location-path-2"), "/srv/gone");
}

#[test]
fn an_environment_location_has_no_remove_button_and_an_added_one_has() {
    let mut r = rig(three);
    r.read();
    assert!(!r.exists("location-remove-0"));
    assert!(r.exists("location-remove-1") && r.exists("location-remove-2"));
}

#[test]
fn a_location_gone_from_disk_says_so_and_one_that_is_there_does_not() {
    let mut r = rig(three);
    r.read();
    assert_eq!(r.text("location-missing-2"), "Missing on disk");
    assert!(!r.exists("location-missing-0") && !r.exists("location-missing-1"));
}

#[test]
fn says_no_locations_yet_for_an_empty_list() {
    let mut r = rig(|_| {});
    r.read();
    assert!(r.text("locations-empty").starts_with("No locations yet"));
    assert!(r.text("locations-empty").contains("Fleet lists the repositories inside it."));
    assert!(!r.exists("locations-list"));
}

#[test]
fn shows_the_servers_words_when_the_list_cant_be_read_and_no_list() {
    let mut r = rig(|f| {
        three(f);
        f.broken = Some((500, json!({ "error": "The roots file is locked." }).to_string()));
    });
    r.read();
    assert_eq!(r.text("locations-error"), "The roots file is locked.");
    assert!(!r.exists("locations-list") && !r.exists("locations-empty"));
    assert!(r.exists("locations-add-card"), "the add box stays");
}

#[test]
fn says_it_couldnt_load_when_the_server_gives_no_reason() {
    let mut r = rig(|f| f.broken = Some((500, "boom".into())));
    r.read();
    assert_eq!(r.text("locations-error"), "Couldn't load your locations.");
}

#[test]
fn a_path_longer_than_its_card_is_cut_with_an_ellipsis() {
    let long = format!("/work/{}", "deeply-nested-folder/".repeat(12));
    let mut r = rig(|f| f.roots = vec![root(Some("loc-1"), &long, "user", true)]);
    r.read();
    let shown = r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("location-path-0")).expect("the path");
    assert!(shown.rect[2] < 760.0, "the path stays inside the card, not as wide as its text");
}

// ─── Refresh ────────────────────────────────────────────────────────────────────

#[test]
fn refresh_reads_the_list_again_and_does_not_scan() {
    let mut r = rig(three);
    r.read();
    click(&mut r.rt, "locations-refresh");
    r.wait("the list to come back", |s| !s.settings.locations.refreshing && !s.settings.locations.loading);
    let heard = r.heard();
    assert_eq!(heard.iter().filter(|c| **c == call("GET", "/api/workspace-roots")).count(), 2);
    assert!(!heard.contains(&call("POST", "/api/repositories/refresh")));
}

#[test]
fn refresh_is_off_while_it_reads() {
    let mut r = rig(three);
    r.rt.frame(FRAME);
    r.rt.update(|s, _| s.settings.locations.loading = true);
    r.settle();
    r.rt.update(|s, _| s.refresh_locations());
    assert!(!r.rt.state.settings.locations.refreshing, "a read under way is not started twice");
}

// ─── Add ────────────────────────────────────────────────────────────────────────

#[test]
fn add_sends_the_trimmed_path_clears_the_box_reads_the_list_and_scans() {
    let mut r = rig(|_| {});
    r.read();
    r.type_path("  /srv/new-place  ");
    click(&mut r.rt, "locations-add");
    r.wait("the add to finish", |s| !s.settings.locations.adding && s.settings.locations.roots.len() == 1);
    r.settle();
    let heard = r.heard();
    let at = heard.iter().position(|c| *c == call("POST", "/api/workspace-roots")).expect("the add");
    assert_eq!(&heard[at + 1..], &[call("GET", "/api/workspace-roots"), call("POST", "/api/repositories/refresh")], "list, then scan");
    let body =
        r.fake.lock().unwrap().heard.iter().find(|(m, p, _)| m == "POST" && p == "/api/workspace-roots").map(|c| c.2.clone()).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap(), json!({ "path": "/srv/new-place" }));
    assert_eq!(r.rt.state.settings.locations.path.text, "");
    assert_eq!(r.text("location-path-0"), "/srv/new-place");
}

#[test]
fn add_with_an_empty_box_says_so_and_sends_nothing() {
    let mut r = rig(|_| {});
    r.read();
    let before = r.writes();
    click(&mut r.rt, "locations-add");
    assert_eq!(r.text("locations-add-error"), "Enter the folder to add.");
    r.type_path("   ");
    click(&mut r.rt, "locations-add");
    assert_eq!(r.text("locations-add-error"), "Enter the folder to add.");
    assert_eq!(r.writes(), before);
}

#[test]
fn enter_in_the_box_adds_the_location() {
    let mut r = rig(|_| {});
    r.read();
    r.type_path("/srv/by-enter");
    r.key(Key::Enter);
    r.wait("the add to finish", |s| !s.settings.locations.adding && s.settings.locations.roots.len() == 1);
    assert!(r.heard().contains(&call("POST", "/api/workspace-roots")));
}

#[test]
fn a_refused_add_shows_the_servers_words_and_keeps_the_box() {
    let mut r = rig(|f| f.refuse = Some((400, json!({ "error": "That folder doesn't exist." }).to_string())));
    r.read();
    r.type_path("/nowhere");
    click(&mut r.rt, "locations-add");
    r.wait("the refusal", |s| s.settings.locations.add_error.is_some());
    r.settle();
    assert_eq!(r.text("locations-add-error"), "That folder doesn't exist.");
    assert_eq!(r.rt.state.settings.locations.path.text, "/nowhere");
    assert!(r.exists("locations-empty"), "the list is as it was");
}

#[test]
fn an_add_the_server_refuses_without_a_reason_says_it_couldnt() {
    let mut r = rig(|f| f.refuse = Some((500, "boom".into())));
    r.read();
    r.type_path("/nowhere");
    click(&mut r.rt, "locations-add");
    r.wait("the refusal", |s| s.settings.locations.add_error.is_some());
    assert_eq!(r.rt.state.settings.locations.add_error.as_deref(), Some("Couldn't add that location."));
}

#[test]
fn the_add_button_says_adding_and_is_off_while_it_works() {
    let mut r = rig(|_| {});
    r.read();
    r.rt.update(|s, _| s.settings.locations.adding = true);
    r.settle();
    assert_eq!(r.text("locations-add"), "Adding…");
    let before = r.writes();
    click(&mut r.rt, "locations-add");
    assert_eq!(r.writes(), before, "a press while adding sends nothing");
}

// ─── Remove ─────────────────────────────────────────────────────────────────────

#[test]
fn remove_deletes_then_reads_the_list_then_scans() {
    let mut r = rig(three);
    r.read();
    click(&mut r.rt, "location-remove-1");
    r.wait("the remove to finish", |s| s.settings.locations.removing.is_none() && s.settings.locations.roots.len() == 2);
    r.settle();
    let heard = r.heard();
    let at = heard.iter().position(|c| *c == call("DELETE", "/api/workspace-roots/loc-1")).expect("the delete");
    assert_eq!(&heard[at + 1..], &[call("GET", "/api/workspace-roots"), call("POST", "/api/repositories/refresh")]);
    assert_eq!(r.text("location-path-1"), "/srv/gone");
}

#[test]
fn a_refused_remove_puts_its_words_where_the_list_was() {
    let mut r = rig(|f| {
        three(f);
        f.refuse = Some((409, json!({ "error": "A session is using that location." }).to_string()));
    });
    r.read();
    click(&mut r.rt, "location-remove-1");
    r.wait("the refusal", |s| s.settings.locations.error.is_some());
    r.settle();
    assert_eq!(r.text("locations-error"), "A session is using that location.");
    assert!(!r.exists("locations-list"));
}

#[test]
fn a_remove_under_way_says_removing_on_its_own_row_only() {
    let mut r = rig(three);
    r.read();
    r.rt.update(|s, _| s.settings.locations.removing = Some("loc-1".into()));
    r.settle();
    assert_eq!(r.text("location-remove-1"), "Removing…");
    assert_eq!(r.text("location-remove-2"), "Remove");
}

// ─── Browse ─────────────────────────────────────────────────────────────────────

#[test]
fn browse_opens_on_the_roots_and_a_folder_opens_into_its_folders() {
    let mut r = rig(|_| {});
    r.read();
    click(&mut r.rt, "locations-browse");
    r.wait("the roots", |s| s.settings.locations.browse.as_ref().is_some_and(|b| !b.loading && !b.entries.is_empty()));
    r.settle();
    assert!(r.exists("locations-popover"));
    assert!(r.text("locations-popover").contains("Locations"));
    click(&mut r.rt, "locations-browse-entry-0");
    r.wait("the folder", |s| s.settings.locations.browse.as_ref().is_some_and(|b| !b.loading && b.current.as_deref() == Some("/work")));
    r.settle();
    assert!(r.text("locations-browse-entry-0").starts_with("lantern"));
    assert!(r.exists("locations-browse-crumb-0") && r.exists("locations-browse-roots"));
    assert!(r.heard().contains(&call("GET", "/api/directories?unconstrained=true")), "the roots are listed with no path");
    assert!(r.heard().contains(&call("GET", "/api/directories?path=%2Fwork&unconstrained=true")), "any folder, not only the locations");
}

#[test]
fn use_this_directory_puts_the_folder_in_the_box_and_closes_the_popover() {
    let mut r = rig(|_| {});
    r.read();
    click(&mut r.rt, "locations-browse");
    r.wait("the roots", |s| s.settings.locations.browse.as_ref().is_some_and(|b| !b.loading && !b.entries.is_empty()));
    r.settle();
    assert!(!r.exists("locations-browse-use"), "nothing to use at the roots");
    click(&mut r.rt, "locations-browse-entry-0");
    r.wait("the folder", |s| s.settings.locations.browse.as_ref().is_some_and(|b| !b.loading && b.current.as_deref() == Some("/work")));
    r.settle();
    click(&mut r.rt, "locations-browse-use");
    assert_eq!(r.rt.state.settings.locations.path.text, "/work");
    assert!(!r.exists("locations-popover"));
}

#[test]
fn select_on_a_row_picks_that_folder_without_going_into_it() {
    let mut r = rig(|_| {});
    r.read();
    click(&mut r.rt, "locations-browse");
    r.wait("the roots", |s| s.settings.locations.browse.as_ref().is_some_and(|b| !b.loading && !b.entries.is_empty()));
    r.settle();
    click(&mut r.rt, "locations-browse-select-0");
    assert_eq!(r.rt.state.settings.locations.path.text, "/work");
    assert!(!r.exists("locations-popover"));
}

#[test]
fn the_search_box_narrows_the_folders_shown() {
    let mut r = rig(|_| {});
    r.read();
    click(&mut r.rt, "locations-browse");
    r.wait("the roots", |s| s.settings.locations.browse.as_ref().is_some_and(|b| !b.loading && !b.entries.is_empty()));
    click(&mut r.rt, "locations-browse-entry-0");
    r.wait("the folder", |s| s.settings.locations.browse.as_ref().is_some_and(|b| !b.loading && b.entries.len() == 2));
    r.settle();
    click(&mut r.rt, "locations-browse-search");
    r.key(Key::Text("lan".into()));
    assert!(r.exists("locations-browse-entry-0") && !r.exists("locations-browse-entry-1"));
    assert!(r.text("locations-browse-entry-0").starts_with("lantern"));
}

// ─── The machine and the page ───────────────────────────────────────────────────

#[test]
fn asks_the_machine_settings_shows_never_home() {
    let home: Shared = Default::default();
    let other: Shared = Default::default();
    other.lock().unwrap().roots = vec![root(Some("loc-9"), "/srv/harbor-code", "user", true)];
    let other_base = serve(other.clone());
    home.lock().unwrap().machines = vec![json!({ "id": "m-harbor", "name": "harbor", "baseUrl": other_base, "token": "t-harbor" })];
    let (link, queue) = link_to(serve(home.clone()));
    let mut rt = app();
    rt.update(move |s, _| {
        s.show_settings_machines(true);
        s.link = Some(link);
        let machine = s.machines.list.iter_mut().find(|m| m.id == "m-harbor").expect("harbor");
        machine.base_url = other_base;
        machine.addresses.clear();
        s.rail = Rail::Settings;
        s.open_settings("folders", None);
        s.settings.choice.pick("m-harbor");
    });
    let mut r = Rig { rt, queue, fake: home.clone() };
    r.wait("harbor's locations", |s| s.settings.locations.roots.len() == 1);
    assert!(other.lock().unwrap().heard.iter().any(|(_, p, _)| p == "/api/workspace-roots"), "harbor was asked");
    assert!(home.lock().unwrap().heard.iter().all(|(_, p, _)| !p.starts_with("/api/workspace-roots")), "home was not");
    r.settle();
    r.type_path("/srv/more");
    click(&mut r.rt, "locations-add");
    r.wait("harbor's add", |s| !s.settings.locations.adding && s.settings.locations.roots.len() == 2);
    assert!(
        home.lock()
            .unwrap()
            .heard
            .iter()
            .all(|(m, p, _)| !(m == "POST" && (p.starts_with("/api/workspace-roots") || p.starts_with("/api/repositories")))),
        "home heard no write"
    );
}

#[test]
fn reads_again_when_the_page_is_left_and_come_back_to() {
    let mut r = rig(three);
    r.read();
    let before = r.heard().iter().filter(|c| **c == call("GET", "/api/workspace-roots")).count();
    r.rt.update(|s, _| s.open_settings("appearance", None));
    r.rt.frame(FRAME);
    assert_eq!(r.rt.state.settings.locations, LocationsPage::default());
    r.rt.update(|s, _| s.open_settings("folders", None));
    r.read();
    assert_eq!(r.heard().iter().filter(|c| **c == call("GET", "/api/workspace-roots")).count(), before + 1);
}

#[test]
fn with_no_fleet_the_page_works_on_the_invented_list() {
    let mut rt = app();
    rt.update(|s, _| {
        s.rail = Rail::Settings;
        s.open_settings("folders", None);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.settings.locations.roots.len(), 3);
    assert!(rt.find("location-remove-1").is_some());
    click(&mut rt, "location-remove-1");
    assert_eq!(rt.state.settings.locations.roots.len(), 2);
}

#[test]
fn the_page_names_the_pieces_it_is_built_from() {
    let mut r = rig(three);
    r.read();
    assert!(r.exists("settings-header") && r.exists("locations-card"));
    assert!(r.text("locations-card").contains("up to three folders deep"));
}
