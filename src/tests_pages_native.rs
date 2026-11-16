//! Piece P of the New session component: the Welcome and Repositories pages (`WelcomePage.vue`, `RepositoriesPage.vue`, rows C15, C16,
//! C17.17-C17.19). Fleet has no tests for either page; these are the cases its checklist suggests, against a fake machine that
//! records every request and answers the scan as the test says.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::repositories::Body;
use crate::fleet::store::Route;
use crate::tests::{FRAME, app, click};
use crate::{State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fake {
    /// What `GET /api/repositories` answers (status, body).
    scan: Option<(u16, Value)>,
    /// What `POST /api/repositories/refresh` answers.
    refresh: Option<(u16, Value)>,
    log: Vec<String>,
}

type Shared = Arc<Mutex<Fake>>;

fn serve(fake: Shared) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let fake = fake.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let mut first = head.lines().next().unwrap_or("").split(' ');
                let (method, path) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
                let mut f = fake.lock().unwrap();
                f.log.push(format!("{method} {path}"));
                let (status, reply) = match (method.as_str(), path.as_str()) {
                    ("GET", "/api/repositories") => f.scan.clone().unwrap_or((200, json!({ "repositories": [], "scannedAt": 0 }))),
                    ("POST", "/api/repositories/refresh") => f.refresh.clone().unwrap_or((200, json!({}))),
                    _ => (200, json!({})),
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

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Shared,
}

fn repo(name: &str, root: &str) -> Value {
    json!({ "name": name, "path": format!("{root}/{name}"), "parentRoot": root })
}

fn rig(scan: Value) -> Rig {
    let fake: Shared = Arc::new(Mutex::new(Fake { scan: Some((200, scan)), ..Fake::default() }));
    let base = serve(fake.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    let link = Link::start(base, post);
    rt.update(move |s, _| s.link = Some(link));
    Rig { rt, queue, fake }
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("page {:?}, route {:?}", s.repos_page, s.route));
    }

    fn asked(&self) -> Vec<String> {
        self.fake.lock().unwrap().log.iter().filter(|l| l.contains("/api/repositories")).cloned().collect()
    }

    fn open(&mut self) {
        self.rt.update(|s, _| s.open_repositories_page());
        self.rt.frame(FRAME);
        self.wait("the first read to end", |s| !s.repos_page.loading);
    }
}

fn two_roots() -> Value {
    json!({ "repositories": [repo("rocket", "/home/ada/src"), repo("comet", "/home/ada/src"), repo("Aurora-docs", "/home/ada/src")],
        "scannedAt": 1_763_000_000_000i64 })
}

#[test]
fn the_welcome_page_shows_its_words_and_asks_for_nothing() {
    let mut r = rig(two_roots());
    r.rt.update(|s, _| s.open_welcome_page());
    r.rt.frame(FRAME);
    for key in ["welcome-page", "welcome-logo", "welcome-title", "welcome-subtitle", "welcome-lede", "welcome-version"] {
        assert!(r.rt.find(key).is_some(), "{key} is drawn");
    }
    assert!(r.rt.find("repositories-refresh").is_none(), "no buttons: the page is static (C15.8)");
    assert_eq!(r.rt.state.route, Route::Welcome);
    assert!(r.asked().is_empty(), "no request");
    assert!(crate::view::pages::version_text().starts_with("v"), "{}", crate::view::pages::version_text());
}

#[test]
fn a_page_no_rail_owns_shows_no_list_panel_and_lights_no_rail_entry() {
    let mut r = rig(two_roots());
    assert!(r.rt.find("sidebar-gutter").is_some() && !r.rt.state.on_bare_page());
    for open in [|s: &mut State| s.open_welcome_page(), |s: &mut State| s.open_repositories_page()] {
        r.rt.update(move |s, _| open(s));
        r.rt.frame(FRAME);
        assert!(r.rt.state.on_bare_page());
        assert!(r.rt.find("sidebar-gutter").is_none(), "the session list belongs to Sessions only");
        assert!(r.rt.find("new-session").is_none());
        assert!(r.rt.find("rail-sessions").is_some(), "the rail stays");
    }
    // A rail entry takes the window back.
    click(&mut r.rt, "rail-sessions");
    assert!(!r.rt.state.on_bare_page());
    assert!(r.rt.find("sidebar-gutter").is_some());
}

#[test]
fn repositories_reads_the_scan_on_arrival_and_lists_it_by_name() {
    let mut r = rig(two_roots());
    r.rt.update(|s, _| s.open_repositories_page());
    assert_eq!(r.rt.state.repos_page.body(), Body::Loading, "the first read shows the spinner box");
    r.rt.frame(FRAME);
    assert!(r.rt.find("repositories-loading").is_some());
    r.wait("the first read to end", |s| !s.repos_page.loading);
    r.rt.frame(FRAME);
    assert_eq!(r.asked(), ["GET /api/repositories"]);
    let names: Vec<_> = r.rt.state.repos_page.sorted().into_iter().map(|x| x.name).collect();
    assert_eq!(names, ["Aurora-docs", "comet", "rocket"], "by name, not by path or by arrival; case doesn't split a name");
    for n in 0..3 {
        assert!(r.rt.find(&format!("repository-{n}")).is_some(), "card {n}");
    }
    assert!(r.rt.find("repository-3").is_none());
    assert!(r.rt.state.repositories_last_scan().starts_with("Last scan: "));
    assert!(!r.rt.state.repositories_last_scan().contains("Not scanned yet"));
}

#[test]
fn repositories_says_not_scanned_yet_until_a_scan_answers() {
    let page = crate::fleet::repositories::Page::default();
    assert_eq!(page.last_scan(0, 0), "Last scan: Not scanned yet");
    let mut page = crate::fleet::repositories::Page::default();
    page.scanned_at = Some(1_763_000_000_000);
    assert!(page.last_scan(1_763_000_000_000, 0).starts_with("Last scan: Nov 13, "), "{}", page.last_scan(1_763_000_000_000, 0));
    // A year other than this one is written out.
    assert!(page.last_scan(1_800_000_000_000, 0).starts_with("Last scan: Nov 13, 2025, "), "{}", page.last_scan(1_800_000_000_000, 0));
}

#[test]
fn repositories_with_none_says_so_and_points_to_settings() {
    let mut r = rig(json!({ "repositories": [], "scannedAt": 5 }));
    r.open();
    r.rt.frame(FRAME);
    assert_eq!(r.rt.state.repos_page.body(), Body::Empty);
    assert!(r.rt.find("repositories-empty").is_some());
    click(&mut r.rt, "repositories-open-settings");
    assert_eq!((r.rt.state.rail, r.rt.state.settings.page.clone()), (crate::Rail::Settings, crate::fleet::settings_nav::Page::Folders));
}

#[test]
fn refresh_scans_then_reads_and_keeps_the_list_meanwhile() {
    let mut r = rig(two_roots());
    r.open();
    r.rt.frame(FRAME);
    click(&mut r.rt, "repositories-refresh");
    assert!(r.rt.state.repos_page.refreshing);
    assert_eq!(r.rt.state.repos_page.body(), Body::List, "the list stays while the scan runs");
    r.wait("the refresh to end", |s| !s.repos_page.refreshing);
    assert_eq!(
        r.asked(),
        ["GET /api/repositories", "POST /api/repositories/refresh", "GET /api/repositories"],
        "the scan first, then the read"
    );
}

#[test]
fn a_failed_refresh_skips_the_read_and_clears_the_list() {
    let mut r = rig(two_roots());
    r.open();
    r.fake.lock().unwrap().refresh = Some((500, json!({ "error": "git is missing" })));
    r.rt.update(|s, _| s.read_repositories_page(true));
    r.wait("the refresh to end", |s| !s.repos_page.refreshing);
    r.rt.frame(FRAME);
    assert_eq!(r.asked(), ["GET /api/repositories", "POST /api/repositories/refresh"], "no read after a failed scan");
    let page = &r.rt.state.repos_page;
    assert_eq!(page.body(), Body::Error);
    assert!(page.list.is_empty() && page.scanned_at.is_none());
    assert!(page.error.as_deref().is_some_and(|e| e.contains("git is missing")), "{:?}", page.error);
    assert!(r.rt.find("repositories-error").is_some());
    assert_eq!(r.rt.state.repositories_last_scan(), "Last scan: Not scanned yet");
}

#[test]
fn a_failed_read_shows_the_reason_and_the_next_refresh_recovers() {
    let mut r = rig(two_roots());
    r.fake.lock().unwrap().scan = Some((500, json!({ "error": "scan failed" })));
    r.open();
    assert_eq!(r.rt.state.repos_page.body(), Body::Error);
    r.fake.lock().unwrap().scan = Some((200, two_roots()));
    r.rt.update(|s, _| s.read_repositories_page(true));
    r.wait("the refresh to end", |s| !s.repos_page.refreshing);
    assert_eq!(r.rt.state.repos_page.body(), Body::List, "the error is gone");
}

#[test]
fn refresh_is_off_while_a_read_runs() {
    let mut r = rig(two_roots());
    r.rt.update(|s, _| s.open_repositories_page());
    r.rt.frame(FRAME);
    // Pressing it now asks for nothing more: Fleet disables the button while `isLoading`.
    r.rt.update(|s, _| s.read_repositories_page(true));
    r.wait("the first read to end", |s| !s.repos_page.loading);
    assert_eq!(r.asked(), ["GET /api/repositories"]);
}

#[test]
fn with_no_server_the_page_draws_the_invented_machine() {
    let mut rt = app();
    rt.update(|s, _| s.open_repositories_page());
    rt.frame(FRAME);
    assert_eq!(rt.state.repos_page.body(), Body::List);
    assert_eq!(rt.state.repos_page.list.len(), 4);
}
