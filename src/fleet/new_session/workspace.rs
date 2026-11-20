//! What the workspace and base chips show and do (`NSD/WorkspacePicker.vue`, `NSD/BasePicker.vue`, and the parts of
//! `NewSessionComposer.vue` that feed them: rows B3.1-B3.22, B4.1-B4.14, A12 (chips), A17 (preview)). Plain state changes:
//! nothing here draws. The branches and worktrees are the page's reads (`feed_calls.rs`); this decides what the chips say
//! from them, and reads them again when the folder changes.
use super::feeds::{Branch, Detail, Worktree};
use super::naming::{Context, Name, resolve_worktree_name};
use super::plan::{PlanInput, PlanPart, base_name, describe_new_session, tilde_path};
use super::request::{Folder, Workspace, resolve_new_worktree_branch};
use crate::State;
use rune::TextEdit;

pub const WORKSPACE_POPOVER: &str = "workspace";
pub const BASE_POPOVER: &str = "base";
pub const WORKSPACE_CHIP: &str = "new-session-workspace-chip";
pub const BASE_CHIP: &str = "new-session-base-chip";
pub const BASE_SEARCH_KEY: &str = "new-session-base-search";
pub const BRANCH_KEY: &str = "new-session-branch-name";

/// What the two menus keep while they are open.
#[derive(Default)]
pub struct PickerMenus {
    pub base_query: TextEdit,
    pub base_highlight: usize,
    /// The New branch field. The draft holds the text; this is the field's caret and selection, filled on open.
    pub branch: TextEdit,
    /// The "…" menu's Project list and its Title and Tags fields (piece K).
    pub more: super::more::MoreMenu,
}

/// `WorktreeInfo` rows as the menu lists them: the one used last first, then the server's order (`orderedWorktrees`).
pub fn order_worktrees(worktrees: &[Worktree], last: Option<&str>) -> Vec<Worktree> {
    let mut sorted = worktrees.to_vec();
    // `Array.sort` with this comparator is stable: only the last-used one moves.
    if let Some(last) = last
        && let Some(at) = sorted.iter().position(|w| w.path == last)
    {
        let first = sorted.remove(at);
        sorted.insert(0, first);
    }
    sorted
}

/// A worktree next to the repository as `…-worktrees/x`, anything else as a `~` path (`worktreeLocation`).
pub fn worktree_location(repository: &str, path: &str) -> String {
    match path.strip_prefix(repository) {
        Some(rest) => format!("…{rest}"),
        None => tilde_path(path),
    }
}

/// The base menu's rows: the branches matching the search, the default first (`options`).
pub fn base_options(branches: &[Branch], default_base: Option<&str>, query: &str) -> Vec<Branch> {
    let needle = query.trim().to_lowercase();
    let mut rows: Vec<Branch> = branches.iter().filter(|b| needle.is_empty() || b.name.to_lowercase().contains(&needle)).cloned().collect();
    if let Some(default) = default_base
        && let Some(at) = rows.iter().position(|b| b.name == default)
    {
        let first = rows.remove(at);
        rows.insert(0, first);
    }
    rows
}

/// The words under an empty base list (`Loading branches…`, no match, none at all).
pub fn base_empty_note(loading: bool, query: &str) -> String {
    if loading {
        "Loading branches…".into()
    } else if !query.trim().is_empty() {
        format!("No branch matches “{}”.", query.trim())
    } else {
        "No branches found.".into()
    }
}

/// Today as the naming templates write `{date}`: `2026-09-21` (UTC, as `toISOString` is).
fn today() -> String {
    let (y, m, d, ..) = crate::conv::time::civil(crate::fleet::link::now_ms(), 0);
    format!("{y:04}-{:02}-{d:02}", m + 1)
}

impl State {
    /// The repository the draft is in, when the folder is one.
    pub fn repository_path(&self) -> Option<String> {
        if self.run_box_active() {
            return self.workflows_nav.repository_path().map(str::to_string);
        }
        match self.new_draft.draft()?.folder.as_ref()? {
            Folder::Repository(p) => Some(p.clone()),
            _ => None,
        }
    }

    /// The base picked: the Run box's while that is on screen, else the draft's.
    fn chosen_base(&self) -> Option<String> {
        let chosen = if self.run_box_active() {
            self.workflows_nav.run_box.base_branch.clone()
        } else {
            self.new_draft.draft().and_then(|d| d.base_branch.clone())
        };
        chosen.filter(|b| !b.is_empty())
    }

    /// The chosen repository's branches and remotes, once they are in for it.
    pub fn repository_detail(&self) -> Option<&Detail> {
        let path = self.repository_path()?;
        let slot = &self.feeds.detail;
        if slot.path.as_deref() == Some(path.as_str()) { slot.value.as_ref() } else { None }
    }

    /// `isLoadingRepositoryDetail || !repositoryDetail`.
    pub fn base_is_loading(&self) -> bool {
        self.repository_detail().is_none()
    }

    pub fn current_branch(&self) -> Option<String> {
        self.repository_detail().map(|d| d.branch.clone()).filter(|b| !b.is_empty())
    }

    pub fn default_base(&self) -> Option<String> {
        self.repository_detail().map(|d| d.default_base.clone()).filter(|b| !b.is_empty())
    }

    /// The ref a new worktree starts from as the base chip says it, or `None` while that isn't known (`effectiveBase`).
    pub fn effective_base(&self) -> Option<String> {
        if let Some(chosen) = self.chosen_base() {
            return Some(chosen);
        }
        if self.base_is_loading() {
            return None;
        }
        Some(self.default_base().or_else(|| self.current_branch()).unwrap_or_else(|| "HEAD".into()))
    }

    /// Where a new worktree starts for the plan line: the chosen base, else the default once the detail is in
    /// (`newWorktreeBase`).
    pub fn new_worktree_base(&self) -> Option<String> {
        if let Some(chosen) = self.chosen_base() {
            return Some(chosen);
        }
        self.repository_detail()?;
        self.default_base().or_else(|| self.current_branch())
    }

    /// The chosen base is one of origin's, so Fetch origin applies (`isOriginBase`).
    pub fn base_is_origin(&self) -> bool {
        self.effective_base().is_some_and(|b| b.starts_with("origin/"))
    }

    pub fn fetch_origin(&self) -> bool {
        // A run always fetches (the Run box leaves the switch out).
        if self.run_box_active() {
            return true;
        }
        self.new_draft.draft().is_none_or(|d| d.fetch_origin)
    }

    pub fn base_rows(&self) -> Vec<Branch> {
        let branches = self.repository_detail().map(|d| d.branches.as_slice()).unwrap_or_default();
        base_options(branches, self.default_base().as_deref(), &self.pickers_ns.base_query.text)
    }

    /// The worktrees listed for the chosen repository, the last used first.
    pub fn workspace_worktrees(&self) -> Vec<Worktree> {
        let Some(path) = self.repository_path() else { return Vec::new() };
        let slot = &self.feeds.worktrees;
        if slot.path.as_deref() != Some(path.as_str()) {
            return Vec::new();
        }
        let key = self.target_key();
        let last = self.sidebar.new_session_defaults(&key).last_worktree_for(&path);
        order_worktrees(slot.value.as_deref().unwrap_or_default(), last.as_deref())
    }

    /// Worktrees are still on their way and none are known yet.
    pub fn worktrees_loading(&self) -> bool {
        self.feeds.worktrees.loading
    }

    pub fn workspace_menu_open(&self) -> bool {
        self.ui.open == Some(crate::ui::Open::Popover(WORKSPACE_POPOVER))
    }

    pub fn base_menu_open(&self) -> bool {
        self.ui.open == Some(crate::ui::Open::Popover(BASE_POPOVER))
    }

    /// `update:workspace`: the draft's workspace; the menu closes (B4.11).
    pub fn choose_workspace(&mut self, workspace: Workspace) {
        if let Some(d) = self.new_draft.draft_mut() {
            d.workspace = workspace;
        }
        self.close_picker_menu();
        // An existing worktree is asked about as it is.
        self.settle_choices();
    }

    pub fn close_picker_menu(&mut self) {
        if self.workspace_menu_open() || self.base_menu_open() {
            self.ui.open = None;
        }
    }

    pub fn open_workspace_menu(&mut self) {
        self.ui.open = Some(crate::ui::Open::Popover(WORKSPACE_POPOVER));
    }

    /// The `open` watch (B3.12): the search is empty and the highlight is on the base in use, even when a search had hidden it.
    pub fn open_base_menu(&mut self) {
        self.ui.open = Some(crate::ui::Open::Popover(BASE_POPOVER));
        self.pickers_ns.base_query = TextEdit::new("");
        self.pickers_ns.base_highlight = self.base_selected_row().unwrap_or(0);
        let typed = self.new_draft.draft().map(|d| d.branch_name.clone()).unwrap_or_default();
        self.pickers_ns.branch = TextEdit::new(typed);
    }

    /// The row of the base in use in the unfiltered list.
    fn base_selected_row(&self) -> Option<usize> {
        let base = self.effective_base()?;
        self.base_rows().iter().position(|b| b.name == base)
    }

    pub fn base_query_changed(&mut self) {
        self.pickers_ns.base_highlight = 0;
    }

    /// ArrowDown and ArrowUp wrap (B3.13); nothing with no rows.
    pub fn base_step(&mut self, by: isize) -> Option<usize> {
        let n = self.base_rows().len();
        if n == 0 {
            return None;
        }
        let at = (self.pickers_ns.base_highlight + n).saturating_add_signed(by) % n;
        self.pickers_ns.base_highlight = at;
        Some(at)
    }

    /// Enter on the highlighted row.
    pub fn base_choose_highlighted(&mut self) {
        if let Some(b) = self.base_rows().get(self.pickers_ns.base_highlight) {
            let name = b.name.clone();
            self.choose_base(&name);
        }
    }

    /// `choose`: the default is "no choice", so the server's default rules apply to it (B3.14).
    pub fn choose_base(&mut self, name: &str) {
        if self.run_box_active() {
            self.run_box_choose_base(name);
            self.close_picker_menu();
            return;
        }
        let default = self.default_base();
        if let Some(d) = self.new_draft.draft_mut() {
            d.base_branch = if default.as_deref() == Some(name) { None } else { Some(name.to_string()) };
        }
        self.close_picker_menu();
    }

    /// The switch (B3.18); it does nothing for a base that isn't origin's.
    pub fn toggle_fetch_origin(&mut self) {
        if !self.base_is_origin() {
            return;
        }
        if let Some(d) = self.new_draft.draft_mut() {
            d.fetch_origin = !d.fetch_origin;
        }
    }

    /// The New branch field was edited (B3.20): the draft holds what is typed.
    pub fn branch_name_edited(&mut self) {
        let typed = self.pickers_ns.branch.text.clone();
        if let Some(d) = self.new_draft.draft_mut() {
            d.branch_name = typed;
        }
    }

    // ─── the preview and the plan line ──────────────────────────────────────────

    /// What the naming templates would call this worktree. A preview only: the request carries a branch just when someone
    /// chose one, and the server does the naming (`previewedName`).
    pub fn previewed_name(&self) -> Option<Name> {
        let path = self.repository_path()?;
        let draft = self.new_draft.draft()?;
        if draft.workspace != Workspace::New {
            return None;
        }
        // {user} and {shortid} are the server's to fill in; a preview that guessed at them would show a name the worktree
        // doesn't get.
        let context = Context { repository_path: path, user: String::new(), date: today(), short_id: String::new(), home: String::new() };
        let typed = resolve_new_worktree_branch(Some(&draft.branch_name), draft.github_preset.as_ref());
        Some(resolve_worktree_name(&self.feeds.naming.answer.effective, &context, &draft.message, typed.as_deref()))
    }

    /// The name the message gives the new branch, for the New branch field's placeholder.
    pub fn generated_branch(&self) -> Option<String> {
        self.previewed_name().and_then(|n| n.branch)
    }

    /// `planParts`: what Enter will do.
    pub fn plan_parts(&self) -> Vec<PlanPart> {
        let draft = self.new_draft.draft();
        let folder = draft.and_then(|d| d.folder.clone());
        let workspace = draft.map(|d| d.workspace.clone()).unwrap_or(Workspace::New);
        let name = self.previewed_name();
        let mut input = PlanInput::new(folder, workspace.clone());
        input.current_branch = self.current_branch();
        input.new_branch = name.as_ref().and_then(|n| n.branch.clone());
        input.worktree_path =
            name.as_ref().and_then(|n| n.branch.as_ref().filter(|_| !n.folder.is_empty()).map(|_| format!("{}/{}", n.root, n.folder)));
        if let Workspace::Existing(path) = &workspace {
            input.existing_branch =
                self.feeds.worktrees.value.as_ref().and_then(|w| w.iter().find(|w| &w.path == path)).and_then(|w| w.branch.clone());
        }
        input.default_branch = self.repository_detail().map(|d| d.default_branch.clone()).filter(|b| !b.is_empty());
        input.base = self.new_worktree_base();
        input.fetch_origin = self.fetch_origin();
        describe_new_session(&input)
    }

    // ─── reading for the folder ─────────────────────────────────────────────────

    /// The page watches the folder: a repository's branches, worktrees and naming templates are read when it is chosen
    /// (`useRepositoryDetail`, `useWorktrees`, the naming watch). With no server the mock's stay, and only the repository
    /// they belong to uses them.
    pub fn read_folder_feeds(&mut self) {
        let key = self.target_key();
        if self.feed_base(&key).is_none() {
            return;
        }
        let path = self.repository_path();
        if self.feeds.detail.path != path {
            self.read_detail(&key, path.as_deref());
        }
        if self.feeds.worktrees.path != path {
            self.read_worktrees(&key, path.as_deref());
        }
        if let Some(p) = path.as_deref()
            && (self.feeds.naming.loaded_for.as_deref() != Some(p) || self.feeds.naming.loaded_on.as_deref() != Some(key.as_str()))
        {
            self.read_naming(&key, Some(p));
        }
    }

    /// A remembered worktree that's gone falls back to a new one once the list arrives (the watch on `isLoadingWorktrees`).
    pub(crate) fn drop_missing_worktree(&mut self) {
        let Some(Workspace::Existing(path)) = self.new_draft.draft().map(|d| d.workspace.clone()) else { return };
        if self.worktrees_loading() {
            return;
        }
        if !self.feeds.worktrees.value.as_ref().is_some_and(|w| w.iter().any(|w| w.path == path))
            && let Some(d) = self.new_draft.draft_mut()
        {
            d.workspace = Workspace::New;
        }
    }

    /// The label of an existing worktree on the chip: its branch, else its folder's name (B4.3).
    pub fn worktree_chip_label(&self, path: &str) -> String {
        let found = self.feeds.worktrees.value.as_ref().and_then(|w| w.iter().find(|w| w.path == path)).and_then(|w| w.branch.clone());
        found.unwrap_or_else(|| base_name(path).to_string())
    }
}

// ─── scenes ─────────────────────────────────────────────────────────────────────

/// A state of the two pickers for the pictures. The data is the scratch repository Fleet's own client shows beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    /// The workspace menu with a worktree picked.
    WorkspaceMenu,
    /// The same, when a session was started in that worktree before: it is listed first and marked.
    WorkspaceLastUsed,
    /// The base menu on the default.
    BaseMenu,
    /// The base menu after a local branch was picked: Fetch origin first can't be turned on.
    BaseLocal,
    /// The base menu searched for a name nothing matches.
    BaseNothing,
    /// The plan line for a new worktree named from a message.
    PlanNew,
    /// A branch and an unfetched base picked by hand.
    PlanPicked,
    /// The current checkout, which is not on the default branch.
    PlanCurrent,
    /// A worktree that exists.
    PlanExisting,
}

/// Where the pictures' scratch repository is (the folder the scratch Fleet was made in).
const SCENE_ROOT: &str = "/tmp/fleet-native-scratch-I/src";

impl State {
    pub(crate) fn show_picker_scene(&mut self, scene: Scene) {
        use crate::mock::feeds::{detail_in, worktrees_in};
        let repo = format!("{SCENE_ROOT}/rocket");
        let seq = self.feeds.detail.begin(Some(&repo), false).unwrap_or_default();
        self.feeds.detail.finish(seq, Ok(detail_in(SCENE_ROOT)));
        let seq = self.feeds.worktrees.begin(Some(&repo), true).unwrap_or_default();
        self.feeds.worktrees.finish(seq, Ok(worktrees_in(SCENE_ROOT)));
        let key = self.target_key();
        let used_last = format!("{SCENE_ROOT}/rocket-worktrees/spike-cache");
        if scene == Scene::WorkspaceLastUsed {
            self.sidebar.update_new_session_defaults(&key, |d| {
                d.remember(&Folder::Repository(repo.clone()), &Workspace::Existing(used_last.clone()), None)
            });
        }
        self.open_new_session_page(None);
        self.set_folder(Folder::Repository(repo), true);
        let message = "Fix the login redirect".to_string();
        if let Some(d) = self.new_draft.draft_mut() {
            d.message = message;
            match scene {
                Scene::PlanCurrent => d.workspace = Workspace::Current,
                Scene::PlanExisting | Scene::WorkspaceMenu => d.workspace = Workspace::Existing(used_last),
                Scene::PlanPicked => {
                    d.base_branch = Some("origin/release/2.0".into());
                    d.fetch_origin = false;
                    d.branch_name = "hotfix/login".into();
                }
                Scene::BaseLocal => d.base_branch = Some("spike-cache".into()),
                _ => {}
            }
        }
        match scene {
            Scene::WorkspaceMenu | Scene::WorkspaceLastUsed => self.open_workspace_menu(),
            Scene::BaseMenu | Scene::BaseLocal => self.open_base_menu(),
            Scene::BaseNothing => {
                self.open_base_menu();
                self.pickers_ns.base_query = TextEdit::new("nothing like it");
            }
            _ => {}
        }
    }
}
