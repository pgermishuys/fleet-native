//! Component 18, piece G: the run card above the composer. Fleet's `WorkflowRunCard.test.ts` (12) under their names, then what
//! the native view keeps: the card starts again for a new wait, End run, the busy button, the line that says where the run went
//! and the link in it. A fake Fleet answers; every wait is for a request to arrive or for the state to show it.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::Run;
use crate::mock::workflow_card::{Case, PLAN, json as run_json};
use crate::tests::{FRAME, app, click};
use crate::tests_workflow_runs::{Fake, fake};
use crate::{State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    home: Fake,
    /// The open session: the run's Plan step.
    session: String,
}

/// The app on a fake Fleet, with the Plan step's session open and the run of `case` (after `edit`) in the store.
fn rig(case: Case, edit: impl FnOnce(&mut Value) + Send + 'static) -> Rig {
    let home = fake();
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link =
        Link::start_machine(HOME_MACHINE_KEY.to_string(), home.base.clone(), Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.show_run_session(true);
    });
    let session = rt.state.session().id.clone();
    let mut json = run_json(&session, case);
    edit(&mut json);
    rt.update(move |s, _| {
        s.workflows.of_mut(HOME_MACHINE_KEY).upsert(Run::from_json(&json).expect("a run"));
    });
    let mut r = Rig { rt, queue, home, session };
    r.frames();
    r
}

const ANSWER: &str = "/api/workflows/runs/run-card/answer";

impl Rig {
    fn frames(&mut self) {
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    fn has(&self, key: &str) -> bool {
        self.rt.find(key).is_some()
    }

    fn words(&self, key: &str) -> Option<String> {
        self.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text)
    }

    fn label(&self, key: &str) -> Option<String> {
        self.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.label)
    }

    /// Changes the run in the store, as an event would, and draws.
    fn change(&mut self, case: Case, edit: impl FnOnce(&mut Value) + Send + 'static) {
        let session = self.session.clone();
        self.rt.update(move |s, _| {
            let mut json = run_json(&session, case);
            edit(&mut json);
            s.workflows.of_mut(HOME_MACHINE_KEY).upsert(Run::from_json(&json).expect("a run"));
        });
        self.frames();
    }

    /// Waits for the request of `method` and `path` to reach the fake, then for the answer to be applied.
    fn sent(&mut self, method: &'static str, path: &'static str) -> Vec<Value> {
        let calls = self.home.calls_handle();
        test_wait::drive(
            &mut self.rt,
            &self.queue,
            "the request to reach the server",
            move |_| calls().iter().any(|(m, p)| m == method && p == path),
            |s| format!("busy {:?}", s.workflows_nav.card.busy),
        );
        test_wait::drive(
            &mut self.rt,
            &self.queue,
            "the answer to be applied",
            |s| s.workflows_nav.card.busy.is_none(),
            |s| format!("busy {:?}", s.workflows_nav.card.busy),
        );
        self.frames();
        self.home.bodies(method, path)
    }

    fn type_note(&mut self, words: &str) {
        let words = words.to_string();
        self.rt.update(move |s, _| s.workflows_nav.card.note = rune::TextEdit::new(words));
        self.frames();
    }
}

/// The run as the server answers once the answer is in: running, nothing waits.
fn moved_on(session: &str, case: Case) -> Value {
    let mut v = run_json(session, case);
    v["status"] = json!("running");
    v["waiting"] = Value::Null;
    v["updatedAt"] = json!("2026-11-19T10:31:00Z");
    v
}

// ─── WorkflowRunCard.test.ts ────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn asks_the_you_step_s_question_in_the_session_before_it_and_approves() {
    let mut r = rig(Case::You, |_| {});
    assert_eq!(r.words("workflow-card-ask").as_deref(), Some("Build it this way? page-size.md is open on the right."));
    assert_eq!(r.label("workflow-card").as_deref(), Some("Approve the plan"));
    r.home.route("POST", ANSWER, 200, moved_on(&r.session, Case::You));
    click(&mut r.rt, "workflow-card-choice-0");
    let bodies = r.sent("POST", ANSWER);
    assert_eq!(bodies, [json!({"choice": "choice:0", "note": null})]);
    assert!(!r.has("workflow-card"), "the card goes once the run has moved on");
}

#[test]
fn sends_the_plan_back_with_the_note_the_user_wrote() {
    let mut r = rig(Case::You, |_| {});
    r.home.route("POST", ANSWER, 200, moved_on(&r.session, Case::You));
    click(&mut r.rt, "workflow-card-choice-1");
    assert!(r.has("workflow-card-note"), "a choice that takes a note asks for it first");
    assert!(r.home.workflow_calls().iter().all(|(_, p, _)| p != ANSWER), "nothing is sent yet");
    let send = r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("workflow-card-send-note")).expect("the button");
    assert!(send.opacity < 1.0 && !send.focusable, "Send back waits for words");
    r.type_note("Leave the status bar alone.");
    click(&mut r.rt, "workflow-card-send-note");
    let bodies = r.sent("POST", ANSWER);
    assert_eq!(bodies, [json!({"choice": "choice:1", "note": "Leave the status bar alone."})]);
}

#[test]
fn offers_two_ways_to_approve_and_sends_how_involved_the_user_wants_to_be() {
    let mut r = rig(Case::Involvement, |_| {});
    assert_eq!(
        r.words("workflow-card-ask").as_deref(),
        Some(
            "Build it this way? page-size.md is open on the right. Then choose how involved you want to be in the rest of the run; you can change it later from the header."
        )
    );
    assert_eq!(r.label("workflow-card-let-it-run").as_deref(), Some("Approve, let it run"));
    assert!(
        r.rt.elements()
            .into_iter()
            .any(|e| e.text.as_deref() == Some("Implement and Review go ahead on their own. You're asked again at Open the pull request."))
    );
    assert_eq!(r.label("workflow-card-check-with-me").as_deref(), Some("Approve, check with me after each step"));
    assert_eq!(r.label("workflow-card-choice-1").as_deref(), Some("Send back with a note"));
    assert!(r.has("workflow-card-end"));
    assert!(!r.has("workflow-card-choice-0"), "the choice with a say in involvement is offered twice, not once more");
    r.home.route("POST", ANSWER, 200, moved_on(&r.session, Case::Involvement));
    click(&mut r.rt, "workflow-card-check-with-me");
    assert_eq!(r.sent("POST", ANSWER), [json!({"choice": "choice:0", "note": null, "checkWithMe": true})]);
}

#[test]
fn approves_and_lets_the_rest_run() {
    let mut r = rig(Case::Involvement, |_| {});
    r.home.route("POST", ANSWER, 200, moved_on(&r.session, Case::Involvement));
    click(&mut r.rt, "workflow-card-let-it-run");
    assert_eq!(r.sent("POST", ANSWER), [json!({"choice": "choice:0", "note": null, "checkWithMe": false})]);
}

#[test]
fn opens_the_plan_next_to_approve_the_plan_once() {
    let mut r = rig(Case::You, |_| {});
    // The panel opens beside the card with the plan in it, kept.
    assert!(r.rt.state.content_open);
    let tabs = r.rt.state.open_canvases();
    let plans: Vec<_> = tabs.canvases.iter().filter_map(|c| c.file.as_ref()).filter(|f| f.path == PLAN).collect();
    assert_eq!(plans.len(), 1);
    assert!(!plans[0].preview, "kept, not a preview");
    // The user closes the panel; seeing the session again (another frame, leaving and coming back) doesn't open it again.
    r.rt.update(|s, _| s.set_content_open(false));
    r.frames();
    r.rt.update(|s, _| s.press_rail(crate::Rail::Fleet));
    r.frames();
    r.rt.update(|s, _| s.press_rail(crate::Rail::Sessions));
    r.frames();
    assert!(!r.rt.state.content_open, "once per run, step and visit");
}

#[test]
fn offers_to_move_on_anyway_past_a_missing_file() {
    let mut r = rig(Case::NoOutcome, |v| {
        v["currentStepId"] = json!("design");
        v["waiting"] = json!({
            "kind": "missing-files", "stepId": "design", "stepTitle": "Design", "question": null,
            "message": "Design declares docs/design/sheet.html, but it isn't in the run's worktree. Reply to the agent in its session, or move on anyway.",
            "sessionId": v["waiting"]["sessionId"].clone(),
            "choices": [{"id": "move-on-anyway", "label": "Move on anyway", "note": false, "to": null}],
            "files": ["docs/design/sheet.md", "docs/design/sheet.html"]
        });
    });
    let words = r.words("workflow-card-ask").expect("the ask");
    assert!(words.starts_with("Design declares docs/design/sheet.html, but it isn't in the run's worktree."), "{words}");
    assert!(r.rt.elements().into_iter().any(|e| e.text.as_deref() == Some("Design needs you")));
    assert_eq!(r.label("workflow-card-choice-0").as_deref(), Some("Move on anyway"));
    let mut done = moved_on(&r.session, Case::NoOutcome);
    done["waiting"] = Value::Null;
    r.home.route("POST", ANSWER, 200, done);
    click(&mut r.rt, "workflow-card-choice-0");
    assert_eq!(r.sent("POST", ANSWER), [json!({"choice": "move-on-anyway", "note": null})]);
}

/// A run that moved on: this session's step is over, the next step has its own session.
fn over(files: Value, extra: Value) -> impl FnOnce(&mut Value) + Send + 'static {
    move |v| {
        let mine = &mut v["sessions"][0];
        mine["files"] = files;
        mine["filesChecked"] = json!(true);
        mine["filesCommit"] = Value::Null;
        for (k, x) in extra.as_object().cloned().unwrap_or_default() {
            mine[k] = x;
        }
    }
}

#[test]
fn says_the_files_were_checked_once_the_run_moved_on() {
    let r = rig(Case::Resolved, over(json!(["docs/design/sheet.md"]), json!({})));
    assert_eq!(r.words("workflow-card-files-checked").as_deref(), Some("Files checked: sheet.md"));
    assert!(r.rt.elements().into_iter().any(|e| e.text.as_deref() == Some("Plan finished: ready.")));
}

#[test]
fn says_fleet_committed_the_files_it_checked() {
    let r = rig(Case::Resolved, over(json!(["docs/design/sheet.md", "docs/design/sheet.html"]), json!({"filesCommit": "a1b2c3d"})));
    assert_eq!(r.words("workflow-card-files-checked").as_deref(), Some("Files checked and committed as a1b2c3d: sheet.md and sheet.html"));
    assert!(!r.has("workflow-card-commit-failed"));
}

#[test]
fn says_why_fleet_couldn_t_commit_the_files_and_the_run_still_moved_on() {
    let r = rig(
        Case::Resolved,
        over(
            json!(["docs/design/sheet.md"]),
            json!({"filesCommit": null, "filesCommitError": "no email was given and auto-detection is disabled"}),
        ),
    );
    assert_eq!(r.words("workflow-card-files-checked").as_deref(), Some("Files checked: sheet.md"));
    assert_eq!(
        r.words("workflow-card-commit-failed").as_deref(),
        Some("Fleet couldn't commit it: no email was given and auto-detection is disabled")
    );
    assert!(r.has("workflow-card-next"), "and where the run is now");
}

#[test]
fn shows_nothing_in_a_session_the_run_isn_t_waiting_in() {
    let mut r = rig(Case::You, |_| {});
    assert!(r.has("workflow-card"));
    r.change(Case::You, |v| {
        v["waiting"]["sessionId"] = json!("another-session");
        v["updatedAt"] = json!("2026-11-19T10:32:00Z");
    });
    assert!(!r.has("workflow-card"));
    assert!(!r.has("workflow-card-resolved"), "this step has no outcome yet");
}

#[test]
fn waits_on_a_step_whose_skill_was_turned_off_with_retry_start_without_it_and_a_way_to_settings_skills_tools() {
    let mut r = rig(Case::SkillOff, |_| {});
    assert!(r.rt.elements().into_iter().any(|e| e.text.as_deref() == Some("Plan needs you")));
    assert_eq!(r.words("workflow-card-ask").as_deref(), Some("Plan uses write-a-plan, which is now off."));
    assert_eq!(r.label("workflow-card-choice-0").as_deref(), Some("Retry"));
    assert_eq!(r.label("workflow-card-choice-1").as_deref(), Some("Start without it"));
    assert!(r.has("workflow-card-skill-off"));
    click(&mut r.rt, "workflow-card-skills-link");
    assert_eq!(r.rt.state.rail, crate::Rail::Settings);
    assert_eq!(r.rt.state.settings.page, crate::fleet::settings_nav::Page::Skills);
    // Back in the session, Start without it is sent.
    // (The invented session is not restored by the rail, as a real one is: show it again.)
    r.rt.update(|s, _| {
        s.press_rail(crate::Rail::Sessions);
        s.show_run_session(true);
    });
    r.frames();
    assert_eq!(r.rt.state.session().id, r.session);
    assert!(r.has("workflow-card"), "the card is back");
    r.home.route("POST", ANSWER, 200, moved_on(&r.session, Case::SkillOff));
    click(&mut r.rt, "workflow-card-choice-1");
    assert_eq!(r.sent("POST", ANSWER), [json!({"choice": "without-skill", "note": null})]);
}

#[test]
fn says_a_skill_that_s_still_off_from_the_server_when_retry_is_pressed_too_soon() {
    let mut r = rig(Case::SkillOff, |_| {});
    let words = "write-a-plan is still off. Turn it on in Settings \u{2192} Skills & tools, then retry.";
    r.home.route("POST", ANSWER, 400, json!({"error": words}));
    click(&mut r.rt, "workflow-card-choice-0");
    r.sent("POST", ANSWER);
    assert_eq!(r.words("workflow-card-error").as_deref(), Some(words));
    assert!(r.has("workflow-card"), "the card stays for another try");
}

// ─── What the native view keeps ─────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_pressed_button_waits_and_every_button_is_off_meanwhile() {
    let mut r = rig(Case::You, |_| {});
    r.home.hold("POST", ANSWER);
    click(&mut r.rt, "workflow-card-choice-0");
    let calls = r.home.calls_handle();
    test_wait::drive(
        &mut r.rt,
        &r.queue,
        "the request to reach the server",
        move |_| calls().iter().any(|(_, p)| p == ANSWER),
        |_| String::new(),
    );
    assert_eq!(r.rt.state.workflows_nav.card.busy.as_deref(), Some("choice:0"));
    for key in ["workflow-card-choice-0", "workflow-card-choice-1", "workflow-card-end"] {
        let e = r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).expect(key);
        assert!(e.opacity < 1.0 && !e.focusable, "{key} is off while the answer is on its way");
    }
    // A second press sends nothing.
    click(&mut r.rt, "workflow-card-choice-0");
    assert_eq!(r.home.bodies("POST", ANSWER).len(), 1);
    r.home.route("POST", ANSWER, 200, moved_on(&r.session, Case::You));
    test_wait::drive(&mut r.rt, &r.queue, "the answer", |s| s.workflows_nav.card.busy.is_none(), |_| String::new());
}

#[test]
fn end_run_ends_the_run_and_a_refusal_shows_under_the_card() {
    let mut r = rig(Case::You, |_| {});
    const END: &str = "/api/workflows/runs/run-card/end";
    r.home.route("POST", END, 500, json!({}));
    click(&mut r.rt, "workflow-card-end");
    r.sent("POST", END);
    assert_eq!(r.words("workflow-card-error").as_deref(), Some("Couldn't end the run."));
    let mut ended = moved_on(&r.session, Case::You);
    ended["status"] = json!("ended");
    r.home.route("POST", END, 200, ended);
    click(&mut r.rt, "workflow-card-end");
    test_wait::drive(
        &mut r.rt,
        &r.queue,
        "the run to end",
        |s| s.stepper_run().is_some_and(|r| r.status == crate::fleet::workflows::RunStatus::Ended),
        |_| String::new(),
    );
    r.frames();
    assert!(!r.has("workflow-card"));
}

#[test]
fn a_new_wait_clears_the_note_its_choice_and_the_error() {
    let mut r = rig(Case::You, |_| {});
    r.home.route("POST", ANSWER, 400, json!({"error": "The run has moved on."}));
    click(&mut r.rt, "workflow-card-choice-1");
    r.type_note("Use a smaller page.");
    click(&mut r.rt, "workflow-card-send-note");
    r.sent("POST", ANSWER);
    assert!(r.words("workflow-card-error").is_some());
    assert_eq!(r.rt.state.workflows_nav.card.note_for.as_deref(), Some("choice:1"), "a refusal keeps the note");
    // The run waits at another step in this session.
    r.change(Case::You, |v| {
        v["waiting"]["stepId"] = json!("ok-plan-2");
        v["updatedAt"] = json!("2026-11-19T10:33:00Z");
    });
    assert!(!r.has("workflow-card-error"));
    assert!(!r.has("workflow-card-note"));
    assert!(r.rt.state.workflows_nav.card.note.text.is_empty());
}

#[test]
fn the_resolved_line_opens_the_session_the_run_is_in_now() {
    let mut r = rig(Case::Resolved, |_| {});
    let other = r.rt.state.projects[0].sessions.iter().map(|s| s.id.clone()).find(|id| *id != r.session).expect("another session");
    let next = other.clone();
    r.change(Case::Resolved, move |v| v["sessions"][1]["sessionId"] = json!(next));
    assert_eq!(r.label("workflow-card-next").as_deref(), Some("Implement"));
    click(&mut r.rt, "workflow-card-next");
    assert_eq!(r.rt.state.session().id, other);
}

#[test]
fn a_finished_run_says_it_is_done() {
    let r = rig(Case::Resolved, |v| {
        v["status"] = json!("done");
        v["result"] = json!("Opened pull request 12.");
        v["sessions"].as_array_mut().unwrap().truncate(1);
        v["sessions"][0]["outcome"] = json!("ready");
    });
    assert!(r.rt.elements().into_iter().any(|e| e.text.as_deref() == Some("Plan finished: ready. Opened pull request 12.")));
    assert!(!r.has("workflow-card-next"));
}
