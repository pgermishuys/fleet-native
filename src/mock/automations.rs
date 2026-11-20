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
    /// An automation open (its header alone until piece E).
    Open,
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
            Scene::List | Scene::Create | Scene::Open => {
                store.is_loading = false;
                store.list = automations();
            }
        }
        match scene {
            Scene::Create => self.automations_nav.start_create(),
            Scene::Open => self.automations_nav.set_active("a-weekly"),
            _ => {}
        }
    }
}
