//! What a machine heading's right-click menu does (`SessionsPanel.vue` `handleNewSessionOn`, `handleNewProjectOn`,
//! `handleWorkHere`; `machines.ts` `setCurrent`). Plain state changes.
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::store::Route;
use crate::{Rail, State};

impl State {
    /// Right-click on a machine's heading: opens its menu at the point.
    pub fn show_machine_heading_menu(&mut self, key: &str, x: f32, y: f32) {
        self.ui.point = (x, y);
        self.machine_menu = Some((key.to_string(), String::new()));
        self.ui.open = Some(crate::ui::Open::MachineHeadingMenu);
    }

    /// New session on a machine: the New session page with no project, its draft started on that machine. Not on one that
    /// isn't answering. Home is the machine the app works in, so it is no pick at all.
    pub fn new_session_on(&mut self, key: &str) {
        if !self.machines.is_answering(key) {
            return;
        }
        self.open_new_session_page(None);
        let picked = (key != HOME_MACHINE_KEY).then_some(key);
        self.new_draft.set_machine(picked, HOME_MACHINE_KEY);
    }

    /// New project on a machine (`handleNewProjectOn`): the dialog opens with that machine picked. Not on one that isn't answering.
    pub fn new_project_on(&mut self, key: &str, tx: &mut crate::Tx) {
        if self.machines.is_answering(key) {
            self.open_new_project(Some(key), tx);
        }
    }

    /// Work here: the machine becomes the one you're on, and the window goes to the first page, the Sessions rail with
    /// nothing open (`setCurrent`, then `navigate({to: "/"})`). A machine that is gone is ignored.
    pub fn work_here(&mut self, key: &str) {
        if self.machines.machine(key).is_none() && key != HOME_MACHINE_KEY {
            return;
        }
        self.machines.set_current(key);
        self.rail = Rail::Sessions;
        self.go(Route::None);
    }
}
