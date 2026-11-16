//! What the header's toolbar does to the open session (`use-session-detail-actions.ts`, `SessionActionToolbar.vue`): abort,
//! fork, archive, restore and delete, each checking what the session's capabilities allow first, and the one error line each
//! keeps under the buttons. Rename and Export are in `header_actions.rs` and `export.rs`; the menu only calls them.
//! The requests run on a thread; with no server the invented list takes the change, so scenes and tests need none.
use super::actions::Call;
use super::{http, row_status::display_title};
use crate::State;
use crate::Tx;
use crate::ui::confirm::Confirm;
use serde_json::json;

/// `PATCH /api/sessions/{id}/retention`.
pub fn retention_path(id: &str) -> String {
    format!("/api/sessions/{}/retention", crate::conv::image_sniff::encode_component(id))
}

/// `POST /api/sessions/{id}/abort`.
pub fn abort_path(id: &str) -> String {
    format!("/api/sessions/{}/abort", crate::conv::image_sniff::encode_component(id))
}

pub fn retention_body(archived: bool) -> serde_json::Value {
    json!({ "retentionStatus": if archived { "archived" } else { "active" } })
}

/// Makes the request, blocking: a refusal is the server's own words (`readErrorMessage`: `error`, `detail`, `title`), else
/// `HTTP <status>`. Run it on a thread.
pub fn request(base: &str, method: &str, path: &str, body: Option<&serde_json::Value>) -> Result<(), String> {
    let (status, text) = http::text_with(method, base, path, body)?;
    if (200..300).contains(&status) { Ok(()) } else { Err(super::store::error_text(status, &text)) }
}

/// What a refused archive says (`archiveQueue`'s toast): the title in quotes, as Fleet writes it.
pub fn archive_failure(title: &str) -> String {
    format!("Couldn't archive \"{title}\".")
}

/// One message about one session: its error line under the buttons.
type Note = Option<(String, String)>;

#[derive(Default)]
pub struct Toolbar {
    /// The session an abort, a restore, an archive or a rename is in flight for (`isAnyActionPending`).
    pub aborting: Option<String>,
    pub restoring: Option<String>,
    pub archiving: Option<String>,
    pub renaming: Option<String>,
    /// The error lines, each with its session: abort, delete and rename (`actionErrors`).
    pub abort_error: Note,
    pub delete_error: Note,
    pub rename_error: Note,
    /// The fork dialog, while it is open (it opens only when forking is allowed).
    pub fork: Option<super::fork::ForkDialog>,
}

impl Toolbar {
    fn of<'a>(note: &'a Note, id: &str) -> Option<&'a str> {
        note.as_ref().filter(|(s, _)| s == id).map(|(_, e)| e.as_str())
    }

    /// The lines under the toolbar for `id`, in Fleet's order.
    pub fn errors(&self, id: &str) -> Vec<&str> {
        [&self.abort_error, &self.delete_error, &self.rename_error].into_iter().filter_map(|n| Self::of(n, id)).collect()
    }
}

impl State {
    /// A delete's confirm is open and running for the open session.
    fn deleting(&self, id: &str) -> bool {
        self.ui.confirm.as_ref().is_some_and(|c| c.busy && matches!(&c.call, Call::Delete { id: d } if d == id))
    }

    /// `isAnyActionPending`: abort, restore, delete or rename in flight for the open session.
    pub fn toolbar_pending(&self) -> bool {
        let id = self.session().id.as_str();
        let bar = &self.head.bar;
        [&bar.aborting, &bar.restoring, &bar.renaming].iter().any(|p| p.as_deref() == Some(id)) || self.deleting(id)
    }

    /// The ⋯ button's spinner: renaming, archiving or deleting.
    pub fn toolbar_busy(&self) -> bool {
        let id = self.session().id.as_str();
        let bar = &self.head.bar;
        bar.renaming.as_deref() == Some(id) || bar.archiving.as_deref() == Some(id) || self.deleting(id)
    }

    /// Abort (`handleAbort`): the request is made; a refusal is the line under the toolbar. The session's own events say
    /// when it has stopped.
    pub fn toolbar_abort(&mut self, _tx: &mut Tx) {
        let m = self.header_model();
        if !self.has_session() || !m.actions.can_abort || self.toolbar_pending() {
            return;
        }
        let id = self.session().id.clone();
        self.head.bar.abort_error = None;
        let Some(link) = self.link_for(&id) else {
            // The invented session stops the way Esc stops it.
            crate::interrupt(self);
            return;
        };
        self.head.bar.aborting = Some(id.clone());
        let session = id.clone();
        link.off_thread(
            move |base| request(base, "POST", &abort_path(&session), None),
            move |s, _, result| {
                s.head.bar.aborting = None;
                if let Err(why) = result {
                    s.head.bar.abort_error = Some((id, why));
                }
            },
        );
    }

    /// Fork (`handleFork`): the dialog opens only when forking is allowed.
    pub fn toolbar_fork(&mut self, tx: &mut Tx) {
        let m = self.header_model();
        if !self.has_session() || !m.actions.can_fork || self.toolbar_pending() {
            return;
        }
        self.open_fork_dialog(tx);
    }

    /// Delete… (`handleDelete`): asks first, naming the session; the delete runs inside the confirm, which goes to the
    /// sessions view once it has gone (`forget_session`).
    pub fn toolbar_delete(&mut self, tx: &mut Tx) {
        let m = self.header_model();
        if !self.has_session() || !m.actions.can_delete || self.toolbar_pending() {
            return;
        }
        let session = self.session();
        let (id, title) = (session.id.clone(), display_title(None, &session.title));
        self.head.bar.delete_error = None;
        self.ask(Confirm::delete_session(title, Call::Delete { id }), tx);
    }

    /// Archive (`handleArchive`). Fleet queues it behind an Undo window (component 6, piece I); until that is built the
    /// request is made at once.
    pub fn toolbar_archive(&mut self, tx: &mut Tx) {
        let m = self.header_model();
        if !self.has_session() || !m.actions.can_archive || self.head.bar.archiving.is_some() {
            return;
        }
        self.set_retention(true, tx);
    }

    /// Restore (`handleRestore`): un-archives; a refusal is a toast.
    pub fn toolbar_restore(&mut self, tx: &mut Tx) {
        let m = self.header_model();
        if !self.has_session() || !m.actions.can_restore || self.toolbar_pending() {
            return;
        }
        self.set_retention(false, tx);
    }

    fn set_retention(&mut self, archive: bool, tx: &mut Tx) {
        let id = self.session().id.clone();
        if archive {
            self.head.bar.archiving = Some(id.clone());
        } else {
            self.head.bar.restoring = Some(id.clone());
        }
        let Some(link) = self.link_for(&id) else {
            self.retention_done(&id, archive, Ok(()), tx);
            return;
        };
        let path = retention_path(&id);
        let body = retention_body(archive);
        link.off_thread(
            move |base| request(base, "PATCH", &path, Some(&body)),
            move |s, tx, result| s.retention_done(&id, archive, result, tx),
        );
    }

    pub(crate) fn retention_done(&mut self, id: &str, archive: bool, result: Result<(), String>, tx: &mut Tx) {
        let bar = &mut self.head.bar;
        let slot = if archive { &mut bar.archiving } else { &mut bar.restoring };
        if slot.as_deref() == Some(id) {
            *slot = None;
        }
        match result {
            Ok(()) => {
                let mut projects = std::mem::take(&mut self.projects);
                self.store.patch(&mut projects, id, |s| s.row.archived = archive);
                self.projects = projects;
                // The list is the truth: an archived session leaves it, a restored one comes back.
                if self.link_for(id).is_some() {
                    let key = self.machine_key_of(id).to_string();
                    self.list_again_on(&key);
                }
            }
            Err(why) => {
                let title = self.store.session_by_id(&self.projects, id).map(|s| display_title(None, &s.title)).unwrap_or_default();
                let message = if archive { archive_failure(&title) } else { why };
                self.toast(message, None, 5.0, tx);
            }
        }
    }

    /// A header rename is in flight (`isRenaming`), and its refusal is the line under the toolbar.
    pub(crate) fn header_rename_sent(&mut self, id: &str) {
        self.head.bar.renaming = Some(id.to_string());
        self.head.bar.rename_error = None;
    }

    /// A rename answered: ends the pending state and keeps a refusal's words. True when it was the header's.
    pub(crate) fn header_rename_done(&mut self, id: &str, result: &Result<(), String>) -> bool {
        if self.head.bar.renaming.as_deref() != Some(id) {
            return false;
        }
        self.head.bar.renaming = None;
        if let Err(why) = result {
            self.head.bar.rename_error = Some((id.to_string(), why.clone()));
        }
        true
    }
}

// ---- scenes ---------------------------------------------------------------------------------------------------------------

/// What a toolbar scene shows on top of the header.
#[derive(Clone, Copy)]
pub enum Scene {
    /// Just the buttons (a working session shows Abort).
    Plain,
    /// The ⋯ menu open.
    Menu,
    /// The ⋯ menu open, for a session with no folder.
    MenuNoDirectory,
    /// A harness that can't fork, with its reason.
    ForkOff,
    /// The error lines under the buttons.
    Errors,
    /// The delete question.
    Delete,
    /// The fork dialog: as it opens, while the request runs, and after a refusal.
    Fork,
    Forking,
    ForkFailed,
    /// The analytics popover, with numbers and with neither known.
    Analytics,
    AnalyticsEmpty,
}

impl State {
    /// The header of `case` with `scene` on it.
    pub fn show_toolbar(&mut self, case: crate::mock::header::Case, dark: bool, scene: Scene) {
        self.show_header(case, dark, true);
        let id = self.session().id.clone();
        match scene {
            Scene::Plain => {}
            Scene::Menu => self.ui.open = Some(crate::ui::Open::Menu("more")),
            Scene::MenuNoDirectory => {
                self.projects[0].sessions[0].row.directory.clear();
                self.ui.open = Some(crate::ui::Open::Menu("more"));
            }
            Scene::ForkOff => {
                self.projects[0].sessions[0].row.capabilities = super::rows::Capabilities {
                    can_fork: Some(false),
                    fork_disabled_reason: Some("This harness can't copy a conversation.".into()),
                    ..Default::default()
                };
            }
            Scene::Errors => {
                self.head.bar.abort_error = Some((id.clone(), "The harness did not answer the abort.".into()));
                self.head.bar.rename_error = Some((id, "Title is too long.".into()));
            }
            Scene::Fork | Scene::Forking | Scene::ForkFailed => {
                let mut d = super::fork::ForkDialog::open(&id, &self.session().title);
                let end = d.title.text.len();
                d.title.set_selection(0, end);
                d.forking = matches!(scene, Scene::Forking);
                if matches!(scene, Scene::ForkFailed) {
                    d.error = Some("This session has no finished reply to copy yet.".into());
                }
                self.head.bar.fork = Some(d);
            }
            Scene::Analytics | Scene::AnalyticsEmpty => {
                let row = &mut self.projects[0].sessions[0].row;
                (row.total_tokens, row.total_cost) =
                    if matches!(scene, Scene::Analytics) { (Some(128_400), Some(0.4231)) } else { (None, None) };
                self.ui.open = Some(crate::ui::Open::Popover("analytics"));
            }
            Scene::Delete => {
                let title = display_title(None, &self.session().title);
                self.ui.confirm = Some(Confirm::delete_session(title, Call::Delete { id }));
            }
        }
    }

    /// An abort on its way, for a scene.
    pub fn show_toolbar_aborting(&mut self) {
        self.head.bar.aborting = Some(self.session().id.clone());
    }
}
