//! Piece C of component 21: the three stores the GitHub pages share (your work, the repositories you follow, the repository cache)
//! and the cache row on the connection page. Fleet has no tests for these, so they are native: a fake Fleet answers
//! `/api/integrations/github/*` and logs what it was asked; no real GitHub, account or token is reached. Every name is invented.
use super::{state, state_ref};
use crate::fleet::link::{Job, Link, Post};
use crate::tests::{FRAME, app, click};
use crate::{Rail, State, test_http, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fake {
    /// What a request answers, by `METHOD path` (query included); 200 `{}` when nothing is set.
    answers: HashMap<String, (u16, String)>,
    /// Every request to the GitHub routes: method, path with its query, body.
    log: Vec<(String, String, Value)>,
    /// The next request to one of these paths waits for a message first.
    gates: HashMap<String, Arc<Mutex<Receiver<()>>>>,
}

type Shared = Arc<Mutex<Fake>>;

fn fleet(fake: Shared) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let fake = fake.clone();
            std::thread::spawn(move || {
                let req = test_http::read(&mut stream);
                if req.method.is_empty() {
                    return;
                }
                if !req.path.starts_with("/api/integrations/github/") {
                    return test_http::respond_json(&mut stream, "200 OK", "{}");
                }
                let key = format!("{} {}", req.method, req.path);
                let (status, body, gate) = {
                    let mut f = fake.lock().unwrap();
                    f.log.push((req.method.clone(), req.path.clone(), req.json()));
                    let (status, body) = f.answers.get(&key).cloned().unwrap_or((200, "{}".into()));
                    (status, body, f.gates.remove(&req.path))
                };
                if let Some(gate) = gate {
                    let _ = gate.lock().unwrap().recv_timeout(test_wait::BOUND);
                }
                test_http::respond_json(&mut stream, format!("{status} X"), &body);
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

fn rig() -> Rig {
    let fake: Shared = Arc::default();
    let base = fleet(fake.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(
        base,
        Post::new(move |job| {
            let _ = jobs.lock().unwrap().send(job);
        }),
    );
    let mut rt = app();
    rt.update(move |s, _| s.link = Some(link));
    rt.frame(FRAME);
    Rig { rt, queue, fake }
}

const WORK: &str = "/api/integrations/github/work";
const MARKS: &str = "/api/integrations/github/bookmarks";

fn page(from: usize, n: usize) -> String {
    let repos: Vec<Value> = (from..from + n)
        .map(|i| json!({"id": i, "full_name": format!("acme/repo-{i:03}"), "name": format!("repo-{i:03}"), "owner": {"login": "acme"}, "private": i % 2 == 0, "language": "Rust", "stargazers_count": i}))
        .collect();
    Value::Array(repos).to_string()
}

fn repo_path(page: usize) -> String {
    format!("/api/integrations/github/repos?page={page}&per_page=100&sort=updated")
}

impl Rig {
    fn answer(&self, method: &str, path: &str, status: u16, body: impl ToString) {
        self.fake.lock().unwrap().answers.insert(format!("{method} {path}"), (status, body.to_string()));
    }
    fn gate(&self, path: &str) -> std::sync::mpsc::Sender<()> {
        let (go, wait) = channel();
        self.fake.lock().unwrap().gates.insert(path.into(), Arc::new(Mutex::new(wait)));
        go
    }
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| {
            let g = state_ref(&s.places);
            format!("work {:?} bookmarks {:?} repos {:?}", g.work, g.bookmarks, *g.repos.get())
        });
        self.rt.frame(FRAME);
    }
    fn set(&mut self, f: impl FnOnce(&mut State) + 'static) {
        self.rt.update(move |s, _| f(s));
        self.rt.frame(FRAME);
    }
    /// How many requests of `method` reached `path` (the path as sent, query included).
    fn asked(&self, method: &str, path: &str) -> usize {
        self.fake.lock().unwrap().log.iter().filter(|(m, p, _)| m == method && p == path).count()
    }
    fn bodies(&self, method: &str, path: &str) -> Vec<Value> {
        self.fake.lock().unwrap().log.iter().filter(|(m, p, _)| m == method && p == path).map(|(_, _, b)| b.clone()).collect()
    }
    fn total(&self) -> usize {
        self.fake.lock().unwrap().log.len()
    }
    fn work_is_in(&mut self) {
        self.wait("your work", |s| state_ref(&s.places).work.work.is_some() || state_ref(&s.places).work.error.is_some());
    }
    fn words(&self) -> String {
        self.rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
    }
}

// ─── your work ──────────────────────────────────────────────────────────────

#[test]
fn work_is_shared_and_kept_for_a_minute() {
    let mut r = rig();
    r.answer("GET", WORK, 200, super::mock::work());
    r.set(|s| {
        s.github_work_load();
        s.github_work_load();
    });
    r.work_is_in();
    assert_eq!(r.asked("GET", WORK), 1, "two callers, one request");
    let work = state_ref(&r.rt.state.places).work.work.clone().expect("work");
    assert_eq!(work.login.as_deref(), Some("mara-quinn"));

    r.set(|s| s.github_work_load());
    assert_eq!(r.asked("GET", WORK), 1, "a copy under a minute old is used");
    r.rt.frame(59.0);
    r.set(|s| s.github_work_load());
    assert_eq!(r.asked("GET", WORK), 1, "still fresh at 59 s");
    r.rt.frame(2.0);
    r.set(|s| s.github_work_load());
    let fake = r.fake.clone();
    r.wait("a stale copy to be read again", move |_| fake.lock().unwrap().log.len() == 2);
    assert_eq!(r.asked("GET", WORK), 2);
}

#[test]
fn a_refresh_asks_again_whatever_the_age() {
    let mut r = rig();
    r.answer("GET", WORK, 200, super::mock::work());
    r.set(|s| s.github_work_load());
    r.work_is_in();
    r.set(|s| s.github_work_refresh());
    let fake = r.fake.clone();
    r.wait("the forced read", move |_| fake.lock().unwrap().log.len() == 2);
    r.wait("it to finish", |s| !state_ref(&s.places).work.loading);
    assert_eq!(r.asked("GET", WORK), 2);
    // A refresh while one is out joins it.
    let go = r.gate(WORK);
    r.set(|s| {
        s.github_work_refresh();
        s.github_work_refresh();
    });
    let fake = r.fake.clone();
    r.wait("the held read to arrive", move |_| fake.lock().unwrap().log.len() == 3);
    assert!(state_ref(&r.rt.state.places).work.loading);
    go.send(()).unwrap();
    r.wait("it to finish", |s| !state_ref(&s.places).work.loading);
    assert_eq!(r.asked("GET", WORK), 3, "two refreshes while one was out made one request");
}

#[test]
fn work_says_why_it_could_not_load() {
    let mut r = rig();
    for (status, body, words) in [
        (401, "{}", "Connect GitHub in Settings to see this."),
        (500, r#"{"error":"The token store is locked."}"#, "The token store is locked."),
        (502, "", "GitHub didn't answer (HTTP 502)."),
    ] {
        r.answer("GET", WORK, status, body);
        r.set(|s| s.github_work_refresh());
        r.wait("the answer", |s| {
            let w = &state_ref(&s.places).work;
            !w.loading && w.error.is_some()
        });
        assert_eq!(state_ref(&r.rt.state.places).work.error.as_deref(), Some(words));
        assert!(state_ref(&r.rt.state.places).work.work.is_none());
    }
    // A good answer after a bad one clears the reason.
    r.answer("GET", WORK, 200, super::mock::work());
    r.set(|s| s.github_work_refresh());
    r.wait("the copy", |s| state_ref(&s.places).work.work.is_some());
    assert_eq!(state_ref(&r.rt.state.places).work.error, None);
}

#[test]
fn work_with_no_server_says_so() {
    let mut r = rig();
    r.set(|s| {
        s.link = None;
        s.github_work_load();
    });
    assert_eq!(state_ref(&r.rt.state.places).work.error.as_deref(), Some("Couldn't load your GitHub work."));
    assert!(!state_ref(&r.rt.state.places).work.loading);
}

// ─── bookmarks ──────────────────────────────────────────────────────────────

fn marks(names: &[&str]) -> Value {
    Value::Array(names.iter().map(|n| json!({"fullName": n, "owner": n.split('/').next(), "name": n.split('/').nth(1)})).collect())
}

fn names(r: &Rig) -> Vec<String> {
    state_ref(&r.rt.state.places).bookmarks.list.iter().map(|b| b.full_name.clone()).collect()
}

fn loaded(r: &mut Rig) {
    r.set(|s| s.github_bookmarks_use());
    r.wait("the list", |s| state_ref(&s.places).bookmarks.has_loaded);
}

#[test]
fn bookmarks_load_once_and_sort_by_full_name() {
    let mut r = rig();
    r.answer("GET", MARKS, 200, marks(&["lumen-labs/beacon", "acme/rocket", "Acme/Anvil", "acme/ant"]));
    loaded(&mut r);
    assert_eq!(names(&r), ["acme/ant", "Acme/Anvil", "acme/rocket", "lumen-labs/beacon"]);
    r.set(|s| {
        s.github_bookmarks_use();
        s.github_bookmarks_use();
    });
    assert_eq!(r.asked("GET", MARKS), 1, "a list that has loaded is not asked for again by whoever shows it");
    // `refresh` asks again; two at once make one.
    let go = r.gate(MARKS);
    r.set(|s| {
        s.github_bookmarks_load();
        s.github_bookmarks_load();
    });
    let fake = r.fake.clone();
    r.wait("the second read to arrive", move |_| fake.lock().unwrap().log.len() == 2);
    go.send(()).unwrap();
    r.wait("it to finish", |s| !state_ref(&s.places).bookmarks.loading);
    assert_eq!(r.asked("GET", MARKS), 2);
}

#[test]
fn bookmarks_may_come_as_a_list_or_as_an_object() {
    let mut r = rig();
    r.answer("GET", MARKS, 200, json!({"bookmarks": marks(&["acme/rocket", "acme/anvil"])}));
    loaded(&mut r);
    assert_eq!(names(&r), ["acme/anvil", "acme/rocket"]);
    r.answer("GET", MARKS, 200, json!({"nothing": true}));
    r.set(|s| s.github_bookmarks_load());
    r.wait("an empty answer", |s| {
        let b = &state_ref(&s.places).bookmarks;
        !b.loading && b.list.is_empty()
    });
}

#[test]
fn adding_a_bookmark_that_is_there_asks_nothing() {
    let mut r = rig();
    r.answer("GET", MARKS, 200, marks(&["acme/rocket"]));
    loaded(&mut r);
    r.set(|s| s.github_bookmark_add("acme/rocket"));
    assert_eq!(r.asked("POST", MARKS), 0);
    r.set(|s| s.github_bookmark_add("acme/anvil"));
    r.wait("the add", |s| state_ref(&s.places).bookmarks.has("acme/anvil"));
    assert_eq!(r.bodies("POST", MARKS), [json!({"repo": "acme/anvil"})]);
    assert_eq!(names(&r), ["acme/anvil", "acme/rocket"], "added in its place");
    let added = state_ref(&r.rt.state.places).bookmarks.list[0].clone();
    assert_eq!((added.owner.as_str(), added.name.as_str()), ("acme", "anvil"));
}

#[test]
fn removing_a_bookmark_names_both_halves_in_the_address() {
    let mut r = rig();
    r.answer("GET", MARKS, 200, marks(&["acme/rocket", "acme/anvil"]));
    loaded(&mut r);
    let before = r.total();
    for bad in ["rocket", "/rocket", "acme/", ""] {
        r.set(move |s| s.github_bookmark_remove(bad));
        assert_eq!(state_ref(&r.rt.state.places).bookmarks.action_error.as_deref(), Some("Invalid bookmark name."), "{bad:?}");
    }
    assert_eq!(r.total(), before, "a bad name is refused before asking");
    r.set(|s| s.github_bookmark_remove("acme/rocket"));
    r.wait("the removal", |s| !state_ref(&s.places).bookmarks.has("acme/rocket"));
    assert_eq!(r.asked("DELETE", "/api/integrations/github/bookmarks/acme/rocket"), 1);
    assert_eq!(names(&r), ["acme/anvil"]);
    assert_eq!(state_ref(&r.rt.state.places).bookmarks.action_error, None);
}

#[test]
fn each_failure_has_its_words() {
    let mut r = rig();
    r.answer("GET", MARKS, 500, "");
    r.set(|s| s.github_bookmarks_load());
    r.wait("the failed read", |s| state_ref(&s.places).bookmarks.error.is_some());
    assert_eq!(state_ref(&r.rt.state.places).bookmarks.error.as_deref(), Some("Failed to fetch bookmarks from server."));
    assert!(!state_ref(&r.rt.state.places).bookmarks.has_loaded, "a list that failed loads again when shown");

    r.answer("GET", MARKS, 500, r#"{"error":"GitHub is down","message":"ignored"}"#);
    r.set(|s| s.github_bookmarks_load());
    r.wait("the server's reason", |s| state_ref(&s.places).bookmarks.error.as_deref() == Some("GitHub is down"));
    r.answer("GET", MARKS, 500, r#"{"message":"Try later"}"#);
    r.set(|s| s.github_bookmarks_load());
    r.wait("the server's message", |s| state_ref(&s.places).bookmarks.error.as_deref() == Some("Try later"));

    r.answer("POST", MARKS, 500, "");
    r.set(|s| s.github_bookmark_add("acme/rocket"));
    r.wait("the failed add", |s| state_ref(&s.places).bookmarks.action_error.is_some());
    assert_eq!(state_ref(&r.rt.state.places).bookmarks.action_error.as_deref(), Some("Failed to add bookmark."));
    assert!(names(&r).is_empty(), "a refused add changes nothing");

    r.answer("DELETE", "/api/integrations/github/bookmarks/acme/rocket", 500, "");
    r.set(|s| s.github_bookmark_remove("acme/rocket"));
    r.wait("the failed removal", |s| state_ref(&s.places).bookmarks.action_error.as_deref() == Some("Failed to remove bookmark."));

    r.answer("PUT", MARKS, 500, r#"{"message":"Read only"}"#);
    r.set(|s| s.github_bookmarks_replace(vec![]));
    r.wait("the failed sync", |s| state_ref(&s.places).bookmarks.action_error.as_deref() == Some("Read only"));
    r.answer("PUT", MARKS, 500, "");
    r.set(|s| s.github_bookmarks_replace(vec![]));
    r.wait("the fallback", |s| state_ref(&s.places).bookmarks.action_error.as_deref() == Some("Failed to sync bookmarks."));
}

#[test]
fn replacing_sends_the_sorted_list_and_keeps_it() {
    let mut r = rig();
    let all = ["lumen-labs/beacon", "acme/rocket"].map(|n| super::bookmarks::Bookmark::of(n).unwrap());
    r.set(move |s| s.github_bookmarks_replace(all.to_vec()));
    r.wait("the sync", |s| !state_ref(&s.places).bookmarks.list.is_empty());
    assert_eq!(
        r.bodies("PUT", MARKS),
        [json!({"bookmarks": [
            {"fullName": "acme/rocket", "owner": "acme", "name": "rocket"},
            {"fullName": "lumen-labs/beacon", "owner": "lumen-labs", "name": "beacon"},
        ]})]
    );
    assert_eq!(names(&r), ["acme/rocket", "lumen-labs/beacon"]);
}

// ─── the repository cache ───────────────────────────────────────────────────

fn cache(r: &Rig) -> (usize, bool, Option<String>) {
    let c = state_ref(&r.rt.state.places).repos.get();
    (c.repos.len(), c.loading, c.error.clone())
}

fn settled(r: &mut Rig, what: &str) {
    r.wait(what, |s| {
        let c = state_ref(&s.places).repos.get();
        !c.loading && (!c.repos.is_empty() || c.error.is_some())
    });
}

#[test]
fn the_cache_reads_pages_until_a_short_one() {
    let mut r = rig();
    r.answer("GET", &repo_path(1), 200, page(0, 100));
    r.answer("GET", &repo_path(2), 200, page(100, 100));
    r.answer("GET", &repo_path(3), 200, page(200, 3));
    r.set(|s| s.github_repos_refresh(false));
    settled(&mut r, "the cache");
    assert_eq!(cache(&r), (203, false, None));
    for p in 1..=3 {
        assert_eq!(r.asked("GET", &repo_path(p)), 1, "page {p}");
    }
    assert_eq!(r.asked("GET", &repo_path(4)), 0, "a short page ends it");
    let c = state_ref(&r.rt.state.places).repos.get();
    let first = &c.repos[0];
    let got = (first.full_name.as_str(), first.owner_login.as_str(), first.language.as_deref(), first.stars, first.private);
    assert_eq!(got, ("acme/repo-000", "acme", Some("Rust"), 0, true));
}

#[test]
fn a_page_of_exactly_one_hundred_asks_for_the_next() {
    let mut r = rig();
    r.answer("GET", &repo_path(1), 200, page(0, 100));
    r.answer("GET", &repo_path(2), 200, "[]");
    r.set(|s| s.github_repos_refresh(false));
    settled(&mut r, "the cache");
    assert_eq!((cache(&r).0, r.asked("GET", &repo_path(2))), (100, 1));
}

/// The cache is `secs` old, from now: a frame that long would draw them all, one by one.
fn age(r: &mut Rig, secs: f64) {
    r.set(move |s| {
        let at = s.clock - secs;
        state(&mut s.places).repos.get().updated = Some(at);
    });
}

#[test]
fn the_cache_is_kept_fifteen_minutes_and_a_refresh_forces() {
    let mut r = rig();
    r.answer("GET", &repo_path(1), 200, page(0, 5));
    r.set(|s| s.github_repos_refresh(false));
    settled(&mut r, "the cache");
    r.set(|s| s.github_repos_refresh(false));
    assert_eq!(r.asked("GET", &repo_path(1)), 1, "a fresh cache is used");
    age(&mut r, 15.0 * 60.0 - 5.0);
    r.set(|s| s.github_repos_refresh(false));
    assert_eq!(r.asked("GET", &repo_path(1)), 1, "not yet stale");
    r.set(|s| s.github_repos_refresh(true));
    let fake = r.fake.clone();
    r.wait("the forced read", move |_| fake.lock().unwrap().log.len() == 2);
    r.wait("it to finish", |s| !state_ref(&s.places).repos.get().loading);
    age(&mut r, 15.0 * 60.0 + 1.0);
    r.set(|s| s.github_repos_refresh(false));
    let fake = r.fake.clone();
    r.wait("the stale read", move |_| fake.lock().unwrap().log.len() == 3);
    r.wait("it to finish", |s| !state_ref(&s.places).repos.get().loading);
    assert_eq!(r.asked("GET", &repo_path(1)), 3);
}

#[test]
fn two_callers_make_one_request() {
    let mut r = rig();
    r.answer("GET", &repo_path(1), 200, page(0, 5));
    let go = r.gate(&repo_path(1));
    r.set(|s| {
        s.github_repos_refresh(false);
        s.github_repos_refresh(true);
        s.github_repos_refresh(false);
    });
    let fake = r.fake.clone();
    r.wait("the request to arrive", move |_| fake.lock().unwrap().log.len() == 1);
    assert!(cache(&r).1);
    go.send(()).unwrap();
    settled(&mut r, "the cache");
    assert_eq!((cache(&r).0, r.total()), (5, 1));
}

#[test]
fn clearing_drops_an_answer_that_was_on_its_way() {
    let mut r = rig();
    r.answer("GET", &repo_path(1), 200, page(0, 5));
    let go = r.gate(&repo_path(1));
    r.set(|s| s.github_repos_refresh(false));
    let fake = r.fake.clone();
    r.wait("the request to arrive", move |_| fake.lock().unwrap().log.len() == 1);
    r.set(|s| state(&mut s.places).repos.clear());
    assert_eq!(cache(&r), (0, false, None), "clear forgets and stops waiting");
    go.send(()).unwrap();
    // The answer comes back to the UI thread as a job: take it, and it must change nothing.
    let job = test_wait::recv("the late answer to be handed back", &r.queue);
    r.rt.update(job);
    r.rt.frame(FRAME);
    assert_eq!(cache(&r), (0, false, None), "the old answer did not come back");
}

#[test]
fn the_cache_says_why_it_failed() {
    let mut r = rig();
    r.answer("GET", &repo_path(1), 500, r#"{"error":"Rate limited","message":"ignored"}"#);
    r.set(|s| s.github_repos_refresh(false));
    settled(&mut r, "the error");
    assert_eq!(cache(&r).2.as_deref(), Some("Rate limited"));
    r.answer("GET", &repo_path(1), 500, "");
    r.set(|s| s.github_repos_refresh(true));
    r.wait("the fallback", |s| state_ref(&s.places).repos.get().error.as_deref() == Some("Failed to fetch repositories."));
    r.answer("GET", &repo_path(1), 200, r#"{"not":"a list"}"#);
    r.set(|s| s.github_repos_refresh(true));
    r.wait("a bad answer", |s| state_ref(&s.places).repos.get().error.as_deref() == Some("Failed to load repositories."));
}

#[test]
fn a_page_that_fails_part_way_keeps_what_was_there() {
    let mut r = rig();
    r.answer("GET", &repo_path(1), 200, page(0, 4));
    r.set(|s| s.github_repos_refresh(false));
    settled(&mut r, "the cache");
    r.answer("GET", &repo_path(1), 200, page(0, 100));
    r.answer("GET", &repo_path(2), 500, r#"{"error":"Rate limited"}"#);
    r.set(|s| s.github_repos_refresh(true));
    r.wait("the error", |s| state_ref(&s.places).repos.get().error.is_some());
    assert_eq!(cache(&r), (4, false, Some("Rate limited".into())), "the old repositories stay, with the reason beside them");
    let c = state_ref(&r.rt.state.places).repos.get();
    assert_eq!(c.error_beside(), Some("Rate limited"));
    assert!(c.summary(0, 0).starts_with("4 repos loaded"));
}

#[test]
fn the_summary_says_what_the_cache_holds() {
    let mut r = rig();
    let said = |r: &Rig| state_ref(&r.rt.state.places).repos.get().summary(crate::fleet::link::now_ms(), 0);
    assert_eq!(said(&r), "No repositories cached yet.");
    r.answer("GET", &repo_path(1), 200, page(0, 5));
    let go = r.gate(&repo_path(1));
    r.set(|s| s.github_repos_refresh(false));
    assert_eq!(said(&r), "Loading repositories\u{2026}");
    go.send(()).unwrap();
    settled(&mut r, "the cache");
    assert_eq!(said(&r), "5 repos loaded \u{b7} updated just now");
    r.set(|s| state(&mut s.places).repos.clear());
    r.answer("GET", &repo_path(1), 500, r#"{"error":"Rate limited"}"#);
    r.set(|s| s.github_repos_refresh(false));
    settled(&mut r, "the error");
    assert_eq!(said(&r), "Rate limited");
}

// ─── the connection and the Clone view ─────────────────────────────────────

fn open_page(r: &mut Rig) {
    r.set(|s| {
        s.rail = Rail::Settings;
        s.open_settings("plugin:github", None);
    });
}

#[test]
fn connecting_fills_the_cache_and_disconnecting_clears_it() {
    let mut r = rig();
    r.answer("GET", "/api/integrations/github/auth/status", 200, r#"{"connected":true}"#);
    r.answer("GET", &repo_path(1), 200, page(0, 12));
    open_page(&mut r);
    settled(&mut r, "the cache after the status");
    assert_eq!(cache(&r).0, 12);
    assert!(r.words().contains("12 repos loaded \u{b7} updated just now"), "{}", r.words());

    click(&mut r.rt, "github-refresh");
    let fake = r.fake.clone();
    r.wait("the refresh", move |_| fake.lock().unwrap().log.iter().filter(|l| l.1 == repo_path(1)).count() == 2);
    r.wait("it to finish", |s| !state_ref(&s.places).repos.get().loading);
    assert_eq!(r.asked("GET", &repo_path(1)), 2, "Refresh forces");

    click(&mut r.rt, "github-disconnect");
    r.wait("the disconnect", |s| !state_ref(&s.places).auth.connected);
    assert_eq!(cache(&r), (0, false, None), "disconnecting forgets the repositories");
}

#[test]
fn the_row_says_refreshing_while_it_reads() {
    let mut r = rig();
    r.answer("GET", "/api/integrations/github/auth/status", 200, r#"{"connected":true}"#);
    r.answer("GET", &repo_path(1), 200, page(0, 3));
    let go = r.gate(&repo_path(1));
    open_page(&mut r);
    let fake = r.fake.clone();
    r.wait("the cache request", move |_| fake.lock().unwrap().log.iter().any(|l| l.1 == repo_path(1)));
    let w = r.words();
    assert!(w.contains("Loading repositories\u{2026}") && w.contains("Refreshing\u{2026}"), "{w}");
    go.send(()).unwrap();
    settled(&mut r, "the cache");
    let w = r.words();
    assert!(w.contains("3 repos loaded") && w.contains("Refresh") && !w.contains("Refreshing"), "{w}");
}

#[test]
fn a_not_connected_status_leaves_the_cache_empty() {
    let mut r = rig();
    r.answer("GET", "/api/integrations/github/auth/status", 200, r#"{"connected":false}"#);
    open_page(&mut r);
    r.wait("the status", |s| state_ref(&s.places).auth.asked && !state_ref(&s.places).auth.checking);
    assert_eq!(r.asked("GET", &repo_path(1)), 0, "nothing is read while not connected");
    assert_eq!(cache(&r), (0, false, None));
}

#[test]
fn the_clone_view_suggests_repositories_from_github() {
    let mut r = rig();
    r.answer("GET", &repo_path(1), 200, page(0, 3));
    r.set(|s| s.enter_clone_view());
    let names = |r: &Rig| r.rt.state.clone_suggestions().into_iter().map(|s| s.full_name).collect::<Vec<_>>();
    // The view's open is what asked; the plugin's `keep` answers on the next frame.
    settled(&mut r, "the cache the Clone view asked for");
    assert_eq!(names(&r), ["acme/repo-000", "acme/repo-001", "acme/repo-002"]);
    assert_eq!(r.rt.state.clone_suggestions()[0].icon, "github");
    assert_eq!(r.asked("GET", &repo_path(1)), 1);
    r.set(|s| s.enter_clone_view());
    r.rt.frame(FRAME);
    assert_eq!(r.asked("GET", &repo_path(1)), 1, "a fresh cache is not read again each time the view opens");
}

#[test]
fn a_plugin_switched_off_suggests_nothing() {
    let mut r = rig();
    r.answer("GET", &repo_path(1), 200, page(0, 3));
    r.set(|s| {
        s.pickers.prefs.adopt(&json!({ "plugins.disabled": "[\"github\"]" }));
        s.enter_clone_view();
    });
    r.rt.frame(FRAME);
    assert!(r.rt.state.places.repository_sources(&r.rt.state.pickers.prefs).is_empty());
    assert!(r.rt.state.clone_suggestions().is_empty());
    assert_eq!(r.total(), 0, "a plugin that is off asks nothing");
}
