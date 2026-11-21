//! A workflow target (rows 1.7, 4.10, 5.9, 12.1-12.9, 13.3): the Runs chip's kind (`AutomationTargetPicker.vue`), the
//! Library of the run's repository on the machine the runs go to (`loadLibrary` in `AutomationComposer.vue`), the workflow
//! and its optional steps (`AutomationWorkflowPicker.vue`), the harnesses that run workflows (`workflowHarness`), the
//! warning while Workflows are off (`workflowsOffWarning`), a workflow run's session (`sessionOf`) and the Library's Repeat on
//! a schedule… (`startCreateFromWorkflow`). Plain state: the chips and menus are `view/automations/workflow.rs`.
use super::composer::{ComposerState, TargetKind};
use super::model::AutomationRun;
use crate::fleet::new_session::request::{Folder, Workspace};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::{Run, Workflow};
use crate::ui::Open;
use crate::{State, Tx};

pub const TARGET_POPOVER: &str = "automation-target";
pub const TARGET_CHIP: &str = "automation-target-chip";
pub const WORKFLOW_POPOVER: &str = "automation-workflow";
pub const WORKFLOW_CHIP: &str = "automation-workflow-chip";

pub const WORKFLOWS_OFF: &str = "Workflows are turned off in Settings.";
pub const PICK_REPOSITORY: &str = "Pick the repository it runs in first.";
const LIBRARY_FAILED: &str = "Couldn't load the workflows.";

/// What `startCreateFromWorkflow` is handed: the Library's workflow and the Run box's choices (`WorkflowAutomationSeed`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkflowSeed {
    pub workflow_id: String,
    /// The repository the Run box has.
    pub folder: String,
    pub request: String,
    pub optional_steps: Vec<String>,
    pub base_branch: Option<String>,
    pub harness_type: Option<String>,
}

impl ComposerState {
    /// A fresh draft that runs the seed's workflow: its request as the text, its repository chosen, a new worktree each run
    /// from its base, its optional steps and harness; When is still to add.
    pub fn from_workflow(seed: &WorkflowSeed) -> ComposerState {
        let mut c = ComposerState {
            folder: Some(Folder::Repository(seed.folder.clone())),
            has_chosen_folder: true,
            workspace: Workspace::New,
            base_branch: seed.base_branch.clone(),
            target_type: "workflow".into(),
            workflow_id: Some(seed.workflow_id.clone()),
            workflow_steps: seed.optional_steps.clone(),
            harness_type: seed.harness_type.clone(),
            ..ComposerState::fresh()
        };
        c.set_text(seed.request.clone());
        c
    }
}

/// What the Library was asked for: `watch([isWorkflow, repositoryPath, isWorkflowsEnabled])`, for the composer on screen and
/// the machine its runs go to.
#[derive(Clone, Debug, PartialEq, Eq)]
struct LibraryKey {
    editing: bool,
    workflow: bool,
    machine: String,
    path: Option<String>,
    on: bool,
}

/// The composer's Library: what the workflow chip lists, or why it can't.
#[derive(Debug, Default)]
pub struct LibraryFeed {
    key: Option<LibraryKey>,
    /// The repository's workflows; `None` until they have loaded.
    pub workflows: Option<Vec<Workflow>>,
    /// Why there's none to show: Workflows off, no repository, or the load's error.
    pub note: Option<String>,
    pub loading: bool,
    /// A newer load wins: an answer for an older one is dropped.
    generation: u64,
}

impl LibraryFeed {
    /// The workflows, when they were loaded for the composer asked about (the draft's or the open automation's).
    pub(super) fn for_composer(&self, editing: bool) -> Option<&[Workflow]> {
        self.workflows.as_deref().filter(|_| self.key.as_ref().is_some_and(|k| k.editing == editing))
    }
}

/// The session a run opens (`sessionOf`): a workflow run's waiting session, else its newest step's, else the run's own.
pub fn session_of<'a>(run: &'a AutomationRun, workflow_run: Option<&'a Run>) -> Option<&'a str> {
    let waiting = workflow_run.and_then(|w| w.waiting.as_ref()?.session_id.as_deref()).filter(|s| !s.is_empty());
    waiting.or_else(|| workflow_run.and_then(|w| w.sessions.last()).map(|s| s.session_id.as_str())).or(run.session_id.as_deref())
}

impl State {
    /// `isWorkflowsEnabled`: the Workflows switch in Settings → Labs, this machine's preference.
    pub fn automation_workflows_on(&self) -> bool {
        crate::fleet::rails::Features::from_prefs(&self.pickers.prefs).workflows
    }

    /// `workflowsOffWarning`: the open automation runs a workflow and Workflows are off (row 4.10).
    pub fn automation_workflows_off_warning(&self) -> bool {
        !self.automation_workflows_on() && self.open_automation().is_some_and(|a| a.target_type.as_deref() == Some("workflow"))
    }

    /// The Runs chip shows while Workflows are on, or for a composer that already runs a workflow (row 12.1).
    pub fn automation_shows_target_chip(&self) -> bool {
        self.automation_workflows_on() || self.automations_nav.composer(self.composer_editing()).is_workflow()
    }

    /// The harnesses a workflow can run on: those turned on that give the step tool to step sessions, of the machine the runs
    /// go to (`workflowHarnesses`), as (type, name).
    pub fn automation_workflow_harnesses(&self) -> Vec<(String, String)> {
        let machine = self.composer_machine(self.composer_editing());
        let all = self.harness_choice(&machine);
        crate::fleet::workflows::run_box::workflow_harnesses(&all).into_iter().map(|h| (h.kind.clone(), h.display_name.clone())).collect()
    }

    /// The harness a workflow runs on, as the chip shows it (row 12.6).
    pub fn automation_workflow_harness(&self) -> String {
        self.with_composer(self.composer_editing(), |c, ctx| c.workflow_harness(ctx)).unwrap_or_default()
    }

    /// The composer's Library, while it is for the composer on screen.
    pub fn automation_library(&self) -> &LibraryFeed {
        &self.automations_nav.composer.library
    }

    /// The workflow chip's words: the workflow's name, else its id without `builtin:`/`repo:`, else "Pick a workflow".
    pub fn automation_workflow_label(&self) -> String {
        let Some(id) = self.automations_nav.composer(self.composer_editing()).workflow_id.clone() else { return "Pick a workflow".into() };
        let named = self.automation_library().workflows.as_ref().and_then(|l| l.iter().find(|w| w.id == id)).map(|w| w.name.clone());
        named.unwrap_or_else(|| id.strip_prefix("builtin:").or_else(|| id.strip_prefix("repo:")).unwrap_or(&id).to_string())
    }

    /// The chosen workflow's optional steps, each a switch, in the workflow's order (`optionalSteps`).
    pub fn automation_optional_steps(&self) -> Vec<(String, String, Option<String>)> {
        let c = self.automations_nav.composer(self.composer_editing());
        let Some(id) = c.workflow_id.as_deref() else { return Vec::new() };
        let Some(w) = self.automation_library().workflows.as_ref().and_then(|l| l.iter().find(|w| w.id == id)) else { return Vec::new() };
        w.steps.iter().filter(|s| s.optional).map(|s| (s.id.clone(), s.title.clone(), s.optional_hint.clone())).collect()
    }

    /// `loadLibrary`, when what it depends on changes: nothing for a session target; a note with Workflows off or no
    /// repository; else the repository's Library on the machine the runs go to. With no server the invented Library stands for
    /// it, so scenes come out the same. True when something changed.
    pub(crate) fn keep_automation_library(&mut self, tx: &mut Tx) -> bool {
        let (changed, read) = self.settle_automation_library();
        let Some((mine, machine, path)) = read else { return changed };
        self.automations_nav.composer.library.loading = true;
        self.workflow_library(&machine, Some(&path), tx, move |s, _, result| {
            let feed = &mut s.automations_nav.composer.library;
            if feed.generation != mine {
                return;
            }
            feed.loading = false;
            match result {
                Ok(library) => s.automation_library_loaded(library.workflows),
                Err(e) => s.automations_nav.composer.library.note = Some(if e.is_empty() { LIBRARY_FAILED.into() } else { e }),
            }
        });
        changed
    }

    /// What `keep_automation_library` decides without asking a server: whether the Library's inputs changed (and the note,
    /// or the invented Library with no server), and the read to make if one is wanted (its generation, machine and path).
    pub(crate) fn settle_automation_library(&mut self) -> (bool, Option<(u64, String, String)>) {
        let editing = self.composer_editing();
        let c = self.automations_nav.composer(editing);
        let path = match &c.folder {
            Some(Folder::Repository(p)) => Some(p.clone()),
            _ => None,
        };
        let key = LibraryKey {
            editing,
            workflow: c.is_workflow(),
            machine: self.composer_machine(editing),
            path,
            on: self.automation_workflows_on(),
        };
        let feed = &mut self.automations_nav.composer.library;
        if feed.key.as_ref() == Some(&key) {
            return (false, None);
        }
        feed.generation += 1;
        let mine = feed.generation;
        feed.workflows = None;
        feed.note = None;
        feed.loading = false;
        feed.key = Some(key.clone());
        if !key.workflow {
            return (true, None);
        }
        if !key.on {
            feed.note = Some(WORKFLOWS_OFF.into());
            return (true, None);
        }
        let Some(path) = key.path else {
            feed.note = Some(PICK_REPOSITORY.into());
            return (true, None);
        };
        if self.link_for_machine(&key.machine).is_none() {
            let invented = crate::mock::workflows::library(Some(&path)).workflows;
            self.automation_library_loaded(invented);
            return (true, None);
        }
        (true, Some((mine, key.machine, path)))
    }

    /// The Library came: with no workflow chosen, the first without errors is (`watch(library)`).
    fn automation_library_loaded(&mut self, workflows: Vec<Workflow>) {
        let first = workflows.iter().find(|w| w.errors.is_empty()).map(|w| w.id.clone());
        self.automations_nav.composer.library.workflows = Some(workflows);
        let c = self.composer_mut();
        if c.workflow_id.is_none() {
            c.workflow_id = first;
        }
    }

    // ── The Runs chip ─────────────────────────────────────────────────────────

    pub fn target_menu_open(&self) -> bool {
        self.ui.open == Some(Open::Popover(TARGET_POPOVER))
    }

    pub fn open_target_menu(&mut self) {
        self.ui.open = Some(Open::Popover(TARGET_POPOVER));
    }

    /// A row of the Runs menu (`setTargetKind`, row 12.2).
    pub fn automation_set_target_kind(&mut self, kind: TargetKind) {
        self.composer_mut().set_target_kind(kind);
        self.close_strip_menus();
    }

    // ── The workflow chip ─────────────────────────────────────────────────────

    pub fn workflow_menu_open(&self) -> bool {
        self.ui.open == Some(Open::Popover(WORKFLOW_POPOVER))
    }

    pub fn open_workflow_menu(&mut self) {
        self.ui.open = Some(Open::Popover(WORKFLOW_POPOVER));
    }

    /// A workflow picked (`setWorkflow`): another one clears the steps switched on for the last. A file with errors can't be.
    pub fn automation_set_workflow(&mut self, id: &str) {
        let broken =
            self.automation_library().workflows.as_ref().and_then(|l| l.iter().find(|w| w.id == id)).is_some_and(|w| !w.errors.is_empty());
        if !broken {
            self.composer_mut().set_workflow(id);
        }
        self.close_strip_menus();
    }

    /// An optional step's switch (`toggleWorkflowStep`).
    pub fn automation_toggle_step(&mut self, id: &str) {
        self.composer_mut().toggle_workflow_step(id);
    }

    /// The harness menu's pick on the composer: the workflow runs on it (`workflowHarness`'s setter).
    pub(crate) fn automation_choose_workflow_harness(&mut self, kind: &str) {
        self.composer_mut().harness_type = Some(kind.to_string());
    }

    // ── Ways in ───────────────────────────────────────────────────────────────

    /// `startCreateFromWorkflow` (row 1.7): a fresh draft that runs the workflow, opened in create mode.
    pub fn start_create_from_workflow(&mut self, seed: &WorkflowSeed) {
        self.automations_nav.start_create_from_workflow(seed);
    }

    /// The Library's **Repeat on a schedule…** (row 13.3): a new automation that runs the open workflow in the Run box's
    /// repository with the Run box's request, steps, base and harness; Automations opens on it, the box taking the keys.
    pub fn repeat_workflow_on_schedule(&mut self, tx: &mut Tx) {
        let Some(workflow) = self.workflows_nav.active_workflow().filter(|w| w.errors.is_empty()).map(|w| w.id.clone()) else { return };
        let Some(folder) = self.workflows_nav.repository_path().map(str::to_string) else { return };
        let (request, optional_steps, base_branch, harness_type) = self.run_box_schedule_draft();
        let seed = WorkflowSeed { workflow_id: workflow, folder, request, optional_steps, base_branch, harness_type };
        self.press_rail(crate::Rail::Automations);
        self.start_create_from_workflow(&seed);
        tx.focus(crate::view::automations::composer::MESSAGE, false);
    }

    /// The workflow run an automation's run started, as this machine's store has it.
    pub(super) fn workflow_run_of(&self, run: &AutomationRun) -> Option<&Run> {
        self.workflow_runs(HOME_MACHINE_KEY).get(run.workflow_run_id.as_deref()?)
    }

    /// `ensureLoaded` while Workflows are on, so a workflow run's row opens where it is now (row 12.9).
    pub(crate) fn keep_automation_workflow_runs(&mut self) {
        if self.automation_workflows_on() {
            self.workflow_runs_ensure_loaded(HOME_MACHINE_KEY);
        }
    }
}

// ─── scenes ─────────────────────────────────────────────────────────────────────

/// What a picture of piece N shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    /// An automation open that runs Fix a bug in the invented repository with Check it runs on: New worktree, the base,
    /// the workflow chip, the step switch, the harness chip; a run that waits on you, one done.
    Workflow,
    /// The same with the workflow menu open: the built-ins, the repo's own file, the one with errors.
    WorkflowMenu,
    /// The same with Workflows off: the warning with its link to Labs, and the run that was skipped for it.
    WorkflowsOff,
}

impl State {
    pub(crate) fn show_automation_workflow(&mut self, scene: Scene) {
        use super::model::{AutomationRun, RunState};
        self.pickers.prefs.set(crate::fleet::rails::WORKFLOWS_KEY, if scene == Scene::WorkflowsOff { "false" } else { "true" });
        for a in self.automations.of_mut(HOME_MACHINE_KEY).list.iter_mut().filter(|a| a.id == "a-flow") {
            a.name = "Monday bug sweep".into();
            a.prompt = "Every Monday at 9, fix the bug in the timer that drifts after midnight.".into();
            a.workflow_id = Some("builtin:fix-a-bug".into());
            a.workflow_steps = Some(vec!["verify".into()]);
        }
        self.automations_nav.set_active("a-flow");
        self.reset_edit_state();
        self.settle_automation_library();
        let run = |id: &str, at: &str, state: RunState| AutomationRun {
            id: id.into(),
            automation_id: "a-flow".into(),
            trigger: "schedule".into(),
            scheduled_for: Some(at.into()),
            started_at: at.into(),
            state,
            ..AutomationRun::default()
        };
        let runs = if scene == Scene::WorkflowsOff {
            vec![AutomationRun {
                error: Some("Skipped: Workflows are turned off in Settings.".into()),
                ..run("r-f2", "2026-09-14T09:00:00Z", RunState::Skipped)
            }]
        } else {
            vec![
                AutomationRun {
                    session_id: Some("s-flow-2".into()),
                    workflow_run_id: Some("wr-2".into()),
                    ..run("r-f2", "2026-09-14T09:00:00Z", RunState::Waiting)
                },
                AutomationRun {
                    session_id: Some("s-flow-1".into()),
                    workflow_run_id: Some("wr-1".into()),
                    summary: Some("Opened a pull request: the timer keeps its offset across midnight.".into()),
                    ..run("r-f1", "2026-09-07T09:00:00Z", RunState::Done)
                },
            ]
        };
        self.automations_nav.runs.show(HOME_MACHINE_KEY, "a-flow", runs);
        if scene == Scene::WorkflowMenu {
            self.open_workflow_menu();
        }
    }
}
