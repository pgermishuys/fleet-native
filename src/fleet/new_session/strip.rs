//! What the chips under the New session box show and when (`NewSessionComposer.vue`'s strip; rows A11.1, A11.3-A11.6, A11.8).
//! Plain state changes: nothing here draws. The pickers behind the chips are pieces F-K; this decides which chips there are.
use super::plan::{base_name, tilde_path};
use super::request::{Folder, Workspace};
use crate::State;

/// Which chips the strip has now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chips {
    /// Always on a desktop (`showMachineChip`: `hasMachines || !isMobile`).
    pub machine: bool,
    /// Always: "Choose a folder" until one is picked.
    pub folder: bool,
    /// Only for a repository; a plain folder and no folder have no checkout to choose (A11.5).
    pub workspace: bool,
    /// Only for a repository on a new worktree: where it starts (A11.6).
    pub base: bool,
}

impl Chips {
    pub fn of(folder: Option<&Folder>, workspace: &Workspace) -> Chips {
        let repository = matches!(folder, Some(Folder::Repository(_)));
        Chips { machine: true, folder: true, workspace: repository, base: repository && *workspace == Workspace::New }
    }
}

impl State {
    /// The strip's chips for the draft on the page.
    pub fn strip_chips(&self) -> Chips {
        match self.new_draft.draft() {
            Some(d) => Chips::of(d.folder.as_ref(), &d.workspace),
            None => Chips::of(None, &Workspace::New),
        }
    }

    /// The machine key the draft starts on: the machine the app works in unless the draft picked another (`target.key`).
    pub fn target_key(&self) -> String {
        self.new_draft.machine().unwrap_or_else(|| self.machines.current_key()).to_string()
    }

    /// `targetName`: the machine's name as the chip shows it.
    pub fn target_name(&self) -> String {
        let key = self.target_key();
        let entries = self.machines.entries();
        entries.iter().find(|e| e.key == key).or(entries.first()).map(|e| e.name.clone()).unwrap_or_default()
    }

    /// Whether the machine the draft starts on can't be reached, so the chip and its dot turn red (`selectedDown`).
    pub fn target_is_down(&self) -> bool {
        let key = self.target_key();
        self.machines.others.get(&key).is_some_and(|m| m.error.is_some())
    }

    /// The folder chip's words (`chipLabel`): a repository by its name, a plain folder by its path from the home folder.
    pub fn folder_label(&self, folder: Option<&Folder>) -> String {
        match folder {
            None => "Choose a folder".into(),
            Some(Folder::None) => "No folder".into(),
            Some(Folder::Directory(path)) => tilde_path(path),
            Some(Folder::Repository(path)) => self
                .feeds
                .repos
                .get(&self.target_key())
                .and_then(|feed| feed.list.iter().find(|r| &r.path == path))
                .map_or_else(|| base_name(path).to_string(), |r| r.name.clone()),
        }
    }

    /// `setFolder(folder, chosen)`: a different folder forgets the base, fetch and branch name; a repository gets its
    /// remembered workspace, anything else a new one.
    pub fn set_folder(&mut self, folder: Folder, chosen: bool) {
        let workspace = match &folder {
            // An attached item always starts in a new worktree (A7.11).
            Folder::Repository(path) if !self.new_draft.draft().is_some_and(|d| d.github_preset.is_some()) => {
                let machine = self.target_key();
                self.sidebar.new_session_defaults(&machine).workspace_for(path, None)
            }
            _ => Workspace::New,
        };
        let Some(draft) = self.new_draft.draft_mut() else { return };
        if draft.folder.as_ref() != Some(&folder) {
            draft.base_branch = None;
            draft.fetch_origin = true;
            draft.branch_name.clear();
        }
        draft.has_chosen_folder |= chosen;
        draft.folder = Some(folder);
        draft.workspace = workspace;
        self.read_folder_feeds();
        self.settle_choices();
    }
}
