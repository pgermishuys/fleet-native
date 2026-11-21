//! Which box the folder, workspace and base pickers work on. Fleet mounts `FolderPicker`, `WorkspacePicker` and `BasePicker`
//! in three places and hands each its own state as props: the New session page, the Workflows page's Run box and the
//! automation composer. Here the pickers read and write `State`, so they ask this which of the three is on screen: the rail
//! decides, since each box is the only one its page shows.
use crate::{Rail, State};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Host {
    /// The New session page's draft.
    NewSession,
    /// The Workflows page's Run box (component 18, piece D).
    RunBox,
    /// The automation composer's draft or the open automation's changes (component 19, piece K).
    Automation,
}

impl State {
    pub fn picker_host(&self) -> Host {
        match self.rail {
            Rail::Workflows => Host::RunBox,
            Rail::Automations => Host::Automation,
            _ => Host::NewSession,
        }
    }

    /// The box is sending, so its chips are off.
    pub fn picker_busy(&self) -> bool {
        match self.picker_host() {
            Host::NewSession => self.new_draft.draft().is_some_and(|d| d.is_starting),
            Host::RunBox => self.workflows_nav.run_box.starting,
            Host::Automation => self.automations_nav.submitting,
        }
    }

    /// Where the keys go when a picker closes: its chip, or on the automation composer the message box
    /// (`returnFocusToMessage`).
    pub fn picker_focus_return(&self, chip: &str) -> String {
        match self.picker_host() {
            Host::Automation => crate::view::automations::composer::MESSAGE.to_string(),
            _ => chip.to_string(),
        }
    }
}
