//! Picking rows and acting on the picked ones (`SessionsPanel.vue` `handleSelectionKeydown`, `archiveSelected`,
//! `restoreSelected`; `SessionItem.vue` `handleSelect`; rows 1.48-1.53, 1.58, 1.59, 3.70). The pure store is `selection.rs`; the
//! rows on screen (which a Shift range and the pruning need) are read from `home_layout` and the machine trees.
use super::home_layout::visible_ids;
use super::machine_tree;
use super::settings::HOME_MACHINE_KEY;
use crate::{State, Tx};

impl State {
    /// The id of the open session, even a stand-in's (a scene and a test have only those).
    fn open_route_id(&self) -> Option<String> {
        match &self.route {
            super::store::Route::Session(id) if !id.is_empty() => Some(id.clone()),
            _ => None,
        }
    }

    /// The rows another machine's tree shows, top to bottom: every row of every open group, children included (`MachineSessionsGroup`).
    fn machine_visible_ids(&self, key: &str) -> Vec<String> {
        if !self.machine_is_open(key) {
            return Vec::new();
        }
        let Some(held) = self.machines.others.get(key) else { return Vec::new() };
        let needle = self.filter.text.trim().to_lowercase();
        let tree = machine_tree::build_hiding(&held.sessions, &held.projects, &needle, |x| self.hides(x));
        tree.groups
            .iter()
            .filter(|g| self.machine_project_is_open(key, &g.id))
            .flat_map(|g| g.rows.iter().map(|r| held.sessions[r.index].id.clone()))
            .collect()
    }

    /// Gives the store the rows `machine`'s tree shows now (`None` is the live machine's). True when a picked row left it.
    fn sync_order(&mut self, machine: Option<&str>) -> bool {
        let ids = match machine {
            None => visible_ids(self, &self.home_layout()),
            Some(key) => self.machine_visible_ids(key),
        };
        self.selection.set_visible_order(ids, machine)
    }

    /// Keeps the selection to what is on screen. Runs on the clock while rows are picked: a row that was archived, filtered out
    /// or folded away stops being picked, and leaving the Sessions view lets them all go. True when the selection changed. Looks
    /// again every 100 ms while rows are picked, because a press changes the list after this frame's poll has run.
    pub fn sync_selection(&mut self, tx: &mut Tx) -> bool {
        if !self.selection.is_selecting() {
            return false;
        }
        tx.wake_after(std::time::Duration::from_millis(100));
        if self.rail != crate::Rail::Sessions {
            self.selection.clear();
            return true;
        }
        let machine = self.selection.machine().map(str::to_string);
        self.sync_order(machine.as_deref())
    }

    /// A row of the list was pressed (`handleSelect`): Shift takes a range, Ctrl/Cmd (or any press while rows are picked) toggles
    /// one, a plain press opens the session. `machine` is the key of another machine's tree, `None` for the live one.
    pub fn press_row(&mut self, id: &str, machine: Option<&str>, open: impl FnOnce(&mut State, &mut Tx), tx: &mut Tx) {
        let mods = tx.mods();
        if mods.shift {
            self.sync_order(machine);
            // The open session is the anchor on the live machine only (a session of another machine is not in its order).
            let fallback = if machine.is_some() { None } else { self.open_route_id() };
            self.selection.extend_to(id, fallback.as_deref(), machine);
        } else if mods.cmd || mods.ctrl || self.selection.is_selecting_on(machine) {
            self.sync_order(machine);
            self.selection.toggle(id, machine);
        } else {
            open(self, tx);
        }
    }

    /// The bar's Archive: the rows vanish at once and go to the server after the Undo window, on the machine they were picked on.
    pub fn archive_selected(&mut self, tx: &mut Tx) {
        let ids = self.selection.ids().to_vec();
        let machine = self.selection.machine().map(str::to_string);
        self.selection.clear();
        self.archive_sessions(&ids, machine.as_deref(), tx);
    }

    /// The bar's Restore (the Archived view's): one `PATCH` per picked session, in order; the selection is cleared first and a
    /// failure on one doesn't stop the rest.
    pub fn restore_selected(&mut self) {
        let ids = self.selection.ids().to_vec();
        let machine = self.selection.machine().map(str::to_string);
        self.selection.clear();
        for id in ids {
            self.restore_session(&id, machine.as_deref().filter(|m| *m != HOME_MACHINE_KEY));
        }
    }
}

impl State {
    /// A scene: two sessions of different projects pinned, so the Pinned group shows above the projects.
    pub fn show_pinned(&mut self) {
        self.projects[0].sessions[2].listing.pin_order = Some(1.0);
        self.projects[1].sessions[0].listing.pin_order = Some(2.0);
    }

    /// A scene: the pins of `show_pinned` and three rows picked, one of them pinned, so the bar and the checkboxes show.
    pub fn show_selection(&mut self) {
        self.show_pinned();
        for (p, i) in [(0, 2), (0, 0), (1, 1)] {
            let id = self.projects[p].sessions[i].id.clone();
            self.selection.toggle(&id, None);
        }
    }
}
