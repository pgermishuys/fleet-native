//! Invented automations and runs for scenes and tests (component 19): a weekly one in a worktree, a nightly one, one that is
//! off, one that waits for an event, a one-off, one with a run going, one that failed, and one workflow target, in the
//! invented repository `kettle`. Times are in 2026, so a test fixes "now" at `NOW_MS`.
use crate::fleet::automations::model::{Automation, AutomationRun, RunState};

/// Tuesday 15 September 2026, 10:00 UTC, the clock Fleet's own tests use.
pub const NOW_MS: i64 = 1_789_466_400_000;

pub const REPOSITORY: &str = "/work/kettle";

fn run(id: &str, automation: &str, trigger: &str, state: RunState) -> AutomationRun {
    AutomationRun {
        id: id.into(),
        automation_id: automation.into(),
        trigger: trigger.into(),
        started_at: "2026-09-14T09:00:02Z".into(),
        state,
        ..AutomationRun::default()
    }
}

fn scheduled(id: &str, name: &str, prompt: &str, cron: &str, next: Option<&str>) -> Automation {
    Automation {
        id: id.into(),
        name: name.into(),
        prompt: prompt.into(),
        trigger_type: "schedule".into(),
        trigger_config: cron.into(),
        is_enabled: true,
        workspace_id: Some(REPOSITORY.into()),
        created_at: "2026-09-01T08:00:00Z".into(),
        time_zone: Some("UTC".into()),
        isolation: Some("worktree".into()),
        base_branch: Some("origin/main".into()),
        next_run_at: next.map(str::to_string),
        ..Automation::default()
    }
}

/// What the list holds, in the order the server sends it.
pub fn automations() -> Vec<Automation> {
    let mut nightly = scheduled("a-nightly", "Nightly changelog tidy", "Tidy the changelog", "0 2 * * *", Some("2026-09-16T02:00:00Z"));
    nightly.isolation = Some("existing".into());
    nightly.last_run = Some(run("r-nightly", "a-nightly", "schedule", RunState::Done));

    let mut paused = scheduled("a-paused", "Dependency check", "Check for outdated dependencies", "0 8 * * 5", None);
    paused.is_enabled = false;

    let mut event = scheduled("a-event", "Greet new sessions", "Say hello", "{\"eventType\":\"session_created\"}", None);
    event.trigger_type = "event".into();
    event.workspace_id = None;
    event.time_zone = None;
    event.isolation = None;

    let mut once = scheduled("a-once", "Release notes", "Draft the release notes", "2026-09-18T16:00", Some("2026-09-18T16:00:00Z"));
    once.trigger_type = "once".into();

    let mut going = scheduled("a-going", "Hourly CI check", "Look at the last CI run", "0 * * * *", Some("2026-09-15T11:00:00Z"));
    going.last_run = Some(run("r-going", "a-going", "schedule", RunState::Running));

    let mut failed = scheduled("a-failed", "Weekly digest", "Summarise the open pull requests", "0 9 * * 1", Some("2026-09-21T09:00:00Z"));
    failed.last_run = Some(AutomationRun {
        error: Some("Couldn't start: Couldn't create the worktree: there's no branch origin/main to start from".into()),
        ..run("r-failed", "a-failed", "manual", RunState::Failed)
    });

    let mut flow = scheduled("a-flow", "Monday feature", "Bump the dependencies", "0 9 * * 1", Some("2026-09-21T09:00:00Z"));
    flow.target_type = Some("workflow".into());
    flow.workflow_id = Some("builtin:build-a-feature".into());
    flow.workflow_steps = Some(vec!["review".into()]);
    flow.last_run = Some(AutomationRun { workflow_run_id: Some("wr-1".into()), ..run("r-flow", "a-flow", "schedule", RunState::Waiting) });

    vec![
        scheduled("a-weekly", "Monday digest", "Summarise the open pull requests", "0 9 * * 1", Some("2026-09-21T09:00:00Z")),
        nightly,
        paused,
        event,
        once,
        going,
        failed,
        flow,
    ]
}

/// Dreaming, the built-in one: every night at 03:00, so `Every day at 03:00.`
pub fn dreaming(paused: bool) -> Automation {
    let mut a = scheduled("a-dream", "Dreaming", "", "0 3 * * *", (!paused).then_some("2026-09-16T03:00:00Z"));
    a.workspace_id = None;
    a.isolation = None;
    a.target_type = Some("dreaming".into());
    a.builtin = Some("dreaming".into());
    a.is_enabled = !paused;
    a
}

/// Dreaming's nights: one that kept and dropped notes, one skipped for want of sessions, and one that failed.
pub fn dreaming_runs() -> Vec<AutomationRun> {
    let mut night = run("n3", "a-dream", "schedule", RunState::Done);
    night.started_at = "2026-09-15T03:00:02Z".into();
    night.summary = Some("kettle: 4 notes changed, 1 note added · 9 min, 214k tokens".into());
    let mut skipped = run("n2", "a-dream", "schedule", RunState::Skipped);
    skipped.started_at = "2026-09-14T03:00:02Z".into();
    skipped.error = Some("Skipped: no new sessions since the last run, nothing to read.".into());
    let mut manual = run("n1", "a-dream", "manual", RunState::Done);
    manual.started_at = "2026-09-13T16:20:00Z".into();
    manual.summary = Some("kettle: 2 notes changed · 4 min, 96k tokens".into());
    vec![night, skipped, manual]
}

/// One automation's runs, newest first: a schedule run, a catch-up, a manual run that failed, an event run, one on another
/// machine that didn't answer, and a skipped one.
pub fn runs(automation: &str) -> Vec<AutomationRun> {
    let mut manual = run("r4", automation, "manual", RunState::Failed);
    manual.error = Some("Couldn't start: Couldn't create the worktree: there's no branch origin/main to start from".into());
    let mut away = run("r2", automation, "schedule", RunState::Unanswered);
    away.machine_id = Some("m-atlas".into());
    away.machine_name = Some("atlas".into());
    let mut skipped = run("r1", automation, "schedule", RunState::Skipped);
    skipped.error = Some("Another run was still going".into());
    vec![
        AutomationRun { session_id: Some("s-1".into()), ..run("r6", automation, "schedule", RunState::Done) },
        run("r5", automation, "catch_up", RunState::Done),
        manual,
        run("r3", automation, "session_created", RunState::Ended),
        away,
        skipped,
    ]
}

/// Every state a run can be in, and each way a note shows: one that is starting, one going, one that needs you, a failure with
/// its error, a skipped one, one on another machine that didn't answer, a night's summary, an event run, a catch-up and one
/// that finished with a session to open.
pub fn runs_states(automation: &str) -> Vec<AutomationRun> {
    let at = |mut r: AutomationRun, started: &str| {
        r.started_at = started.into();
        r
    };
    let mut failed = run("r-failed", automation, "manual", RunState::Failed);
    failed.error = Some("Couldn't start: Couldn't create the worktree: there's no branch origin/main to start from".into());
    let mut away = run("r-away", automation, "schedule", RunState::Unanswered);
    away.machine_name = Some("atlas".into());
    let mut skipped = run("r-skipped", automation, "schedule", RunState::Skipped);
    skipped.error = Some("Another run was still going".into());
    let mut night = run("r-night", automation, "schedule", RunState::Done);
    night.summary = Some("Kept 4 memories and dropped 2 that said the same thing.".into());
    let session = |r: AutomationRun| AutomationRun { session_id: Some("s-1".into()), ..r };
    vec![
        at(session(run("r-starting", automation, "manual", RunState::Starting)), "2026-09-15T10:00:00Z"),
        at(session(run("r-running", automation, "schedule", RunState::Running)), "2026-09-15T09:00:01Z"),
        at(session(run("r-waiting", automation, "schedule", RunState::Waiting)), "2026-09-14T09:00:02Z"),
        failed,
        skipped,
        away,
        night,
        run("r-event", automation, "session_created", RunState::Ended),
        run("r-catch", automation, "catch_up", RunState::Done),
        session(run("r-done", automation, "schedule", RunState::Done)),
    ]
}

/// What a picture or a test of the Automations page shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    /// The list with each state a row can say, nothing open.
    List,
    Empty,
    Loading,
    Failed,
    /// New automation pressed: the draft row, and the detail's title says so.
    Create,
    /// New automation with a sentence typed (piece H): the draft row takes its title from the sentence; the composer reads
    /// the schedule out of it (piece I).
    CreateTyped,
    /// A sentence that says "on Friday" in a repository: the plan line asks "Every Friday, or just once?".
    CreateAsk,
    /// A create the server refused: the form error over the composer, the text kept.
    CreateError,
    /// A sentence typed and the When menu open over its chip (piece J).
    When,
    /// The same after Once: the menu offers the message's words back, and asks for the date and the time.
    WhenOnce,
    /// The same with a cron of four parts applied: the error under its box.
    WhenCronError,
    /// An automation open with its prompt changed: Save shows.
    EditDirty,
    /// An automation open in a repository, a new worktree each run from a base: the strip's chips (piece K).
    StripRepo,
    /// The same with its base menu open: no Fetch origin first, no branch name.
    StripBase,
    /// A new one in a repository, Same session each run, the Runs in menu open.
    StripSame,
    /// An automation open with the "…" popover: its name and skipping.
    StripMore,
    /// A new one with no folder after Send: the folder menu opened instead of creating.
    StripFolder,
    /// An automation open that runs on its own agent and model: the toolbar's chips name them (piece L).
    AgentModel,
    /// The same with the model menu open.
    AgentModelMenu,
    /// A new one in a repository: the chips say what Default is there.
    AgentModelNew,
    /// Piece M: an automation that runs on another machine.
    Machine(crate::fleet::automations::machine::Scene),
    /// An automation open: its header and nothing else yet.
    Open,
    /// The same, with the info note Run now leaves.
    Started,
    /// One that is switched off.
    Off,
    /// A run that was skipped: the error note.
    Skipped,
    /// The ⋯ menu open.
    Menu,
    /// Delete asked for: the confirm.
    Delete,
    /// An automation open with its runs (the Run now note over them).
    Runs,
    /// Open, and no run yet.
    RunsEmpty,
    /// Open, with a run in every state and each kind of note.
    RunsStates,
    /// The built-in Dreaming automation open, with its nights.
    Builtin,
    /// The same with Change time open.
    BuiltinTime,
    /// Paused, and no night yet.
    BuiltinPaused,
}

impl crate::State {
    /// The Automations page for the pictures and tests: no server, so the list is the invented one.
    pub fn show_automations(&mut self, dark: bool, scene: Scene) {
        use crate::fleet::settings::HOME_MACHINE_KEY;
        self.dark = Some(dark);
        self.now_ms = Some(NOW_MS);
        self.enter_automations();
        // The scene is the state; nothing is read again.
        self.automations_nav.mark_asked(HOME_MACHINE_KEY);
        let store = self.automations.of_mut(HOME_MACHINE_KEY);
        match scene {
            Scene::Loading => {}
            Scene::Empty => store.is_loading = false,
            Scene::Failed => {
                store.is_loading = false;
                store.error = Some("Fleet is restarting".into());
            }
            Scene::List
            | Scene::Create
            | Scene::CreateTyped
            | Scene::When
            | Scene::WhenOnce
            | Scene::WhenCronError
            | Scene::CreateAsk
            | Scene::CreateError
            | Scene::EditDirty
            | Scene::StripRepo
            | Scene::StripBase
            | Scene::StripSame
            | Scene::StripMore
            | Scene::StripFolder
            | Scene::AgentModel
            | Scene::AgentModelMenu
            | Scene::AgentModelNew
            | Scene::Open
            | Scene::Started
            | Scene::Off
            | Scene::Skipped
            | Scene::Menu
            | Scene::Delete
            | Scene::Runs
            | Scene::RunsEmpty
            | Scene::RunsStates
            | Scene::Machine(_) => {
                store.is_loading = false;
                store.list = automations();
            }
            Scene::Builtin | Scene::BuiltinTime | Scene::BuiltinPaused => {
                store.is_loading = false;
                store.list = automations();
                store.list.insert(0, dreaming(scene == Scene::BuiltinPaused));
            }
        }
        match scene {
            Scene::Create => self.automations_nav.start_create(),
            Scene::CreateTyped => {
                self.automations_nav.start_create();
                self.automations_nav
                    .set_text(false, "Every Monday at 9, summarise the open pull requests and say what each one still needs.");
            }
            Scene::When | Scene::WhenOnce | Scene::WhenCronError => {
                self.automations_nav.start_create();
                self.automations_nav.set_text(false, "Every Monday at 9, summarise the open pull requests.");
                self.show_when_menu();
                if scene == Scene::WhenOnce {
                    self.when_once();
                }
                if scene == Scene::WhenCronError {
                    self.automations_nav.when.cron.replace_text("0 9 * *");
                    self.when_apply_cron();
                }
            }
            Scene::CreateAsk | Scene::CreateError => {
                use crate::fleet::new_session::request::Folder;
                self.automations_nav.start_create();
                let words = if scene == Scene::CreateAsk {
                    "Tidy the changelog on Friday."
                } else {
                    "Every Monday at 9, summarise the open pull requests and say what each one still needs."
                };
                self.automations_nav.set_text(false, words);
                self.automations_nav.draft.set_folder(Folder::Repository(REPOSITORY.into()), true, false);
                if scene == Scene::CreateError {
                    self.automations_nav.form_error = Some("That time has already passed. Pick a later one.".into());
                }
            }
            Scene::EditDirty => {
                self.automations_nav.set_active("a-weekly");
                self.reset_edit_state();
                self.automations_nav.set_text(true, "Summarise the open pull requests and the drafts");
            }
            Scene::Open
            | Scene::Started
            | Scene::Skipped
            | Scene::Menu
            | Scene::Delete
            | Scene::Runs
            | Scene::RunsEmpty
            | Scene::RunsStates => self.automations_nav.set_active("a-weekly"),
            Scene::Off => self.automations_nav.set_active("a-paused"),
            Scene::StripRepo | Scene::StripBase | Scene::StripMore | Scene::StripSame | Scene::StripFolder => {
                self.show_automation_strip(scene)
            }
            Scene::AgentModel | Scene::AgentModelMenu | Scene::AgentModelNew => self.show_automation_agent_model(scene),
            Scene::Machine(m) => self.show_automation_machine(m),
            Scene::Builtin | Scene::BuiltinTime | Scene::BuiltinPaused => self.automations_nav.set_active("a-dream"),
            _ => {}
        }
        match scene {
            Scene::RunsEmpty => self.automations_nav.runs.show(HOME_MACHINE_KEY, "a-weekly", Vec::new()),
            Scene::RunsStates => self.automations_nav.runs.show(HOME_MACHINE_KEY, "a-weekly", runs_states("a-weekly")),
            Scene::Runs
            | Scene::EditDirty
            | Scene::StripRepo
            | Scene::StripBase
            | Scene::StripMore
            | Scene::AgentModel
            | Scene::AgentModelMenu => self.automations_nav.runs.show(HOME_MACHINE_KEY, "a-weekly", runs("a-weekly")),
            Scene::Builtin | Scene::BuiltinTime => self.automations_nav.runs.show(HOME_MACHINE_KEY, "a-dream", dreaming_runs()),
            Scene::BuiltinPaused => self.automations_nav.runs.show(HOME_MACHINE_KEY, "a-dream", Vec::new()),
            _ => {}
        }
        match scene {
            Scene::Started => self.automation_say(false, crate::fleet::automations::actions::STARTED),
            Scene::Skipped => self.automation_say(true, "Another run was still going"),
            Scene::BuiltinTime => self.builtin_start_time(),
            Scene::Menu => self.ui.open = Some(crate::ui::Open::Menu("automation-more")),
            Scene::Delete => {
                let call = crate::fleet::actions::Call::DeleteAutomation { machine: HOME_MACHINE_KEY.into(), id: "a-weekly".into() };
                self.ui.confirm = Some(crate::ui::confirm::Confirm::delete_automation("Monday digest".into(), call));
            }
            _ => {}
        }
    }

    /// Piece K's scenes: the open automation (or a new one) with the strip's chips and one of its menus.
    fn show_automation_strip(&mut self, scene: Scene) {
        use crate::fleet::new_session::request::Folder;
        // The base chip's branches: the invented repository's, under the automations' folder.
        let seq = self.feeds.detail.begin(Some(REPOSITORY), false).unwrap_or_default();
        self.feeds.detail.finish(seq, Ok(crate::mock::feeds::detail_in("/work")));
        match scene {
            Scene::StripSame | Scene::StripFolder => {
                self.automations_nav.start_create();
                self.automations_nav.set_text(false, "Every weekday at 8, triage the new issues and label each one.");
                if scene == Scene::StripSame {
                    self.automations_nav.draft.set_folder(Folder::Repository(REPOSITORY.into()), true, false);
                    self.automations_nav.draft.target_type = crate::fleet::automations::strip::SAME_SESSION.into();
                    self.open_runs_in_menu();
                } else {
                    self.begin_folder_menu();
                }
            }
            _ => {
                self.automations_nav.set_active("a-weekly");
                self.reset_edit_state();
                match scene {
                    Scene::StripBase => self.open_base_menu(),
                    Scene::StripMore => self.open_automation_more(),
                    _ => {}
                }
            }
        }
    }

    /// Piece L's scenes: the toolbar's agent and model chips on the invented catalog (`build` and `plan`, Sonnet and Opus).
    fn show_automation_agent_model(&mut self, scene: Scene) {
        use crate::fleet::new_session::request::Folder;
        if scene == Scene::AgentModelNew {
            self.automations_nav.start_create();
            self.automations_nav.set_text(false, "Every weekday at 8, triage the new issues and label each one.");
            self.automations_nav.draft.set_folder(Folder::Repository(REPOSITORY.into()), true, false);
        } else {
            // Saved with its own agent and model, on the machine's default harness.
            for a in self.automations.of_mut(crate::fleet::settings::HOME_MACHINE_KEY).list.iter_mut().filter(|a| a.id == "a-weekly") {
                a.agent = Some("plan".into());
                a.model = Some("anthropic/opus".into());
                a.harness_type = Some("opencode2".into());
            }
            self.automations_nav.set_active("a-weekly");
            self.reset_edit_state();
        }
        self.keep_automation_catalog();
        if scene == Scene::AgentModelMenu {
            self.open_selector("model");
        }
    }
}
