//! Invented workflow runs for the Sessions list and the dashboard (component 18, piece I): "Pager fix" with its Fix step open
//! with the user, "Dependency bump" waiting for the plan's approval (an automation started it), and one plain session.
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::Run;
use crate::mock::Status;
use crate::mock::machines::listed;
use serde_json::{Value, json};

pub const PAGER: &str = "run-pager";
pub const BUMP: &str = "run-bump";

fn step(id: &str, title: &str, state: &str, session: &str) -> Value {
    json!({"id": id, "title": title, "kind": "agent", "state": state, "enabled": true, "visits": 1, "sessionId": session})
}

fn base(id: &str, workflow: &str, title: &str, status: &str) -> Value {
    json!({
        "id": id, "workflowId": "builtin:x", "workflowName": workflow, "title": title, "request": title, "status": status,
        "currentStepId": null, "result": null, "repositoryPath": "/work/kettle", "branch": null, "baseBranch": null,
        "worktreePath": null, "harnessType": "opencode", "createdAt": "2026-11-19T10:00:00Z", "updatedAt": "2026-11-19T10:30:00Z",
        "endedAt": null, "checkWithMe": false
    })
}

/// "Pager fix": Plan is done, Fix runs and the user finishes it.
pub fn pager_run() -> Run {
    let mut v = base(PAGER, "Fix a bug", "The pager skips the last page", "running");
    v["currentStepId"] = json!("fix");
    v["steps"] = json!([step("plan", "Plan", "done", "s-pager-plan"), step("fix", "Fix", "running", "s-pager-fix")]);
    v["sessions"] =
        json!([{"sessionId": "s-pager-plan", "stepId": "plan", "visit": 1}, {"sessionId": "s-pager-fix", "stepId": "fix", "visit": 1}]);
    v["withYou"] = json!({"stepId": "fix", "stepTitle": "Fix", "sessionId": "s-pager-fix", "files": [], "wrappingUp": false, "moves": []});
    Run::from_json(&v).expect("a run")
}

/// "Dependency bump": started by an automation, waiting for the user to approve the plan.
pub fn bump_run() -> Run {
    let mut v = base(BUMP, "Build a feature", "Bump the tile library", "waiting");
    v["currentStepId"] = json!("approve");
    v["steps"] = json!([step("plan", "Plan", "done", "s-bump-plan"), step("approve", "Approve the plan", "waiting", "s-bump-approve")]);
    v["sessions"] = json!([{"sessionId": "s-bump-plan", "stepId": "plan", "visit": 1}, {"sessionId": "s-bump-approve", "stepId": "approve", "visit": 1}]);
    v["waiting"] = json!({
        "kind": "you", "stepId": "approve", "stepTitle": "Approve the plan", "message": "Read the plan and decide.",
        "sessionId": "s-bump-approve", "choices": [], "files": [], "thenAlone": []
    });
    v["startedBy"] = json!({"automationId": "a-bump", "automationName": "Weekly dependency bump"});
    Run::from_json(&v).expect("a run")
}

impl crate::State {
    /// The list holds two runs and a plain session in one project, the runs known to the live machine's store.
    pub fn show_workflow_runs(&mut self, dark: bool) {
        self.dark = Some(dark);
        self.now_ms = Some(crate::mock::rows::NOW);
        let project = ("p-kettle", "Kettle");
        let member = |id: &str, title: &str, status: Status, age: &'static str, run: Option<&str>| {
            listed(id, title, status, age, Some(project), |l| l.workflow_run_id = run.map(str::to_string))
        };
        let rows = vec![
            member("s-pager-fix", "Pager fix \u{b7} Fix", Status::Working, "now", Some(PAGER)),
            member("s-pager-plan", "Pager fix \u{b7} Plan", Status::Idle, "20m", Some(PAGER)),
            member("s-bump-approve", "Bump the tile library \u{b7} Approve the plan", Status::Idle, "40m", Some(BUMP)),
            member("s-bump-plan", "Bump the tile library \u{b7} Plan", Status::Idle, "1h", Some(BUMP)),
            member("s-changelog", "Tidy the changelog", Status::Idle, "3h", None),
        ];
        self.projects.truncate(1);
        self.projects[0].name = "Kettle".into();
        self.projects[0].sessions = rows;
        let runs = self.workflows.of_mut(HOME_MACHINE_KEY);
        runs.upsert(pager_run());
        runs.upsert(bump_run());
    }

    /// The dashboard with the bump run waiting on the user in its approval step.
    pub fn show_dashboard_workflow_waiting(&mut self, dark: bool) {
        self.show_workflow_runs(dark);
        self.show_home(dark);
        // `show_home` sets its own clock; the rows' ages are the list's.
        self.now_ms = Some(crate::mock::rows::NOW);
    }
}
