//! What the user can do to a project from the session list: rename, move up or down, delete (`ProjectGroup.vue`,
//! `ConfirmDeleteProjectDialog.vue`, `InlineEdit.vue`, `useUpdateProject`, `useReorderProject`, `useDeleteProject`).
//! The requests and their rules are plain data; `State`'s methods at the end run them on a thread and re-read the list.
use super::groups::{ReorderTarget, UNGROUPED_ID};
use super::http;
use crate::mock::Project;
use crate::{State, Tx};
use rune::TextEdit;
use serde_json::{Value, json};

/// What deleting a project does with its sessions (`DeleteProjectMode`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeleteMode {
    /// Keep them, in Ungrouped.
    MoveToScratch,
    DeleteSessions,
}

impl DeleteMode {
    pub fn word(self) -> &'static str {
        match self {
            DeleteMode::MoveToScratch => "move_to_scratch",
            DeleteMode::DeleteSessions => "delete_sessions",
        }
    }
}

pub fn update_path(id: &str) -> String {
    format!("/api/projects/{id}")
}

/// `description: null` is what Fleet sends too; the server may clear a description with it (docs/parity row 2.48).
pub fn rename_body(name: &str) -> Value {
    json!({ "name": name, "description": null })
}

pub fn delete_path(id: &str, mode: DeleteMode) -> String {
    format!("/api/projects/{id}?mode={}", mode.word())
}

pub fn reorder_path(id: &str) -> String {
    format!("/api/projects/{id}/reorder")
}

pub fn reorder_body(position: usize) -> Value {
    json!({ "position": position })
}

/// What a finished edit asks for (`InlineEdit`'s `commit` and `handleRename`): the trimmed name, or nothing when it is empty
/// or the name it already has.
pub fn rename_outcome(current: &str, typed: &str) -> Option<String> {
    let name = typed.trim();
    (!name.is_empty() && name != current).then(|| name.to_string())
}

/// Where each project goes when `at` swaps with its neighbour (`buildReorderTargets`, `swapProjects`): every project in the
/// new order with its place from 1. `None` at the end of the list.
pub fn move_targets(order: &[String], at: usize, up: bool) -> Option<Vec<ReorderTarget>> {
    let other = if up { at.checked_sub(1)? } else { at + 1 };
    if at >= order.len() || other >= order.len() {
        return None;
    }
    let mut next = order.to_vec();
    next.swap(at, other);
    Some(next.into_iter().enumerate().map(|(i, project_id)| ReorderTarget { project_id, position: i + 1 }).collect())
}

/// A request to the server.
#[derive(Clone, PartialEq, Debug)]
pub enum Call {
    Rename {
        id: String,
        name: String,
    },
    /// One request per project, in order; the first failure stops the rest.
    Reorder(Vec<ReorderTarget>),
    Delete {
        id: String,
        mode: DeleteMode,
    },
}

impl Call {
    /// What a failure says it was doing.
    fn failure(&self) -> &'static str {
        match self {
            Call::Rename { .. } => "Couldn't rename the project",
            Call::Reorder(_) => "Couldn't move the project",
            Call::Delete { .. } => "Couldn't delete the project",
        }
    }
}

/// Makes the call, blocking: run it on a thread.
pub fn run(base: &str, call: &Call) -> Result<(), String> {
    match call {
        Call::Rename { id, name } => http::patch(base, &update_path(id), &rename_body(name)).map(drop),
        Call::Reorder(targets) => {
            for t in targets {
                http::patch(base, &reorder_path(&t.project_id), &reorder_body(t.position))?;
            }
            Ok(())
        }
        Call::Delete { id, mode } => http::delete(base, &delete_path(id, *mode)).map(drop),
    }
}

/// The header's name field while a project is renamed.
pub struct Rename {
    pub id: String,
    original: String,
    /// The field has held focus; losing it after that is the blur that commits.
    seen_focus: bool,
    /// The machine the project is on, when it isn't home's.
    machine: Option<String>,
}

/// The delete dialog.
pub struct DeleteDialog {
    pub id: String,
    pub name: String,
    pub count: usize,
    pub mode: DeleteMode,
    pub deleting: bool,
    /// The machine the project is on, when it isn't home's.
    pub machine: Option<String>,
}

impl Projects {
    /// A rename on another machine a scene shows already under way.
    pub fn show_machine_rename(&mut self, machine: &str, id: &str, name: &str) {
        self.edit = TextEdit::new(name.to_string());
        self.edit.set_selection(0, name.len());
        self.edit.focused = true;
        self.rename = Some(Rename { id: id.into(), original: name.into(), seen_focus: true, machine: Some(machine.into()) });
    }
}

#[derive(Default)]
pub struct Projects {
    pub rename: Option<Rename>,
    /// The name field's text (a field needs a place in the state to live in, whether or not one is shown).
    pub edit: TextEdit,
    pub delete: Option<DeleteDialog>,
    /// The New project dialog.
    pub new: Option<super::new_project::NewProject>,
    /// A rename, move or delete is on its way (`isAnyActionPending`): the + and the menu wait.
    pub pending: bool,
    /// The machine the call on its way goes to; `None` is home.
    on: Option<String>,
    /// The project the + or the menu's New session asked for; the New session page (component 7) reads it.
    pub new_session_in: Option<String>,
}

/// The key of the name field.
pub const RENAME_FIELD: &str = "project-rename";

impl State {
    /// A group that has the + and the menu: not Ungrouped (`canShowContextMenu`).
    pub fn project_is_real(&self, p: usize) -> bool {
        self.projects.get(p).is_some_and(|project| project.id != UNGROUPED_ID)
    }

    /// The ids of the groups that can be ordered, in the order they are shown.
    fn orderable(&self) -> Vec<String> {
        self.projects.iter().filter(|p| p.id != UNGROUPED_ID).map(|p| p.id.clone()).collect()
    }

    /// Where project `p` is among the orderable groups.
    fn order_index(&self, p: usize) -> Option<usize> {
        let id = &self.projects.get(p)?.id;
        self.orderable().iter().position(|o| o == id)
    }

    pub fn can_move(&self, p: usize, up: bool) -> bool {
        let order = self.orderable();
        self.order_index(p).is_some_and(|at| move_targets(&order, at, up).is_some())
    }

    pub fn project_renaming(&self, p: usize) -> bool {
        self.projects.get(p).is_some_and(|project| self.proj.rename.as_ref().is_some_and(|r| r.id == project.id))
    }

    /// The + and the menu's New session (`handleNewSessionRequest`). Opens the New session page for that project.
    pub fn new_session_in(&mut self, p: usize, _tx: &mut Tx) {
        let Some(project) = self.projects.get(p).filter(|_| !self.proj.pending) else { return };
        let id = project.id.clone();
        self.proj.new_session_in = Some(id.clone());
        self.open_new_session_page(Some(id));
    }

    /// Rename starts from the menu or F2 (`startRename`): the name is selected and has focus.
    pub fn start_rename(&mut self, p: usize, tx: &mut Tx) {
        if self.proj.pending || !self.project_is_real(p) {
            return;
        }
        let name = self.projects[p].name.clone();
        self.proj.edit = TextEdit::new(name.clone());
        self.proj.edit.set_selection(0, name.len());
        self.proj.rename = Some(Rename { id: self.projects[p].id.clone(), original: name, seen_focus: false, machine: None });
        tx.focus(RENAME_FIELD, true);
    }

    /// F2 or the menu's Rename on a project of another machine; the name goes to that machine. Not while it isn't answering.
    pub fn start_machine_rename(&mut self, machine: &str, id: &str, tx: &mut Tx) {
        if self.proj.pending || !self.machines.is_answering(machine) {
            return;
        }
        let Some(project) = self.machines.others.get(machine).and_then(|m| m.projects.iter().find(|p| p.id == id)) else { return };
        let name = project.name.clone();
        self.proj.edit = TextEdit::new(name.clone());
        self.proj.edit.set_selection(0, name.len());
        self.proj.rename = Some(Rename { id: id.to_string(), original: name, seen_focus: false, machine: Some(machine.to_string()) });
        tx.focus(RENAME_FIELD, true);
    }

    pub fn machine_project_renaming(&self, machine: &str, id: &str) -> bool {
        self.proj.rename.as_ref().is_some_and(|r| r.id == id && r.machine.as_deref() == Some(machine))
    }

    /// Enter, Escape or blur ends the edit; only the first counts (`hasCompleted`).
    pub fn finish_rename(&mut self, commit: bool, tx: &mut Tx) {
        let Some(r) = self.proj.rename.take() else { return };
        // Back to the header, where the F2 that started it can be pressed again.
        match &r.machine {
            Some(m) => tx.focus(format!("machine-project-{m}-{}", r.id), true),
            None => {
                if let Some(key) = self.projects.iter().position(|p| p.id == r.id) {
                    tx.focus(format!("project-{key}"), true);
                }
            }
        }
        if !commit {
            return;
        }
        if let Some(name) = rename_outcome(&r.original, &self.proj.edit.text) {
            self.proj.on = r.machine;
            self.send_project_call(Call::Rename { id: r.id, name }, tx);
        }
    }

    /// Losing focus after having it commits the edit; polled because the field reports it only through its model.
    pub fn keep_rename(&mut self, tx: &mut Tx) -> bool {
        let focused = self.proj.edit.focused;
        let Some(r) = self.proj.rename.as_mut() else { return false };
        if focused {
            r.seen_focus = true;
            return false;
        }
        if r.seen_focus {
            self.finish_rename(true, tx);
            return true;
        }
        false
    }

    /// Move up or down (`handleMoveUp`, `handleMoveDown`).
    pub fn move_project(&mut self, p: usize, up: bool, tx: &mut Tx) {
        if self.proj.pending {
            return;
        }
        let order = self.orderable();
        let Some(targets) = self.order_index(p).and_then(|at| move_targets(&order, at, up)) else { return };
        self.send_project_call(Call::Reorder(targets), tx);
    }

    /// The menu's Delete… (`openDeleteDialog`): the count is what the group shows.
    pub fn open_delete(&mut self, p: usize, count: usize) {
        if self.proj.pending || !self.project_is_real(p) {
            return;
        }
        let project = &self.projects[p];
        self.proj.delete = Some(DeleteDialog {
            id: project.id.clone(),
            name: project.name.clone(),
            count,
            mode: DeleteMode::MoveToScratch,
            deleting: false,
            machine: None,
        });
    }

    /// The ids of another machine's groups that can be ordered, in the order it shows them.
    fn machine_orderable(&self, machine: &str) -> Vec<String> {
        let Some(held) = self.machines.others.get(machine) else { return Vec::new() };
        let mut projects: Vec<&super::groups::ProjectInfo> = held.projects.iter().filter(|p| p.id != UNGROUPED_ID).collect();
        projects.sort_by_key(|p| p.position);
        projects.into_iter().map(|p| p.id.clone()).collect()
    }

    pub fn can_move_machine_project(&self, machine: &str, id: &str, up: bool) -> bool {
        let order = self.machine_orderable(machine);
        order.iter().position(|o| o == id).is_some_and(|at| move_targets(&order, at, up).is_some())
    }

    /// Move up or down in another machine's tree; the order goes to that machine.
    pub fn move_machine_project(&mut self, machine: &str, id: &str, up: bool, tx: &mut Tx) {
        if self.proj.pending || !self.machines.is_answering(machine) {
            return;
        }
        let order = self.machine_orderable(machine);
        let Some(targets) = order.iter().position(|o| o == id).and_then(|at| move_targets(&order, at, up)) else { return };
        self.proj.on = Some(machine.to_string());
        self.send_project_call(Call::Reorder(targets), tx);
    }

    /// The menu's Delete… on a project of another machine; `count` is what its group shows.
    pub fn open_machine_delete(&mut self, machine: &str, id: &str, count: usize) {
        if self.proj.pending || !self.machines.is_answering(machine) {
            return;
        }
        let Some(project) = self.machines.others.get(machine).and_then(|m| m.projects.iter().find(|p| p.id == id)) else { return };
        let name = project.name.clone();
        self.proj.delete = Some(DeleteDialog {
            id: id.to_string(),
            name,
            count,
            mode: DeleteMode::MoveToScratch,
            deleting: false,
            machine: Some(machine.to_string()),
        });
    }

    /// The + and the menu's New session on a project of another machine: the page opens on that machine with the project.
    pub fn new_session_in_machine_project(&mut self, machine: &str, id: &str) {
        if self.proj.pending || !self.machines.is_answering(machine) {
            return;
        }
        self.proj.new_session_in = Some(id.to_string());
        self.open_new_session_page(Some(id.to_string()));
        self.new_session_on(machine);
        // Picking the machine starts the draft's choices again; the project is the one asked for.
        if let Some(d) = self.new_draft.draft_mut() {
            d.project_id = Some(id.to_string());
        }
    }

    /// Cancel and Escape; nothing closes the dialog while it deletes.
    pub fn cancel_delete(&mut self) {
        if self.proj.delete.as_ref().is_some_and(|d| !d.deleting) {
            self.proj.delete = None;
        }
    }

    pub fn pick_delete_mode(&mut self, mode: DeleteMode) {
        if let Some(d) = self.proj.delete.as_mut().filter(|d| !d.deleting) {
            d.mode = mode;
        }
    }

    pub fn confirm_delete(&mut self, tx: &mut Tx) {
        let Some(d) = self.proj.delete.as_mut().filter(|d| !d.deleting) else { return };
        d.deleting = true;
        let call = Call::Delete { id: d.id.clone(), mode: d.mode };
        self.proj.on = d.machine.clone();
        self.send_project_call(call, tx);
    }

    /// Sends the call on a thread, or with no server applies it to the invented list.
    fn send_project_call(&mut self, call: Call, tx: &mut Tx) {
        self.proj.pending = true;
        let link = match &self.proj.on {
            Some(machine) => self.link_for_machine(machine),
            None => self.link.clone(),
        };
        let Some(link) = link else {
            let result = self.apply_to_mock(&call, tx);
            self.project_call_done(&call, result, tx);
            return;
        };
        let sent = call.clone();
        link.off_thread(move |base| run(base, &call), move |s, tx, result| s.project_call_done(&sent, result, tx));
    }

    /// A call finished. The list is read again on success (`projectChanged`); a failure is told in a toast, which Fleet's
    /// silent failure isn't (docs/parity row 2.59).
    pub(crate) fn project_call_done(&mut self, call: &Call, result: Result<(), String>, tx: &mut Tx) {
        self.proj.pending = false;
        let machine = self.proj.on.take();
        let deleting = matches!(call, Call::Delete { .. });
        match result {
            Ok(()) => {
                if deleting {
                    self.proj.delete = None;
                }
                // The name shows at once on that machine; its list is read again either way.
                if let (Some(m), Call::Rename { id, name }) = (&machine, call)
                    && let Some(project) = self.machines.others.get_mut(m).and_then(|o| o.projects.iter_mut().find(|p| p.id == *id))
                {
                    project.name = name.clone();
                }
                self.list_again_on(machine.as_deref().unwrap_or(super::settings::HOME_MACHINE_KEY));
            }
            Err(why) => {
                if let Some(d) = self.proj.delete.as_mut() {
                    d.deleting = false;
                }
                self.toast(format!("{}: {why}", call.failure()), None, 5.0, tx);
            }
        }
    }

    /// The invented list takes the change at once, so a scene or test needs no server.
    fn apply_to_mock(&mut self, call: &Call, tx: &mut Tx) -> Result<(), String> {
        if self.proj.on.is_some() {
            return Ok(());
        }
        let mut projects = std::mem::take(&mut self.projects);
        match call {
            Call::Rename { id, name } => {
                if let Some(p) = projects.iter_mut().find(|p| p.id == *id) {
                    p.name = name.clone();
                }
            }
            Call::Reorder(targets) => {
                for t in targets {
                    if let Some(p) = projects.iter_mut().find(|p| p.id == t.project_id) {
                        p.position = t.position as i64;
                    }
                }
                // Ungrouped stays last whatever its position.
                projects.sort_by_key(|p| if p.id == UNGROUPED_ID { i64::MAX } else { p.position });
            }
            Call::Delete { id, mode } => {
                if let Some(at) = projects.iter().position(|p| p.id == *id) {
                    let gone = projects.remove(at);
                    if *mode == DeleteMode::MoveToScratch && !gone.sessions.is_empty() {
                        match projects.iter_mut().find(|p| p.id == UNGROUPED_ID) {
                            Some(u) => u.sessions.extend(gone.sessions),
                            None => projects.push(Project {
                                id: UNGROUPED_ID.into(),
                                kind: Default::default(),
                                position: i64::MAX,
                                name: super::groups::UNGROUPED_NAME.into(),
                                path: String::new(),
                                open: true,
                                sessions: gone.sessions,
                            }),
                        }
                    }
                }
            }
        }
        self.on_list(projects, tx);
        Ok(())
    }
}

impl State {
    /// How many sessions group `p` shows: the filtered rows, which is the count the delete dialog gives (row 2.60).
    pub fn shown_count(&self, p: usize) -> usize {
        use super::groups::{matching_sessions, session_matches_query};
        let Some(project) = self.projects.get(p) else { return 0 };
        let needle = self.filter.text.trim().to_lowercase();
        let matches = |s: &crate::mock::Session| session_matches_query(s, &needle, &self.project_load.projects);
        matching_sessions(&project.name, &project.sessions, &needle, matches).map_or(0, |kept| kept.len())
    }
}

// ---- scenes ---------------------------------------------------------------------------------------------------------------

impl State {
    /// The project menu open on project `p`, as a right-click on its header (at `y`, in the list's column) leaves it.
    pub fn show_project_menu(&mut self, p: usize, y: f32) {
        self.ui.point = (120.0, y);
        self.ui.open = Some(crate::ui::Open::ProjectMenu(p));
    }

    /// Project `p` renamed in place, the name selected (a scene has no focus to give, so the field is shown focused).
    pub fn show_project_rename(&mut self, p: usize) {
        let name = self.projects[p].name.clone();
        self.proj.edit = TextEdit::new(name.clone());
        self.proj.edit.set_selection(0, name.len());
        self.proj.edit.focused = true;
        self.proj.rename = Some(Rename { id: self.projects[p].id.clone(), original: name, seen_focus: true, machine: None });
    }

    /// The delete dialog for project `p` with `mode` picked, and whether the request is on its way.
    pub fn show_project_delete(&mut self, p: usize, mode: DeleteMode, deleting: bool) {
        let count = self.shown_count(p);
        self.open_delete(p, count);
        if let Some(d) = self.proj.delete.as_mut() {
            d.mode = mode;
            d.deleting = deleting;
        }
        self.proj.pending = deleting;
    }
}
