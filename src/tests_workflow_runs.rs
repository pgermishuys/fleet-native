//! Component 18, piece A: the runs store against a fake Fleet. `stores/__tests__/workflows-machine.test.ts` under its name (the
//! save of a workflow file is slice 2, so the third request there is Ending a run), then the rules the store keeps: an older
//! event never replaces a newer run, the list load leaves the events' runs alone, a load that failed fills from events and is
//! tried again, Workflows off loads nothing, and each change reaches the server as Fleet sends it and is kept at once.
use crate::fleet::events::Event;
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::state::Topic;
use crate::fleet::workflows::runs::{Choice, RECENT_RUNS, RunDone, StartRequest};
use crate::fleet::workflows::{Library, Run, RunStatus};
use crate::mock::machines::Case;
use crate::tests::{FRAME, app};
use crate::{State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

type Heard = Arc<Mutex<Vec<(String, String, Value)>>>;
type Routes = Arc<Mutex<HashMap<String, (u16, String)>>>;

/// A Fleet that answers the routes a test gives it ("METHOD /path?query" → status, body) and 404 for the rest, and records
/// every request.
pub(crate) struct Fake {
    pub(crate) base: String,
    heard: Heard,
    routes: Routes,
}

pub(crate) fn fake() -> Fake {
    let listener = crate::test_http::bind().expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let (heard, routes): (Heard, Routes) = Default::default();
    let (log, table) = (heard.clone(), routes.clone());
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let (log, table) = (log.clone(), table.clone());
            std::thread::spawn(move || {
                let req = crate::test_http::read(&mut stream);
                if req.method.is_empty() {
                    return;
                }
                log.lock().unwrap().push((req.method.clone(), req.path.clone(), serde_json::from_slice(&req.body).unwrap_or(Value::Null)));
                // A route held with `hold` (status 0) waits here until the test gives it an answer.
                let (status, body) = loop {
                    let found = table.lock().unwrap().get(&format!("{} {}", req.method, req.path)).cloned().unwrap_or((404, "{}".into()));
                    if found.0 != 0 {
                        break found;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                };
                crate::test_http::respond_json(&mut stream, format!("{status} X"), &body);
            });
        }
    });
    Fake { base, heard, routes }
}

impl Fake {
    pub(crate) fn route(&self, method: &str, path: &str, status: u16, body: Value) {
        self.routes.lock().unwrap().insert(format!("{method} {path}"), (status, body.to_string()));
    }

    /// The route is heard but not answered until `route` gives it an answer.
    pub(crate) fn hold(&self, method: &str, path: &str) {
        self.routes.lock().unwrap().insert(format!("{method} {path}"), (0, String::new()));
    }

    /// A way to read the requests heard so far from a wait that must be `'static`.
    pub(crate) fn calls_handle(&self) -> impl Fn() -> Vec<(String, String)> + use<> {
        let heard = self.heard.clone();
        move || heard.lock().unwrap().iter().map(|(m, p, _)| (m.clone(), p.clone())).collect()
    }

    /// Every request heard, as (method, path).
    pub(crate) fn calls(&self) -> Vec<(String, String)> {
        self.heard.lock().unwrap().iter().map(|(m, p, _)| (m.clone(), p.clone())).collect()
    }

    /// The bodies of the requests heard with this method and path, in order.
    pub(crate) fn bodies(&self, method: &str, path: &str) -> Vec<Value> {
        self.heard.lock().unwrap().iter().filter(|(m, p, _)| m == method && p == path).map(|(.., b)| b.clone()).collect()
    }

    /// Only the requests about workflows: the hub and the pollers of a link talk to the fake too.
    pub(crate) fn workflow_calls(&self) -> Vec<(String, String, Value)> {
        self.heard.lock().unwrap().iter().filter(|(_, p, _)| p.starts_with("/api/workflows")).cloned().collect()
    }
}

fn run_json(id: &str, status: &str, updated: &str) -> Value {
    json!({
        "id": id, "workflowId": "builtin:fix-a-bug", "workflowName": "Fix a bug", "title": "The pager skips the last page",
        "request": "The pager skips the last page when the count divides evenly", "status": status, "currentStepId": "diagnose",
        "result": null, "repositoryPath": "/work/repo", "branch": "fleet/pager", "baseBranch": "main", "worktreePath": "/work/repo-worktrees/pager",
        "harnessType": "opencode", "createdAt": "2026-09-23T10:00:00Z", "updatedAt": updated, "endedAt": null,
        "steps": [{"id": "diagnose", "title": "Diagnose", "kind": "agent", "state": "running", "visits": 1, "enabled": true}],
        "sessions": [{"sessionId": format!("{id}-s1"), "stepId": "diagnose", "visit": 1}]
    })
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    home: Fake,
}

/// The app connected to a fake Fleet, with other machines listed and `m-harbor` answering at a second fake.
fn rig() -> (Rig, Fake) {
    let (home, harbor) = (fake(), fake());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link =
        Link::start_machine(HOME_MACHINE_KEY.to_string(), home.base.clone(), Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app();
    let base = harbor.base.clone();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.show_machines_only(Case::Several);
        s.machines.list.iter_mut().find(|m| m.id == "m-harbor").expect("harbor is listed").base_url = base;
    });
    rt.frame(FRAME);
    (Rig { rt, queue, home }, harbor)
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("home runs {:?}", s.workflow_runs(HOME_MACHINE_KEY)));
    }

    fn runs(&self, machine: &str) -> Vec<(String, String)> {
        self.rt.state.workflow_runs(machine).ordered().iter().map(|r| (r.id.clone(), r.updated_at.clone())).collect()
    }

    /// A `workflow_run` event on the sessions topic, as the hub sends it.
    fn event(&mut self, run: Value) {
        let event = Event::from_wire(&json!({"type": "workflow_run", "properties": run})).expect("an event");
        self.rt.update(move |s, _| s.dispatch(&Topic::Sessions, &[event]));
    }

    fn load(&mut self, machine: &'static str) {
        self.rt.update(move |s, _| s.workflow_runs_ensure_loaded(machine));
    }
}

type Heard2 = Arc<Mutex<Vec<Result<Run, String>>>>;

fn done_into(sink: &Heard2) -> RunDone {
    let sink = sink.clone();
    Box::new(move |_, _, result| sink.lock().unwrap().push(result))
}

// ─── stores/__tests__/workflows-machine.test.ts ─────────────────────────────────────────────────────────────────

#[test]
fn asks_that_machine_for_the_library_the_runs_and_every_change_never_the_live_one() {
    let (mut rig, harbor) = rig();
    harbor.route(
        "GET",
        "/api/workflows?directory=%2Fwork%2Frepo",
        200,
        json!({"workflows": [], "repository": null, "repositoryName": null}),
    );
    harbor.route("GET", RECENT_RUNS, 200, json!([]));
    harbor.route("POST", "/api/workflows/runs/run-1/end", 200, run_json("run-1", "ended", "2026-09-23T10:20:00Z"));
    // The live machine would answer all of them too; it must not be asked.
    rig.home.route("GET", RECENT_RUNS, 200, json!([run_json("live-run", "running", "2026-09-23T10:00:00Z")]));

    let library: Arc<Mutex<Vec<Result<Library, String>>>> = Default::default();
    let sink = library.clone();
    rig.rt.update(move |s, tx| s.workflow_library("m-harbor", Some("/work/repo"), tx, move |_, _, r| sink.lock().unwrap().push(r)));
    rig.wait("the library", |_| library.lock().unwrap().len() == 1);
    assert_eq!(library.lock().unwrap()[0], Ok(Library::default()));

    rig.load("m-harbor");
    rig.wait("harbor's runs", |s| s.workflow_runs("m-harbor").loaded);

    let ended: Heard2 = Default::default();
    let done = done_into(&ended);
    rig.rt.update(move |s, tx| s.workflow_end("m-harbor", "run-1", tx, done));
    rig.wait("the run to end", |_| ended.lock().unwrap().len() == 1);

    let paths: Vec<(String, String)> = harbor.workflow_calls().into_iter().map(|(m, p, _)| (m, p)).collect();
    assert_eq!(
        paths,
        [
            ("GET".to_string(), "/api/workflows?directory=%2Fwork%2Frepo".to_string()),
            ("GET".to_string(), RECENT_RUNS.to_string()),
            ("POST".to_string(), "/api/workflows/runs/run-1/end".to_string()),
        ]
    );
    assert!(rig.home.workflow_calls().is_empty(), "the live machine heard nothing: {:?}", rig.home.workflow_calls());
    assert!(rig.runs(HOME_MACHINE_KEY).is_empty(), "and holds none of harbor's runs");
    assert_eq!(rig.runs("m-harbor"), [("run-1".to_string(), "2026-09-23T10:20:00Z".to_string())]);
}

// ─── The store ──────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn an_older_event_doesn_t_replace_a_newer_run() {
    let (mut rig, _) = rig();
    rig.home.route("GET", RECENT_RUNS, 200, json!([]));
    rig.load(HOME_MACHINE_KEY);
    rig.wait("the list", |s| s.workflow_runs(HOME_MACHINE_KEY).loaded);

    rig.event(run_json("run-1", "running", "2026-09-23T10:20:00Z"));
    rig.event(run_json("run-1", "waiting", "2026-09-23T10:10:00Z"));
    let run = rig.rt.state.workflow_runs(HOME_MACHINE_KEY).get("run-1").cloned().unwrap();
    assert_eq!((run.status, run.updated_at.as_str()), (RunStatus::Running, "2026-09-23T10:20:00Z"));

    rig.event(run_json("run-1", "waiting", "2026-09-23T10:30:00Z"));
    assert_eq!(rig.rt.state.workflow_runs(HOME_MACHINE_KEY).get("run-1").unwrap().status, RunStatus::Waiting);
    rig.event(json!({"id": "not-a-run"}));
    assert_eq!(rig.runs(HOME_MACHINE_KEY).len(), 1, "an event that isn't a run is dropped");
}

#[test]
fn the_list_load_leaves_the_runs_events_brought_alone() {
    let (mut rig, _) = rig();
    rig.home.route(
        "GET",
        RECENT_RUNS,
        200,
        json!([run_json("run-1", "running", "2026-09-23T10:10:00Z"), run_json("run-2", "done", "2026-09-23T09:00:00Z"), {"id": "junk"}]),
    );
    rig.load(HOME_MACHINE_KEY);
    assert!(rig.rt.state.workflow_runs(HOME_MACHINE_KEY).loading, "No runs yet waits for the answer");
    assert!(!rig.rt.state.workflow_runs(HOME_MACHINE_KEY).loaded);
    // The event arrives before the answer is read, and is newer than what the list holds of the same run.
    rig.event(run_json("run-1", "waiting", "2026-09-23T10:20:00Z"));

    rig.wait("the list", |s| s.workflow_runs(HOME_MACHINE_KEY).loaded);

    let store = rig.rt.state.workflow_runs(HOME_MACHINE_KEY);
    assert!(!store.loading);
    assert_eq!(store.get("run-1").unwrap().status, RunStatus::Waiting, "the event's copy stays");
    assert!(store.get("run-2").is_some());
    assert_eq!(store.ordered().len(), 2, "the entry that isn't a run is skipped");
    assert_eq!(rig.home.workflow_calls().len(), 1);
}

#[test]
fn a_list_that_failed_is_filled_from_events_and_asked_for_again_by_the_next_load() {
    let (mut rig, _) = rig();
    rig.home.route("GET", RECENT_RUNS, 500, json!({"error": "no"}));
    rig.load(HOME_MACHINE_KEY);
    rig.wait("the failed answer", |s| !s.workflow_runs(HOME_MACHINE_KEY).loading);
    assert!(!rig.rt.state.workflow_runs(HOME_MACHINE_KEY).loaded);

    rig.event(run_json("run-1", "running", "2026-09-23T10:20:00Z"));
    assert_eq!(rig.runs(HOME_MACHINE_KEY).len(), 1);

    rig.home.route("GET", RECENT_RUNS, 200, json!([run_json("run-2", "done", "2026-09-23T09:00:00Z")]));
    rig.load(HOME_MACHINE_KEY);
    rig.wait("the second list", |s| s.workflow_runs(HOME_MACHINE_KEY).loaded);
    assert_eq!(rig.runs(HOME_MACHINE_KEY).len(), 2);
    assert_eq!(rig.home.workflow_calls().len(), 2);

    // Once read, asking again asks nothing.
    rig.load(HOME_MACHINE_KEY);
    assert!(!rig.rt.state.workflow_runs(HOME_MACHINE_KEY).loading);
}

#[test]
fn workflows_off_loads_nothing_and_on_loads_the_runs_whichever_page_is_open() {
    let (mut rig, _) = rig();
    rig.home.route("GET", RECENT_RUNS, 200, json!([run_json("run-1", "running", "2026-09-23T10:10:00Z")]));

    rig.rt.update(|s, _| s.workflows_settle());
    assert!(rig.rt.state.workflows.of(HOME_MACHINE_KEY).is_none(), "off: no request is made, no store started");
    // An event with nobody listening is not kept either.
    rig.event(run_json("early", "running", "2026-09-23T10:10:00Z"));
    assert!(rig.runs(HOME_MACHINE_KEY).is_empty());

    rig.rt.update(|s, _| {
        s.pickers.prefs.set("Workflows", "true");
        s.workflows_settle();
    });
    rig.wait("the runs", |s| s.workflow_runs(HOME_MACHINE_KEY).loaded);
    assert_eq!(rig.runs(HOME_MACHINE_KEY).len(), 1);

    // Settling again with Workflows on asks nothing more.
    rig.rt.update(|s, _| s.workflows_settle());
    rig.event(run_json("run-3", "running", "2026-09-23T10:40:00Z"));
    assert_eq!(rig.runs(HOME_MACHINE_KEY).len(), 2);
    assert_eq!(rig.home.workflow_calls().len(), 1);
}

#[test]
fn reads_the_preferences_and_starts_loading_when_workflows_is_on() {
    let (mut rig, _) = rig();
    rig.home.route("GET", "/api/preferences", 200, json!({"Workflows": "true"}));
    rig.home.route("GET", RECENT_RUNS, 200, json!([run_json("run-1", "running", "2026-09-23T10:10:00Z")]));
    rig.rt.update(|s, _| s.load_preferences());
    rig.wait("the runs, after the preferences", |s| s.workflow_runs(HOME_MACHINE_KEY).loaded);
    assert_eq!(rig.runs(HOME_MACHINE_KEY).len(), 1);
}

#[test]
fn keeps_the_run_a_change_answers_with_at_once_and_sends_the_change_as_fleet_does() {
    let (mut rig, _) = rig();
    let waiting = |updated: &str| run_json("run-1", "waiting", updated);
    rig.home.route("POST", "/api/workflows/runs", 200, run_json("run-1", "running", "2026-09-23T10:00:00Z"));
    rig.home.route("POST", "/api/workflows/runs/run-1/answer", 200, waiting("2026-09-23T10:01:00Z"));
    rig.home.route("POST", "/api/workflows/runs/run-1/move-on", 200, run_json("run-1", "running", "2026-09-23T10:02:00Z"));
    rig.home.route("PUT", "/api/workflows/runs/run-1/check-with-me", 200, run_json("run-1", "running", "2026-09-23T10:03:00Z"));
    rig.home.route("POST", "/api/workflows/runs/run-1/end", 200, run_json("run-1", "ended", "2026-09-23T10:04:00Z"));
    let heard: Heard2 = Default::default();
    let mut expect = 0;
    let mut step = |rig: &mut Rig, send: &dyn Fn(&mut State, &mut rune::Tx, RunDone), want: &str| {
        let done = done_into(&heard);
        rig.rt.update(|s, tx| send(s, tx, done));
        expect += 1;
        let sink = heard.clone();
        rig.wait("the answer", move |_| sink.lock().unwrap().len() == expect);
        assert_eq!(rig.rt.state.workflow_runs(HOME_MACHINE_KEY).get("run-1").unwrap().updated_at, want, "kept before the caller hears it");
    };

    let request = StartRequest {
        workflow_id: "builtin:fix-a-bug".into(),
        directory: "/work/repo".into(),
        request: "The pager skips the last page".into(),
        check_with_me: Some(true),
        ..Default::default()
    };
    step(&mut rig, &|s, tx, d| s.workflow_start(HOME_MACHINE_KEY, &request, tx, d), "2026-09-23T10:00:00Z");
    step(
        &mut rig,
        &|s, tx, d| {
            s.workflow_answer(
                HOME_MACHINE_KEY,
                "run-1",
                &Choice { id: "approve".into(), note: Some("Looks right.".into()), check_with_me: Some(false) },
                tx,
                d,
            )
        },
        "2026-09-23T10:01:00Z",
    );
    step(&mut rig, &|s, tx, d| s.workflow_move_on(HOME_MACHINE_KEY, "run-1", Some("pass"), None, tx, d), "2026-09-23T10:02:00Z");
    step(&mut rig, &|s, tx, d| s.workflow_set_check_with_me(HOME_MACHINE_KEY, "run-1", false, tx, d), "2026-09-23T10:03:00Z");
    step(&mut rig, &|s, tx, d| s.workflow_end(HOME_MACHINE_KEY, "run-1", tx, d), "2026-09-23T10:04:00Z");

    assert!(heard.lock().unwrap().iter().all(|r| r.is_ok()));
    let calls = rig.home.workflow_calls();
    let sent: Vec<(&str, &str)> = calls.iter().map(|(m, p, _)| (m.as_str(), p.as_str())).collect();
    assert_eq!(
        sent,
        [
            ("POST", "/api/workflows/runs"),
            ("POST", "/api/workflows/runs/run-1/answer"),
            ("POST", "/api/workflows/runs/run-1/move-on"),
            ("PUT", "/api/workflows/runs/run-1/check-with-me"),
            ("POST", "/api/workflows/runs/run-1/end"),
        ]
    );
    assert_eq!(
        calls[0].2,
        json!({"workflowId": "builtin:fix-a-bug", "directory": "/work/repo", "request": "The pager skips the last page", "checkWithMe": true})
    );
    assert_eq!(calls[1].2, json!({"choice": "approve", "note": "Looks right.", "checkWithMe": false}));
    assert_eq!(calls[2].2, json!({"outcome": "pass", "note": null}));
    assert_eq!(calls[3].2, json!({"on": false}));
    assert_eq!(calls[4].2, json!({}));
}

#[test]
fn says_the_server_s_words_for_a_refusal_and_fleet_s_own_when_nothing_useful_comes_back() {
    let (mut rig, _) = rig();
    rig.home.route("POST", "/api/workflows/runs/run-1/answer", 409, json!({"error": "The run moved on."}));
    rig.home.route("POST", "/api/workflows/runs/run-1/move-on", 200, json!({"ok": true}));
    // Nothing is routed for /end: the server answers 404 with an empty body.
    let heard: Heard2 = Default::default();
    for (n, send) in [0, 1, 2].into_iter().enumerate() {
        let done = done_into(&heard);
        rig.rt.update(move |s, tx| match send {
            0 => s.workflow_answer(HOME_MACHINE_KEY, "run-1", &Choice { id: "approve".into(), ..Default::default() }, tx, done),
            1 => s.workflow_move_on(HOME_MACHINE_KEY, "run-1", None, None, tx, done),
            _ => s.workflow_end(HOME_MACHINE_KEY, "run-1", tx, done),
        });
        let sink = heard.clone();
        rig.wait("the answer", move |_| sink.lock().unwrap().len() == n + 1);
    }
    let said: Vec<Result<(), String>> = heard.lock().unwrap().iter().map(|r| r.as_ref().map(|_| ()).map_err(Clone::clone)).collect();
    assert_eq!(
        said,
        [Err("The run moved on.".to_string()), Err("Couldn't move the run on.".to_string()), Err("Couldn't end the run.".to_string()),]
    );
    assert!(rig.runs(HOME_MACHINE_KEY).is_empty(), "a refused change keeps nothing");
    assert_eq!(heard.lock().unwrap().len(), 3);
}

#[test]
fn a_change_with_no_server_says_so_instead_of_waiting() {
    let mut rt = app();
    let said: Heard2 = Default::default();
    let done = done_into(&said);
    rt.update(move |s, tx| s.workflow_end(HOME_MACHINE_KEY, "run-1", tx, done));
    assert_eq!(said.lock().unwrap().as_slice(), [Err("Couldn't end the run.".to_string())]);
    rt.update(|s, _| s.workflow_runs_ensure_loaded(HOME_MACHINE_KEY));
    assert!(!rt.state.workflow_runs(HOME_MACHINE_KEY).loading, "invented data has no runs to read");
}

// ─── Against a live scratch Fleet (not part of `cargo test`) ─────────────────────────────────────────────────────

/// The store following a real server: it reads the library, starts a run of the first built-in workflow that has no errors and
/// prints the store each time it changes, until the run waits on the user. Needs a scratch Fleet with Workflows on (docs/FLEET.md):
/// `FLEET_NATIVE_LIVE_URL=http://127.0.0.1:<port> FLEET_NATIVE_LIVE_DIR=<a git folder> cargo test live_store -- --ignored --nocapture`.
#[test]
#[ignore = "needs a scratch Fleet"]
fn live_store_follows_a_scratch_fleet_s_run() {
    let (Ok(base), Ok(dir)) = (std::env::var("FLEET_NATIVE_LIVE_URL"), std::env::var("FLEET_NATIVE_LIVE_DIR")) else {
        panic!("set FLEET_NATIVE_LIVE_URL and FLEET_NATIVE_LIVE_DIR");
    };
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(base, Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.pickers.prefs.set("Workflows", "true");
        s.workflows_settle();
    });
    let key = HOME_MACHINE_KEY;
    let show = |s: &State| -> String {
        let store = s.workflow_runs(key);
        let runs: Vec<String> = store
            .ordered()
            .iter()
            .map(|r| {
                let steps: Vec<String> = r.steps.iter().map(|st| format!("{}={:?}", st.id, st.state)).collect();
                format!("{} {:?} {} [{}]", r.id.chars().take(8).collect::<String>(), r.status, r.updated_at, steps.join(" "))
            })
            .collect();
        format!(
            "loading={} loaded={} runs={}{}",
            store.loading,
            store.loaded,
            runs.len(),
            runs.iter().map(|r| format!("\n    {r}")).collect::<String>()
        )
    };
    test_wait::drive(&mut rt, &queue, "the first read of the runs", |s| s.workflow_runs(key).loaded, |s| show(s));
    eprintln!("read:  {}", show(&rt.state));

    let library: Arc<Mutex<Option<Result<Library, String>>>> = Default::default();
    let sink = library.clone();
    let d = dir.clone();
    rt.update(move |s, tx| s.workflow_library(key, Some(&d), tx, move |_, _, r| *sink.lock().unwrap() = Some(r)));
    test_wait::drive(&mut rt, &queue, "the library", |_| library.lock().unwrap().is_some(), |s| show(s));
    let library = library.lock().unwrap().take().unwrap().expect("a library");
    eprintln!(
        "library: {:?}",
        library.workflows.iter().map(|w| format!("{} ({} steps, {} errors)", w.name, w.steps.len(), w.errors.len())).collect::<Vec<_>>()
    );
    let workflow = library.workflows.iter().find(|w| w.errors.is_empty() && !w.steps.is_empty()).expect("a workflow that can run");

    let started: Heard2 = Default::default();
    let done = done_into(&started);
    let request = StartRequest {
        workflow_id: workflow.id.clone(),
        directory: dir,
        request: "The pager skips the last page when the count divides evenly".into(),
        ..Default::default()
    };
    rt.update(move |s, tx| s.workflow_start(key, &request, tx, done));
    test_wait::drive(&mut rt, &queue, "the run to start", |_| !started.lock().unwrap().is_empty(), |s| show(s));
    let first = started.lock().unwrap()[0].clone().expect("the run started");
    eprintln!("start: answered {} {:?}; kept: {}", &first.id[..8], first.status, show(&rt.state));

    let mut last = String::new();
    test_wait::drive(
        &mut rt,
        &queue,
        "the run to wait on the user",
        |s| {
            let now = show(s);
            if now != last {
                eprintln!("event: {now}");
                last = now;
            }
            s.workflow_runs(key).get(&first.id).is_some_and(|r| r.status == RunStatus::Waiting)
        },
        |s| show(s),
    );
    let waiting = rt.state.workflow_runs(key).get(&first.id).and_then(|r| r.waiting.clone()).expect("a card");
    eprintln!("waiting: {:?} on {} — {}", waiting.kind, waiting.step_title, waiting.message);
    eprintln!("waiting sessions: {:?}", rt.state.workflow_runs(key).waiting_session_ids());
}
