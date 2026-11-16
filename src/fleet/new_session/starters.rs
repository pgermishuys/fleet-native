//! The starters on the empty New session page (`lib/starters.ts`, `StarterChips.vue`; rows A15, B9). Plain state changes:
//! nothing here draws. A starter fills the message box and never sends.
use super::feeds::{STARTERS_PREFERENCE_KEY, starters_shown, workflows_enabled};
use super::request::Folder;
use crate::State;

/// The built-in Fix a bug workflow the workflow starter opens (`FIX_A_BUG_WORKFLOW_ID`).
pub const FIX_A_BUG_WORKFLOW_ID: &str = "builtin:fix-a-bug";

/// `TextStarter`: `text` is what goes in the box, a "…" marking the part the person fills in.
pub struct Starter {
    pub id: &'static str,
    pub label: &'static str,
    pub text: &'static str,
}

/// Fleet's own starters: plain text, so they mean the same to every harness (`BUILT_IN_STARTERS`).
pub const BUILT_IN: [Starter; 3] = [
    Starter {
        id: "explore", label: "Explore the code", text: "Explore this repository and explain how … works. Don't change any files."
    },
    Starter { id: "plan", label: "Plan a change", text: "I want to … Read the code and propose a plan. Don't change any files yet." },
    Starter {
        id: "review",
        label: "Review my changes",
        text: "Review my uncommitted changes. List bugs and risks, most serious first. Don't edit anything.",
    },
];

/// `starterSelection`: the "…" (so typing replaces it), else nothing at the end. Byte offsets, as Rune's text boxes use.
pub fn starter_selection(text: &str) -> (usize, usize) {
    match text.find('…') {
        Some(at) => (at, at + '…'.len_utf8()),
        None => (text.len(), text.len()),
    }
}

/// Where Fix a bug asked to go: the Workflows page with the workflow chosen and, for a repository, its folder. The Workflows
/// page is another component's, so until it exists this is what a test (and its toast) reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRequest {
    pub workflow: String,
    pub folder: Option<String>,
}

/// What the starters keep between frames.
#[derive(Debug, Default)]
pub struct StartersUi {
    /// What to select in the message box once it exists on screen (piece L takes it): set by a click on a starter.
    pub pending_selection: Option<(usize, usize)>,
    /// The box should take the keyboard (after a fill or after Hide starters).
    pub focus_box: bool,
    pub workflow_request: Option<WorkflowRequest>,
}

impl State {
    /// `showStarters`: preferences are known, the person hasn't hidden them, the box is empty and nothing is starting.
    pub fn starters_visible(&self) -> bool {
        let Some(draft) = self.new_draft.draft() else { return false };
        starters_shown(&self.pickers.prefs) && draft.message.is_empty() && !draft.is_starting
    }

    /// `showWorkflowStarter`: Workflows is on and the page is on the machine the app works in (workflows run there).
    pub fn show_workflow_starter(&self) -> bool {
        workflows_enabled(&self.pickers.prefs) && self.new_draft.machine().is_none()
    }

    /// `fillStarter`: the text goes in the box with its "…" selected. Sends nothing.
    pub fn fill_starter(&mut self, id: &str) {
        let Some(starter) = BUILT_IN.iter().find(|s| s.id == id) else { return };
        let Some(draft) = self.new_draft.draft_mut() else { return };
        draft.message = starter.text.to_string();
        self.starters.pending_selection = Some(starter_selection(starter.text));
        self.starters.focus_box = true;
    }

    /// `openFixABug`: the Workflows page opens on the workflow, with this folder when it is a repository. Starts nothing.
    pub fn open_fix_a_bug(&mut self) {
        let folder = match self.new_draft.draft().and_then(|d| d.folder.as_ref()) {
            Some(Folder::Repository(path)) => Some(path.clone()),
            _ => None,
        };
        self.starters.workflow_request = Some(WorkflowRequest { workflow: FIX_A_BUG_WORKFLOW_ID.to_string(), folder });
    }

    /// `hideStarters`: writes `NewSessionStarters = "false"` (the same switch as Settings → General → Show starters).
    pub fn hide_starters(&mut self) {
        let before = self.pickers.prefs.get(STARTERS_PREFERENCE_KEY).map(str::to_string);
        self.put_preference(STARTERS_PREFERENCE_KEY, "false", before);
        self.starters.focus_box = true;
    }
}

/// Scenes for the pictures: the page with the strip's folder chosen, the starters menu open, and Workflows on.
impl State {
    pub(crate) fn show_new_session_strip(&mut self, folder: Option<Folder>, menu_open: bool, workflows: bool) {
        self.open_new_session_page(None);
        if let Some(folder) = folder {
            self.set_folder(folder, true);
        }
        if workflows {
            self.pickers.prefs.set(super::feeds::WORKFLOWS_PREFERENCE_KEY, "true");
        }
        self.ui.open = menu_open.then_some(crate::ui::Open::Popover("starters"));
    }
}
