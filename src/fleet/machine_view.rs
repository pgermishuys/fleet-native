//! What pressing in the machine part of the session list does: fold a machine or one of its projects (under the keys Fleet
//! keeps them under, `machine:<key>` and `project:<key>:<id>`), open a parent's children, and ask for the Add a machine form.
use super::settings::{machine_group_key, project_group_key};
use crate::State;

impl State {
    /// A machine's heading was pressed: fold or unfold its whole tree and remember it (`toggleGroupCollapsed(machineGroupKey)`).
    pub fn toggle_machine(&mut self, machine: &str) {
        let key = machine_group_key(machine);
        let folded = self.sidebar.is_group_collapsed(&key);
        self.sidebar.set_group_collapsed(&key, !folded);
        self.save_settings();
    }

    pub fn machine_is_open(&self, machine: &str) -> bool {
        !self.sidebar.is_group_collapsed(&machine_group_key(machine))
    }

    /// One of a machine's project groups (or its Pinned group) was pressed. Project ids are only unique within a machine.
    pub fn toggle_machine_project(&mut self, machine: &str, group: &str) {
        let key = project_group_key(machine, group);
        let folded = self.sidebar.is_group_collapsed(&key);
        self.sidebar.set_group_collapsed(&key, !folded);
        self.save_settings();
    }

    pub fn machine_project_is_open(&self, machine: &str, group: &str) -> bool {
        !self.sidebar.is_group_collapsed(&project_group_key(machine, group))
    }

    /// A parent's caret: shows or hides what came from it. `now_open` is what the row shows now (`toggleChildren`).
    pub fn toggle_children(&mut self, session: &str, now_open: bool) {
        self.machine_children.insert(session.to_string(), !now_open);
    }

    /// The Add a machine row: Settings opens on Machines with the add form open. The poll opens the form (`keep_add_form`), so
    /// it opens even when it was asked for before the page was shown.
    pub fn add_machine(&mut self) {
        self.open_add_machine();
    }
}
