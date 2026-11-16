//! Archive with Undo (`stores/archive-queue.ts`, rows 2.150-2.157, 4.20-4.26): an archive waits six seconds on the app's clock
//! before it is sent, so Undo only has to cancel a timer. While it waits, and while its request is on the way, the rows are
//! hidden from the list; a failure shows them again and says so in the toast. Closing the app inside the wait sends nothing,
//! which leaves the sessions active, the safe way for it to fail.
//!
//! A session is archived on the machine its row was on (`rowMachines`): the call goes through `State::link_for`, which picks
//! that machine's address and token. The requests and their words are plain data; `State`'s methods send them on a thread.
use super::{http, store};
use crate::{State, Tx};
use serde_json::{Value, json};
use std::collections::HashSet;

/// How long an archive waits for Undo before it reaches the server (`ARCHIVE_UNDO_MS`), in seconds on the app clock.
pub const UNDO_SECS: f64 = 6.0;

/// `PATCH /api/sessions/{id}/retention`.
pub fn retention_path(id: &str) -> String {
    format!("/api/sessions/{id}/retention")
}

/// The body of that request: `archived` to archive, `active` to restore (`UpdateSessionRetentionRequest`).
pub fn retention_body(archived: bool) -> Value {
    json!({ "retentionStatus": if archived { "archived" } else { "active" } })
}

/// The toast's words for what is waiting (`Archived "<title>"`, `Archived <n> sessions`).
pub fn archived_message(titles: &[String]) -> String {
    match titles {
        [one] => format!("Archived \"{one}\""),
        many => format!("Archived {} sessions", many.len()),
    }
}

/// What a batch that failed says: the one session by name, else how many (`flush`).
pub fn failed_message(titles: &[String]) -> String {
    match titles {
        [one] => format!("Couldn't archive \"{one}\"."),
        many => format!("Couldn't archive {} sessions.", many.len()),
    }
}

/// A title as the toast names it: trimmed, else "Untitled session" (`titleOf`).
pub fn toast_title(title: &str) -> String {
    let title = title.trim();
    if title.is_empty() { "Untitled session".into() } else { title.to_string() }
}

/// Makes the retention call, blocking: run it on a thread.
pub fn send(base: &str, id: &str, archived: bool) -> Result<(), String> {
    let (status, text) = http::text_with("PATCH", base, &retention_path(id), Some(&retention_body(archived)))?;
    if (200..300).contains(&status) { Ok(()) } else { Err(store::error_text(status, &text)) }
}

/// The archive that is waiting for its six seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    pub ids: Vec<String>,
    pub message: String,
    /// When it goes to the server, on the app clock.
    pub until: f64,
}

/// One flush of the queue: its requests run side by side, and the failures are told once all have answered.
#[derive(Default)]
struct Flight {
    open: usize,
    failed: Vec<String>,
}

#[derive(Default)]
pub struct ArchiveQueue {
    pub pending: Option<Pending>,
    /// Sessions whose request is on its way: still hidden, because the list says active until the server has answered.
    sending: HashSet<String>,
    /// Restores on their way (`isRestoring`).
    restoring: HashSet<String>,
    /// The last failure, until it is dismissed or something new is archived.
    pub error: Option<String>,
    flights: std::collections::HashMap<u64, Flight>,
    next_flight: u64,
}

impl ArchiveQueue {
    /// Whether the list leaves `id` out (`pendingIds`).
    pub fn hides(&self, id: &str) -> bool {
        self.sending.contains(id) || self.pending.as_ref().is_some_and(|p| p.ids.iter().any(|i| i == id))
    }

    /// Whether any row is hidden.
    pub fn any_hidden(&self) -> bool {
        self.pending.is_some() || !self.sending.is_empty()
    }
}

impl State {
    /// Hides the sessions now and archives them once the undo window closes, on `machine` when it is given (the machine of the
    /// rows they were picked from; they are all on one).
    pub fn archive_sessions(&mut self, ids: &[String], machine: Option<&str>, tx: &mut Tx) {
        let mut seen = HashSet::new();
        let ids: Vec<String> = ids.iter().filter(|id| seen.insert(id.as_str())).cloned().collect();
        if ids.is_empty() {
            return;
        }
        // A second archive inside the window sends the first one right away.
        self.flush_archive();
        if let Some(machine) = machine {
            self.machines.remember_sessions(machine, &ids);
        }
        self.archive.error = None;
        let titles: Vec<String> = ids.iter().map(|id| toast_title(&self.title_of(id).unwrap_or_default())).collect();
        let message = archived_message(&titles);
        self.archive.pending = Some(Pending { ids, message, until: self.clock + UNDO_SECS });
        tx.wake_after(std::time::Duration::from_millis(100));
    }

    /// Undo: nothing has reached the server, so the rows come back.
    pub fn undo_archive(&mut self) {
        self.archive.pending = None;
    }

    /// The window closed (or another archive began): one request per session, side by side.
    pub fn flush_archive(&mut self) {
        let Some(pending) = self.archive.pending.take() else { return };
        self.archive.sending.extend(pending.ids.iter().cloned());
        let flight = self.archive.next_flight;
        self.archive.next_flight += 1;
        self.archive.flights.insert(flight, Flight { open: pending.ids.len(), failed: Vec::new() });
        for id in pending.ids {
            match self.link_for(&id) {
                // No server (the invented list): the change just happens.
                None => self.archive_done(flight, &id, Ok(())),
                Some(link) => {
                    let sent = id.clone();
                    link.off_thread(move |base| send(base, &id, true), move |s, _, result| s.archive_done(flight, &sent, result));
                }
            }
        }
    }

    fn archive_done(&mut self, flight: u64, id: &str, result: Result<(), String>) {
        self.archive.sending.remove(id);
        let ok = result.is_ok();
        if ok {
            self.set_retention(id, true);
        }
        let Some(f) = self.archive.flights.get_mut(&flight) else { return };
        f.open -= 1;
        if !ok {
            f.failed.push(id.to_string());
        }
        if f.open > 0 {
            return;
        }
        let failed = self.archive.flights.remove(&flight).map(|f| f.failed).unwrap_or_default();
        if !failed.is_empty() {
            let titles: Vec<String> = failed.iter().map(|id| toast_title(&self.title_of(id).unwrap_or_default())).collect();
            self.archive.error = Some(failed_message(&titles));
        }
    }

    /// Brings an archived session back, on `machine` when it is given (the machine of the row it was restored from).
    pub fn restore_session(&mut self, id: &str, machine: Option<&str>) {
        self.archive.error = None;
        if let Some(machine) = machine {
            self.machines.remember_sessions(machine, &[id.to_string()]);
        }
        let Some(link) = self.link_for(id) else {
            self.restore_done(id, Ok(()));
            return;
        };
        self.archive.restoring.insert(id.to_string());
        let sent = id.to_string();
        let id = id.to_string();
        link.off_thread(move |base| send(base, &id, false), move |s, _, result| s.restore_done(&sent, result));
    }

    fn restore_done(&mut self, id: &str, result: Result<(), String>) {
        self.archive.restoring.remove(id);
        match result {
            Ok(()) => self.set_retention(id, false),
            Err(why) => self.archive.error = Some(if why.is_empty() { "Couldn't restore the session.".into() } else { why }),
        }
    }

    /// The row says archived (or active) now, in home's list or its machine's, which is read again (`setRetention`).
    fn set_retention(&mut self, id: &str, archived: bool) {
        let mut projects = std::mem::take(&mut self.projects);
        self.store.patch(&mut projects, id, |s| s.row.archived = archived);
        self.projects = projects;
        let key = self.machine_key_of(id).to_string();
        self.machines.patch_session(&key, id, |s| s.row.archived = archived);
        self.list_again(id);
    }

    /// Whether the list leaves this row out: an archive is holding it back, or it is archived and the list shows active ones.
    pub fn hides(&self, session: &crate::mock::Session) -> bool {
        self.archive.hides(&session.id) || (self.store.retention == store::Retention::Active && session.row.archived)
    }

    /// The close on the error toast's X.
    pub fn dismiss_errors(&mut self) {
        self.archive.error = None;
        self.pins.error = None;
    }

    /// Whether a restore of `id` is on its way (`isRestoring`).
    pub fn is_restoring(&self, id: &str) -> bool {
        self.archive.restoring.contains(id)
    }

    /// The clock's part: sends the waiting archive once its time is up. True while there is a drain to draw.
    pub fn keep_archive(&mut self, now: f64, tx: &mut Tx) -> bool {
        let Some(p) = &self.archive.pending else { return false };
        if now >= p.until {
            self.flush_archive();
        } else {
            tx.wake_after(std::time::Duration::from_millis(100));
        }
        true
    }

    /// The one toast for what the list lets you undo, and for what failed: the archive waiting, else the first error.
    pub fn queue_toast(&self) -> Option<crate::ui::toast::Toast> {
        use crate::ui::toast::{Kind, Toast};
        if let Some(p) = &self.archive.pending {
            return Some(Toast {
                message: p.message.clone(),
                action: Some("Undo".into()),
                until: p.until,
                total: UNDO_SECS,
                kind: Kind::Undo,
            });
        }
        let error = self.archive.error.clone().or_else(|| self.pins.error.clone())?;
        Some(Toast { message: error, action: None, until: f64::MAX, total: 1.0, kind: Kind::Error })
    }
}

// ---- scenes ---------------------------------------------------------------------------------------------------------------

impl State {
    /// An archive waiting with `left` seconds of its six to go, for a picture of the drain.
    pub fn show_archive_pending(&mut self, id: &str, left: f64) {
        let title = toast_title(&self.title_of(id).unwrap_or_default());
        self.archive.pending = Some(Pending { ids: vec![id.to_string()], message: archived_message(&[title]), until: self.clock + left });
    }
}

impl State {
    /// The toast a refused archive leaves, for a picture.
    pub fn show_archive_error(&mut self) {
        self.archive.error = Some("Couldn't archive \"Tidy the release notes\".".into());
    }
}
