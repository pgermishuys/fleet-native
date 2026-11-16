//! What is on the New session page, kept while you look at something else (`stores/workspace-ui.ts`; rows A3.1-A3.12).
//! In memory only, as Fleet's is: it survives leaving the page and coming back in the same run, not a restart.
//! Pure: no Rune, no I/O. The page edits `draft_mut()`; the session list reads `row()`.
use super::request::{Folder, GitHubPreset, Workspace, title_from_message};
use std::collections::HashMap;

/// `NewSessionDraft`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewSessionDraft {
    /// Keys the draft's list row; the session it becomes keeps it, so the row doesn't move.
    pub row_key: String,
    pub message: String,
    pub folder: Option<Folder>,
    /// The folder came from the person (or a restored draft), so defaults arriving later don't replace it.
    pub has_chosen_folder: bool,
    pub workspace: Workspace,
    /// Where a new worktree starts; `None` for the repository's default.
    pub base_branch: Option<String>,
    pub fetch_origin: bool,
    /// Branch for a new worktree instead of the one named from the message; empty for that one.
    pub branch_name: String,
    pub title: String,
    /// Comma text, as typed.
    pub tags: String,
    pub project_id: Option<String>,
    pub harness_type: String,
    /// The profile picked for this session; `None` follows the harness's default.
    pub harness_profile_id: Option<String>,
    /// The agent to start with; empty for the folder's default.
    pub agent: String,
    /// The model to start with, as a model selection key; empty for the agent's default.
    pub model: String,
    /// The agent or model came from the person, so the folder's remembered ones don't replace them.
    pub has_chosen_agent_or_model: bool,
    pub github_preset: Option<GitHubPreset>,
    /// Set between Enter and the session existing.
    pub is_starting: bool,
}

impl NewSessionDraft {
    /// What a new page starts with, before any default is applied (the `initial` argument of `openNewSessionDraft`).
    pub fn blank(harness_type: &str) -> NewSessionDraft {
        NewSessionDraft {
            row_key: String::new(),
            message: String::new(),
            folder: None,
            has_chosen_folder: false,
            workspace: Workspace::New,
            base_branch: None,
            fetch_origin: true,
            branch_name: String::new(),
            title: String::new(),
            tags: String::new(),
            project_id: None,
            harness_type: harness_type.to_string(),
            harness_profile_id: None,
            agent: String::new(),
            model: String::new(),
            has_chosen_agent_or_model: false,
            github_preset: None,
            is_starting: false,
        }
    }

    /// A draft with something in it stays with its list row when you leave (`hasContent`).
    fn has_content(&self) -> bool {
        !self.message.trim().is_empty() || self.github_preset.is_some() || self.is_starting
    }
}

/// The draft's row in the session list (`NewSessionDraftRow`). `title` is empty while nothing is typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraftRow {
    pub key: String,
    pub title: String,
    pub project_id: Option<String>,
    pub is_starting: bool,
}

#[derive(Debug, Default)]
pub struct DraftStore {
    draft: Option<NewSessionDraft>,
    /// A GitHub issue or pull request handed to the page ("start session") until the page takes it.
    initial_source: Option<GitHubPreset>,
    page_open: bool,
    /// Session id to the draft row key it took over.
    session_row_keys: HashMap<String, String>,
    /// The machine the draft starts on, as a machine key; `None` for the machine the app is working in.
    machine: Option<String>,
    /// Set while the page is rebuilt for another machine, so leaving doesn't drop the draft.
    switching_machine: bool,
    /// Boxes on the page now. A rebuilt box opens before the one it replaces leaves, so for a moment there are two.
    open_boxes: usize,
    count: usize,
    /// A session made by "New session in this folder", to open when the list has it.
    pub open_after_list: Option<String>,
}

impl DraftStore {
    pub fn draft(&self) -> Option<&NewSessionDraft> {
        self.draft.as_ref()
    }

    pub fn draft_mut(&mut self) -> Option<&mut NewSessionDraft> {
        self.draft.as_mut()
    }

    /// Whether the page is showing (a session started from it opens only if it is).
    pub fn is_page_open(&self) -> bool {
        self.page_open
    }

    pub fn machine(&self) -> Option<&str> {
        self.machine.as_deref()
    }

    pub fn initial_source(&self) -> Option<&GitHubPreset> {
        self.initial_source.as_ref()
    }

    pub fn set_initial_source(&mut self, source: Option<GitHubPreset>) {
        self.initial_source = source;
    }

    /// The row the session that took a draft's place keeps (`sessionRowKeys`).
    pub fn row_key_of(&self, session_id: &str) -> Option<&str> {
        self.session_row_keys.get(session_id).map(String::as_str)
    }

    /// The list row of the draft: the typed title, else the message's first line, else the GitHub item's title (A3.9).
    pub fn row(&self) -> Option<DraftRow> {
        let d = self.draft.as_ref()?;
        let from_preset = || d.github_preset.as_ref().map(|p| p.title.trim().to_string()).unwrap_or_default();
        let title = Some(title_from_message(&d.title))
            .filter(|t| !t.is_empty())
            .or_else(|| Some(title_from_message(&d.message)).filter(|t| !t.is_empty()))
            .unwrap_or_else(from_preset);
        Some(DraftRow { key: d.row_key.clone(), title, project_id: d.project_id.clone(), is_starting: d.is_starting })
    }

    /// The draft the page edits: the one left from last time (`true`), or a fresh one made from `initial` (`false`).
    pub fn open(&mut self, initial: NewSessionDraft) -> bool {
        self.open_boxes += 1;
        self.page_open = true;
        self.switching_machine = false;
        if self.draft.is_some() {
            return true;
        }
        self.count += 1;
        self.draft = Some(NewSessionDraft { row_key: format!("new-session-draft-{}", self.count), is_starting: false, ..initial });
        false
    }

    /// Leaving the page: a draft with nothing in it goes; one with a message stays, with its row (A3.4).
    pub fn leave(&mut self) {
        self.open_boxes = self.open_boxes.saturating_sub(1);
        // The box that replaced this one is already open (a rebuild for another machine): the page stays, with its draft.
        if self.open_boxes > 0 {
            return;
        }
        self.page_open = false;
        if self.switching_machine {
            return;
        }
        if self.draft.as_ref().is_some_and(|d| !d.has_content()) {
            self.draft = None;
        }
        if self.draft.is_none() {
            self.machine = None;
        }
    }

    /// A create failed after the page was left: a draft with nothing in it goes (`leaveNewSessionDraft` again, A13.9).
    pub fn drop_if_empty(&mut self) {
        if !self.page_open && self.draft.as_ref().is_some_and(|d| !d.has_content()) {
            self.draft = None;
            self.machine = None;
        }
    }

    /// Starts the draft on another machine (a machine key; `None` for the live one). What belongs to the old machine goes: its
    /// folder, worktree, branch, project, harness and agent. The message, title, tags and any attached issue stay (A3.12).
    /// `current_key` is the machine the app is on; picking it is the same as picking none.
    pub fn set_machine(&mut self, picked: Option<&str>, current_key: &str) {
        let key = picked.filter(|k| *k != current_key).map(str::to_string);
        if key == self.machine {
            return;
        }
        if let Some(d) = self.draft.as_mut() {
            d.folder = None;
            d.has_chosen_folder = false;
            d.workspace = Workspace::New;
            d.base_branch = None;
            d.fetch_origin = true;
            d.branch_name.clear();
            d.project_id = None;
            d.harness_type.clear();
            d.harness_profile_id = None;
            d.agent.clear();
            d.model.clear();
            d.has_chosen_agent_or_model = false;
        }
        self.switching_machine = self.page_open;
        self.machine = key;
    }

    /// The draft became a session: its row now shows that session, in the same place (A3.11).
    pub fn hand_off(&mut self, session_id: &str) {
        let Some(d) = self.draft.take() else { return };
        self.session_row_keys.insert(session_id.to_string(), d.row_key);
        self.machine = None;
    }
}
