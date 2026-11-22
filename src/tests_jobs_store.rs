//! Component 23, piece F: `stores/__tests__/jobs.test.ts` under its names, against a fake home Fleet that records every
//! request. The clock moves only where a test says so (`later`). Nothing is read before the fake's first answer has landed.
use crate::fleet::jobs::store::POLL_SECS;
use crate::fleet::jobs::{JobState, NewJob};
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::tests::{FRAME, app};
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub(crate) struct Heard {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) body: Value,
}

type Routes = Arc<Mutex<HashMap<(String, String), (u16, String)>>>;

/// A fake home Fleet: the answer of each (method, path), 404 `{"error": …}` for the rest.
pub(crate) struct Fleet {
    base: String,
    heard: Arc<Mutex<Vec<Heard>>>,
    routes: Routes,
}

fn waiting() -> Value {
    json!({"id": "j1", "title": "Update the docs", "ask": "Update the README", "repository": "https://example.com/acme/docs-site",
        "labels": [], "pinnedMachineId": null, "state": "waiting", "machineId": null, "machineName": null, "sessionId": null,
        "whyWaiting": "No machine has a free seat.", "error": null, "createdAt": "2026-10-27T08:00:00Z"})
}

fn atlas() -> Value {
    json!({"id": "m1", "name": "atlas", "isLocal": true, "seats": 2, "used": 0, "labels": [], "status": "online"})
}

fn fleet() -> Fleet {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let heard: Arc<Mutex<Vec<Heard>>> = Arc::default();
    let routes: Routes = Arc::default();
    let (log, table) = (heard.clone(), routes.clone());
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let (log, table) = (log.clone(), table.clone());
            std::thread::spawn(move || {
                let req = crate::test_http::read(&mut stream);
                log.lock().unwrap().push(Heard { method: req.method.clone(), path: req.path.clone(), body: req.json() });
                let key = (req.method.clone(), req.route().to_string());
                let (status, body) = table
                    .lock()
                    .unwrap()
                    .get(&key)
                    .cloned()
                    .unwrap_or((404, json!({"error": "Jobs are turned off in Fleet's Settings."}).to_string()));
                // Status 0 is a Fleet that takes the call and never answers it.
                if status != 0 {
                    crate::test_http::respond_json(&mut stream, format!("{status} X"), &body);
                }
            });
        }
    });
    let f = Fleet { base, heard, routes };
    f.route("GET", "/api/jobs", 200, json!({"jobs": [waiting()], "machines": [atlas()]}));
    f
}

impl Fleet {
    pub(crate) fn route(&self, method: &str, path: &str, status: u16, body: Value) {
        self.routes.lock().unwrap().insert((method.into(), path.into()), (status, body.to_string()));
    }

    pub(crate) fn count(&self, method: &str, path: &str) -> usize {
        self.heard.lock().unwrap().iter().filter(|h| h.method == method && h.path == path).count()
    }

    fn counter(&self, method: &'static str, path: &'static str) -> impl Fn() -> usize + use<> {
        let heard = self.heard.clone();
        move || heard.lock().unwrap().iter().filter(|h| h.method == method && h.path == path).count()
    }

    pub(crate) fn last(&self, method: &str) -> Heard {
        self.heard.lock().unwrap().iter().rev().find(|h| h.method == method).cloned().expect("a request")
    }

    /// Every request about jobs; the connected app asks the fake other things too.
    pub(crate) fn jobs_total(&self) -> usize {
        self.heard.lock().unwrap().iter().filter(|h| h.path.starts_with("/api/jobs")).count()
    }
}

pub(crate) struct Rig {
    pub(crate) rt: Runtime<State>,
    queue: Receiver<Job>,
    pub(crate) home: Fleet,
}

/// The app connected to a fake home, Jobs on, on the Sessions rail: the Jobs page isn't shown.
pub(crate) fn rig() -> Rig {
    let home = fleet();
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link =
        Link::start_machine(HOME_MACHINE_KEY.to_string(), home.base.clone(), Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.pickers.prefs.adopt(&json!({"Jobs": "true"}));
    });
    rt.frame(FRAME);
    Rig { rt, queue, home }
}

impl Rig {
    /// Applies what the threads hand back and draws frames, without moving the clock, until `done` holds.
    pub(crate) fn wait(&mut self, what: &str, mut done: impl FnMut(&State) -> bool) {
        let queue = &self.queue;
        test_wait::until(
            &mut self.rt,
            what,
            |rt| {
                while let Ok(job) = queue.try_recv() {
                    rt.update(job);
                }
                rt.frame(0.0);
                done(&rt.state)
            },
            |rt| format!("jobs {}, error {:?}, post {:?}", rt.state.jobs.jobs.len(), rt.state.jobs.error, rt.state.jobs.post),
        );
    }

    pub(crate) fn show(&mut self) {
        self.rt.update(|s, _| s.rail = Rail::Jobs);
        self.rt.frame(FRAME);
    }

    pub(crate) fn later(&mut self, secs: f64) {
        self.rt.frame(secs);
    }
}

// ─── stores/__tests__/jobs.test.ts ──────────────────────────────────────────────────────────────────────────────────

#[test]
fn asks_nothing_until_something_loads_it() {
    let mut r = rig();
    r.later(POLL_SECS * 3.0);
    assert_eq!(r.home.jobs_total(), 0, "Jobs is on, but the page isn't shown and nothing watches");
    assert!(!r.rt.state.jobs.is_loaded);
}

#[test]
fn asks_nothing_with_jobs_off_even_on_the_jobs_rail() {
    let mut r = rig();
    r.rt.update(|s, _| s.pickers.prefs.adopt(&json!({"Jobs": "false"})));
    r.show();
    r.later(POLL_SECS * 2.0);
    assert_eq!(r.home.jobs_total(), 0);
}

#[test]
fn loads_the_jobs_and_the_machines_seats_from_the_home_fleet() {
    let mut r = rig();
    r.show();
    r.wait("the first answer", |s| s.jobs.is_loaded);
    let jobs = &r.rt.state.jobs;
    assert_eq!(jobs.jobs.len(), 1);
    assert_eq!((jobs.jobs[0].id.as_str(), jobs.jobs[0].state), ("j1", JobState::Waiting));
    assert_eq!(jobs.jobs[0].why_waiting.as_deref(), Some("No machine has a free seat."));
    assert_eq!((jobs.machines[0].name.as_str(), jobs.machines[0].seats, jobs.machines[0].is_local), ("atlas", 2, true));
    assert_eq!(r.home.count("GET", "/api/jobs"), 1);
}

#[test]
fn says_what_fleet_said_when_it_refuses() {
    let mut r = rig();
    r.home.route("GET", "/api/jobs", 404, json!({"error": "Jobs are turned off in Fleet's Settings."}));
    r.show();
    r.wait("the refusal", |s| s.jobs.error.is_some());
    assert_eq!(r.rt.state.jobs.error.as_deref(), Some("Jobs are turned off in Fleet's Settings."));
    assert!(!r.rt.state.jobs.is_loaded);

    r.home.route("GET", "/api/jobs", 500, json!({}));
    r.later(POLL_SECS + 0.5);
    r.wait("the next refusal", |s| s.jobs.error.as_deref() == Some("Couldn't load the jobs."));
}

#[test]
fn says_it_could_not_reach_fleet_and_keeps_the_last_list() {
    let mut r = rig();
    r.show();
    r.wait("the first answer", |s| s.jobs.is_loaded);
    r.home.route("GET", "/api/jobs", 0, json!(null));
    r.later(POLL_SECS + 0.1);
    r.wait("the failed read", |s| s.jobs.error.is_some());
    assert_eq!(r.rt.state.jobs.error.as_deref(), Some("Couldn't reach Fleet."));
    assert_eq!(r.rt.state.jobs.jobs.len(), 1, "the list stays");
    assert!(r.rt.state.jobs.is_loaded);

    r.home.route("GET", "/api/jobs", 200, json!({"jobs": [], "machines": []}));
    r.later(POLL_SECS + 0.1);
    r.wait("the next good read", |s| s.jobs.error.is_none() && s.jobs.jobs.is_empty());
}

#[test]
fn asks_again_every_few_seconds_while_watched_and_stops_with_the_last_watcher() {
    let mut r = rig();
    let (first, second) = (r.rt.state.jobs.watch(), r.rt.state.jobs.watch());
    let reads = r.home.counter("GET", "/api/jobs");
    // Both watch the same timer, from the app's own looks: the Jobs page isn't shown, so these two are the only watchers.
    r.later(0.1);
    r.wait("the first read", {
        let reads = r.home.counter("GET", "/api/jobs");
        move |_| reads() >= 1
    });
    assert_eq!(reads(), 1, "two watchers read once, not twice");
    r.later(POLL_SECS + 0.1);
    r.wait("the second read", {
        let reads = r.home.counter("GET", "/api/jobs");
        move |_| reads() == 2
    });
    r.later(POLL_SECS + 0.1);
    r.wait("the third read", {
        let reads = r.home.counter("GET", "/api/jobs");
        move |_| reads() == 3
    });

    r.rt.state.jobs.unwatch(first);
    r.later(POLL_SECS + 0.1);
    r.wait("a read with one watcher left", {
        let reads = r.home.counter("GET", "/api/jobs");
        move |_| reads() == 4
    });

    r.rt.state.jobs.unwatch(second);
    r.rt.state.jobs.unwatch(second);
    let stopped = r.home.count("GET", "/api/jobs");
    r.later(POLL_SECS * 3.0);
    assert_eq!(r.home.count("GET", "/api/jobs"), stopped, "the last watcher stopped the timer");
}

#[test]
fn watches_while_the_jobs_page_is_shown_and_stops_when_it_is_left() {
    let mut r = rig();
    r.show();
    r.wait("the first read", |s| s.jobs.is_loaded);
    r.later(POLL_SECS + 0.1);
    let reads = r.home.counter("GET", "/api/jobs");
    r.wait("a second read", move |_| reads() == 2);

    r.rt.update(|s, _| s.rail = Rail::Sessions);
    r.later(FRAME);
    let left = r.home.count("GET", "/api/jobs");
    r.later(POLL_SECS * 3.0);
    assert_eq!(r.home.count("GET", "/api/jobs"), left, "leaving the page stops the reads");
}

#[test]
fn posts_a_job_and_loads_the_queue_again() {
    let mut r = rig();
    r.home.route("POST", "/api/jobs", 200, json!({"id": "j2"}));
    let job = NewJob {
        title: "Update the docs".into(),
        ask: "Update the README".into(),
        repository: "https://example.com/acme/docs-site".into(),
        labels: vec!["mac".into()],
        pinned_machine_id: None,
    };
    r.rt.update(move |s, _| s.jobs_post(&job));
    assert!(r.rt.state.jobs.post.busy);
    r.wait("the post and the read after it", |s| s.jobs.is_loaded && !s.jobs.post.busy);
    assert_eq!(
        r.home.last("POST").body,
        json!({"title": "Update the docs", "ask": "Update the README", "repository": "https://example.com/acme/docs-site",
               "labels": ["mac"], "pinnedMachineId": null})
    );
    assert_eq!(r.rt.state.jobs.jobs.len(), 1);
    assert_eq!(r.rt.state.jobs.post.error, None);
    assert_eq!(r.home.count("GET", "/api/jobs"), 1, "the queue is read once, after the post");
}

#[test]
fn says_what_the_server_said_when_it_refuses_a_job_and_reads_nothing_after() {
    let mut r = rig();
    r.home.route("POST", "/api/jobs", 400, json!({"error": "Pick a repository."}));
    r.rt.update(|s, _| s.jobs_post(&NewJob::default()));
    r.wait("the refusal", |s| !s.jobs.post.busy);
    assert_eq!(r.rt.state.jobs.post.error.as_deref(), Some("Pick a repository."));
    assert_eq!(r.home.count("GET", "/api/jobs"), 0);

    r.home.route("POST", "/api/jobs", 500, json!({}));
    r.rt.update(|s, _| s.jobs_post(&NewJob::default()));
    r.wait("the second refusal", |s| !s.jobs.post.busy);
    assert_eq!(r.rt.state.jobs.post.error.as_deref(), Some("Couldn't post the job."));
}

#[test]
fn cancels_a_waiting_job_and_loads_the_queue_again() {
    let mut r = rig();
    r.home.route("DELETE", "/api/jobs/a%20b", 204, json!(null));
    r.rt.update(|s, _| s.jobs_cancel("a b"));
    assert_eq!(r.rt.state.jobs.cancelling.as_deref(), Some("a b"));
    r.wait("the cancel and the read after it", |s| s.jobs.is_loaded && !s.jobs.cancel.busy);
    assert_eq!(r.home.count("DELETE", "/api/jobs/a%20b"), 1);
    assert_eq!(r.rt.state.jobs.cancel.error, None);

    r.home.route("DELETE", "/api/jobs/j1", 409, json!({}));
    r.rt.update(|s, _| s.jobs_cancel("j1"));
    r.wait("the refusal", |s| !s.jobs.cancel.busy);
    assert_eq!(r.rt.state.jobs.cancel.error.as_deref(), Some("Couldn't cancel the job."));
}

#[test]
fn keeps_a_machine_s_saved_seats_and_labels() {
    let mut r = rig();
    r.show();
    r.wait("the first answer", |s| s.jobs.is_loaded);
    r.home.route(
        "PUT",
        "/api/jobs/machines/m1",
        200,
        json!({"id": "m1", "name": "atlas", "isLocal": true, "seats": 3, "used": 0, "labels": ["mac"], "status": "online"}),
    );
    r.rt.update(|s, _| s.jobs_save_machine("m1", Some(3), Some(vec!["mac".into()])));
    r.wait("the save", |s| !s.jobs.save.busy);
    let m = &r.rt.state.jobs.machines[0];
    assert_eq!((m.seats, m.labels.clone()), (3, vec!["mac".to_string()]));
    let put = r.home.last("PUT");
    assert_eq!((put.path.as_str(), put.body), ("/api/jobs/machines/m1", json!({"seats": 3, "labels": ["mac"]})));

    // A field left out stays out of the body, and so stays as it is.
    r.home.route("PUT", "/api/jobs/machines/m1", 400, json!({"detail": "Seats are 0 to 16."}));
    r.rt.update(|s, _| s.jobs_save_machine("m1", Some(99), None));
    r.wait("the refusal", |s| !s.jobs.save.busy);
    assert_eq!(r.home.last("PUT").body, json!({"seats": 99}));
    assert_eq!(r.rt.state.jobs.save.error.as_deref(), Some("Seats are 0 to 16."));
    assert_eq!(r.rt.state.jobs.machines[0].seats, 3, "a refused save changes nothing");
}

#[test]
fn lists_the_repositories_new_job_can_pick() {
    let mut r = rig();
    r.home.route(
        "GET",
        "/api/jobs/repositories",
        200,
        json!([{"name": "docs-site", "repository": "https://example.com/acme/docs-site", "machines": ["atlas"]}]),
    );
    r.rt.update(|s, _| s.jobs_load_repositories());
    assert!(r.rt.state.jobs.repositories.loading);
    r.wait("the list", |s| !s.jobs.repositories.loading);
    assert_eq!(r.rt.state.jobs.repositories.list.as_ref().map(Vec::len), Some(1));

    r.home.route("GET", "/api/jobs/repositories", 500, json!({}));
    r.rt.update(|s, _| s.jobs_load_repositories());
    r.wait("the refusal", |s| !s.jobs.repositories.loading);
    assert_eq!(r.rt.state.jobs.repositories.error.as_deref(), Some("Couldn't list the repositories."));
}
