//! Another machine an automation's runs go to (rows 2.11, 5.6, 5.9, 7.8, 11.1-11.6): the machine the composer works on
//! (`AutomationMachineScope.vue`), whether the Runs on chip shows (`showMachineChip`), what picking a machine there does
//! (`pickMachine`), the "Runs on **name**" of a view that shows another machine (`runsOnName`), the plan line's "On name:",
//! and opening a run's session on the machine it went to (`openRun`). Plain state: nothing here draws.
//!
//! The automation itself is always the view's machine's: its list, create, save and runs go there. Only what the composer
//! offers (folders, repositories, harnesses, the catalog) is asked of the machine the runs go to, with that machine's own
//! token, as `feed_call` sends every request.
use super::composer::ComposerState;
use crate::State;
use crate::fleet::new_session::host::Host;
use crate::fleet::settings::HOME_MACHINE_KEY;

impl State {
    /// The view shows another machine than this one (`!target.isLive` in `AutomationDetailPanel.vue`): its list of machines
    /// isn't the one the chip would offer, so runs go to the machine the view shows and no chip is shown.
    pub fn automation_machine_fixed(&self) -> bool {
        self.automations_nav.choice.is_other(&self.machines)
    }

    /// The machine the composer asks and its runs go to (`AutomationMachineScope`): the one its state names while this
    /// machine's list holds it, else the machine the view shows.
    pub fn composer_machine(&self, editing: bool) -> String {
        self.machine_of_composer(self.automations_nav.composer(editing))
    }

    fn machine_of_composer(&self, c: &ComposerState) -> String {
        let view = self.automations_machine();
        if self.automation_machine_fixed() {
            return view;
        }
        match c.machine_id.as_deref() {
            Some(id) if self.machines.machine(id).is_some() => id.to_string(),
            _ => view,
        }
    }

    /// The machine a saved automation's runs go to, as its composer would be: its folder is read against that machine's
    /// repositories (row 11.6).
    pub(super) fn machine_of_automation(&self, a: &super::model::Automation) -> String {
        let c = ComposerState { machine_id: a.target_machine_id.clone().filter(|id| !id.is_empty()), ..ComposerState::default() };
        self.machine_of_composer(&c)
    }

    /// `showMachineChip`: once this Fleet lists another machine, and not while the view shows another machine (row 11.1).
    pub fn automation_shows_machine_chip(&self) -> bool {
        self.machines.has_machines() && !self.automation_machine_fixed()
    }

    /// `runsOnName`: the view shows another machine and the open automation runs on one, named by the run that went there
    /// (row 11.4). Only an open automation has it; a new one on another machine's view has neither chip nor name.
    pub fn automation_runs_on_name(&self) -> Option<String> {
        if !self.automation_machine_fixed() || !self.composer_editing() {
            return None;
        }
        let a = self.open_automation()?;
        a.target_machine_id.as_deref().filter(|id| !id.is_empty())?;
        Some(a.last_run.and_then(|r| r.machine_name).filter(|n| !n.is_empty()).unwrap_or_else(|| "another machine".into()))
    }

    /// The plan line's "On name:" while the runs go to a machine other than this one (row 7.8, `targetName`).
    pub fn automation_plan_machine(&self) -> Option<String> {
        let key = self.composer_machine(self.composer_editing());
        (key != HOME_MACHINE_KEY).then(|| self.target_name())
    }

    /// `pickMachine` from the Runs on menu (row 11.2): the composer's picks on the old machine go. The composer's upkeep sees
    /// the new machine on the next frame and asks it for its folders, harnesses and catalog.
    pub(crate) fn automation_pick_machine(&mut self, key: &str) {
        let editing = self.composer_editing();
        let nav = &mut self.automations_nav;
        if editing { nav.edit.pick_machine(key, HOME_MACHINE_KEY) } else { nav.draft.pick_machine(key, HOME_MACHINE_KEY) }
    }

    /// The menu above the machine chip says which machines a session starts on, or on the composer where runs go.
    pub fn machine_menu_heading(&self) -> &'static str {
        if self.picker_host() == Host::Automation { "Runs on" } else { "Start on" }
    }

    /// The session of a run on another machine is that machine's (`rememberSessions`), so it opens there, in place
    /// (row 5.9). `machine_id` is the key this Fleet's list has it under.
    pub(super) fn run_machine(&mut self, machine_id: Option<&str>, session: &str) -> Option<String> {
        let key = machine_id.filter(|id| !id.is_empty() && self.machines.machine(id).is_some())?.to_string();
        self.machines.remember_sessions(&key, &[session.to_string()]);
        Some(key)
    }
}

// ─── scenes ─────────────────────────────────────────────────────────────────────

/// What a picture of piece M shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    /// An automation open that runs on falcon: the row's machine, the Runs on chip, "On falcon:", a run on falcon and one
    /// whose machine didn't answer.
    Machine,
    /// The same with the Runs on menu open.
    MachineMenu,
    /// The view switched to falcon, whose automation runs on a machine only falcon knows: "Runs on relay", no chip.
    MachineFixed,
}

/// A run of the automation on `falcon`, at `at`.
fn run_on(id: &str, automation: &str, at: &str, state: super::model::RunState, machine: (&str, &str)) -> super::model::AutomationRun {
    super::model::AutomationRun {
        id: id.into(),
        automation_id: automation.into(),
        trigger: "schedule".into(),
        started_at: at.into(),
        state,
        session_id: Some("f-plan".into()),
        machine_id: Some(machine.0.into()),
        machine_name: Some(machine.1.into()),
        ..Default::default()
    }
}

/// The invented automation that runs elsewhere: every night at 2, in a repository of that machine.
fn nightly_elsewhere(machine: (&str, &str)) -> super::model::Automation {
    super::model::Automation {
        id: "a-beacon".into(),
        name: "Nightly beacon check".into(),
        prompt: "Every night at 2, run the beacon tests and say which ones failed.".into(),
        trigger_type: "schedule".into(),
        trigger_config: "0 2 * * *".into(),
        is_enabled: true,
        max_concurrent_runs: 1,
        workspace_id: Some("/srv/code/beacon".into()),
        created_at: "2026-09-01T08:00:00Z".into(),
        time_zone: Some("UTC".into()),
        isolation: Some("worktree".into()),
        next_run_at: Some("2026-09-16T02:00:00Z".into()),
        target_machine_id: Some(machine.0.into()),
        last_run: Some(run_on("r-b2", "a-beacon", "2026-09-15T02:00:01Z", super::model::RunState::Done, machine)),
        ..Default::default()
    }
}

impl State {
    pub(crate) fn show_automation_machine(&mut self, scene: Scene) {
        use super::model::RunState;
        self.show_machines_only(crate::mock::machines::Case::Several);
        // Lantern is still being read for the first time and harbor is down: the menu shows each, as New session's does.
        self.machines.list.retain(|m| m.id != "m-lantern");
        self.machines.others.remove("m-lantern");
        // What each last said, on the page's clock: falcon 3 minutes ago, harbor 2 days ago.
        for (id, minutes) in [("m-falcon", 3), ("m-harbor", 2 * 24 * 60)] {
            if let Some(m) = self.machines.others.get_mut(id) {
                m.loaded_at = Some(crate::mock::automations::NOW_MS - minutes * 60_000);
            }
        }
        let falcon = ("m-falcon", "falcon");
        let (key, automation) = if scene == Scene::MachineFixed {
            // Falcon's own list: its automation runs on relay, a machine this machine's list doesn't hold.
            self.automations_nav.choice.pick("m-falcon");
            self.automations_nav.mark_asked("m-falcon");
            let store = self.automations.of_mut("m-falcon");
            store.is_loading = false;
            store.list = vec![nightly_elsewhere(("m-relay", "relay"))];
            ("m-falcon", "a-beacon")
        } else {
            self.automations.of_mut(HOME_MACHINE_KEY).list.insert(1, nightly_elsewhere(falcon));
            (HOME_MACHINE_KEY, "a-beacon")
        };
        self.automations_nav.set_active(automation);
        self.reset_edit_state();
        let runs = if scene == Scene::MachineFixed {
            vec![run_on("r-b2", "a-beacon", "2026-09-15T02:00:01Z", RunState::Done, ("m-relay", "relay"))]
        } else {
            vec![
                run_on("r-b2", "a-beacon", "2026-09-15T02:00:01Z", RunState::Done, falcon),
                run_on("r-b1", "a-beacon", "2026-09-14T02:00:01Z", RunState::Unanswered, falcon),
            ]
        };
        self.automations_nav.runs.show(key, automation, runs);
        if scene == Scene::MachineMenu {
            self.open_machine_menu();
        }
    }
}
