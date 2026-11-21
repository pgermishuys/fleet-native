//! Component 18, piece F: the stepper under a step session's header. Fleet's `WorkflowStepper.test.ts` (5) and
//! `WorkflowStartedBy` "shows in the run's header above a step's conversation" under their names, then what the native view
//! keeps: a step opens its session, "Started by" opens the automation, an error shows under the switch, and the mark above
//! the prompts Fleet sent into a step.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::Run;
use crate::mock::workflow_stepper::{Case, PROMPT, WRAP_UP};
use crate::tests::{FRAME, app, click};
use crate::tests_workflow_runs::{Fake, fake};
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    home: Fake,
    /// The open session: the run's Fix step.
    session: String,
}

/// The app on a fake Fleet, the open session a step of `run` (the invented one unless a test gives its own JSON).
fn rig(edit: impl FnOnce(&mut Value) + Send + 'static) -> Rig {
    let home = fake();
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link =
        Link::start_machine(HOME_MACHINE_KEY.to_string(), home.base.clone(), Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.show_workflow_stepper(true, Case::Stepper);
    });
    let session = rt.state.session().id.clone();
    let mut json = run_json(&session);
    edit(&mut json);
    rt.update(move |s, _| {
        // The scene's own run goes: this test's run is the only one the session is a step of.
        let runs = s.workflows.of_mut(HOME_MACHINE_KEY);
        *runs = Default::default();
        runs.upsert(Run::from_json(&json).expect("a run"));
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    Rig { rt, queue, home, session }
}

/// Implement running on its own; Review still to come (the test's `implementRun`).
fn run_json(session: &str) -> Value {
    json!({
        "id": "run-1", "workflowId": "builtin:fix-a-bug", "workflowName": "Fix a bug", "title": "The pager skips the last page",
        "request": "x", "status": "running", "currentStepId": "implement", "result": null, "repositoryPath": "/work/kettle",
        "branch": null, "baseBranch": null, "worktreePath": null, "harnessType": "opencode",
        "createdAt": "2026-09-23T10:00:00Z", "updatedAt": "2026-09-23T10:05:00Z", "endedAt": null, "checkWithMe": false,
        "steps": [
            {"id": "plan", "title": "Plan", "kind": "agent", "state": "done", "visits": 1, "enabled": true, "sessionId": "s2"},
            {"id": "implement", "title": "Implement", "kind": "agent", "state": "running", "visits": 1, "enabled": true, "sessionId": session},
            {"id": "review", "title": "Review", "kind": "agent", "state": "pending", "visits": 0, "enabled": true, "withYou": false}
        ],
        "sessions": [{"sessionId": "s2", "stepId": "plan", "visit": 1}, {"sessionId": session, "stepId": "implement", "visit": 1}]
    })
}

impl Rig {
    fn has(&self, key: &str) -> bool {
        self.rt.find(key).is_some()
    }

    fn words(&self, key: &str) -> Option<String> {
        self.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text)
    }

    fn keys(&self, prefix: &str) -> Vec<String> {
        self.rt.elements().into_iter().filter_map(|e| e.key).filter(|k| k.starts_with(prefix)).collect()
    }

    /// Changes the run in the store, as an event would, and draws.
    fn change(&mut self, edit: impl FnOnce(&mut Value) + Send + 'static) {
        let session = self.session.clone();
        self.rt.update(move |s, _| {
            let mut json = run_json(&session);
            edit(&mut json);
            s.workflows.of_mut(HOME_MACHINE_KEY).upsert(Run::from_json(&json).expect("a run"));
        });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("switching {}", s.workflows_nav.stepper.switching));
    }

    fn on(&self) -> bool {
        self.rt.state.stepper_run().is_some_and(|r| r.check_with_me)
    }
}

const PUT: &str = "/api/workflows/runs/run-1/check-with-me";

// ─── WorkflowStepper.test.ts ────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn marks_the_steps_the_user_finishes_with_a_person_until_they_re_done() {
    let r = rig(|v| {
        // The open session is the Plan step, running; Design ran with the user and is done; Implement is the user's too.
        let session = v["steps"][1]["sessionId"].clone();
        v["steps"] = json!([
            {"id": "design", "title": "Design", "kind": "agent", "state": "done", "visits": 1, "enabled": true, "sessionId": "s1", "withYou": true},
            {"id": "plan", "title": "Plan", "kind": "agent", "state": "running", "visits": 1, "enabled": true, "sessionId": session},
            {"id": "implement", "title": "Implement", "kind": "agent", "state": "pending", "visits": 0, "enabled": true, "withYou": true}
        ]);
        v["sessions"] = json!([{"sessionId": "s1", "stepId": "design", "visit": 1}, {"sessionId": session, "stepId": "plan", "visit": 1}]);
        v["currentStepId"] = json!("plan");
    });
    assert_eq!(r.keys("workflow-step-with-you-"), ["workflow-step-with-you-implement"]);
}

#[test]
fn doesn_t_mark_a_step_whose_file_says_finish_agent_with_check_with_me_on() {
    let r = rig(|v| {
        let session = v["steps"][1]["sessionId"].clone();
        v["checkWithMe"] = json!(true);
        v["steps"] = json!([
            {"id": "implement", "title": "Implement", "kind": "agent", "state": "running", "visits": 1, "enabled": true, "sessionId": session, "withYou": true},
            {"id": "review", "title": "Review", "kind": "agent", "state": "pending", "visits": 0, "enabled": true, "withYou": true},
            {"id": "open-pr", "title": "Push and open the PR", "kind": "agent", "state": "pending", "visits": 0, "enabled": true, "finishAgent": true, "withYou": false}
        ]);
        v["sessions"] = json!([{"sessionId": session, "stepId": "implement", "visit": 1}]);
    });
    let marked = r.keys("workflow-step-with-you-");
    assert_eq!(marked.len(), 2);
    assert!(!marked.iter().any(|k| k.ends_with("open-pr")), "{marked:?}");
}

#[test]
fn switches_check_with_me_from_the_header() {
    let mut r = rig(|_| {});
    let mut on = run_json(&r.session);
    on["checkWithMe"] = json!(true);
    on["updatedAt"] = json!("2026-09-23T10:06:00Z");
    r.home.route("PUT", PUT, 200, on);
    assert!(!r.on());
    click(&mut r.rt, "workflow-check-with-me");
    let calls = r.home.calls_handle();
    r.wait("the switch to reach the server", move |_| calls().iter().any(|(m, p)| m == "PUT" && p == PUT));
    r.wait("the run to come back switched on", |s| s.stepper_run().is_some_and(|r| r.check_with_me) && !s.workflows_nav.stepper.switching);
    let puts = r.home.bodies("PUT", PUT);
    assert_eq!(puts, [json!({"on": true})]);
    assert!(r.on());
}

#[test]
fn says_the_change_applies_from_the_next_step_when_it_differs_from_the_running_one() {
    let mut r = rig(|v| v["checkWithMe"] = json!(true));
    assert_eq!(
        r.words("workflow-check-with-me-note").as_deref(),
        Some("Check with me is on from Review. This step started on its own, so it still moves on when the agent reports it's done.")
    );
    r.change(|v| {
        v["checkWithMe"] = json!(false);
        v["steps"][1]["withYou"] = json!(true);
        v["updatedAt"] = json!("2026-09-23T10:07:00Z");
    });
    assert_eq!(
        r.words("workflow-check-with-me-note").as_deref(),
        Some("Check with me is off from Review. You still finish this step, because it started with you.")
    );
}

#[test]
fn says_nothing_when_the_running_step_already_matches_and_has_no_switch_once_the_run_is_over() {
    let mut r = rig(|_| {});
    assert!(!r.has("workflow-check-with-me-note"));
    assert!(r.has("workflow-check-with-me"));
    r.change(|v| {
        v["status"] = json!("done");
        v["updatedAt"] = json!("2026-09-23T10:08:00Z");
    });
    assert!(!r.has("workflow-check-with-me"));
    assert!(r.has("workflow-stepper"), "the steps stay");
}

// ─── WorkflowStartedBy.test.ts ──────────────────────────────────────────────────────────────────────────────────────

#[test]
fn started_by_shows_in_the_run_s_header_above_a_step_s_conversation() {
    let mut r = rig(|_| {});
    assert!(!r.has("workflow-started-by"), "a run from the Run box says nothing");
    r.change(|v| {
        v["startedBy"] = json!({"automationId": "a-wf", "automationName": "Weekly dependency bump"});
        v["updatedAt"] = json!("2026-09-23T10:06:00Z");
    });
    let link = r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("workflow-started-by")).expect("the link");
    assert_eq!(link.label.as_deref(), Some("Started by Weekly dependency bump"));
    click(&mut r.rt, "workflow-started-by");
    assert_eq!(r.rt.state.rail, Rail::Automations);
    assert_eq!(r.rt.state.automations_nav.open_id(), Some("a-wf"));
}

// ─── What the native view keeps ─────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_session_that_is_not_a_step_has_no_stepper() {
    let mut r = rig(|_| {});
    assert!(r.has("workflow-stepper"));
    r.change(|v| {
        v["sessions"] = json!([{"sessionId": "elsewhere", "stepId": "plan", "visit": 1}]);
        v["updatedAt"] = json!("2026-09-23T10:09:00Z");
    });
    assert!(!r.has("workflow-stepper"));
}

#[test]
fn a_step_that_ran_in_another_session_opens_it_and_this_one_and_steps_with_no_session_do_not() {
    let mut r = rig(|_| {});
    let other = r.rt.state.projects[0].sessions.iter().map(|s| s.id.clone()).find(|id| *id != r.session).expect("another session");
    let ran = other.clone();
    r.change(move |v| v["steps"][0]["sessionId"] = json!(ran));
    // This session's own step and a step that hasn't started open nothing.
    click(&mut r.rt, "workflow-step-implement");
    click(&mut r.rt, "workflow-step-review");
    assert_eq!(r.rt.state.session().id, r.session);
    // Plan ran in the other session, which the list holds: it opens.
    click(&mut r.rt, "workflow-step-plan");
    assert_eq!(r.rt.state.session().id, other);
}

#[test]
fn a_refusal_shows_under_the_switch_and_the_switch_can_be_pressed_again() {
    let mut r = rig(|_| {});
    r.home.route("PUT", PUT, 500, json!({"error": "The run has moved on."}));
    click(&mut r.rt, "workflow-check-with-me");
    r.wait("the refusal", |s| s.stepper_error().is_some() && !s.workflows_nav.stepper.switching);
    r.rt.frame(FRAME);
    assert_eq!(r.words("workflow-stepper-error").as_deref(), Some("The run has moved on."));
    assert!(!r.on(), "the switch stays where it was");
    click(&mut r.rt, "workflow-check-with-me");
    assert!(r.rt.state.stepper_error().is_none(), "a new try clears the words");
    r.wait("the second refusal", |s| s.stepper_error().is_some());
    assert_eq!(r.home.bodies("PUT", PUT).len(), 2);
}

#[test]
fn the_prompts_fleet_sent_into_a_step_wear_the_workflow_mark() {
    let mut r = rig(|_| {});
    r.rt.update(|s, _| s.show_workflow_stepper(true, Case::Mark));
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    assert!(r.has("workflow-step-prompt"), "the mark is drawn above the prompt the step started with");
    let mine = r.rt.state.workflow_mark(PROMPT, "anything");
    assert_eq!(mine.as_deref(), Some("Workflow \u{b7} step 2 of 4"));
    assert_eq!(r.rt.state.workflow_mark(WRAP_UP, "anything").as_deref(), Some("Fleet \u{b7} you pressed Move on"));
    assert_eq!(r.rt.state.workflow_mark("a-message-the-user-typed", "Add a test."), None);
    // With the run gone (Workflows off, another machine) nothing is marked.
    r.rt.update(|s, _| *s.workflows.of_mut(HOME_MACHINE_KEY) = Default::default());
    assert_eq!(r.rt.state.workflow_mark(PROMPT, "anything"), None);
}
