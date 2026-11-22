//! What the Workflows page shows, shared by its list and its detail (`composables/use-workflows-nav.ts`,
//! `use-workflows-machine.ts`, `use-workflows-feature.ts`): the Library for the Run box's repository, which workflow is
//! open, the machine the view shows, and Turn on. Plain Rust apart from the `State` methods that start a request on a thread.
use super::runs::WorkflowRuns;
use super::{Library, WORKFLOWS_PREFERENCE_KEY, Workflow};
use crate::State;
use crate::fleet::machine_choice::MachineChoice;
use crate::fleet::new_session::request::Folder;
use crate::fleet::settings::HOME_MACHINE_KEY;

/// The workflow the page opens on.
pub const BUILD_A_FEATURE: &str = "builtin:build-a-feature";

/// The words for a library that wouldn't load, when the server gave none.
pub const LIBRARY_FALLBACK: &str = "Couldn't load the workflows.";

/// How a row's word is coloured (`workflow-row__meta--*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetaTone {
    Plain,
    Working,
    Wait,
    With,
    Error,
}

/// `rowMeta`: the word at the end of a Library row; the first that holds. A workflow with unsaved edits says Edited, one with
/// errors can't be read, then the latest run's state, else its step count.
pub fn row_meta(workflow: &Workflow, latest: Option<&super::Run>, unsaved: bool) -> (String, MetaTone) {
    use super::RunStatus;
    if unsaved {
        return ("Edited".into(), MetaTone::Wait);
    }
    if !workflow.errors.is_empty() {
        return ("Can't read".into(), MetaTone::Error);
    }
    match latest.map(|r| (r.status, r.with_you.is_some())) {
        Some((RunStatus::Waiting, _)) => ("Needs you".into(), MetaTone::Wait),
        Some((RunStatus::Running, true)) => ("With you".into(), MetaTone::With),
        Some((RunStatus::Running, false)) => ("Running".into(), MetaTone::Working),
        _ => {
            let n = workflow.steps.len();
            (format!("{n} step{}", if n == 1 { "" } else { "s" }), MetaTone::Plain)
        }
    }
}

/// A workflow's latest run: the store's runs are newest first.
pub fn latest_run<'a>(runs: &'a WorkflowRuns, workflow: &Workflow) -> Option<&'a super::Run> {
    runs.ordered().into_iter().find(|r| r.workflow_id == workflow.id)
}

#[derive(Debug, Default)]
pub struct Nav {
    /// The machine the list and the pane show; the machine you're on until the view's switch picks another.
    pub choice: MachineChoice,
    active: Option<String>,
    /// The repository the Run box runs in; its `.weave/workflows` are listed under "This repo".
    pub(crate) folder: Option<Folder>,
    pub has_chosen_folder: bool,
    pub library: Option<Library>,
    pub library_error: Option<String>,
    pub loading_library: bool,
    /// The repository the library on screen was loaded for: until the one picked loads, its workflows are still coming.
    pub(crate) library_repository: Option<String>,
    generation: u64,
    /// A repository's workflow shows the run view instead of the designer (Try it).
    pub showing_runs: bool,
    /// The workflow open in the designer with unsaved edits, so its row says Edited (`keep_workflow_frame` sets it).
    pub unsaved: Option<String>,
    /// A test's say before another workflow opens; the real guard is the editor's own question (`workflow_leave`).
    pub leave_guard: Option<fn(&State) -> bool>,
    /// The Run box under the open workflow (piece D).
    pub run_box: super::run_box::RunBox,
    /// The Model roles dialog is open (piece E draws it).
    pub roles_open: bool,
    /// The dialog's own state: its harness, the role being saved, the menu open.
    pub roles: super::roles::Roles,
    /// The stepper under a step session's header: its switch and the error under it (piece F).
    pub stepper: super::stepper::Stepper,
    /// The run card above the composer: its note, the answer on its way and the words of a refusal (piece G).
    pub card: super::card::Card,
    /// The open workflow file: its draft and text, the checks and the saves (piece L; piece M draws it).
    pub editor: super::editor::Editor,
    /// The editor's frame: its toast and the file the page asked for (piece M).
    pub frame: super::frame::Frame,
    /// The finish bar above the composer: its note, the press on its way and the words of a refusal (piece H).
    pub finish: super::finish::Finish,
    /// Turn on is saving.
    pub turning_on: bool,
    /// Something asked for the library to be read again: the folder changed, or Turn on saved.
    pub(crate) reload_wanted: bool,
    /// The machine the state above belongs to; another machine forgets it.
    seen_machine: Option<String>,
    /// The page's machine preferences were asked for since the page opened.
    prefs_asked: bool,
}

impl Nav {
    pub fn active_id(&self) -> &str {
        self.active.as_deref().unwrap_or(BUILD_A_FEATURE)
    }

    /// `repositoryPath`.
    pub fn repository_path(&self) -> Option<&str> {
        match &self.folder {
            Some(Folder::Repository(path)) => Some(path),
            _ => None,
        }
    }

    pub fn built_ins(&self) -> Vec<&Workflow> {
        self.library.iter().flat_map(|l| l.workflows.iter()).filter(|w| w.built_in).collect()
    }

    pub fn repo_workflows(&self) -> Vec<&Workflow> {
        self.library.iter().flat_map(|l| l.workflows.iter()).filter(|w| !w.built_in).collect()
    }

    pub fn active_workflow(&self) -> Option<&Workflow> {
        self.library.as_ref()?.workflows.iter().find(|w| w.id == self.active_id())
    }

    /// `isLoadingRepository`: the repository just picked is still loading, so the list's repo part waits for it instead of
    /// saying it has none.
    pub fn is_loading_repository(&self) -> bool {
        self.loading_library && self.library_repository.as_deref() != self.repository_path()
    }

    /// `setFolder`: the Run box's folder; a different repository reads the library again.
    pub fn set_folder(&mut self, folder: Folder, chosen: bool) {
        let before = self.repository_path().map(str::to_string);
        self.folder = Some(folder);
        self.has_chosen_folder |= chosen;
        if self.repository_path() != before.as_deref() {
            self.reload_wanted = true;
        }
    }

    pub fn folder(&self) -> Option<&Folder> {
        self.folder.as_ref()
    }

    /// `moveTo`: opens a workflow and leaves the run view.
    pub fn move_to(&mut self, id: &str) {
        if id == self.active_id() {
            return;
        }
        self.active = Some(id.to_string());
        self.showing_runs = false;
        self.run_box.workflow_changed();
    }

    /// The start of a read: its number, and the directory it is for. A read that finishes after a newer one began is dropped.
    fn begin_reload(&mut self) -> (u64, Option<String>) {
        self.generation += 1;
        self.reload_wanted = false;
        self.loading_library = true;
        self.library_error = None;
        (self.generation, self.repository_path().map(str::to_string))
    }

    /// The end of a read (`reload`): the first workflow opens when the open one isn't in the new library.
    fn finish_reload(&mut self, generation: u64, directory: Option<String>, result: Result<Library, String>) {
        if generation != self.generation {
            return;
        }
        self.loading_library = false;
        match result {
            Ok(library) => {
                if !library.workflows.iter().any(|w| w.id == self.active_id()) {
                    self.active = Some(library.workflows.first().map_or(BUILD_A_FEATURE, |w| w.id.as_str()).to_string());
                }
                self.library = Some(library);
                self.library_repository = directory;
            }
            Err(error) => self.library_error = Some(error),
        }
    }

    /// Another machine: nothing of the last one's library, folder or open workflow carries over. True when a library had
    /// been loaded, which is read again.
    fn forget_machine(&mut self) -> bool {
        let had = self.library.is_some();
        self.generation += 1;
        self.library = None;
        self.library_repository = None;
        self.library_error = None;
        self.loading_library = false;
        self.folder = None;
        self.has_chosen_folder = false;
        self.active = None;
        self.showing_runs = false;
        self.unsaved = None;
        self.run_box = super::run_box::RunBox::default();
        had
    }

    /// Back in the state of a first visit; the page opens on the machine you're on.
    pub fn reset_visit(&mut self) {
        self.choice.reset();
        self.prefs_asked = false;
        self.run_box.asked = false;
        self.run_box.initial_done = false;
    }
}

impl State {
    /// The machine the view shows (`useWorkflowsMachine().key`).
    pub fn workflows_machine(&self) -> String {
        self.workflows_nav.choice.key(&self.machines).to_string()
    }

    /// Whether Workflows is on for the machine the view shows; `None` until its preferences have loaded (no flash of "is off").
    pub fn workflows_feature(&self) -> Option<bool> {
        let machine = self.workflows_machine();
        let (value, fetched) = if machine == HOME_MACHINE_KEY {
            (self.pickers.prefs.get(WORKFLOWS_PREFERENCE_KEY), self.pickers.prefs.fetched)
        } else {
            let prefs = self.feeds.prefs.get(&machine);
            (prefs.and_then(|p| p.get(WORKFLOWS_PREFERENCE_KEY)), prefs.is_some_and(|p| p.fetched))
        };
        // With no Fleet to ask (scenes, tests) the preference is whatever is kept here.
        if !fetched && self.link.is_some() {
            return None;
        }
        Some(value == Some("true"))
    }

    /// Opening the page: it starts on the machine you're on.
    pub fn enter_workflows(&mut self) {
        if self.rail != crate::Rail::Workflows {
            self.workflows_nav.reset_visit();
        }
        self.rail = crate::Rail::Workflows;
    }

    /// `setActiveWorkflow`: through the leave guard, which can refuse.
    pub fn set_active_workflow(&mut self, id: &str) {
        if id == self.workflows_nav.active_id() {
            return;
        }
        if self.workflows_nav.leave_guard.is_some_and(|guard| !guard(self)) {
            return;
        }
        // Unsaved edits ask first (`setLeaveGuard`); the question's Yes moves.
        self.workflow_leave(super::frame::Leave::Open(id.to_string()));
    }

    /// The link `/workflows?workflow=<id>&folder=<path>` (the Fix a bug starter): the Run box's folder, and the workflow open.
    /// Nothing starts.
    pub fn open_workflows_link(&mut self, workflow: &str, folder: Option<&str>) {
        self.enter_workflows();
        if let Some(path) = folder {
            self.workflows_nav.set_folder(Folder::Repository(path.to_string()), true);
        }
        self.set_active_workflow(workflow);
    }

    /// `reload`: the Library for the Run box's repository, on the machine shown. A newer read wins.
    pub fn workflows_reload(&mut self, tx: &mut rune::Tx) {
        let (generation, directory) = self.workflows_nav.begin_reload();
        let machine = self.workflows_machine();
        if self.link.is_none() {
            // No Fleet to ask: the invented library the scenes and tests draw.
            let library = crate::mock::workflows::library(directory.as_deref());
            self.workflows_nav.finish_reload(generation, directory, Ok(library));
            return;
        }
        let dir = directory.clone();
        self.workflow_library(&machine, directory.as_deref(), tx, move |s, _, result| {
            s.workflows_nav.finish_reload(generation, dir, result);
        });
    }

    /// Turn on (`WorkflowsPane.turnOn`): saves the preference on the machine shown, then reads the library and the runs again,
    /// because what loaded in the gap was refused.
    pub fn workflows_turn_on(&mut self) {
        if self.workflows_nav.turning_on {
            return;
        }
        self.workflows_nav.turning_on = true;
        let machine = self.workflows_machine();
        let on = machine.clone();
        self.save_pref_on_machine(&machine, WORKFLOWS_PREFERENCE_KEY, "true", move |s, _| {
            s.workflows_nav.turning_on = false;
            s.workflows_nav.reload_wanted = true;
            s.workflow_runs_ensure_loaded(&on);
        });
    }

    /// The page's own upkeep, once a frame (`poll`): follows the machine, asks the machine's preferences, reads the library
    /// once Workflows is on and again when asked. True when something changed.
    pub fn keep_workflows(&mut self, tx: &mut rune::Tx) -> bool {
        if let Some(request) = self.starters.workflow_request.take() {
            self.open_workflows_link(&request.workflow, request.folder.as_deref());
        }
        if self.keep_run_session(tx) {
            return true;
        }
        if self.rail != crate::Rail::Workflows {
            // The flag outlives the page and links set it before navigating here, so it is cleared on the way out.
            if self.workflows_nav.roles_open {
                self.close_roles();
            }
            // The Run box's repository is followed from any page (a session row can be the first to ask).
            if self.workflows_nav.reload_wanted && self.workflows_feature() == Some(true) && !self.workflows_nav.turning_on {
                self.workflows_reload(tx);
                return true;
            }
            return false;
        }
        let machine = self.workflows_machine();
        let mut changed = false;
        if self.workflows_nav.seen_machine.as_deref().is_some_and(|seen| seen != machine) && self.workflows_nav.forget_machine() {
            self.workflows_nav.reload_wanted = true;
        }
        if self.workflows_nav.seen_machine.as_deref() != Some(machine.as_str()) {
            self.workflows_nav.seen_machine = Some(machine.clone());
            changed = true;
        }
        if machine != HOME_MACHINE_KEY && !self.workflows_nav.prefs_asked {
            self.workflows_nav.prefs_asked = true;
            self.read_preferences(&machine);
        }
        // While Turn on saves, the server still refuses the library: read it once, after the save, not in the gap too.
        if self.workflows_feature() != Some(true) || self.workflows_nav.turning_on {
            return changed;
        }
        let nav = &self.workflows_nav;
        let first = nav.library.is_none() && !nav.loading_library && nav.library_error.is_none();
        if nav.reload_wanted || first {
            self.workflows_reload(tx);
            changed = true;
        }
        changed | self.keep_run_box() | self.keep_roles(tx)
    }
}
