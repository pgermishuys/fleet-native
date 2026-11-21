//! A step session of an invented run for the finish bar's scenes and tests (component 18, piece H): the open session is a step
//! the user finishes, in each way the bar draws.
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::Run;
use crate::view::conversation::{Message, Part};
use serde_json::{Value, json};

/// Which state a scene shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// Design, one way on, two files handed on.
    Design,
    /// Design with the note typed.
    Note,
    /// Review, two outcomes, no files.
    Outcomes,
    /// Review with the loop's maximum used up.
    Blocked,
    /// Design after Move on: the agent wraps up.
    Wrapping,
}

pub const FILES: [&str; 2] = [".weave/design/pager.md", ".weave/design/pager.html"];

fn step(id: &str, title: &str, kind: &str, state: &str, extra: Value) -> Value {
    let mut v = json!({"id": id, "title": title, "kind": kind, "state": state, "enabled": true, "visits": 0});
    v.as_object_mut().expect("an object").extend(extra.as_object().cloned().unwrap_or_default());
    v
}

/// The run, with `session` as the session of the step the user finishes.
pub fn run(session: &str, case: Case) -> Run {
    Run::from_json(&json(session, case)).expect("a run")
}

/// `run` as the server sends it.
pub fn json(session: &str, case: Case) -> Value {
    let review = matches!(case, Case::Outcomes | Case::Blocked);
    let spent = u32::from(case == Case::Blocked) * 2;
    let (step_id, title, files, moves) = if review {
        (
            "review",
            "Review",
            json!([]),
            json!([
                {"outcome": "pass", "to": "open-pr", "toTitle": "Open the pull request", "back": false, "loopsUsed": null, "maxLoops": null, "allowed": true},
                {"outcome": "changes", "to": "implement", "toTitle": "Implement", "back": true, "loopsUsed": spent, "maxLoops": 2, "allowed": spent < 2}
            ]),
        )
    } else {
        (
            "design",
            "Design",
            json!(FILES),
            json!([{"outcome": "ready", "to": "plan", "toTitle": "Plan", "back": false, "loopsUsed": null, "maxLoops": null, "allowed": true}]),
        )
    };
    let wrapping = case == Case::Wrapping;
    json!({
        "id": "run-finish", "workflowId": "builtin:build-a-feature", "workflowName": "Build a feature", "title": "Pager page size",
        "request": "Make the page size a setting", "status": "running", "currentStepId": step_id,
        "result": null, "repositoryPath": "/work/kettle", "branch": "fleet/page-size", "baseBranch": "main", "worktreePath": null,
        "harnessType": "opencode", "createdAt": "2026-11-19T10:00:00Z", "updatedAt": "2026-11-19T10:30:00Z", "endedAt": null,
        "checkWithMe": review,
        "steps": [
            step("design", "Design", "agent", if review { "done" } else { "running" }, json!({"visits": 1, "finishYou": !review, "withYou": !review, "sessionId": if review { Value::Null } else { json!(session) }, "outcomes": ["ready"]})),
            step("plan", "Plan", "agent", "pending", json!({})),
            step("implement", "Implement", "agent", if review { "done" } else { "pending" }, json!({"visits": u32::from(review)})),
            step("review", "Review", "agent", if review { "running" } else { "pending" }, json!({"visits": u32::from(review), "withYou": review, "maxLoops": 2, "sessionId": if review { json!(session) } else { Value::Null }, "outcomes": ["pass", "changes"]})),
            step("open-pr", "Open the pull request", "you", "pending", json!({}))
        ],
        "sessions": [{"sessionId": session, "stepId": step_id, "visit": 1, "withYou": true, "files": files}],
        "withYou": {
            "stepId": step_id, "stepTitle": title, "sessionId": session, "files": files, "wrappingUp": wrapping,
            "outcome": if wrapping { json!("ready") } else { Value::Null }, "moves": moves
        },
        "waiting": null
    })
}

impl crate::State {
    pub fn show_workflow_finish(&mut self, dark: bool, case: Case) {
        self.dark = Some(dark);
        let reply = Message::assistant(
            "m-a1",
            vec![Part::text("m-a1-t", "The design is written to `.weave/design/pager.md`, with a page that shows the new layout.")],
        );
        self.show_conversation(vec![Message::user("m-u1", "Sketch how the pager should show its page size."), reply]);
        let id = self.session().id.clone();
        self.workflows.of_mut(HOME_MACHINE_KEY).upsert(run(&id, case));
        if case == Case::Note {
            self.workflows_nav.finish.set_note("Keep the status-bar shortcuts in the sheet.");
        }
    }
}
