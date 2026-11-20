//! What a workflow's detail says (`WorkflowDetailPanel.vue`): the models and skills it uses, what a recent run's row says, and
//! which session a click on it opens. Plain Rust; `view::workflows::detail` draws it.
use super::helpers::{Tone, model_short_name, run_status_label};
use super::{MODEL_ROLES_PREFERENCE_KEY, ModelRoles, Role, Run, RunStatus, StepKind, Workflow, parse_model_roles};
use crate::State;

/// The runs a workflow's detail lists.
pub const RECENT: usize = 6;

/// `models`: "Strong · Opus 5.5, Standard · Sonnet 5", then pinned models as they are. A role says the model it is mapped
/// to for `harness`.
pub fn models(workflow: &Workflow, roles: &ModelRoles, harness: &str) -> String {
    let mut seen: Vec<&str> = Vec::new();
    let mut labels = Vec::new();
    for step in &workflow.steps {
        let Some(model) = step.model.as_deref().filter(|m| step.kind == StepKind::Agent && !m.is_empty()) else { continue };
        if seen.contains(&model) {
            continue;
        }
        seen.push(model);
        labels.push(match Role::parse(Some(model)) {
            Some(role) => {
                let mapped = roles.get(harness).and_then(|r| r.get(&role)).and_then(|c| c.model.as_deref());
                format!("{} \u{b7} {}", role.name(), model_short_name(mapped))
            }
            None => model.to_string(),
        });
    }
    labels.join(", ")
}

/// `skills`: each skill the steps use, once, in step order.
pub fn skills(workflow: &Workflow) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    for skill in workflow.steps.iter().filter_map(|s| s.skill.as_deref().filter(|k| !k.is_empty())) {
        if !out.contains(&skill) {
            out.push(skill);
        }
    }
    out
}

/// `runText`: what a recent run's row says at its end, and how it is coloured.
pub fn run_text(run: &Run) -> (String, Tone) {
    let tone = run_status_label(run).1;
    let text = match (run.status, &run.waiting, &run.with_you) {
        (RunStatus::Waiting, Some(w), _) => format!("Needs you \u{b7} {}", w.step_title),
        (RunStatus::Running, _, Some(w)) => format!("With you \u{b7} {}", w.step_title),
        (RunStatus::Running, ..) => match run.steps.iter().find(|s| Some(&s.id) == run.current_step_id.as_ref()) {
            Some(step) => format!("Running \u{b7} {}", step.title),
            None => "Running".to_string(),
        },
        _ => run_status_label(run).0,
    };
    (text, tone)
}

/// `openRun`: the session the run waits in, else its newest step's.
pub fn run_session(run: &Run) -> Option<&str> {
    run.waiting.as_ref().and_then(|w| w.session_id.as_deref()).or_else(|| run.sessions.last().map(|s| s.session_id.as_str()))
}

impl State {
    /// The roles a machine maps to models (`useModelRoles`), read from its preferences.
    pub fn workflow_model_roles(&self, machine: &str) -> ModelRoles {
        parse_model_roles(self.prefs_for(machine).get(MODEL_ROLES_PREFERENCE_KEY))
    }

    /// The harness a run on `machine` starts on when none is named (`defaultHarnessType`).
    pub fn workflow_default_harness(&self, machine: &str) -> String {
        self.harness_choice(machine).default_type()
    }

    /// A click on a recent run: its session opens, on the machine the page shows.
    pub fn open_workflow_run(&mut self, run_id: &str, tx: &mut rune::Tx) {
        let machine = self.workflows_machine();
        let Some(id) = self.workflow_runs(&machine).get(run_id).and_then(run_session).map(str::to_string) else { return };
        // The session belongs to Sessions: the rail follows the route, as a fork's does.
        self.rail = crate::Rail::Sessions;
        if machine == crate::fleet::settings::HOME_MACHINE_KEY {
            self.open_parent(&id, tx);
        } else {
            self.open_machine_session(&machine, &id, tx);
        }
    }
}
