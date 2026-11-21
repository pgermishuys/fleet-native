//! Where an automation runs, as the strip under the composer's box sets it (`AutomationComposer.vue`'s `setFolder`, the
//! workspace and base it hands New session's pickers, `AutomationRunsInPicker.vue`, `AutomationMoreOptions.vue`; rows 7.7,
//! 9.2-9.7). The folder, workspace and base menus are New session's, working on the composer while the Automations page is
//! open (`new_session::host`); this is what they write, and the state of the Runs in and "…" menus.
use super::composer::ComposerState;
use crate::fleet::new_session::request::{Folder, Workspace};
use crate::ui::Open;
use crate::{State, Tx};
use rune::TextEdit;

pub const RUNS_IN_POPOVER: &str = "automation-runs-in";
pub const RUNS_IN_CHIP: &str = "automation-runs-in-chip";
pub const MORE_POPOVER: &str = "automation-more";
pub const MORE_CHIP: &str = "automation-more-chip";
pub const NAME_KEY: &str = "automation-name";

pub const NEW_SESSION: &str = "new_session";
pub const SAME_SESSION: &str = "same_session";

/// An older automation's target, which stays until it's changed: its words, or `None` for a target this page offers.
pub fn older_target(target_type: &str) -> Option<&'static str> {
    match target_type {
        "most_recent_session" => Some("The most recent session"),
        "tagged_session" => Some("The newest session with its tags"),
        _ => None,
    }
}

/// The Runs in chip's words (`chipLabel`).
pub fn runs_in_label(target_type: &str) -> &'static str {
    if target_type == SAME_SESSION { "Same session each run" } else { older_target(target_type).unwrap_or("New session each run") }
}

impl State {
    fn composer_mut(&mut self) -> &mut ComposerState {
        let editing = self.composer_editing();
        let nav = &mut self.automations_nav;
        if editing { &mut nav.edit } else { &mut nav.draft }
    }

    /// The composer's folder, for the folder chip and menu.
    pub fn automation_folder(&self) -> Option<&Folder> {
        self.automations_nav.composer(self.composer_editing()).folder.as_ref()
    }

    pub fn automation_workspace(&self) -> Workspace {
        self.automations_nav.composer(self.composer_editing()).workspace.clone()
    }

    pub fn automation_base(&self) -> Option<String> {
        self.automations_nav.composer(self.composer_editing()).base_branch.clone()
    }

    /// The folder menu's pick (`setFolder`, row 9.4).
    pub(crate) fn automation_set_folder(&mut self, folder: Folder, chosen: bool) {
        let editing = self.composer_editing();
        self.composer_mut().set_folder(folder, chosen, editing);
    }

    /// The workspace menu's pick (`update:workspace`).
    pub(crate) fn automation_set_workspace(&mut self, workspace: Workspace) {
        self.composer_mut().workspace = workspace;
    }

    /// The base menu's pick: the repository's default is no choice, so the server's default applies (`BasePicker`'s
    /// `choose`).
    pub(crate) fn automation_choose_base(&mut self, name: &str) {
        let default = self.default_base();
        self.composer_mut().base_branch = if default.as_deref() == Some(name) { None } else { Some(name.to_string()) };
    }

    // ── Runs in ───────────────────────────────────────────────────────────────

    pub fn runs_in_menu_open(&self) -> bool {
        self.ui.open == Some(Open::Popover(RUNS_IN_POPOVER))
    }

    pub fn open_runs_in_menu(&mut self) {
        self.ui.open = Some(Open::Popover(RUNS_IN_POPOVER));
    }

    /// A row of the Runs in menu: the target it names (an older one picked again stays as it is).
    pub fn set_runs_in(&mut self, target_type: &str) {
        self.composer_mut().target_type = target_type.to_string();
        self.close_strip_menus();
    }

    // ── The "…" chip ──────────────────────────────────────────────────────────

    pub fn automation_more_open(&self) -> bool {
        self.ui.open == Some(Open::Popover(MORE_POPOVER))
    }

    /// Opening fills the Name field with the name typed so far.
    pub fn open_automation_more(&mut self) {
        let name = self.automations_nav.composer(self.composer_editing()).name.clone();
        self.automations_nav.name_box = TextEdit::new(name);
        self.ui.open = Some(Open::Popover(MORE_POPOVER));
    }

    /// The Name field changed: the composer has what is typed (`v-model="state.name"`).
    pub fn automation_name_edited(&mut self) {
        let typed = self.automations_nav.name_box.text.clone();
        self.composer_mut().name = typed;
    }

    /// Enter in the Name field: the menu closes and, on a saved automation, it saves as Enter in the box does
    /// (`commitName`).
    pub fn automation_commit_name(&mut self, tx: &mut Tx) {
        self.close_strip_menus();
        if self.composer_editing() {
            self.automation_submit(tx);
        }
    }

    pub fn toggle_automation_skip(&mut self) {
        let c = self.composer_mut();
        c.skip = !c.skip;
    }

    pub fn close_strip_menus(&mut self) {
        if self.runs_in_menu_open() || self.automation_more_open() {
            self.ui.open = None;
        }
    }
}
