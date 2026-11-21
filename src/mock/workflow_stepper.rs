//! A step session of an invented run for the stepper's scenes and tests (component 18, piece F): the open session is the run's
//! Fix step, the Diagnose step ran before it, a review and the PR are still to come.
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::Run;
use crate::view::conversation::{Message, Part};
use serde_json::{Value, json};

/// Which state a scene shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// Check with me off; an automation started the run.
    Stepper,
    /// Check with me switched on while the running step started on its own: the note says from when it applies.
    Note,
    /// The prompts Fleet sent into the step wear the workflow mark.
    Mark,
}

pub const PROMPT: &str = "m-prompt";
pub const WRAP_UP: &str = "m-wrap-up";

/// The run, with `session` as its Fix step's session.
pub fn run(session: &str, check_with_me: bool) -> Run {
    let step = |id: &str, title: &str, state: &str, extra: Value| {
        let mut v = json!({"id": id, "title": title, "kind": "agent", "state": state, "enabled": true, "visits": 0});
        v.as_object_mut().expect("an object").extend(extra.as_object().cloned().unwrap_or_default());
        v
    };
    let v = json!({
        "id": "run-stepper", "workflowId": "builtin:fix-a-bug", "workflowName": "Fix a bug", "title": "The pager skips the last page",
        "request": "The pager skips the last page", "status": "running", "currentStepId": "fix", "result": null,
        "repositoryPath": "/work/kettle", "branch": "fleet/pager-last-page", "baseBranch": "main", "worktreePath": null,
        "harnessType": "opencode", "createdAt": "2026-11-19T10:00:00Z", "updatedAt": "2026-11-19T10:30:00Z", "endedAt": null,
        "checkWithMe": check_with_me,
        "steps": [
            step("diagnose", "Diagnose", "done", json!({"visits": 1, "sessionId": "s-diagnose", "outcome": "cause found"})),
            step("fix", "Fix", "running", json!({"visits": 2, "sessionId": session})),
            step("review", "Review", "pending", json!({"withYou": true, "maxLoops": 2})),
            step("open-pr", "Open the PR", "pending", json!({})),
        ],
        "sessions": [
            {"sessionId": "s-diagnose", "stepId": "diagnose", "visit": 1},
            {"sessionId": session, "stepId": "fix", "visit": 2, "promptMessageId": PROMPT, "wrapUpMessageId": WRAP_UP},
        ],
        "startedBy": {"automationId": "a-nightly", "automationName": "Nightly sweep"},
    });
    Run::from_json(&v).expect("a run")
}

impl crate::State {
    pub fn show_workflow_stepper(&mut self, dark: bool, case: Case) {
        self.dark = Some(dark);
        let reply =
            Message::assistant("m-a1", vec![Part::text("m-a1-t", "I'll count the pages with a ceiling division instead of a floor.")]);
        self.show_conversation(if case == Case::Mark {
            vec![
                Message::user(PROMPT, "Fix the pager so the last page shows when the total divides evenly by the page size."),
                reply,
                Message::user(WRAP_UP, "Bring the notes up to date with what we agreed, and write a short summary for the next step."),
            ]
        } else {
            vec![reply]
        });
        let id = self.session().id.clone();
        self.workflows.of_mut(HOME_MACHINE_KEY).upsert(run(&id, case == Case::Note));
    }
}
