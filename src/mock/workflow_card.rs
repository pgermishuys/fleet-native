//! A step session of an invented run for the run card's scenes and tests (component 18, piece G): the open session is the
//! Plan step of a feature, and the run waits for the user in it, in each way the card asks.
use crate::fleet::canvases::{FileView, OpenFile};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::Run;
use crate::view::conversation::{Message, Part};
use serde_json::{Value, json};

/// Which state a scene shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// A You step's question, with the plan open on the right.
    You,
    /// The same, with the two ways to go on.
    Involvement,
    /// "Send back with a note" pressed: the note field with its text.
    Note,
    /// The step stopped without choosing an outcome.
    NoOutcome,
    /// The step's skill was turned off.
    SkillOff,
    /// The plan was approved and the run moved on; this session says where it went.
    Resolved,
}

pub const PLAN: &str = ".weave/plans/page-size.md";
const PLAN_TEXT: &str = "# Page size\n\nMake the pager's page size a setting.\n\n## Steps\n\n1. Read the size from the config, with 20 as the default.\n2. Pass it to `paginate` instead of the constant.\n3. Add a test for a size that divides the rows evenly.\n";

fn step(id: &str, title: &str, kind: &str, state: &str, extra: Value) -> Value {
    let mut v = json!({"id": id, "title": title, "kind": kind, "state": state, "enabled": true, "visits": 0});
    v.as_object_mut().expect("an object").extend(extra.as_object().cloned().unwrap_or_default());
    v
}

/// The run, with `session` as its Plan step's session, as `case` has it.
pub fn run(session: &str, case: Case) -> Run {
    Run::from_json(&json(session, case)).expect("a run")
}

/// `run` as the server sends it.
pub fn json(session: &str, case: Case) -> Value {
    let you = |involvement: bool| {
        json!({
            "kind": "you", "stepId": "ok-plan", "stepTitle": "Approve the plan", "message": "Build it this way?",
            "question": "Build it this way?", "sessionId": session,
            "choices": [
                {"id": "choice:0", "label": "Approve", "note": false, "to": "implement", "involvement": involvement},
                {"id": "choice:1", "label": "Send back with a note", "note": true, "to": "plan", "involvement": false}
            ],
            "files": [PLAN], "thenAlone": ["Implement", "Review"], "nextYouTitle": "Open the pull request"
        })
    };
    let waiting = match case {
        Case::You => Some(you(false)),
        Case::Involvement | Case::Note => Some(you(true)),
        Case::NoOutcome => Some(json!({
            "kind": "no-outcome", "stepId": "plan", "stepTitle": "Plan",
            "message": "Plan finished without choosing an outcome. Pick one to move on.", "sessionId": session,
            "choices": [
                {"id": "ready", "label": "Ready", "note": false, "to": "ok-plan"},
                {"id": "unclear", "label": "Unclear", "note": false, "to": "plan"}
            ]
        })),
        Case::SkillOff => Some(json!({
            "kind": "skill-off", "stepId": "plan", "stepTitle": "Plan",
            "message": "Plan uses write-a-plan, which is now off.", "sessionId": session,
            "choices": [
                {"id": "retry", "label": "Retry", "note": false, "to": "plan"},
                {"id": "without-skill", "label": "Start without it", "note": false, "to": "plan"}
            ]
        })),
        Case::Resolved => None,
    };
    let resolved = case == Case::Resolved;
    let status = if waiting.is_some() { "waiting" } else { "running" };
    let plan_state = if matches!(case, Case::NoOutcome | Case::SkillOff) { "waiting" } else { "done" };
    let mut session_row = json!({"sessionId": session, "stepId": "plan", "visit": 1});
    if resolved {
        session_row.as_object_mut().expect("an object").extend(
            json!({"outcome": "ready", "files": [PLAN], "filesChecked": true, "filesCommit": "a1b2c3d"})
                .as_object()
                .cloned()
                .unwrap_or_default(),
        );
    }
    let mut sessions = vec![session_row];
    if resolved {
        sessions.push(json!({"sessionId": "s-implement", "stepId": "implement", "visit": 1}));
    }
    json!({
        "id": "run-card", "workflowId": "builtin:build-a-feature", "workflowName": "Build a feature", "title": "Page size setting",
        "request": "Make the page size a setting", "status": status, "currentStepId": if resolved { "implement" } else { "ok-plan" },
        "result": null, "repositoryPath": "/work/kettle", "branch": "fleet/page-size", "baseBranch": "main", "worktreePath": null,
        "harnessType": "opencode", "createdAt": "2026-11-19T10:00:00Z", "updatedAt": "2026-11-19T10:30:00Z", "endedAt": null,
        "checkWithMe": false,
        "steps": [
            step("plan", "Plan", "agent", plan_state, json!({"visits": 1, "sessionId": session, "outcome": if resolved || plan_state == "done" { json!("ready") } else { Value::Null }})),
            step("ok-plan", "Approve the plan", "you", if resolved { "done" } else if plan_state == "done" { "waiting" } else { "pending" }, json!({"visits": 1})),
            step("implement", "Implement", "agent", if resolved { "running" } else { "pending" }, json!({"visits": u32::from(resolved), "sessionId": if resolved { json!("s-implement") } else { Value::Null }})),
            step("review", "Review", "agent", "pending", json!({"maxLoops": 2})),
            step("open-pr", "Open the pull request", "you", "pending", json!({}))
        ],
        "sessions": sessions,
        "waiting": waiting
    })
}

impl crate::State {
    /// The conversation of the Plan step, with no run yet.
    pub fn show_run_session(&mut self, dark: bool) {
        self.dark = Some(dark);
        let reply = Message::assistant(
            "m-a1",
            vec![Part::text(
                "m-a1-t",
                "The plan is written to `.weave/plans/page-size.md`: read the size from the config, pass it on, test it.",
            )],
        );
        self.show_conversation(vec![Message::user("m-u1", "Make the pager's page size a setting."), reply]);
    }

    pub fn show_workflow_card(&mut self, dark: bool, case: Case) {
        self.show_run_session(dark);
        let id = self.session().id.clone();
        self.workflows.of_mut(HOME_MACHINE_KEY).upsert(run(&id, case));
        if matches!(case, Case::You | Case::Involvement | Case::Note) {
            self.show_plan_beside(&id);
        }
        if case == Case::Note {
            self.workflows_nav.card.set_note("choice:1", "Keep the default at 20 rows.");
        }
    }

    /// The plan open on the right, as the card opens it, with its text read.
    fn show_plan_beside(&mut self, id: &str) {
        self.content_open = true;
        self.edit_canvases(|c, id| {
            c.open_file(id, PLAN, OpenFile { keep: true, view: Some(FileView::Rendered) }, &|_| false);
        });
        let machine = self.machine_key_of(id).to_string();
        if let Some(request) = self.file_buffers.open(&machine, id, PLAN) {
            self.file_buffers.finish(&request, Ok(super::file_tab::answer(PLAN_TEXT)));
        }
    }
}
