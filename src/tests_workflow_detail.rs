//! Component 18, piece C: a workflow's detail and its recent runs. Fleet's `WorkflowsLoading` "says the recent runs are
//! loading…" and `WorkflowStartedBy` "shows in the Library's recent runs" and "waits for a repository, and for a file without
//! errors" under their names, then what the native view keeps: a recent run opens the session it waits in, what a row says, and
//! the facts under the steps.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::detail::{models, run_session, run_text, skills};
use crate::fleet::workflows::helpers::Tone;
use crate::fleet::workflows::parse_model_roles;
use crate::fleet::workflows::runs::{RECENT_RUNS, library_path};
use crate::mock::workflows::{Scene, library, titled};
use crate::tests::{FRAME, app, click};
use crate::tests_workflow_runs::fake;
use crate::{Rail, test_wait};
use serde_json::json;
use std::sync::Mutex;
use std::sync::mpsc::channel;

/// The server's library: the built-ins with the steps the mock gives them.
fn library_json() -> serde_json::Value {
    let step = |t: &crate::fleet::workflows::Step| json!({"id": t.id, "title": t.title, "kind": "agent"});
    let workflows: Vec<_> = library(None)
        .workflows
        .iter()
        .map(|w| json!({"id": w.id, "builtIn": true, "file": null, "name": w.name, "description": w.description, "steps": w.steps.iter().map(step).collect::<Vec<_>>(), "errors": []}))
        .collect();
    json!({"repository": null, "repositoryName": null, "workflows": workflows})
}

// ─── WorkflowsLoading.test.ts ───────────────────────────────────────────────────────────────────────────────────────

#[test]
fn says_the_recent_runs_are_loading_not_no_runs_yet_until_they_ve_been_asked_for() {
    let home = fake();
    home.route("GET", "/api/preferences", 200, json!({"Workflows": "true"}));
    home.route("GET", &library_path(None), 200, library_json());
    home.hold("GET", RECENT_RUNS);
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link =
        Link::start_machine(HOME_MACHINE_KEY.to_string(), home.base.clone(), Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.pickers.prefs = Default::default();
        s.load_preferences();
    });
    rt.frame(FRAME);
    let describe = |s: &crate::State| {
        format!("library {:?}, runs loading {}", s.workflows_nav.library.is_some(), s.workflow_runs(HOME_MACHINE_KEY).loading)
    };
    test_wait::drive(&mut rt, &queue, "the preferences", |s| s.pickers.prefs.fetched, describe);
    rt.update(|s, _| {
        s.enter_workflows();
        s.set_active_workflow("builtin:fix-a-bug");
    });
    // The runs are asked for and the answer is held: the library is there, the runs are on their way.
    let calls = home.calls_handle();
    test_wait::drive(
        &mut rt,
        &queue,
        "the library and a request for the runs",
        move |s| s.workflows_nav.library.is_some() && calls().iter().any(|(m, p)| m == "GET" && p == RECENT_RUNS),
        describe,
    );
    rt.frame(FRAME);
    rt.frame(FRAME);
    let words: Vec<String> = rt.elements().iter().filter_map(|e| e.text.clone()).collect();
    assert!(words.iter().any(|w| w == "RECENT RUNS"), "{words:?}");
    assert!(!words.iter().any(|w| w == "No runs yet"), "{words:?}");
    assert!(rt.find("workflow-runs-loading").is_some());

    home.route("GET", RECENT_RUNS, 200, json!([]));
    test_wait::drive(&mut rt, &queue, "the runs", |s| s.workflow_runs(HOME_MACHINE_KEY).loaded, describe);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("workflow-runs-loading").is_none());
    assert!(rt.find("workflow-runs-empty").is_some());
}

// ─── WorkflowStartedBy.test.ts ──────────────────────────────────────────────────────────────────────────────────────

#[test]
fn shows_in_the_library_s_recent_runs() {
    let mut rt = app();
    rt.update(|s, _| s.show_workflows(true, Scene::DetailStates));
    rt.frame(FRAME);
    let by = rt.elements().into_iter().filter(|e| e.key.as_deref() == Some("workflow-started-by")).count();
    assert_eq!(by, 1, "one run was started by an automation");
    let words: Vec<String> = rt.elements().iter().filter_map(|e| e.text.clone()).collect();
    assert!(words.iter().any(|w| w == "Started by") && words.iter().any(|w| w == "Nightly triage"), "{words:?}");
    // A plain form: not a button of its own, the row is the button.
    let row = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("workflow-recent-run-d3")).unwrap();
    assert_eq!(row.role, "button");
}

#[test]
fn waits_for_a_repository_and_for_a_file_without_errors() {
    let button = |rt: &rune::Runtime<crate::State>, key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap();
    let mut rt = app();
    rt.update(|s, _| s.show_workflows(true, Scene::DetailNoRepo));
    rt.frame(FRAME);
    let repeat = button(&rt, "workflow-repeat-on-schedule");
    assert!(repeat.opacity < 1.0, "off without a repository");
    assert_eq!(repeat.help.as_deref(), Some("Pick a repository in the Run box first"));
    assert!(button(&rt, "workflow-duplicate").opacity < 1.0);
    // A press on an off button does nothing.
    click(&mut rt, "workflow-repeat-on-schedule");
    assert!(rt.state.ui.toast.is_none(), "no toast from an off button");

    rt.update(|s, _| s.show_workflows(true, Scene::DetailErrors));
    rt.frame(FRAME);
    assert!(button(&rt, "workflow-repeat-on-schedule").opacity < 1.0, "off for a file with errors");

    rt.update(|s, _| s.show_workflows(true, Scene::Detail));
    rt.frame(FRAME);
    let repeat = button(&rt, "workflow-repeat-on-schedule");
    assert!((repeat.opacity - 1.0).abs() < f32::EPSILON);
    assert_eq!(repeat.help.as_deref(), Some("Run this workflow on a schedule, as an automation"));
    click(&mut rt, "workflow-repeat-on-schedule");
    assert!(rt.state.ui.toast.is_some(), "the press waits for Automations (component 19) with a toast");
}

// ─── The native detail ──────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_recent_run_opens_the_session_it_waits_in_else_its_newest_step_s() {
    let mut rt = app();
    rt.update(|s, _| s.show_workflows(true, Scene::Detail));
    rt.frame(FRAME);
    let (waits, steps) = (rt.state.projects[0].sessions[0].id.clone(), rt.state.projects[0].sessions[1].id.clone());
    let mut run = titled("w1", "builtin:fix-a-bug", "waiting", false, "2026-11-19T14:00:00Z", "A waiting run", None);
    run.waiting.as_mut().unwrap().session_id = Some(waits.clone());
    run.sessions.push(crate::fleet::workflows::RunSession {
        session_id: steps.clone(),
        step_id: "fix".into(),
        visit: 1,
        ..Default::default()
    });
    assert_eq!(run_session(&run), Some(waits.as_str()));
    run.waiting = None;
    run.status = crate::fleet::workflows::RunStatus::Running;
    assert_eq!(run_session(&run), Some(steps.as_str()), "the newest step's session");
    run.sessions.clear();
    assert_eq!(run_session(&run), None);

    // Waiting: the click opens the session the run waits in, and leaves Workflows for Sessions.
    let mut waiting = titled("w2", "builtin:fix-a-bug", "waiting", false, "2026-11-19T14:05:00Z", "A waiting run", None);
    waiting.waiting.as_mut().unwrap().session_id = Some(waits.clone());
    rt.update(move |s, _| {
        s.workflows.of_mut(HOME_MACHINE_KEY).upsert(waiting);
    });
    rt.frame(FRAME);
    assert_eq!(rt.state.rail, Rail::Workflows);
    click(&mut rt, "workflow-recent-run-w2");
    // The mock's ids are invented ones, which `open_id` leaves out; the route names the session.
    assert_eq!(rt.state.route, crate::fleet::store::Route::Session(waits.clone()));
    assert_eq!(rt.state.rail, Rail::Sessions);
}

#[test]
fn a_run_says_where_it_is_and_the_newest_six_of_this_workflow_show() {
    let (_, tone) = run_text(&titled("a", "w", "failed", false, "2026-11-19T10:00:00Z", "T", None));
    assert_eq!(tone, Tone::Fail);
    assert_eq!(run_text(&titled("a", "w", "waiting", false, "2026-11-19T10:00:00Z", "T", None)).0, "Needs you \u{b7} Find the cause");
    assert_eq!(run_text(&titled("a", "w", "running", true, "2026-11-19T10:00:00Z", "T", None)).0, "With you \u{b7} Find the cause");
    assert_eq!(run_text(&titled("a", "w", "running", false, "2026-11-19T10:00:00Z", "T", None)).0, "Running \u{b7} Find the cause");
    let mut running = titled("a", "w", "running", false, "2026-11-19T10:00:00Z", "T", None);
    running.current_step_id = Some("gone".into());
    assert_eq!(run_text(&running).0, "Running");
    assert_eq!(run_text(&titled("a", "w", "ended", false, "2026-11-19T10:00:00Z", "T", None)).0, "Ended");
    assert_eq!(run_text(&titled("a", "w", "done", false, "2026-11-19T10:00:00Z", "T", None)).0, "Done");
    assert_eq!(run_text(&titled("a", "w", "failed", false, "2026-11-19T10:00:00Z", "T", None)).0, "Couldn't start the step");

    let mut rt = app();
    rt.update(|s, _| s.show_workflows(true, Scene::Detail));
    rt.update(|s, _| {
        let runs = s.workflows.of_mut(HOME_MACHINE_KEY);
        for i in 0..8 {
            runs.upsert(titled(&format!("n{i}"), "builtin:fix-a-bug", "done", false, &format!("2026-11-1{i}T10:00:00Z"), "Older", None));
        }
        runs.upsert(titled("other", "builtin:build-a-feature", "done", false, "2026-11-19T20:00:00Z", "Another workflow's", None));
    });
    rt.frame(FRAME);
    let shown = rt.elements().into_iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("workflow-recent-run-"))).count();
    assert_eq!(shown, 6);
    assert!(rt.find("workflow-recent-run-other").is_none());
}

#[test]
fn the_models_line_names_each_role_once_with_its_mapped_model_and_pinned_models_as_they_are() {
    let lib = library(Some(crate::mock::workflows::REPOSITORY));
    let bug = &lib.workflows[1];
    let roles =
        parse_model_roles(Some(r#"{"opencode": {"strong": {"model": "github-copilot/opus-5.5"}, "standard": {"model": "sonnet-5"}}}"#));
    assert_eq!(models(bug, &roles, "opencode"), "Strong \u{b7} opus-5.5, Standard \u{b7} sonnet-5, Fast \u{b7} Default model");
    assert_eq!(models(bug, &roles, "pi"), "Strong \u{b7} Default model, Standard \u{b7} Default model, Fast \u{b7} Default model");
    let mut pinned = bug.clone();
    pinned.steps[0].model = Some("openai/gpt-5".into());
    assert!(models(&pinned, &roles, "opencode").starts_with("openai/gpt-5, Standard"));
    assert_eq!(skills(bug), ["fleet-debug", "fleet-code-review", "fleet-run"]);
    assert_eq!(models(&lib.workflows[3], &roles, "opencode"), "", "a file with errors has no steps");
}
