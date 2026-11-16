//! What the header does on its own (`SessionDetailHeader.vue`): rename the title in place, add and remove tags, copy the
//! folder path and put a removed checkout back. The requests and their words are plain data; `State`'s methods run them on a
//! thread, and with no server the invented list takes the change, so scenes and tests need none.
//! The toolbar's actions (abort, archive, delete) are piece D's.
use super::actions::Call;
use super::{http, projects::rename_outcome};
use crate::{State, Tx};
use rune::TextEdit;
use serde_json::{Value, json};
use std::time::Duration;

/// The keys of the two fields.
pub const TITLE_FIELD: &str = "session-title-input";
pub const TAG_FIELD: &str = "tag-input";

/// How long "Copied" shows (`setTimeout(…, 1500)`) and how long the note after a restore stays (`6000`).
pub const COPIED_SECONDS: f64 = 1.5;
pub const RESTORED_SECONDS: f64 = 6.0;

/// `PATCH /api/sessions/{id}/tags`.
pub fn tags_path(session: &str) -> String {
    format!("/api/sessions/{}/tags", crate::conv::image_sniff::encode_component(session))
}

pub fn tags_body(tags: &[String]) -> Value {
    json!({ "tags": tags })
}

/// `POST /api/storage/worktrees/{workspaceId}/restore`.
pub fn restore_path(workspace: &str) -> String {
    format!("/api/storage/worktrees/{}/restore", crate::conv::image_sniff::encode_component(workspace))
}

/// `extractApiError(body, fallback)`: the body's `detail`, `error`, `title` or `message` when a string, else the fallback.
pub fn restore_failure(body: &str) -> String {
    const FALLBACK: &str = "Couldn't put the checkout back.";
    let Ok(Value::Object(fields)) = serde_json::from_str::<Value>(body) else {
        return FALLBACK.into();
    };
    ["detail", "error", "title", "message"].iter().find_map(|k| fields.get(*k).and_then(Value::as_str)).unwrap_or(FALLBACK).to_string()
}

/// `addTag`: the tags with `typed` appended, or `None` when it is blank or already there (the input just closes).
pub fn tags_with(current: &[String], typed: &str) -> Option<Vec<String>> {
    let tag = typed.trim();
    if tag.is_empty() || current.iter().any(|t| t == tag) {
        return None;
    }
    let mut next = current.to_vec();
    next.push(tag.to_string());
    Some(next)
}

/// `removeTag`: the tags without `tag`.
pub fn tags_without(current: &[String], tag: &str) -> Vec<String> {
    current.iter().filter(|t| *t != tag).cloned().collect()
}

/// The title field while the header's title is renamed.
pub struct TitleEdit {
    pub id: String,
    /// The raw title it started from (not the "Untitled session" the header says in its place).
    original: String,
    /// The field has held focus; losing it after that is the blur that commits.
    seen_focus: bool,
}

#[derive(Default)]
pub struct HeaderUi {
    pub title: Option<TitleEdit>,
    /// The title field's text (a field needs a place in the state to live in, whether or not one is shown).
    pub title_edit: TextEdit,
    /// The session whose tag is being typed, and whether the field has held focus yet.
    pub adding_tag: Option<(String, bool)>,
    pub tag_edit: TextEdit,
    /// When "Copied" ends, on `State::clock`.
    pub copied_until: Option<f64>,
    /// The session whose checkout is being put back from here.
    pub restoring: Option<String>,
    /// The session whose "Checkout restored" note shows, and when it ends.
    pub restored_until: Option<(String, f64)>,
    /// The server's sentence for the last restore that failed, with its session.
    pub restore_error: Option<(String, String)>,
}

impl HeaderUi {
    pub fn editing_title(&self, id: &str) -> bool {
        self.title.as_ref().is_some_and(|t| t.id == id)
    }

    pub fn adding_tag_to(&self, id: &str) -> bool {
        self.adding_tag.as_ref().is_some_and(|(s, _)| s == id)
    }

    pub fn restoring_here(&self, id: &str) -> bool {
        self.restoring.as_deref() == Some(id)
    }

    pub fn restored_note(&self, id: &str, now: f64) -> bool {
        self.restored_until.as_ref().is_some_and(|(s, until)| s == id && now < *until)
    }

    pub fn restore_error_of(&self, id: &str) -> Option<&str> {
        self.restore_error.as_ref().filter(|(s, _)| s == id).map(|(_, e)| e.as_str())
    }
}

impl State {
    // ---- the title ----------------------------------------------------------------------------------------------------

    /// A double-click, F2 on the title or the menu's Rename (`startTitleEdit`): not for an archived session. The raw title
    /// goes in the field with all of it selected.
    pub fn start_header_rename(&mut self, tx: &mut Tx) {
        let session = self.session();
        if session.row.archived {
            return;
        }
        let (id, title) = (session.id.clone(), session.title.trim().to_string());
        self.ui.open = None;
        self.head.title_edit = TextEdit::new(title.clone());
        self.head.title_edit.set_selection(0, title.len());
        self.head.title = Some(TitleEdit { id, original: title, seen_focus: false });
        tx.focus(TITLE_FIELD, true);
    }

    /// Enter, Escape or blur ends the edit; only the first counts (`titleEditHandled`).
    pub fn finish_header_rename(&mut self, commit: bool, tx: &mut Tx) {
        let Some(edit) = self.head.title.take() else { return };
        tx.focus("session-title", true);
        if !commit {
            return;
        }
        let Some(title) = rename_outcome(&edit.original, &self.head.title_edit.text) else { return };
        // The new title shows at once; a refusal puts the old one back (`useRenameSession`), the same as a row's.
        let was = edit.original;
        self.set_title(&edit.id, &title);
        self.send_session_call(Call::Rename { id: edit.id, title }, Some(was), tx);
    }

    // ---- tags ---------------------------------------------------------------------------------------------------------

    /// The + button (`startAddingTag`).
    pub fn start_adding_tag(&mut self, tx: &mut Tx) {
        let id = self.session().id.clone();
        self.head.tag_edit = TextEdit::new("");
        self.head.adding_tag = Some((id, false));
        tx.focus(TAG_FIELD, true);
    }

    /// Enter or blur (`addTag`): a new tag is sent, a blank or repeated one only closes the field.
    pub fn finish_adding_tag(&mut self, add: bool) {
        let Some((id, _)) = self.head.adding_tag.take() else { return };
        let typed = std::mem::take(&mut self.head.tag_edit.text);
        self.head.tag_edit = TextEdit::new("");
        if add && let Some(next) = tags_with(&self.tags_of(&id), &typed) {
            self.send_tags(&id, next);
        }
    }

    /// The tag's ×.
    pub fn remove_tag(&mut self, tag: &str) {
        let id = self.session().id.clone();
        let next = tags_without(&self.tags_of(&id), tag);
        self.send_tags(&id, next);
    }

    fn tags_of(&self, id: &str) -> Vec<String> {
        self.store.session_by_id(&self.projects, id).map(|s| s.row.tags.clone()).unwrap_or_default()
    }

    /// Sends the tags on a thread; the store takes them when the server has (`updateTags`: no optimistic change, and a
    /// failure is only a console error). With no server the invented list takes them at once.
    fn send_tags(&mut self, id: &str, tags: Vec<String>) {
        let Some(link) = self.link_for(id) else {
            self.set_tags(id, tags);
            return;
        };
        let (session, path, body) = (id.to_string(), tags_path(id), tags_body(&tags));
        link.off_thread(
            move |base| http::patch(base, &path, &body).map(drop),
            move |s, _, result| {
                if result.is_ok() {
                    s.set_tags(&session, tags);
                }
            },
        );
    }

    fn set_tags(&mut self, id: &str, tags: Vec<String>) {
        let mut projects = std::mem::take(&mut self.projects);
        self.store.patch(&mut projects, id, |s| s.row.tags = tags);
        self.projects = projects;
    }

    // ---- the folder path ------------------------------------------------------------------------------------------------

    /// The copy button (`copyDirectory`): the whole path on the clipboard, then "Copied". Rune's `Tx::copy` has no answer to
    /// wait for, so a refusal can't be seen here; `directory_copied(false)` is what it would do.
    pub fn copy_directory(&mut self, directory: &str, tx: &mut Tx) {
        if directory.is_empty() {
            return;
        }
        tx.copy(directory.to_string());
        self.directory_copied(true, tx);
    }

    /// The clipboard answered: only a success shows "Copied", for 1.5 s; a refusal says nothing.
    pub fn directory_copied(&mut self, ok: bool, tx: &mut Tx) {
        if !ok {
            return;
        }
        self.head.copied_until = Some(self.clock + COPIED_SECONDS);
        tx.wake_after(Duration::from_millis(100));
    }

    // ---- the removed checkout -------------------------------------------------------------------------------------------

    /// Restore now (`restoreCheckout`): no workspace does nothing. The request goes to the session's own machine; success
    /// clears the row's removal and shows the note for 6 s, a failure keeps the server's sentence beside the button.
    pub fn restore_checkout(&mut self, tx: &mut Tx) {
        let session = self.session();
        let (id, workspace) = (session.id.clone(), session.row.workspace_id.clone());
        if workspace.is_empty() || self.head.restoring.is_some() {
            return;
        }
        self.head.restoring = Some(id.clone());
        self.head.restore_error = None;
        let Some(link) = self.link_for(&id) else {
            // The invented list has no server to ask: the checkout comes back at once.
            self.checkout_restored(&id, Ok(()), tx);
            return;
        };
        let path = restore_path(&workspace);
        link.off_thread(
            move |base| {
                let (status, body) = http::text_with("POST", base, &path, None)?;
                if (200..300).contains(&status) { Ok(()) } else { Err(restore_failure(&body)) }
            },
            move |s, tx, result| s.checkout_restored(&id, result, tx),
        );
    }

    pub(crate) fn checkout_restored(&mut self, id: &str, result: Result<(), String>, tx: &mut Tx) {
        self.head.restoring = None;
        match result {
            Ok(()) => {
                let mut projects = std::mem::take(&mut self.projects);
                self.store.patch(&mut projects, id, |s| {
                    s.row.checkout_removed_at = None;
                    s.row.checkout_restoring = false;
                });
                self.projects = projects;
                self.head.restored_until = Some((id.to_string(), self.clock + RESTORED_SECONDS));
                tx.wake_after(Duration::from_millis(100));
            }
            // A request that never got an answer says the same sentence a refusal without one does.
            Err(sentence) => self.head.restore_error = Some((id.to_string(), sentence)),
        }
    }

    // ---- the clock and the focus ------------------------------------------------------------------------------------------

    /// Polled every frame: a field that lost focus after having it commits, and "Copied" and the restored note end by
    /// themselves. True when something changed.
    pub fn keep_header(&mut self, now: f64, tx: &mut Tx) -> bool {
        let mut changed = false;
        if let Some(edit) = self.head.title.as_mut() {
            if self.head.title_edit.focused {
                edit.seen_focus = true;
            } else if edit.seen_focus {
                self.finish_header_rename(true, tx);
                changed = true;
            }
        }
        if let Some((_, seen)) = self.head.adding_tag.as_mut() {
            if self.head.tag_edit.focused {
                *seen = true;
            } else if *seen {
                self.finish_adding_tag(true);
                changed = true;
            }
        }
        if self.head.copied_until.is_some_and(|until| now >= until) {
            self.head.copied_until = None;
            changed = true;
        }
        if self.head.restored_until.as_ref().is_some_and(|(_, until)| now >= *until) {
            self.head.restored_until = None;
            changed = true;
        }
        if self.head.copied_until.is_some() || self.head.restored_until.is_some() {
            tx.wake_after(Duration::from_millis(100));
        }
        changed
    }
}

// ---- scenes ---------------------------------------------------------------------------------------------------------------

impl State {
    /// The open session's title in its field, all of it selected (a scene has no focus to give, so it is shown focused).
    pub fn show_header_rename(&mut self) {
        let session = self.session();
        let (id, title) = (session.id.clone(), session.title.trim().to_string());
        self.head.title_edit = TextEdit::new(title.clone());
        self.head.title_edit.set_selection(0, title.len());
        self.head.title_edit.focused = true;
        self.head.title = Some(TitleEdit { id, original: title, seen_focus: true });
    }

    /// The tag field open with `typed` in it.
    pub fn show_header_tag(&mut self, typed: &str) {
        let id = self.session().id.clone();
        self.head.tag_edit = TextEdit::new(typed);
        self.head.tag_edit.focused = true;
        self.head.adding_tag = Some((id, true));
    }

    /// "Copied" showing for good (a scene's clock doesn't run).
    pub fn show_header_copied(&mut self) {
        self.head.copied_until = Some(f64::MAX);
    }

    /// The checkout chip's other states, for scenes.
    pub fn show_header_restoring(&mut self) {
        self.head.restoring = Some(self.session().id.clone());
    }

    pub fn show_header_restored(&mut self) {
        // What a finished restore leaves: the row has no removal, the note shows.
        self.projects[0].sessions[0].row.checkout_removed_at = None;
        self.head.restored_until = Some((self.session().id.clone(), f64::MAX));
    }

    pub fn show_header_restore_error(&mut self, sentence: &str) {
        self.head.restore_error = Some((self.session().id.clone(), sentence.to_string()));
    }
}
