//! Dragging a session row (`SessionsPanel.vue` rows 1.82-1.87, `ProjectGroup.vue` 2.120-2.127, `SessionItem.vue` 3.73).
//! Rune has no drag-and-drop with targets, so the row's pointer drag does the whole of it: each move asks the runtime which
//! keyed element is under the pointer (`Tx::element_at`; the path names every keyed ancestor), and the release drops on the
//! last target found. Fleet's own targets are the same four: a project's group, the Pinned group (with the line where the
//! row will go), the Archive box that shows while a drag is on, and, for a fork, its own project (stands on its own).
use crate::{State, Tx};
use rune::Picked;

/// What a drag is over, found from the keys on the path of the element under the pointer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Over {
    #[default]
    Nothing,
    /// A project group, by its index in `State::projects` (`drop-project-{p}`).
    Project(usize),
    /// The Pinned group; the row it is over is the one the dragged row will go before (`None`: the end).
    Pinned(Option<String>),
    /// The Archive box (`drop-archive`).
    Archive,
    /// A project of another machine's tree: its machine's key and the project's id (`drop-mproject-{machine}~{project}`).
    MProject(String, String),
    /// The Pinned group of another machine's tree, and the row the dragged one goes before (`None`: the end).
    MPinned(String, Option<String>),
}

#[derive(Clone, Debug)]
pub struct RowDrag {
    pub id: String,
    pub over: Over,
}

/// Keys of the targets. A press that moves under Rune's 4 pt threshold is still a click, so a row still opens.
pub const PINNED_KEY: &str = "pinned-group";
pub const ARCHIVE_KEY: &str = "drop-archive";
pub fn project_key(p: usize) -> String {
    format!("drop-project-{p}")
}
/// A project group of another machine's tree, and its Pinned group.
pub fn machine_project_key(machine: &str, project: &str) -> String {
    format!("drop-mproject-{machine}~{project}")
}
pub fn machine_pinned_key(machine: &str) -> String {
    format!("mpinned-{machine}")
}

/// `session-{p}-{i}`, and no other key that starts with `session-`.
fn row_at(seg: &str) -> Option<(usize, usize)> {
    let rest = seg.strip_prefix("session-")?;
    let (p, i) = rest.split_once('-')?;
    Some((p.parse().ok()?, i.parse().ok()?))
}

/// The target a pick's path names: its innermost keyed drop target, and in the Pinned group the row it is over.
pub fn over_from_path(path: &str, id_at: impl Fn(usize, usize) -> Option<String>) -> Over {
    let mut row = None;
    for seg in path.rsplit('/') {
        if row.is_none() {
            row = row_at(seg);
        }
        if seg == ARCHIVE_KEY {
            return Over::Archive;
        }
        if let Some(machine) = seg.strip_prefix("mpinned-") {
            // The row is `machine-session-{machine}-{id}`.
            let prefix = format!("machine-session-{machine}-");
            let before = path.rsplit('/').find_map(|s| s.strip_prefix(prefix.as_str()));
            return Over::MPinned(machine.to_string(), before.map(str::to_string));
        }
        if let Some((machine, project)) = seg.strip_prefix("drop-mproject-").and_then(|r| r.split_once('~')) {
            return Over::MProject(machine.to_string(), project.to_string());
        }
        if seg == PINNED_KEY {
            return Over::Pinned(row.and_then(|(p, i)| id_at(p, i)));
        }
        if let Some(p) = seg.strip_prefix("drop-project-").and_then(|p| p.parse().ok()) {
            return Over::Project(p);
        }
    }
    Over::Nothing
}

impl State {
    /// The row's drag moved: it begins on the first move (`handleSessionDragStart`: only for a session that exists) and then
    /// asks what is under the pointer.
    pub fn row_drag_move(&mut self, id: &str, at: [f32; 2], tx: &mut Tx) {
        if self.row_drag.is_none() {
            if self.title_of(id).is_none() {
                return;
            }
            self.row_drag = Some(RowDrag { id: id.to_string(), over: Over::Nothing });
        }
        tx.element_at(at[0], at[1], |s: &mut State, picked: Option<Picked>, _| {
            let over = picked.map_or(Over::Nothing, |p| s.over_of(p.path()));
            if let Some(d) = s.row_drag.as_mut() {
                d.over = over;
            }
        });
    }

    fn over_of(&self, path: &str) -> Over {
        let id_at = |p: usize, i: usize| self.projects.get(p)?.sessions.get(i).map(|s| s.id.clone());
        let over = over_from_path(path, id_at);
        let Some(d) = &self.row_drag else { return over };
        match over {
            // A drag never leaves the machine it started on, and the project drops are off while a filter is typed.
            Over::MProject(m, p) => {
                if self.filter.text.trim().is_empty() && self.machine_key_of(&d.id) == m && self.machine_accepts(&d.id, &m, &p) {
                    Over::MProject(m, p)
                } else {
                    Over::Nothing
                }
            }
            Over::MPinned(m, before) => {
                if self.machine_key_of(&d.id) != m || before.as_deref() == Some(d.id.as_str()) {
                    Over::Nothing
                } else {
                    Over::MPinned(m, before)
                }
            }
            // Fleet leaves the project drops (move, unpin, move out) off while a filter is typed.
            Over::Project(p) if !self.filter.text.trim().is_empty() || !self.accepts(&d.id, p) => Over::Nothing,
            // Dropping a row on itself changes nothing.
            Over::Pinned(Some(before)) if before == d.id => Over::Nothing,
            over => over,
        }
    }

    /// The release: the last target takes the session (`cancelled`: Escape or a lost pointer, nothing moves).
    pub fn row_drag_end(&mut self, cancelled: bool, tx: &mut Tx) {
        let Some(RowDrag { id, over }) = self.row_drag.take() else { return };
        if cancelled {
            return;
        }
        match over {
            Over::Nothing => {}
            Over::Archive => {
                let machine = Some(self.machine_key_of(&id).to_string()).filter(|k| k != crate::fleet::settings::HOME_MACHINE_KEY);
                self.archive_sessions(&[id], machine.as_deref(), tx);
            }
            Over::Pinned(before) | Over::MPinned(_, before) => self.pin_session(&id, before.as_deref()),
            Over::Project(p) => self.drop_on_project(&id, p, tx),
            Over::MProject(machine, project) => self.drop_on_machine_project(&id, &machine, &project, tx),
        }
    }

    /// `acceptsDrag`: a project takes a session from another project, a pinned one on its own project (it unpins) and a fork or
    /// started session on its own project (it stands on its own).
    fn accepts(&self, id: &str, p: usize) -> bool {
        let (Some(project), Some(session)) = (self.projects.get(p), self.row_copy_of(id)) else { return false };
        let own = session.listing.project_id.as_deref().unwrap_or(crate::fleet::groups::UNGROUPED_ID) == project.id;
        !own || session.listing.pin_order.is_some() || crate::fleet::lineage_moves::movable_out_of(&session.listing).is_some()
    }

    /// `acceptsDrag` in another machine's tree: the project is one the machine lists (or ungrouped), and the row is not already
    /// plainly in it (a pinned one, or a fork that can stand on its own, is).
    fn machine_accepts(&self, id: &str, machine: &str, project: &str) -> bool {
        let Some(held) = self.machines.others.get(machine) else { return false };
        let Some(session) = self.row_copy_of(id) else { return false };
        if project != crate::fleet::groups::UNGROUPED_ID && !held.projects.iter().any(|p| p.id == project) {
            return false;
        }
        let own = session.listing.project_id.as_deref().unwrap_or(crate::fleet::groups::UNGROUPED_ID) == project;
        !own || session.listing.pin_order.is_some() || crate::fleet::lineage_moves::movable_out_of(&session.listing).is_some()
    }

    /// Drop on a project of another machine (`handleMove`, `handleUnpin`, `handleMoveOutOfParent` of `MachineSessionsGroup`):
    /// the same three outcomes as home's, sent to that machine.
    fn drop_on_machine_project(&mut self, id: &str, machine: &str, project: &str, tx: &mut Tx) {
        if !self.machine_accepts(id, machine, project) {
            return;
        }
        let Some(session) = self.row_copy_of(id) else { return };
        let own = session.listing.project_id.as_deref().unwrap_or(crate::fleet::groups::UNGROUPED_ID) == project;
        if own && session.listing.pin_order.is_some() {
            return self.unpin_session(id);
        }
        if own {
            if crate::fleet::lineage_moves::movable_out_of(&session.listing).is_some() {
                self.move_lineage(id, true, tx);
            }
            return;
        }
        let name = self.machines.others.get(machine).and_then(|m| m.projects.iter().find(|p| p.id == project)).map(|p| p.name.clone());
        let to = (project != crate::fleet::groups::UNGROUPED_ID).then_some(project);
        self.move_to_project(id, to, &name.unwrap_or_default());
    }

    /// Drop on a project (`handleMoveSession`, `handleUnpinSession`, `handleMoveOutOfParent`).
    fn drop_on_project(&mut self, id: &str, p: usize, tx: &mut Tx) {
        let Some(project) = self.projects.get(p).filter(|_| self.accepts(id, p)) else { return };
        let (target, name) = (project.id.clone(), project.name.clone());
        let Some(session) = self.row_copy_of(id) else { return };
        let own = session.listing.project_id.as_deref().unwrap_or(crate::fleet::groups::UNGROUPED_ID) == target;
        if own && session.listing.pin_order.is_some() {
            // A pinned row dropped on its own project goes back to it.
            return self.unpin_session(id);
        }
        if own {
            // A fork or started session dropped on its own project stands on its own; anything else is already there.
            if crate::fleet::lineage_moves::movable_out_of(&session.listing).is_some() {
                self.move_lineage(id, true, tx);
            }
            return;
        }
        let to = (target != crate::fleet::groups::UNGROUPED_ID).then_some(target.as_str());
        self.move_to_project(id, to, &name);
    }
}

impl State {
    /// Whether a row is being dragged (`isSessionDragUnderway`).
    pub fn dragging_row(&self) -> bool {
        self.row_drag.is_some()
    }

    /// Whether the dragged row is one of home's (the Pinned group and Archive box of home's tree show for those only).
    pub fn dragging_home_row(&self) -> bool {
        self.row_drag.as_ref().is_some_and(|d| self.machine_key_of(&d.id) == crate::fleet::settings::HOME_MACHINE_KEY)
    }

    /// The machine whose tree the dragged row is in, when it is another machine's.
    pub fn dragging_on(&self) -> Option<&str> {
        let d = self.row_drag.as_ref()?;
        Some(self.machine_key_of(&d.id)).filter(|k| *k != crate::fleet::settings::HOME_MACHINE_KEY)
    }

    pub fn drag_over(&self) -> Over {
        self.row_drag.as_ref().map(|d| d.over.clone()).unwrap_or_default()
    }

    /// The accent line before a pinned row the drag will land before (`project-row--drop-before`).
    pub fn pin_line_before(&self, id: &str) -> bool {
        matches!(self.drag_over(), Over::Pinned(Some(before)) | Over::MPinned(_, Some(before)) if before == id)
    }
}

/// The scenes: a row mid-drag over each kind of target, for the pictures (a scene can't drag).
pub enum Scene {
    Project,
    PinRow,
    PinEmpty,
    Archive,
}

impl State {
    pub fn show_row_drag(&mut self, scene: Scene, dark: bool) {
        self.dark = Some(dark);
        let id = self.projects[0].sessions[1].id.clone();
        let over = match scene {
            Scene::Project => Over::Project(1),
            Scene::PinEmpty => Over::Pinned(None),
            Scene::Archive => Over::Archive,
            Scene::PinRow => {
                let first = self.projects[0].sessions[0].id.clone();
                self.set_pin_order(&first, Some(1.0));
                let other = self.projects[0].sessions[2].id.clone();
                self.set_pin_order(&other, Some(2.0));
                Over::Pinned(Some(other))
            }
        };
        self.row_drag = Some(RowDrag { id, over });
    }
}

impl State {
    /// A row of another machine mid-drag, for the pictures: over its machine's other project, or over its Pinned group.
    pub fn show_machine_drag(&mut self, pinned: bool, dark: bool) {
        self.show_machines(crate::mock::machines::Case::Several, dark);
        let key = "m-falcon".to_string();
        self.machines.remember_sessions(&key, &["f-fence".to_string()]);
        let over = if pinned { Over::MPinned(key, None) } else { Over::MProject(key, "p-garden".into()) };
        self.row_drag = Some(RowDrag { id: "f-fence".into(), over });
    }
}
