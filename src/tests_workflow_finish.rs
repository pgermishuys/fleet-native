//! Component 18, piece H: the finish bar above the composer. Fleet's `WorkflowFinishBar.test.ts` (5) under their names, then what
//! the native view keeps: the buttons are off while a press is on its way, a refusal shows, another step starts the bar again.
//! A fake Fleet answers; every wait is for a request to arrive or for the state to show it.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::Run;
use crate::mock::workflow_finish::{Case, json as run_json};
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
    session: String,
}

fn rig(case: Case) -> Rig {
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
    let json = run_json(&session, case);
    rt.update(move |s, _| {
        s.workflows.of_mut(HOME_MACHINE_KEY).upsert(Run::from_json(&json).expect("a run"));
    });
    let mut r = Rig { rt, queue, home, session };
    r.frames();
    r
}

const MOVE_ON: &str = "/api/workflows/runs/run-finish/move-on";
const END: &str = "/api/workflows/runs/run-finish/end";

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

    fn off(&self, key: &str) -> bool {
        let e = self.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).expect(key);
        e.opacity < 1.0 && !e.focusable
    }

    fn type_note(&mut self, words: &str) {
        let words = words.to_string();
        self.rt.update(move |s, _| s.workflows_nav.finish.note = rune::TextEdit::new(words));
        self.frames();
    }

    /// Waits for the request of `method` and `path` to reach the fake, then for its answer to be applied.
    fn sent(&mut self, method: &'static str, path: &'static str) -> Vec<Value> {
        let calls = self.home.calls_handle();
        test_wait::drive(
            &mut self.rt,
            &self.queue,
            "the request to reach the server",
            move |_| calls().iter().any(|(m, p)| m == method && p == path),
            |s| format!("busy {:?}", s.workflows_nav.finish.busy),
        );
        test_wait::drive(
            &mut self.rt,
            &self.queue,
            "the answer to be applied",
            |s| s.workflows_nav.finish.busy.is_none(),
            |s| format!("busy {:?}", s.workflows_nav.finish.busy),
        );
        self.frames();
        self.home.bodies(method, path)
    }
}

/// The run as the server answers once it has moved on: the step the user finished is over.
fn after(case: Case) -> Value {
    let mut v = run_json("gone", case);
    v["withYou"] = Value::Null;
    v["updatedAt"] = json!("2026-11-19T10:31:00Z");
    v
}

// ─── WorkflowFinishBar.test.ts ──────────────────────────────────────────────────────────────────────────────────────

#[test]
fn lets_the_user_finish_design_a_note_and_move_on_to_plan_with_the_files_it_hands_on() {
    let mut r = rig(Case::Design);
    let all: Vec<String> = r.rt.elements().into_iter().filter_map(|e| e.text).collect();
    let says = |words: &str| all.iter().any(|t| t == words);
    assert_eq!(r.label("workflow-finish-bar").as_deref(), Some("You finish Design"));
    assert!(says("You finish Design"));
    assert!(says("\u{b7} The agent can't end this step. Keep talking until it's right."));
    assert!(says(".weave/design/pager.md") && says(".weave/design/pager.html"));
    assert!(all.iter().any(|t| t.ends_with("Then Plan starts with the files, the summary and your note.")));
    assert_eq!(r.label("workflow-finish-note").as_deref(), Some("Note for Plan"));

    r.type_note("Keep the status-bar shortcuts in the sheet.");
    assert_eq!(r.label("workflow-move-ready").as_deref(), Some("Move on to Plan"));
    r.home.route("POST", MOVE_ON, 200, after(Case::Design));
    click(&mut r.rt, "workflow-move-ready");
    let bodies = r.sent("POST", MOVE_ON);
    assert_eq!(bodies, [json!({"outcome": null, "note": "Keep the status-bar shortcuts in the sheet."})]);
    assert!(!r.has("workflow-finish-bar"), "the bar goes once the run has moved on");
}

#[test]
fn offers_one_button_per_outcome_and_says_how_many_times_changes_has_been_used() {
    let mut r = rig(Case::Outcomes);
    let all: Vec<String> = r.rt.elements().into_iter().filter_map(|e| e.text).collect();
    assert!(all.iter().any(|t| t == "You finish Review"));
    assert!(all.iter().any(|t| t == "\u{b7} Pick the outcome. The run goes where it leads."));
    assert!(all.iter().any(|t| t.starts_with("This step declares no files.")));
    assert_eq!(r.label("workflow-move-pass").as_deref(), Some("Pass: on to Open the pull request"));
    assert_eq!(r.label("workflow-move-changes").as_deref(), Some("Changes: back to Implement (1 of 2)"));

    r.home.route("POST", MOVE_ON, 200, after(Case::Outcomes));
    click(&mut r.rt, "workflow-move-changes");
    assert_eq!(r.sent("POST", MOVE_ON), [json!({"outcome": "changes", "note": null})]);
}

#[test]
fn says_why_changes_is_off_once_the_loop_s_maximum_is_used_keeps_pass_and_offers_to_end_the_run() {
    let mut r = rig(Case::Blocked);
    assert!(r.off("workflow-move-changes"));
    assert_eq!(r.label("workflow-move-changes").as_deref(), Some("Changes: back to Implement (2 of 2 used)"));
    assert!(!r.off("workflow-move-pass"));
    assert!(r.has("workflow-finish-blocked"));
    let all: Vec<String> = r.rt.elements().into_iter().filter_map(|e| e.text).collect();
    assert!(all.iter().any(|t| t == "Review has sent the work back twice, the most this run allows. Pick another outcome, or "));
    r.home.route("POST", END, 200, after(Case::Blocked));
    click(&mut r.rt, "workflow-finish-end");
    r.sent("POST", END);
    assert!(r.home.workflow_calls().iter().all(|(_, p, _)| p != MOVE_ON), "Changes sent nothing");
}

#[test]
fn says_what_happens_while_the_agent_wraps_up() {
    let r = rig(Case::Wrapping);
    assert!(!r.has("workflow-finish-bar"));
    assert_eq!(
        r.label("workflow-wrapping-up").as_deref(),
        Some(
            "Wrapping up Design: the agent is updating its files and writing a summary for Plan. Then Fleet checks the files and starts Plan."
        )
    );
}

#[test]
fn shows_nothing_in_a_step_the_agent_finishes_or_in_another_session() {
    let mut r = rig(Case::Design);
    let session = r.session.clone();
    r.rt.update(move |s, _| {
        let mut v = run_json(&session, Case::Design);
        v["withYou"] = Value::Null;
        v["updatedAt"] = json!("2026-11-19T10:32:00Z");
        s.workflows.of_mut(HOME_MACHINE_KEY).upsert(Run::from_json(&v).expect("a run"));
    });
    r.frames();
    assert!(!r.has("workflow-finish-bar") && !r.has("workflow-wrapping-up"));

    let session = r.session.clone();
    r.rt.update(move |s, _| {
        let mut v = run_json(&session, Case::Design);
        v["withYou"]["sessionId"] = json!("another-session");
        v["updatedAt"] = json!("2026-11-19T10:33:00Z");
        s.workflows.of_mut(HOME_MACHINE_KEY).upsert(Run::from_json(&v).expect("a run"));
    });
    r.frames();
    assert!(!r.has("workflow-finish-bar") && !r.has("workflow-wrapping-up"));
}

// ─── What the native view keeps ─────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_pressed_button_waits_and_everything_is_off_meanwhile() {
    let mut r = rig(Case::Outcomes);
    r.home.hold("POST", MOVE_ON);
    click(&mut r.rt, "workflow-move-pass");
    let calls = r.home.calls_handle();
    test_wait::drive(
        &mut r.rt,
        &r.queue,
        "the request to reach the server",
        move |_| calls().iter().any(|(_, p)| p == MOVE_ON),
        |_| String::new(),
    );
    assert_eq!(r.rt.state.workflows_nav.finish.busy.as_deref(), Some("pass"));
    for key in ["workflow-move-pass", "workflow-move-changes"] {
        assert!(r.off(key), "{key} is off while the request is on its way");
    }
    let note = r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("workflow-finish-note")).expect("the note");
    assert!(note.opacity < 1.0, "the note is dimmed while the request is on its way");
    click(&mut r.rt, "workflow-move-changes");
    assert_eq!(r.rt.state.workflows_nav.finish.busy.as_deref(), Some("pass"), "a second press sends nothing");
}

#[test]
fn a_refusal_shows_under_the_bar_and_the_note_stays() {
    let mut r = rig(Case::Design);
    r.type_note("Keep it short.");
    r.home.route("POST", MOVE_ON, 409, json!({"error": "The run has moved on."}));
    click(&mut r.rt, "workflow-move-ready");
    r.sent("POST", MOVE_ON);
    assert_eq!(r.words("workflow-finish-error").as_deref(), Some("The run has moved on."));
    assert_eq!(r.rt.state.workflows_nav.finish.note.text, "Keep it short.");
    assert!(r.has("workflow-finish-bar"), "the bar stays for another try");
}

#[test]
fn another_step_starts_the_bar_again() {
    let mut r = rig(Case::Design);
    r.type_note("For Plan.");
    r.rt.update(|s, _| s.workflows_nav.finish.error = Some("old".into()));
    let session = r.session.clone();
    r.rt.update(move |s, _| {
        let mut v = run_json(&session, Case::Design);
        v["withYou"]["stepId"] = json!("plan");
        v["updatedAt"] = json!("2026-11-19T10:34:00Z");
        s.workflows.of_mut(HOME_MACHINE_KEY).upsert(Run::from_json(&v).expect("a run"));
    });
    r.frames();
    assert_eq!(r.rt.state.workflows_nav.finish.note.text, "");
    assert!(r.rt.state.workflows_nav.finish.error.is_none());
}
