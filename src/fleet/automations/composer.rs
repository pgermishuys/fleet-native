//! What the automation composer edits, and what it sends (`AutomationComposerState` in `composables/use-automations-nav.ts`,
//! the state rules of `AutomationComposer.vue`, `stateFor` and the repository watch of `AutomationDetailPanel.vue`; rows 7.10,
//! 7.13-7.18, 9.3, 9.4, 11.2, 12.2, 12.7). Plain Rust: no Rune and no I/O. What Fleet's component reads from its stores (the
//! automation being changed, the clock and zone, the default harness, the harnesses that run workflow steps, the Library, the
//! repository's default base) is handed in as a [`Context`], so a test fixes it.
//!
//! Fleet's component derives `when`, `prompt`, `request` and `isDirty` as computed values and keeps "Just once" honest with a
//! watch on the schedule phrase. Here they are methods, and the watch is [`ComposerState::set_text`]: every change to the
//! text goes through it.
use super::model::{Automation, AutomationRequest};
use super::schedule::{ScheduleHit, Tz, When, auto_name, from_trigger, parse_schedule, prompt_from, to_trigger};
use super::words::{Plan, PlanInput, browser_time_zone, describe_automation_plan};
use crate::composer::choice::{key_for_model, model_from_key, model_from_path, model_to_path};
use crate::fleet::new_session::request::{Folder, Workspace};
use crate::fleet::workflows::Workflow;
use std::ops::Range;

/// A new automation's target types: anything else is an older automation's (a session picked by recency or tags).
const NEW_SESSION: &str = "new_session";
const SAME_SESSION: &str = "same_session";
const WORKFLOW: &str = "workflow";

/// The title of the draft row while nothing names it yet (`draftTitle`).
pub const DRAFT_TITLE: &str = "New automation";

/// What the composer is editing: a new automation's draft, or an open automation's changes (`AutomationComposerState`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposerState {
    /// What it should do, and usually when. Change it with [`ComposerState::set_text`].
    pub text: String,
    /// A schedule chosen in the When menu, which takes over from the words in the text.
    pub manual_when: Option<When>,
    /// The text said "on Friday" and "Just once" was chosen.
    pub force_once: bool,
    /// `None` until a folder is picked or defaulted; `Some(Folder::None)` is "no folder".
    pub folder: Option<Folder>,
    /// Set once someone picked a folder, so the default doesn't replace it.
    pub has_chosen_folder: bool,
    /// `New` is a new worktree for each run; `Current` the folder as it is.
    pub workspace: Workspace,
    pub base_branch: Option<String>,
    /// `new_session`, `same_session`, `workflow`, or an older automation's target type.
    pub target_type: String,
    /// The workflow a `workflow` target runs.
    pub workflow_id: Option<String>,
    /// The workflow's optional steps switched on.
    pub workflow_steps: Vec<String>,
    pub name: String,
    /// Skip a run while the last one is still going.
    pub skip: bool,
    /// The agent runs go to; empty for the folder's default.
    pub agent: String,
    /// The model runs use, as a model selection key; empty for the agent's default.
    pub model: String,
    /// The harness the agent and model come from, or a workflow runs on; `None` for the default harness.
    pub harness_type: Option<String>,
    /// Another machine its runs go to (its key); `None` for this machine. The folder and harness are that machine's.
    pub machine_id: Option<String>,
}

impl Default for ComposerState {
    /// `freshComposerState`.
    fn default() -> ComposerState {
        ComposerState {
            text: String::new(),
            manual_when: None,
            force_once: false,
            folder: None,
            has_chosen_folder: false,
            workspace: Workspace::New,
            base_branch: None,
            target_type: NEW_SESSION.into(),
            workflow_id: None,
            workflow_steps: Vec::new(),
            name: String::new(),
            skip: true,
            agent: String::new(),
            model: String::new(),
            harness_type: None,
            machine_id: None,
        }
    }
}

/// Which kind of thing a run is (the Runs chip).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetKind {
    Session,
    Workflow,
}

/// What the composer needs from outside itself.
#[derive(Clone, Copy)]
pub struct Context<'a> {
    /// The automation being changed; `None` while making a new one.
    pub automation: Option<&'a Automation>,
    /// The zone the schedule is saved in ([`composer_time_zone`]).
    pub time_zone: &'a str,
    /// Now (Unix ms) and the clock's offset at an instant, for a one-off's date and the plan's next run.
    pub from: i64,
    pub tz: Tz<'a>,
    /// The harness a run uses when the automation names none; `None` until the harnesses have loaded.
    pub default_harness: Option<&'a str>,
    /// The enabled harnesses that support workflow steps, in their order; empty until loaded.
    pub workflow_harnesses: &'a [String],
    /// The Library of the repository a workflow runs in; `None` until it has loaded.
    pub library: Option<&'a [Workflow]>,
    /// The repository's default base and current branch, for the plan line's "from …".
    pub default_base: Option<&'a str>,
    pub current_branch: Option<&'a str>,
}

impl<'a> Context<'a> {
    /// Only what every composer has: the automation (if any), the zone and the clock. Nothing else has loaded.
    pub fn new(automation: Option<&'a Automation>, time_zone: &'a str, from: i64, tz: Tz<'a>) -> Context<'a> {
        Context {
            automation,
            time_zone,
            from,
            tz,
            default_harness: None,
            workflow_harnesses: &[],
            library: None,
            default_base: None,
            current_branch: None,
        }
    }

    fn is_editing(&self) -> bool {
        self.automation.is_some()
    }

    fn default_runs_workflows(&self) -> bool {
        self.default_harness.is_some_and(|d| self.workflow_harnesses.iter().any(|h| h == d))
    }
}

/// The zone new and changed schedules are saved in: the machine's IANA zone, else UTC (row 7.18). With none known
/// (Windows today) the schedule is computed and saved in UTC, which is right, only less friendly.
pub fn composer_time_zone() -> String {
    browser_time_zone().unwrap_or_else(|| "UTC".into())
}

/// The folder an automation names: none, a repository when it runs in a worktree or the repository list knows it, else a
/// plain folder (`folderFor`).
fn folder_for(path: Option<&str>, known_repository: bool, repositories: &[String]) -> Folder {
    match path {
        None => Folder::None,
        Some(p) if known_repository || repositories.iter().any(|r| r == p) => Folder::Repository(p.into()),
        Some(p) => Folder::Directory(p.into()),
    }
}

fn folder_path(folder: Option<&Folder>) -> Option<&str> {
    match folder {
        Some(Folder::Repository(p) | Folder::Directory(p)) => Some(p),
        _ => None,
    }
}

impl ComposerState {
    /// `freshComposerState`.
    pub fn fresh() -> ComposerState {
        ComposerState::default()
    }

    /// The composer's state for an automation as it's saved (`stateFor`, row 7.13). `repositories` are the paths the
    /// repository list of the automation's machine holds so far.
    pub fn state_for(automation: &Automation, repositories: &[String]) -> ComposerState {
        let worktree = automation.isolation.as_deref() == Some("worktree");
        ComposerState {
            text: automation.prompt.clone(),
            // A cron it has no words for is still a schedule: the When menu shows it as Custom.
            manual_when: Some(
                from_trigger(&automation.trigger_type, &automation.trigger_config)
                    .unwrap_or_else(|| When::Cron { expr: automation.trigger_config.clone() }),
            ),
            folder: Some(folder_for(automation.workspace_id.as_deref(), worktree, repositories)),
            has_chosen_folder: true,
            workspace: if worktree { Workspace::New } else { Workspace::Current },
            base_branch: automation.base_branch.clone(),
            target_type: automation.target_type.clone().unwrap_or_else(|| NEW_SESSION.into()),
            workflow_id: automation.workflow_id.clone(),
            workflow_steps: automation.workflow_steps.clone().unwrap_or_default(),
            name: automation.name.clone(),
            skip: automation.max_concurrent_runs > 0,
            agent: automation.agent.clone().unwrap_or_default(),
            model: model_from_path(automation.model.as_deref()).map(|m| key_for_model(&m)).unwrap_or_default(),
            harness_type: automation.harness_type.clone(),
            machine_id: automation.target_machine_id.clone(),
            ..ComposerState::default()
        }
    }

    // ── Reading the text ──────────────────────────────────────────────────────

    /// The schedule words in the text, whether or not the When menu has taken over.
    pub fn hit(&self) -> Option<ScheduleHit> {
        parse_schedule(&self.text)
    }

    /// The schedule words that decide: none once the When menu has chosen.
    pub fn active_hit(&self) -> Option<ScheduleHit> {
        if self.manual_when.is_some() { None } else { self.hit() }
    }

    fn when_of(&self, active: Option<&ScheduleHit>) -> Option<When> {
        if let Some(manual) = &self.manual_when {
            return Some(manual.clone());
        }
        let parsed = active.map(|h| h.when.clone())?;
        // "Just once" answers "on Friday": it applies only while the text still says that.
        if let When::Weekly { days, t, ambiguous: true } = &parsed
            && self.force_once
            && days.len() == 1
        {
            return Some(When::Once { t: *t, day: Some(days[0]), rel: None, date: None });
        }
        Some(parsed)
    }

    /// When it runs: the When menu's choice, else what the words say (a single "on Friday" made a one-off by "Just once").
    pub fn when(&self) -> Option<When> {
        self.when_of(self.active_hit().as_ref())
    }

    /// What the agent is asked: the text without the schedule words that decide.
    pub fn prompt(&self) -> String {
        prompt_from(&self.text, self.active_hit().as_ref())
    }

    /// The bytes of the text to highlight as the schedule (the mirror's `<mark>`), while the words decide.
    pub fn highlight(&self) -> Option<Range<usize>> {
        self.active_hit().map(|h| h.start..h.end)
    }

    /// The Name field's placeholder: the name it gets if none is typed.
    pub fn name_placeholder(&self) -> String {
        auto_name(&self.prompt())
    }

    /// The draft row's title: the name typed, else the name from the prompt, else "New automation" (`draftTitle`).
    pub fn title(&self) -> String {
        let typed = self.name.trim();
        if !typed.is_empty() {
            return typed.into();
        }
        let auto = self.name_placeholder();
        if auto.is_empty() { DRAFT_TITLE.into() } else { auto }
    }

    pub fn is_workflow(&self) -> bool {
        self.target_type == WORKFLOW
    }

    /// Each run gets a new worktree of a repository.
    pub fn is_worktree(&self) -> bool {
        matches!(self.folder, Some(Folder::Repository(_))) && self.workspace == Workspace::New
    }

    /// The harness a session run uses: the automation's own, else the default.
    pub fn harness(&self, ctx: &Context) -> Option<String> {
        self.harness_type.clone().or_else(|| ctx.default_harness.map(str::to_string))
    }

    /// The harness a workflow runs on, as the chip shows it: the automation's, else the default if it runs workflow steps,
    /// else the first that does (row 12.6).
    pub fn workflow_harness(&self, ctx: &Context) -> Option<String> {
        if let Some(own) = &self.harness_type {
            return Some(own.clone());
        }
        if ctx.default_runs_workflows() {
            return ctx.default_harness.map(str::to_string);
        }
        ctx.workflow_harnesses.first().cloned().or_else(|| ctx.default_harness.map(str::to_string))
    }

    fn chosen_workflow<'w>(&self, ctx: &Context<'w>) -> Option<&'w Workflow> {
        let id = self.workflow_id.as_deref()?;
        ctx.library?.iter().find(|w| w.id == id)
    }

    /// The plan line under the box (`describeAutomationPlan` as the composer calls it).
    pub fn plan(&self, ctx: &Context) -> Plan {
        let hit = self.active_hit();
        let when = self.when_of(hit.as_ref());
        let prompt = prompt_from(&self.text, hit.as_ref());
        let worktree = self.is_worktree();
        let base = if worktree { self.base_branch.as_deref().or(ctx.default_base).or(ctx.current_branch) } else { None };
        let workflow_name = self.workflow_id.as_deref().map(|id| {
            self.chosen_workflow(ctx)
                .map(|w| w.name.clone())
                .unwrap_or_else(|| id.strip_prefix("builtin:").or_else(|| id.strip_prefix("repo:")).unwrap_or(id).to_string())
        });
        let saved_time_zone = ctx.automation.filter(|a| a.trigger_type != "event").map(|a| a.time_zone.as_deref());
        describe_automation_plan(&PlanInput {
            text: &self.text,
            when: when.as_ref(),
            hit: hit.as_ref(),
            prompt: &prompt,
            force_once: self.force_once,
            folder: self.folder.as_ref(),
            worktree,
            base,
            same_session: self.target_type == SAME_SESSION,
            workflow: self.is_workflow().then_some(workflow_name.as_deref()),
            time_zone: ctx.time_zone,
            saved_time_zone,
            legacy_folderless: ctx.automation.is_some_and(|a| a.isolation.is_none() && a.workspace_id.is_none()),
            from: ctx.from,
            tz: ctx.tz,
        })
    }

    // ── What is sent ──────────────────────────────────────────────────────────

    /// What the server is sent (`request`, rows 7.15, 7.16, 12.7); `None` (so no Send) without a schedule or a prompt, and
    /// for a workflow without one picked or a repository. The per-hour and timeout limits aren't shown, so they're kept.
    pub fn request(&self, ctx: &Context) -> Option<AutomationRequest> {
        let hit = self.active_hit();
        let schedule = self.when_of(hit.as_ref())?;
        let prompt = prompt_from(&self.text, hit.as_ref());
        if prompt.is_empty() {
            return None;
        }
        let trigger = to_trigger(&schedule, ctx.from, ctx.tz)?;
        let existing = ctx.automation;
        let workspace_id = folder_path(self.folder.as_ref()).map(str::to_string);
        let typed = self.name.trim();
        let name = if !typed.is_empty() {
            typed.to_string()
        } else {
            existing.map(|a| a.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| auto_name(&prompt))
        };
        let base = AutomationRequest {
            name,
            prompt,
            trigger_type: trigger.trigger_type,
            trigger_config: trigger.trigger_config,
            max_runs_per_hour: Some(existing.map_or(10, |a| a.max_runs_per_hour)),
            timeout_minutes: Some(existing.map_or(30, |a| a.timeout_minutes)),
            workspace_id: Some(workspace_id.clone()),
            time_zone: Some(Some(ctx.time_zone.to_string())),
            target_machine_id: Some(self.machine_id.clone()),
            ..AutomationRequest::default()
        };

        if self.is_workflow() {
            let workflow_id = self.workflow_id.clone()?;
            if !matches!(self.folder, Some(Folder::Repository(_))) {
                return None;
            }
            // A saved step the Library hasn't loaded yet stays; one the workflow no longer has goes.
            let steps = match ctx.library {
                None => self.workflow_steps.clone(),
                Some(_) => {
                    let offered: Vec<&str> = self
                        .chosen_workflow(ctx)
                        .map(|w| w.steps.iter().filter(|s| s.optional).map(|s| s.id.as_str()).collect())
                        .unwrap_or_default();
                    self.workflow_steps.iter().filter(|id| offered.contains(&id.as_str())).cloned().collect()
                }
            };
            // None follows the default harness when it fires, unless that one can't run workflows. Until the harnesses have
            // loaded there's nothing to go on, so nothing is pinned.
            let harness = self.harness_type.clone().or_else(|| {
                (!ctx.workflow_harnesses.is_empty() && !ctx.default_runs_workflows()).then(|| self.workflow_harness(ctx)).flatten()
            });
            return Some(AutomationRequest {
                // A workflow run never stacks on an unfinished one, whatever this says; the server decides.
                max_concurrent_runs: Some(existing.map_or(1, |a| a.max_concurrent_runs).max(1)),
                model: Some(None),
                agent: Some(None),
                harness_type: Some(harness),
                target_type: Some(WORKFLOW.into()),
                target_tags: Some(Vec::new()),
                isolation: Some(Some("worktree".into())),
                base_branch: Some(self.base_branch.clone()),
                workflow_id: Some(Some(workflow_id)),
                workflow_steps: Some(steps),
                ..base
            });
        }

        let worktree = self.is_worktree();
        // An older automation with no folder keeps running in the first workspace root until one is picked.
        let isolation = if existing.is_some_and(|a| a.isolation.is_none()) && workspace_id.is_none() {
            None
        } else if worktree {
            Some("worktree".to_string())
        } else {
            Some("existing".to_string())
        };
        let chose_agent_or_model = !self.agent.is_empty() || !self.model.is_empty();
        Some(AutomationRequest {
            // Skipping keeps an older automation's own limit; the page only says on or off.
            max_concurrent_runs: Some(if self.skip { existing.map_or(1, |a| a.max_concurrent_runs).max(1) } else { 0 }),
            model: Some(model_to_path(model_from_key(&self.model).as_ref())),
            agent: Some(Some(self.agent.clone()).filter(|a| !a.is_empty())),
            // An agent or model only means something on the harness it came from.
            harness_type: Some(if chose_agent_or_model { self.harness(ctx) } else { None }),
            target_type: Some(self.target_type.clone()),
            target_tags: Some(existing.map(|a| a.target_tags.clone()).unwrap_or_default()),
            isolation: Some(isolation),
            base_branch: Some(if worktree { self.base_branch.clone() } else { None }),
            ..base
        })
    }

    /// Whether saving would change anything about the automation being edited (`isDirty`, row 7.17). A new automation, and
    /// one with no request, always count as changed; Send is still held back by [`ComposerState::can_submit`].
    pub fn is_dirty(&self, ctx: &Context) -> bool {
        let (Some(existing), Some(next)) = (ctx.automation, self.request(ctx)) else { return true };
        let flat = |v: &Option<Option<String>>| v.clone().flatten();
        let sorted = |v: Option<&Vec<String>>| {
            let mut v = v.cloned().unwrap_or_default();
            v.sort();
            v
        };
        next.name != existing.name
            || next.prompt != existing.prompt
            || next.trigger_type != existing.trigger_type
            || next.trigger_config != existing.trigger_config
            || flat(&next.workspace_id) != existing.workspace_id
            // An older automation with a folder ran in it as it is, which is what "existing" means.
            || flat(&next.isolation)
                != existing.isolation.clone().or_else(|| existing.workspace_id.as_ref().map(|_| "existing".to_string()))
            || flat(&next.base_branch) != existing.base_branch
            || next.target_type.as_deref() != Some(existing.target_type.as_deref().unwrap_or(NEW_SESSION))
            || flat(&next.agent) != existing.agent
            || flat(&next.model) != existing.model
            || flat(&next.harness_type) != existing.harness_type
            || flat(&next.workflow_id) != existing.workflow_id
            || sorted(next.workflow_steps.as_ref()) != sorted(existing.workflow_steps.as_ref())
            || next.max_concurrent_runs != Some(existing.max_concurrent_runs)
            || flat(&next.target_machine_id) != existing.target_machine_id
    }

    /// Send can be pressed: not busy, there's a request, and it changes something (`canSubmit`).
    pub fn can_submit(&self, ctx: &Context, busy: bool) -> bool {
        !busy && self.request(ctx).is_some() && self.is_dirty(ctx)
    }

    /// Send (or Save) is shown: always for a new automation, once something changed for a saved one (`showSend`).
    pub fn show_send(&self, ctx: &Context) -> bool {
        !ctx.is_editing() || self.is_dirty(ctx)
    }

    // ── Changing it ───────────────────────────────────────────────────────────

    /// The text changes. A new schedule phrase is a new question: "Just once" was the answer to the old one (row 7.10).
    pub fn set_text(&mut self, text: impl Into<String>) {
        let phrase = |s: &ComposerState| s.hit().map(|h| s.text[h.start..h.end].to_string());
        let before = phrase(self);
        self.text = text.into();
        if phrase(self) != before {
            self.force_once = false;
        }
    }

    /// The When menu chose (or gave the schedule back to the words): it takes over, and "Just once" goes (`setWhen`).
    pub fn set_when(&mut self, next: Option<When>) {
        self.manual_when = next;
        self.force_once = false;
    }

    /// The plan line's "Just once" or "Every Friday instead".
    pub fn set_force_once(&mut self, once: bool) {
        self.force_once = once;
    }

    /// Another folder (`setFolder`, row 9.4): a different place clears the base, and a new automation in a repository gets a
    /// new worktree each run unless that's changed after. `chosen` is a pick by the person, not the default.
    pub fn set_folder(&mut self, next: Folder, chosen: bool, editing: bool) {
        let same_place = self.folder.as_ref() == Some(&next);
        if !same_place {
            self.base_branch = None;
        }
        let repository = matches!(next, Folder::Repository(_));
        self.folder = Some(next);
        self.has_chosen_folder |= chosen;
        if !same_place && repository && !editing {
            self.workspace = Workspace::New;
        }
    }

    /// A new automation starts in the folder New session would (`applyInitialFolder`, row 9.3): once the repositories have
    /// loaded (or failed), when none was chosen or set, and never a plain folder in cloud mode. Editing never changes the
    /// folder by itself. `initial` is `NewSessionDefaults::initial_folder` for the machine the runs go to.
    pub fn apply_initial_folder(&mut self, editing: bool, repositories_ready: bool, initial: Option<Folder>, cloud_mode: bool) {
        if editing || !repositories_ready || self.has_chosen_folder || self.folder.is_some() {
            return;
        }
        if let Some(initial) = initial.filter(|f| !(cloud_mode && matches!(f, Folder::Directory(_)))) {
            self.set_folder(initial, false, false);
        }
    }

    /// A plain folder the repository list now knows becomes a repository (row 7.14).
    pub fn upgrade_folder(&mut self, repositories: &[String]) {
        if let Some(Folder::Directory(p)) = &self.folder
            && repositories.iter().any(|r| r == p)
        {
            self.folder = Some(Folder::Repository(p.clone()));
        }
    }

    /// Runs go to another machine (`pickMachine`, row 11.2): what was picked on this one (folder, workspace, base, agent,
    /// model, harness, workflow and steps) goes, and an older target, which picks a session on this machine, becomes a new
    /// session. `live_key` is this machine's key: picking it is `None`.
    pub fn pick_machine(&mut self, machine_key: &str, live_key: &str) {
        let machine_id = (machine_key != live_key).then(|| machine_key.to_string());
        if machine_id == self.machine_id {
            return;
        }
        let other = machine_id.is_some();
        *self = ComposerState {
            machine_id,
            folder: None,
            has_chosen_folder: false,
            workspace: Workspace::New,
            base_branch: None,
            agent: String::new(),
            model: String::new(),
            harness_type: None,
            workflow_id: None,
            workflow_steps: Vec::new(),
            ..std::mem::take(self)
        };
        if other && ![NEW_SESSION, SAME_SESSION, WORKFLOW].contains(&self.target_type.as_str()) {
            self.target_type = NEW_SESSION.into();
        }
    }

    /// A session or a workflow (`setTargetKind`, row 12.2): the agent, model and harness don't carry over (a workflow's steps
    /// take theirs from the roles); a workflow gets a new worktree, and a plain folder goes.
    pub fn set_target_kind(&mut self, kind: TargetKind) {
        let current = if self.is_workflow() { TargetKind::Workflow } else { TargetKind::Session };
        if kind == current {
            return;
        }
        self.target_type = if kind == TargetKind::Workflow { WORKFLOW } else { NEW_SESSION }.into();
        self.agent.clear();
        self.model.clear();
        self.harness_type = None;
        if kind == TargetKind::Workflow {
            self.workspace = Workspace::New;
            if matches!(self.folder, Some(Folder::Directory(_))) {
                self.folder = None;
            }
        }
    }

    /// Another workflow clears the steps switched on for the last one (`setWorkflow`).
    pub fn set_workflow(&mut self, workflow_id: &str) {
        if self.workflow_id.as_deref() != Some(workflow_id) {
            self.workflow_steps.clear();
        }
        self.workflow_id = Some(workflow_id.into());
    }

    /// An optional step's switch (`toggleWorkflowStep`).
    pub fn toggle_workflow_step(&mut self, step_id: &str) {
        if let Some(i) = self.workflow_steps.iter().position(|s| s == step_id) {
            self.workflow_steps.remove(i);
        } else {
            self.workflow_steps.push(step_id.into());
        }
    }
}
