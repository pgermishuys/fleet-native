//! The Run box's state (component 18, piece D; `WorkflowRunBox.vue`, `ProjectPicker.vue`): what to build, which optional steps
//! run, Check with me, the base, the harness and the project, and the rules for when Run is on and what is said under the
//! chips. Plain state changes: nothing here draws. The folder, base and harness menus are the New session page's, pointed at
//! this box while the Workflows page is open (`State::run_box_active`).
use super::helpers::skills_in_use;
use super::runs::StartRequest;
use super::{ModelChoice, Role, Workflow};
use crate::State;
use crate::fleet::new_session::harness_choice::{HarnessEntry, Harnesses};
use crate::fleet::new_session::request::Folder;
use rune::{TextEdit, Tx};
use std::collections::HashSet;

pub const REQUEST_KEY: &str = "workflow-request";
pub const RUN_KEY: &str = "workflow-run";
pub const PROJECT_CHIP: &str = "workflow-project";
pub const PROJECT_POPOVER: &str = "workflow-project";

/// What the box holds. The workflow it is for is the Library's open one: another workflow starts it over.
#[derive(Debug, Default)]
pub struct RunBox {
    pub request: TextEdit,
    /// The optional steps switched on, by id.
    pub optional_on: HashSet<String>,
    /// "Check with me after each step": every agent step is one the user finishes, from the first.
    pub check_with_me: bool,
    /// The base picked; none is the repository's default.
    pub base_branch: Option<String>,
    /// The harness picked; none is the default one when it runs workflows, else the first that does.
    pub harness_choice: Option<String>,
    /// The project the run's sessions go in; none is Scratch.
    pub project_id: Option<String>,
    /// The models picked for this run, a role each (piece E fills it).
    pub overrides: Vec<(Role, ModelChoice)>,
    pub starting: bool,
    pub error: Option<String>,
    /// The feeds the box needs were asked for on this visit.
    pub(crate) asked: bool,
    /// The folder started as the page's initial one (once repositories are known).
    pub(crate) initial_done: bool,
    /// A run just started: its first session opens as soon as the machine's list holds it (machine, session, give up at).
    pub(crate) open_when_listed: Option<(String, String, i64)>,
}

impl RunBox {
    /// The workflow changed: the optional steps, the overrides and the error are the last one's.
    pub(crate) fn workflow_changed(&mut self) {
        self.optional_on.clear();
        self.overrides.clear();
        self.error = None;
    }

    pub fn toggle_optional(&mut self, id: &str) {
        if !self.optional_on.remove(id) {
            self.optional_on.insert(id.to_string());
        }
    }
}

/// What is said under the chips: the first that holds (`cannotRun`, `noHarness`, `skillOff`, `startError`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warning {
    FixFile,
    NoHarness(String),
    SkillOff { step: String, skill: String },
    StartError(String),
}

/// The harnesses a run can use: the ones turned on that give the step tool to step sessions.
pub fn workflow_harnesses(harnesses: &Harnesses) -> Vec<&HarnessEntry> {
    harnesses.enabled().into_iter().filter(|h| h.supports_workflow_steps).collect()
}

/// `harnessType`: the one picked; else the default when it runs workflows, else the first that does.
pub fn harness_type(harnesses: &Harnesses, picked: Option<&str>) -> String {
    if let Some(p) = picked {
        return p.to_string();
    }
    let default = harnesses.default_type();
    let runs = workflow_harnesses(harnesses);
    if runs.iter().any(|h| h.kind == default) { default } else { runs.first().map_or(default, |h| h.kind.clone()) }
}

/// `noHarness`: none of the harnesses that run workflows is installed and on, by name. Said only once the list has loaded.
pub fn no_harness(harnesses: &Harnesses) -> Option<String> {
    if harnesses.all.is_empty() || !workflow_harnesses(harnesses).is_empty() {
        return None;
    }
    let names: Vec<&str> = harnesses.all.iter().filter(|h| h.supports_workflow_steps).map(|h| h.display_name.as_str()).collect();
    let list = match names.as_slice() {
        // Fleet's own words would say "undefined" here; no harness of this server runs workflows.
        [] => return Some("None of the harnesses here can run workflows yet.".into()),
        [one] => one.to_string(),
        [init @ .., last] => format!("{} or {last}", init.join(", ")),
    };
    Some(format!("Workflows run on {list}, and none of them is installed and on."))
}

/// `skillOff`: a built-in skill an enabled step uses that is off, so the run wouldn't start. The step's title and the skill.
pub fn skill_off(
    skills: Option<&[crate::fleet::skills::BuiltInSkill]>,
    workflow: &Workflow,
    optional_on: &HashSet<String>,
) -> Option<(String, String)> {
    let shipped = skills?;
    skills_in_use(workflow, optional_on).into_iter().find_map(|(step, skill)| {
        let off = shipped.iter().any(|s| s.name == skill && !s.enabled);
        off.then(|| (workflow.steps.iter().find(|s| s.id == step).map_or(step, |s| s.title.as_str()).to_string(), skill.to_string()))
    })
}

impl State {
    /// The Run box is the one on screen, so the folder, base and harness menus work on it, not on the New session draft.
    pub fn run_box_active(&self) -> bool {
        self.rail == crate::Rail::Workflows
    }

    pub fn run_box_harnesses(&self) -> Harnesses {
        self.harness_choice(&self.workflows_machine())
    }

    pub fn run_box_harness_type(&self) -> String {
        harness_type(&self.run_box_harnesses(), self.workflows_nav.run_box.harness_choice.as_deref())
    }

    pub fn run_box_skill_off(&self, workflow: &Workflow) -> Option<(String, String)> {
        let run_box = &self.workflows_nav.run_box;
        skill_off(self.skills.list.as_deref(), workflow, &run_box.optional_on)
    }

    /// The words under the chips, if any (the foot is for none).
    pub fn run_box_warning(&self, workflow: &Workflow) -> Option<Warning> {
        if !workflow.errors.is_empty() {
            return Some(Warning::FixFile);
        }
        if let Some(words) = no_harness(&self.run_box_harnesses()) {
            return Some(Warning::NoHarness(words));
        }
        if let Some((step, skill)) = self.run_box_skill_off(workflow) {
            return Some(Warning::SkillOff { step, skill });
        }
        self.workflows_nav.run_box.error.clone().map(Warning::StartError)
    }

    /// `canRun`.
    pub fn run_box_can_run(&self, workflow: &Workflow) -> bool {
        let run_box = &self.workflows_nav.run_box;
        !run_box.starting
            && matches!(self.run_box_warning(workflow), None | Some(Warning::StartError(_)))
            && !workflow_harnesses(&self.run_box_harnesses()).is_empty()
            && !run_box.request.text.trim().is_empty()
            && self.workflows_nav.repository_path().is_some()
    }

    /// The folder menu's pick for the Run box (`setFolder`): a repository only; another repository forgets the base.
    pub(crate) fn run_box_set_folder(&mut self, folder: Folder, chosen: bool) {
        let Folder::Repository(path) = &folder else { return };
        if self.workflows_nav.repository_path() != Some(path.as_str()) {
            self.workflows_nav.run_box.base_branch = None;
        }
        self.workflows_nav.set_folder(folder, chosen);
    }

    /// The base the menu picked; the default is no choice.
    pub(crate) fn run_box_choose_base(&mut self, name: &str) {
        let default = self.default_base();
        self.workflows_nav.run_box.base_branch = if default.as_deref() == Some(name) { None } else { Some(name.to_string()) };
    }

    pub fn run_box_choose_harness(&mut self, kind: &str) {
        self.workflows_nav.run_box.harness_choice = Some(kind.to_string());
        self.close_chip_menu();
    }

    pub fn run_box_choose_project(&mut self, id: Option<String>) {
        self.workflows_nav.run_box.project_id = id;
        self.close_run_box_menu();
    }

    pub fn close_run_box_menu(&mut self) {
        if self.run_box_project_menu_open() {
            self.ui.open = None;
        }
    }

    pub fn run_box_project_menu_open(&self) -> bool {
        self.ui.open == Some(crate::ui::Open::Popover(PROJECT_POPOVER))
    }

    /// The projects the Project chip offers: the user's, never Scratch.
    pub fn run_box_projects(&self) -> Vec<crate::fleet::groups::ProjectInfo> {
        self.new_session_projects(&self.workflows_machine())
    }

    /// `handleKeydown` / the Run button: with no repository the folder menu opens instead; otherwise the run starts.
    pub fn run_box_run(&mut self, tx: &mut Tx) {
        if self.workflows_nav.repository_path().is_none() {
            self.open_folder_menu(tx);
            return;
        }
        let Some(workflow) = self.workflows_nav.active_workflow().cloned() else { return };
        if !self.run_box_can_run(&workflow) {
            return;
        }
        let request = self.run_box_request(&workflow);
        let machine = self.workflows_machine();
        let run_box = &mut self.workflows_nav.run_box;
        run_box.starting = true;
        run_box.error = None;
        self.workflow_start(
            &machine,
            &request,
            tx,
            Box::new(move |s, tx, result| {
                let run_box = &mut s.workflows_nav.run_box;
                run_box.starting = false;
                match result {
                    Ok(run) => {
                        run_box.request = TextEdit::new("");
                        if let Some(first) = run.sessions.first() {
                            let machine = s.workflows_machine();
                            // The new session may not be in the list yet: it opens once it is.
                            if !s.open_workflow_session(&machine, &first.session_id, tx) {
                                let until = crate::fleet::link::now_ms() + 30_000;
                                s.workflows_nav.run_box.open_when_listed = Some((machine.clone(), first.session_id.clone(), until));
                                s.list_again_on(&machine);
                            }
                        }
                    }
                    Err(words) => run_box.error = Some(words),
                }
            }),
        );
    }

    /// The body of `POST /api/workflows/runs` for the box as it is.
    pub fn run_box_request(&self, workflow: &Workflow) -> StartRequest {
        let run_box = &self.workflows_nav.run_box;
        let optional: Vec<String> = workflow.steps.iter().filter(|s| run_box.optional_on.contains(&s.id)).map(|s| s.id.clone()).collect();
        StartRequest {
            workflow_id: workflow.id.clone(),
            directory: self.workflows_nav.repository_path().unwrap_or_default().to_string(),
            request: run_box.request.text.trim().to_string(),
            base_branch: run_box.base_branch.clone(),
            harness_type: Some(self.run_box_harness_type()),
            harness_profile_id: None,
            optional_steps: Some(optional),
            role_overrides: Some(run_box.overrides.clone()),
            check_with_me: Some(run_box.check_with_me),
            project_id: run_box.project_id.clone(),
        }
    }

    /// What `scheduleDraft()` hands "Repeat on a schedule…": the request, the optional steps, the base and the harness picked
    /// here (component 19 reads it).
    pub fn run_box_schedule_draft(&self) -> (String, Vec<String>, Option<String>, Option<String>) {
        let run_box = &self.workflows_nav.run_box;
        let mut steps: Vec<String> = run_box.optional_on.iter().cloned().collect();
        steps.sort();
        (run_box.request.text.trim().to_string(), steps, run_box.base_branch.clone(), run_box.harness_choice.clone())
    }

    /// The session of a run just started opens once the list holds it, if the user is still on the page.
    pub(crate) fn keep_run_session(&mut self, tx: &mut Tx) -> bool {
        let Some((machine, id, until)) = self.workflows_nav.run_box.open_when_listed.clone() else { return false };
        if self.rail != crate::Rail::Workflows || crate::fleet::link::now_ms() > until {
            self.workflows_nav.run_box.open_when_listed = None;
            return false;
        }
        if self.open_workflow_session(&machine, &id, tx) {
            self.workflows_nav.run_box.open_when_listed = None;
            return true;
        }
        false
    }

    /// The box's upkeep, once a frame while the page is open: asks for what it shows once, reads the repository's branches
    /// when the folder changes, and starts the folder as the New session page's initial one (`applyInitialFolder`).
    pub(crate) fn keep_run_box(&mut self) -> bool {
        let machine = self.workflows_machine();
        if self.feed_base(&machine).is_none() {
            return false;
        }
        let mut changed = false;
        if !self.workflows_nav.run_box.asked {
            self.workflows_nav.run_box.asked = true;
            self.read_repositories(&machine, false);
            self.read_harnesses(&machine, false);
            self.skills_ensure_loaded();
            changed = true;
        }
        let feed = self.feeds.repos(&machine);
        let ready = feed.scanned_at.is_some() || feed.error.is_some();
        if ready && !self.workflows_nav.run_box.initial_done {
            self.workflows_nav.run_box.initial_done = true;
            if !self.workflows_nav.has_chosen_folder && self.workflows_nav.folder().is_none() {
                let paths: Vec<String> = feed.list.iter().map(|r| r.path.clone()).collect();
                if let Some(folder @ Folder::Repository(_)) = self.sidebar.new_session_defaults(&machine).initial_folder(&paths) {
                    self.workflows_nav.set_folder(folder, false);
                }
            }
            changed = true;
        }
        if let Some(path) = self.workflows_nav.repository_path().map(str::to_string)
            && self.feeds.detail.path.as_deref() != Some(path.as_str())
        {
            self.read_detail(&machine, Some(&path));
            changed = true;
        }
        changed
    }
}
