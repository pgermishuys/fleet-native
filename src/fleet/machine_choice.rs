//! Ports `composables/use-machine-choice.ts`: the machine a view with its own machine switch shows (Settings, Automations,
//! Analytics, Workflows). It is the machine you're on until the view's switch picks another; a pick of a machine that has
//! been forgotten is ignored. `reset` forgets the pick, so the view opens on the machine you're on each time it's visited.
//! Each view holds its own `MachineChoice`, so a pick in one doesn't move another.
use super::machines::{Entry, Machines};
use super::settings::HOME_MACHINE_KEY;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MachineChoice {
    /// The machine picked in the view's switch; none until picked since the view opened.
    picked: Option<String>,
    /// The switch's menu is open.
    pub open: bool,
}

impl MachineChoice {
    /// Whether the view has a switch at all: only when another machine is listed (`enabled`).
    pub fn enabled(&self, machines: &Machines) -> bool {
        machines.has_machines()
    }

    /// The machine the view shows: the pick while it is still listed, else the machine you're on; home with no others (`key`).
    pub fn key<'a>(&'a self, machines: &'a Machines) -> &'a str {
        if !self.enabled(machines) {
            return HOME_MACHINE_KEY;
        }
        match self.picked.as_deref() {
            Some(wanted) if wanted == HOME_MACHINE_KEY || machines.machine(wanted).is_some() => wanted,
            _ => machines.current_key(),
        }
    }

    /// The entry of `key`; home when it is gone (`entry`).
    pub fn entry(&self, machines: &Machines) -> Entry {
        let key = self.key(machines);
        let mut entries = machines.entries();
        let at = entries.iter().position(|e| e.key == key).unwrap_or(0);
        entries.swap_remove(at)
    }

    /// Another machine than home (`isOther`).
    pub fn is_other(&self, machines: &Machines) -> bool {
        self.key(machines) != HOME_MACHINE_KEY
    }

    pub fn pick(&mut self, key: &str) {
        self.picked = Some(key.to_string());
    }

    /// Forgets the pick and closes the menu: the view was opened again (`reset`).
    pub fn reset(&mut self) {
        self.picked = None;
        self.open = false;
    }
}
