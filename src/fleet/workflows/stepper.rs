//! What the stepper under a step session's header holds and does (`WorkflowStepper.vue`): the run the open session is a step
//! of, the "Check with me" switch and the error under it, a step that opens its session, and "Started by" that opens the
//! automation. The stepper and the mark on Fleet's own prompts (`workflowStep`) read the machine the session is on, never the
//! one the Workflows page shows.
use super::helpers::workflow_message_label;
use super::{Run, RunStatus};
use crate::State;

/// The switch while it is being sent, and the words of why it failed, with the session they were said in: another session
/// opens its own stepper with no error (the component is mounted again there).
#[derive(Debug, Default)]
pub struct Stepper {
    pub switching: bool,
    pub error: Option<(String, String)>,
}

/// The words when the server gave none.
pub const SWITCH_FALLBACK: &str = "Couldn't change Check with me.";

/// A run that is over has no switch to flip (`finished`).
pub fn finished(run: &Run) -> bool {
    matches!(run.status, RunStatus::Done | RunStatus::Ended | RunStatus::Failed)
}

impl State {
    /// `runForSession` for the open session, on the machine it is on.
    pub fn stepper_run(&self) -> Option<&Run> {
        let id = self.session().id.as_str();
        self.workflow_runs(self.machine_key_of(id)).run_for_session(id)
    }

    /// The words under the switch, when this session's last try failed.
    pub fn stepper_error(&self) -> Option<&str> {
        self.workflows_nav.stepper.error.as_ref().filter(|(session, _)| *session == self.session().id).map(|(_, words)| words.as_str())
    }

    /// `toggleCheckWithMe`: sends the other way round. Nothing while a switch is on its way or the run is over.
    pub fn stepper_toggle_check(&mut self, tx: &mut rune::Tx) {
        let session = self.session().id.clone();
        let machine = self.machine_key_of(&session).to_string();
        let Some((run_id, on)) = self.stepper_run().filter(|r| !finished(r)).map(|r| (r.id.clone(), !r.check_with_me)) else { return };
        if self.workflows_nav.stepper.switching {
            return;
        }
        self.workflows_nav.stepper.switching = true;
        self.workflows_nav.stepper.error = None;
        self.workflow_set_check_with_me(
            &machine,
            &run_id,
            on,
            tx,
            Box::new(move |s, _, result| {
                s.workflows_nav.stepper.switching = false;
                if let Err(words) = result {
                    s.workflows_nav.stepper.error = Some((session, words));
                }
            }),
        );
    }

    /// `open`: a step that ran in another session opens it, on the machine this one is on.
    pub fn stepper_open_step(&mut self, step_session: &str, tx: &mut rune::Tx) {
        let here = self.session().id.clone();
        if step_session.is_empty() || step_session == here {
            return;
        }
        let machine = self.machine_key_of(&here).to_string();
        self.open_workflow_session(&machine, step_session, tx);
    }

    /// `WorkflowStartedBy`'s click: the automation opens in Automations.
    pub fn open_started_by(&mut self, automation_id: &str) {
        self.press_rail(crate::Rail::Automations);
        self.set_active_automation(automation_id);
    }

    /// `workflowMessageLabel` for a user message of the open session: the mark above a prompt Fleet sent into a step.
    pub fn workflow_mark(&self, message_id: &str, text: &str) -> Option<String> {
        let id = self.session().id.as_str();
        workflow_message_label(self.workflow_runs(self.machine_key_of(id)).run_for_session(id), id, message_id, text)
    }
}
