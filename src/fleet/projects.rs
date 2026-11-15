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
}

/// The delete dialog.
pub struct DeleteDialog {
    pub id: String,
    pub name: String,
    pub count: usize,
    pub mode: DeleteMode,
    pub deleting: bool,
}

#[derive(Default)]
pub struct Projects {
    pub rename: Option<Rename>,
    /// The name field's text (a field needs a place in the state to live in, whether or not one is shown).
    pub edit: TextEdit,
    pub delete: Option<DeleteDialog>,
    /// A rename, move or delete is on its way (`isAnyActionPending`): the + and the menu wait.
    pub pending: bool,
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

    /// The + and the menu's New session (`handleNewSessionRequest`). The page that starts it is component 7's.
    pub fn new_session_in(&mut self, p: usize, tx: &mut Tx) {
        let Some(project) = self.projects.get(p).filter(|_| !self.proj.pending) else { return };
        let (id, name) = (project.id.clone(), project.name.clone());
        self.proj.new_session_in = Some(id);
        self.toast(format!("New session in {name} isn't in the native app yet (component 7 of the port)."), None, 4.0, tx);
    }

    /// Rename starts from the menu or F2 (`startRename`): the name is selected and has focus.
    pub fn start_rename(&mut self, p: usize, tx: &mut Tx) {
        if self.proj.pending || !self.project_is_real(p) {
            return;
        }
        let name = self.projects[p].name.clone();
        self.proj.edit = TextEdit::new(name.clone());
        self.proj.edit.set_selection(0, name.len());
        self.proj.rename = Some(Rename { id: self.projects[p].id.clone(), original: name, seen_focus: false });
        tx.focus(RENAME_FIELD, true);
    }

    /// Enter, Escape or blur ends the edit; only the first counts (`hasCompleted`).
    pub fn finish_rename(&mut self, commit: bool, tx: &mut Tx) {
        let Some(r) = self.proj.rename.take() else { return };
        // Back to the header, where the F2 that started it can be pressed again.
        if let Some(key) = self.projects.iter().position(|p| p.id == r.id) {
            tx.focus(format!("project-{key}"), true);
        }
        if !commit {
            return;
        }
        if let Some(name) = rename_outcome(&r.original, &self.proj.edit.text) {
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
        });
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
        self.send_project_call(call, tx);
    }

    /// Sends the call on a thread, or with no server applies it to the invented list.
    fn send_project_call(&mut self, call: Call, tx: &mut Tx) {
        self.proj.pending = true;
        let Some(link) = &self.link else {
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
        let deleting = matches!(call, Call::Delete { .. });
        match result {
            Ok(()) => {
                if deleting {
                    self.proj.delete = None;
                }
                if let Some(link) = &self.link {
                    link.refetch_list();
                }
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
        self.proj.rename = Some(Rename { id: self.projects[p].id.clone(), original: name, seen_focus: true });
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
