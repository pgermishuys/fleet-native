//! What the user can do to one session from its row: rename it in place, clear its attachments, delete it
//! (`SessionItem.vue` `startRename`, `handleRename`, `handleClearAttachments`, `handleDelete`; `useRenameSession`,
//! `useDeleteSession`, `api/session-attachments.ts`). The requests and their words are plain data; `State`'s methods run
//! them on a thread, and with no server the invented list takes the change, so scenes and tests need none.
use super::{http, projects::rename_outcome, store};
use crate::ui::confirm::Confirm;
use crate::{State, Tx};
use rune::TextEdit;
use serde_json::{Value, json};

/// The key of the title field.
pub const RENAME_FIELD: &str = "session-rename";
/// The keys of the confirm dialog's two buttons.
pub const CONFIRM_CANCEL: &str = "confirm-cancel";
pub const CONFIRM_ACTION: &str = "confirm-action";

/// A request about one session.
#[derive(Clone, PartialEq, Debug)]
pub enum Call {
    Rename { id: String, title: String },
    ClearAttachments { id: String },
    Delete { id: String },
}

pub fn session_path(id: &str) -> String {
    format!("/api/sessions/{id}")
}

pub fn attachments_path(id: &str) -> String {
    format!("/api/sessions/{id}/attachments")
}

/// `PATCH /api/sessions/{id}`: just the title (`UpdateSessionTitleRequest`).
pub fn rename_body(title: &str) -> Value {
    json!({ "title": title })
}

/// What a refusal says (`readErrorMessage`, and `clearSessionAttachments`' own rule: the body's `error` when it is a non-empty
/// string, else `Failed to clear attachments: HTTP <status>`).
pub fn failure_text(call: &Call, status: u16, body: &str) -> String {
    match call {
        Call::ClearAttachments { .. } => {
            let reason = serde_json::from_str::<Value>(body)
                .ok()
                .and_then(|v| v.get("error").and_then(Value::as_str).filter(|e| !e.is_empty()).map(str::to_string));
            reason.unwrap_or_else(|| format!("Failed to clear attachments: HTTP {status}"))
        }
        _ => store::error_text(status, body),
    }
}

/// Makes the call, blocking: run it on a thread.
pub fn run(base: &str, call: &Call) -> Result<(), String> {
    let (method, path, body) = match call {
        Call::Rename { id, title } => ("PATCH", session_path(id), Some(rename_body(title))),
        Call::ClearAttachments { id } => ("DELETE", attachments_path(id), None),
        Call::Delete { id } => ("DELETE", session_path(id), None),
    };
    let (status, text) = http::text_with(method, base, &path, body.as_ref())?;
    if (200..300).contains(&status) { Ok(()) } else { Err(failure_text(call, status, &text)) }
}

/// The title field while a row is renamed.
pub struct RowRename {
    pub id: String,
    /// The raw title it started from (not the "Untitled session" the row says in its place).
    original: String,
    /// The field has held focus; losing it after that is the blur that commits.
    seen_focus: bool,
}

impl RowRename {
    /// A rename a scene shows already under way.
    pub fn scene(id: &str, title: &str) -> RowRename {
        RowRename { id: id.into(), original: title.into(), seen_focus: true }
    }
}

#[derive(Default)]
pub struct RowActions {
    pub rename: Option<RowRename>,
    /// The title field's text (a field needs a place in the state to live in, whether or not one is shown).
    pub edit: TextEdit,
}

impl State {
    /// Where session `id` is in the list now.
    pub fn row_of(&self, id: &str) -> Option<(usize, usize)> {
        self.projects.iter().enumerate().find_map(|(p, project)| project.sessions.iter().position(|s| s.id == id).map(|i| (p, i)))
    }

    pub fn row_renaming(&self, p: usize, i: usize) -> bool {
        let id = self.projects.get(p).and_then(|project| project.sessions.get(i)).map(|s| s.id.as_str());
        id.is_some_and(|id| self.act.rename.as_ref().is_some_and(|r| r.id == id))
    }

    /// F2, a double-click or the menu's Rename (`startRename`): the raw title is in a field, all of it selected.
    pub fn start_row_rename(&mut self, p: usize, i: usize, tx: &mut Tx) {
        let Some(session) = self.projects.get(p).and_then(|project| project.sessions.get(i)) else { return };
        let (id, title) = (session.id.clone(), session.title.clone());
        self.begin_row_rename(id, title, tx);
    }

    /// F2, a double-click or the menu's Rename on a row of another machine. Not while that machine isn't answering: its rows
    /// only open (`MachineSessionsGroup`'s `readOnly`).
    pub fn start_machine_row_rename(&mut self, machine: &str, id: &str, tx: &mut Tx) {
        if !self.machines.is_answering(machine) {
            return;
        }
        let Some(session) = self.machines.others.get(machine).and_then(|m| m.sessions.iter().find(|s| s.id == id)) else { return };
        let title = session.title.clone();
        self.machines.remember_sessions(machine, &[id.to_string()]);
        self.begin_row_rename(id.to_string(), title, tx);
    }

    pub fn machine_row_renaming(&self, id: &str) -> bool {
        self.act.rename.as_ref().is_some_and(|r| r.id == id)
    }

    fn begin_row_rename(&mut self, id: String, title: String, tx: &mut Tx) {
        // Starting on another row leaves the first one, which is the blur that saves it.
        if self.act.rename.as_ref().is_some_and(|r| r.id != id) {
            self.finish_row_rename(true, tx);
        }
        self.ui.open = None;
        self.act.edit = TextEdit::new(title.clone());
        self.act.edit.set_selection(0, title.len());
        self.act.rename = Some(RowRename { id, original: title, seen_focus: false });
        tx.focus(RENAME_FIELD, true);
    }

    /// Enter, Escape or blur ends the edit; only the first counts (`hasHandledInlineRename`).
    pub fn finish_row_rename(&mut self, commit: bool, tx: &mut Tx) {
        let Some(r) = self.act.rename.take() else { return };
        if let Some((p, i)) = self.row_of(&r.id) {
            tx.focus(format!("session-{p}-{i}"), true);
        } else {
            tx.focus(format!("machine-session-{}-{}", self.machine_key_of(&r.id), r.id), true);
        }
        if !commit {
            return;
        }
        let Some(title) = rename_outcome(r.original.trim(), &self.act.edit.text) else { return };
        // The new title shows at once; a refusal puts the old one back (`useRenameSession`).
        let was = r.original;
        self.set_title(&r.id, &title);
        self.send_session_call(Call::Rename { id: r.id, title }, Some(was), tx);
    }

    /// Losing focus after having it commits the edit; polled because the field reports it only through its model.
    pub fn keep_row_rename(&mut self, tx: &mut Tx) -> bool {
        let focused = self.act.edit.focused;
        let Some(r) = self.act.rename.as_mut() else { return false };
        if focused {
            r.seen_focus = true;
            return false;
        }
        if r.seen_focus {
            self.finish_row_rename(true, tx);
            return true;
        }
        false
    }

    pub(crate) fn set_title(&mut self, id: &str, title: &str) {
        let mut projects = std::mem::take(&mut self.projects);
        self.store.patch(&mut projects, id, |s| s.title = title.to_string());
        self.projects = projects;
        // On another machine the title lives in that machine's list too, until its next read says the same (`patchSession`).
        let key = self.machine_key_of(id).to_string();
        self.machines.patch_session(&key, id, |s| s.title = title.to_string());
    }

    /// The title `id` shows now, in home's list or another machine's.
    pub(crate) fn title_of(&self, id: &str) -> Option<String> {
        if let Some((p, i)) = self.row_of(id) {
            return Some(self.projects[p].sessions[i].title.clone());
        }
        let key = self.machine_key_of(id);
        self.machines.others.get(key)?.sessions.iter().find(|s| s.id == id).map(|s| s.title.clone())
    }

    /// The menu's Clear attachments… on a row of another machine.
    pub fn ask_machine_clear_attachments(&mut self, machine: &str, id: &str, tx: &mut Tx) {
        let Some(title) = self.machine_row_title(machine, id) else { return };
        self.ask(Confirm::clear_attachments(title, Call::ClearAttachments { id: id.to_string() }), tx);
    }

    /// The menu's Delete permanently… on a row of another machine.
    pub fn ask_machine_delete_session(&mut self, machine: &str, id: &str, tx: &mut Tx) {
        let Some(title) = self.machine_row_title(machine, id) else { return };
        self.ask(Confirm::delete_session(title, Call::Delete { id: id.to_string() }), tx);
    }

    /// The title a dialog names, and a note of where the session lives so the call finds its machine.
    fn machine_row_title(&mut self, machine: &str, id: &str) -> Option<String> {
        if !self.machines.is_answering(machine) {
            return None;
        }
        let session = self.machines.others.get(machine)?.sessions.iter().find(|s| s.id == id)?;
        let title = crate::fleet::row_status::display_title(None, &session.title);
        self.machines.remember_sessions(machine, &[id.to_string()]);
        Some(title)
    }

    /// The menu's Copy path on a row of another machine: the folder is on that machine, so only its text is copied.
    pub fn copy_machine_path(&mut self, machine: &str, id: &str, tx: &mut Tx) {
        let path = self.machines.others.get(machine).and_then(|m| m.sessions.iter().find(|s| s.id == id));
        if let Some(session) = path.filter(|s| !s.listing.workspace_directory.is_empty()) {
            tx.copy(session.listing.workspace_directory.clone());
        }
    }

    /// The menu's Clear attachments… (`handleClearAttachments`).
    pub fn ask_clear_attachments(&mut self, p: usize, i: usize, tx: &mut Tx) {
        let Some(session) = self.projects.get(p).and_then(|project| project.sessions.get(i)) else { return };
        let call = Call::ClearAttachments { id: session.id.clone() };
        let title = crate::fleet::row_status::display_title(None, &session.title);
        self.ask(Confirm::clear_attachments(title, call), tx);
    }

    /// The menu's Delete permanently… (`handleDelete`).
    pub fn ask_delete_session(&mut self, p: usize, i: usize, tx: &mut Tx) {
        let Some(session) = self.projects.get(p).and_then(|project| project.sessions.get(i)) else { return };
        let call = Call::Delete { id: session.id.clone() };
        let title = crate::fleet::row_status::display_title(None, &session.title);
        self.ask(Confirm::delete_session(title, call), tx);
    }

    /// Sends the call on a thread, or with no server applies it to the invented list. `was` is the title a rename puts back.
    pub(crate) fn send_session_call(&mut self, call: Call, was: Option<String>, tx: &mut Tx) {
        let session = match &call {
            Call::Rename { id, .. } | Call::ClearAttachments { id } | Call::Delete { id } => id.clone(),
        };
        let Some(link) = self.link_for(&session) else {
            self.session_call_done(&call, was, Ok(()), tx);
            return;
        };
        let sent = call.clone();
        link.off_thread(move |base| run(base, &call), move |s, tx, result| s.session_call_done(&sent, was, result, tx));
    }

    /// A call finished. A rename that failed puts the old title back and says nothing (Fleet swallows it); a confirm's
    /// answer goes to its dialog, which stays open on a failure.
    pub(crate) fn session_call_done(&mut self, call: &Call, was: Option<String>, result: Result<(), String>, tx: &mut Tx) {
        match call {
            Call::Rename { id, title } => {
                self.header_rename_done(id, &result);
                if result.is_err() {
                    // Only while it still holds the title this rename put there.
                    if let Some(was) = was
                        && self.title_of(id).as_deref() == Some(title.as_str())
                    {
                        self.set_title(id, &was);
                    }
                } else {
                    self.list_again(id);
                }
            }
            Call::ClearAttachments { .. } | Call::Delete { .. } => self.confirm_done(result, tx),
        }
    }

    /// The session left the server: out of the list, and out of the window when it was the open one (`removeSessionFromStore`).
    pub(crate) fn forget_session(&mut self, id: &str, tx: &mut Tx) {
        let key = self.machine_key_of(id).to_string();
        let mut projects = std::mem::take(&mut self.projects);
        self.store.remove(&mut projects, &mut self.route, id);
        self.on_list(projects, tx);
        // Out of that machine's list at once, before its next read.
        if let Some(m) = self.machines.others.get_mut(&key) {
            m.sessions.retain(|s| s.id != id);
        }
        self.list_again(id);
    }

    /// Reads the list `session` is in again: home's own, or only that machine's (`refreshList`, `refreshMachine`).
    pub(crate) fn list_again(&mut self, session: &str) {
        let key = self.machine_key_of(session).to_string();
        self.list_again_on(&key);
    }
}

// ---- scenes ---------------------------------------------------------------------------------------------------------------

impl State {
    /// The row at (`p`, `i`) renamed in place, the title selected (a scene has no focus to give, so the field is shown focused).
    pub fn show_row_rename(&mut self, p: usize, i: usize) {
        let session = &self.projects[p].sessions[i];
        let (id, title) = (session.id.clone(), session.title.clone());
        self.act.edit = TextEdit::new(title.clone());
        self.act.edit.set_selection(0, title.len());
        self.act.edit.focused = true;
        self.act.rename = Some(RowRename { id, original: title, seen_focus: true });
    }
}
