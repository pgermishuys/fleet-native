//! The open automation's runs (`AutomationDetailPanel.vue`: `runs`, `hasLoadedRuns`, `loadRuns`, `openRun`, `firstRunHint`).
//! They are read when an automation opens, then again every 3 s while one is starting or running and every 20 s otherwise,
//! each poll also reading the list again. The poll is the app's own tick (`keep_automation_runs`); every read runs on a
//! thread, and an answer for an automation that is no longer the open one is dropped (a generation per open automation).
use super::model::{AutomationRun, RunState};
use super::schedule::{describe_date, from_trigger, next_run};
use crate::fleet::work::parse_iso_ms;
use crate::{State, Tx};

/// While a run is starting or going: its row changes the moment it finishes.
pub const BUSY_POLL_MS: i64 = 3000;
/// Otherwise.
pub const IDLE_POLL_MS: i64 = 20_000;

/// What the page holds of the open automation's runs.
#[derive(Debug, Default)]
pub struct Runs {
    /// The machine and the automation the runs are for; none while nothing is open.
    pub of: Option<(String, String)>,
    /// Newest first.
    pub list: Vec<AutomationRun>,
    /// A read has finished (`hasLoadedRuns`); the empty state shows only then.
    pub loaded: bool,
    /// Counts the automations opened, so a late answer knows it is out of date.
    generation: u64,
    in_flight: bool,
    /// When the next read is due; 0 for now.
    pub next_poll: i64,
    /// Run now's runs made with no server to make them.
    made: u32,
}

impl Runs {
    /// A run is starting or going, which is what makes the poll quick.
    pub fn busy(&self) -> bool {
        self.list.iter().any(|r| matches!(r.state, RunState::Starting | RunState::Running))
    }

    fn reset(&mut self, of: Option<(String, String)>) {
        self.of = of;
        self.list.clear();
        self.loaded = false;
        self.generation += 1;
        self.in_flight = false;
        self.next_poll = 0;
    }

    /// Run now's answer goes to the top of the list at once (`runs = [run, ...runs.filter(...)]`), and the list is read
    /// again on the next tick. An answer already on its way is stale, so it is dropped.
    pub fn add_first(&mut self, run: AutomationRun) {
        self.list.retain(|r| r.id != run.id);
        self.list.insert(0, run);
        self.loaded = true;
        self.generation += 1;
        self.in_flight = false;
        self.next_poll = 0;
    }

    /// What a picture or a test shows with no server: these runs, already read, for this automation.
    pub fn show(&mut self, machine: &str, id: &str, list: Vec<AutomationRun>) {
        self.reset(Some((machine.to_string(), id.to_string())));
        self.list = list;
        self.loaded = true;
    }
}

/// The session a run opens (`sessionOf`): the run's own; a workflow run's waiting or newest step is piece N's.
pub fn session_of(run: &AutomationRun) -> Option<&str> {
    run.session_id.as_deref()
}

impl State {
    /// The poll and the open (`loadRuns`, and the watch on the open automation): which automation the runs are for follows
    /// the open one; with a server they are read when due, without one they are the invented runs. True when something
    /// changed.
    pub fn keep_automation_runs(&mut self, tx: &mut Tx) -> bool {
        let machine = self.automations_machine();
        let wanted = self.automations_nav.open_id().map(|id| (machine.clone(), id.to_string()));
        let mut changed = false;
        if self.automations_nav.runs.of != wanted {
            self.automations_nav.runs.reset(wanted.clone());
            if let Some((m, id)) = &wanted
                && self.link_for_machine(m).is_none()
            {
                let list = crate::mock::automations::runs(id);
                self.automations_nav.runs.show(m, id, list);
            }
            changed = true;
        }
        let Some((machine, id)) = wanted else { return changed };
        if self.link_for_machine(&machine).is_none() {
            return changed;
        }
        let now = crate::view::message::now_ms(self);
        let runs = &self.automations_nav.runs;
        if runs.in_flight || now < runs.next_poll {
            return changed;
        }
        // The first read of an automation comes with opening it; the ones after it are polls, which read the list too.
        if runs.loaded {
            self.automations_refresh(&machine, tx, None);
        }
        self.load_automation_runs(&machine, &id, tx);
        changed
    }

    /// `loadRuns`: reads the runs of `id`; a failed read keeps what was there, and either way the next is scheduled by what
    /// is running.
    fn load_automation_runs(&mut self, machine: &str, id: &str, tx: &mut Tx) {
        let runs = &mut self.automations_nav.runs;
        runs.in_flight = true;
        let generation = runs.generation;
        self.automation_runs(
            machine,
            id,
            tx,
            Box::new(move |s, _, result| {
                let now = crate::view::message::now_ms(s);
                let runs = &mut s.automations_nav.runs;
                if runs.generation != generation {
                    return;
                }
                runs.in_flight = false;
                if let Ok(list) = result {
                    runs.list = list;
                    runs.loaded = true;
                }
                let wait = if runs.busy() { BUSY_POLL_MS } else { IDLE_POLL_MS };
                runs.next_poll = now + wait;
            }),
        );
    }

    /// Run now's run, put first. With no server to make one, a run that is starting stands for it.
    pub fn automation_run_started(&mut self, run: Option<AutomationRun>) {
        let Some((_, id)) = self.automations_nav.runs.of.clone() else { return };
        let now = crate::view::message::now_ms(self);
        let runs = &mut self.automations_nav.runs;
        let run = run.unwrap_or_else(|| {
            runs.made += 1;
            AutomationRun {
                id: format!("run-now-{}", runs.made),
                automation_id: id,
                trigger: "manual".into(),
                started_at: crate::fleet::session_commands::iso_utc(now),
                state: RunState::Starting,
                ..AutomationRun::default()
            }
        });
        runs.add_first(run);
    }

    /// A click on a run: the session it started opens in Sessions (`openRun`), on the machine the run went to. A run with
    /// none opens nothing, and so does a session that machine's list doesn't hold (yet).
    pub fn open_automation_run(&mut self, run_id: &str, tx: &mut Tx) {
        let machine = self.automations_machine();
        // The built-in page lists its runs but its rows don't open (`BuiltinAutomationDetail.vue` binds no `open`).
        if self.open_automation().is_some_and(|a| a.builtin.is_some()) {
            return;
        }
        let Some(run) = self.automations_nav.runs.list.iter().find(|r| r.id == run_id).cloned() else { return };
        let Some(session) = session_of(&run).map(str::to_string) else { return };
        // A run that went to another machine started its session there: it opens on that machine (row 5.9).
        let machine = self.run_machine(run.machine_id.as_deref(), &session).unwrap_or(machine);
        self.open_workflow_session(&machine, &session, tx);
    }

    /// What the empty runs list says about the first run (`firstRunHint`).
    pub fn first_run_hint(&self, a: &super::model::Automation) -> String {
        if !a.is_enabled {
            return "Switch it on, or press Run now.".into();
        }
        let tz = |ms: i64| crate::conv::time::local_offset_at(ms);
        if let Some(ms) = a.next_run_at.as_deref().and_then(parse_iso_ms) {
            return format!("The first run is {}, or press Run now.", describe_date(ms, &tz));
        }
        if a.trigger_type == "event" {
            return "It runs when its event happens, or press Run now.".into();
        }
        let next = from_trigger(&a.trigger_type, &a.trigger_config).and_then(|w| next_run(&w, crate::view::message::now_ms(self), &tz));
        match next {
            Some(ms) => format!("The first run is {}, or press Run now.", describe_date(ms, &tz)),
            None => "Press Run now to try it.".into(),
        }
    }
}
