//! The open session's changed files (`composables/use-diffs.ts`, `lib/diff-base.ts`, `GET /api/sessions/{id}/diffs`): the list
//! Changes, the tab's count and the file tree's marks read. `Diffs` is pure: it says which request to make and takes the answer;
//! the thread, the 250 ms window and the events are in the `State` half at the bottom. Contents come per file (piece C), not here.
use super::events::{Event, Kind};
use super::http;
use super::state::Topic;
use crate::State;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// How long edits gather before Changes asks again (OpenChamber waits 250 ms after a tool call settles).
pub const EDIT_WINDOW: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Deleted,
    #[default]
    Modified,
}

/// One entry of the list (`FileDiffItem`). `before` and `after` are empty in the list; a file's contents come on their own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileDiff {
    pub file: String,
    pub status: FileStatus,
    pub additions: u64,
    pub deletions: u64,
    pub before: String,
    pub after: String,
    /// `isBinary` or `binary`.
    pub binary: bool,
    /// `isTruncated` or `truncated`.
    pub truncated: bool,
}

fn flag(v: &Value, long: &str, short: &str) -> bool {
    v.get(long).and_then(Value::as_bool).or_else(|| v.get(short).and_then(Value::as_bool)).unwrap_or(false)
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// An entry without a path is not a file; an unknown status reads as modified.
fn parse_item(v: &Value) -> Option<FileDiff> {
    let count = |key: &str| v.get(key).and_then(Value::as_u64).unwrap_or(0);
    Some(FileDiff {
        file: text(v, "file")?,
        status: match v.get("status").and_then(Value::as_str) {
            Some("added") => FileStatus::Added,
            Some("deleted") => FileStatus::Deleted,
            _ => FileStatus::Modified,
        },
        additions: count("additions"),
        deletions: count("deletions"),
        before: text(v, "before").unwrap_or_default(),
        after: text(v, "after").unwrap_or_default(),
        binary: flag(v, "isBinary", "binary"),
        truncated: flag(v, "isTruncated", "truncated"),
    })
}

/// One file's answer (`GET .../diffs/file`): the item itself, with the path asked for when the server leaves it out.
pub(super) fn parse_file(body: &Value, path: &str) -> FileDiff {
    let mut body = body.clone();
    if body.get("file").is_none() && body.is_object() {
        body["file"] = Value::String(path.to_string());
    }
    parse_item(&body).unwrap_or_else(|| FileDiff { file: path.to_string(), ..Default::default() })
}

/// What the changes are compared with (`SessionDiffBase`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Base {
    /// Where the folder's branch left the main branch.
    Branch { branch: Option<String>, commit: Option<String> },
    /// The folder as it was when the session started.
    Session,
}

impl Base {
    /// `baseKey`: a file's base contents are the same for as long as this is.
    pub fn key(base: Option<&Base>) -> String {
        match base {
            None => String::new(),
            Some(Base::Branch { commit, .. }) => format!("branch:{}", commit.as_deref().unwrap_or("")),
            Some(Base::Session) => "session".to_string(),
        }
    }

    fn parse(v: &Value) -> Option<Base> {
        match v.get("kind").and_then(Value::as_str)? {
            "branch" => Some(Base::Branch { branch: text(v, "branch"), commit: text(v, "commit") }),
            "session" => Some(Base::Session),
            _ => None,
        }
    }
}

/// Why the changes can't be read: git refuses the folder (another user owns it), and the command that lets it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub message: String,
    pub fix_command: String,
}

/// `diffBaseLabel`'s answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseLabel {
    /// "Compared with main", or "Since this session started".
    pub text: String,
    /// The short commit the branch left main at; `None` when compared with the session start.
    pub commit: Option<String>,
    /// What that means, for a tooltip.
    pub detail: String,
}

pub fn diff_base_label(base: Option<&Base>) -> Option<BaseLabel> {
    Some(match base? {
        Base::Branch { branch, commit } => {
            let branch = branch.as_deref().filter(|b| !b.is_empty()).unwrap_or("main");
            let commit = commit.as_deref().filter(|c| !c.is_empty()).map(|c| c.chars().take(7).collect::<String>());
            let at = commit.as_ref().map(|c| format!(" at {c}")).unwrap_or_default();
            BaseLabel {
                text: format!("Compared with {branch}"),
                detail: format!(
                    "What this branch changes since it left {branch}{at}, committed or not. Rebasing onto {branch} or pulling it doesn't add other people's work."
                ),
                commit,
            }
        }
        Base::Session => BaseLabel {
            text: "Since this session started".to_string(),
            commit: None,
            detail: "This folder isn't on a branch of its own, so its changes are compared with how it was when the session started."
                .to_string(),
        },
    })
}

/// A request that has been asked for: its answer goes to `finish`, which drops it when the session or a newer request has moved on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub session: String,
    id: u64,
    refresh: bool,
}

#[derive(Debug, Default)]
pub struct Diffs {
    session: String,
    files: Vec<FileDiff>,
    index: HashMap<String, usize>,
    pub available: bool,
    /// Why the changes are unavailable when git refuses the folder.
    pub refusal: Option<Refusal>,
    pub base: Option<Base>,
    /// `HTTP <status>` or the failure's own words.
    pub error: Option<String>,
    /// The first answer (or failure) has arrived: until then an empty list means "not known yet".
    pub has_loaded: bool,
    pub is_stale: bool,
    /// Copy command was pressed (`copied` in ChangesCanvas): it stays "Copied" until the body is mounted again.
    pub copied_fix: bool,
    request_id: u64,
    /// The newest request still on its way.
    in_flight: Option<u64>,
    window_open: bool,
    refreshing: bool,
    refresh_again: bool,
}

impl Diffs {
    /// The session these are the changes of; empty before one is opened.
    pub fn session(&self) -> &str {
        &self.session
    }

    /// The changed files with their line counts, in the server's order.
    pub fn files(&self) -> &[FileDiff] {
        &self.files
    }

    /// `byFile`: one path's entry.
    pub fn by_file(&self, path: &str) -> Option<&FileDiff> {
        self.index.get(path).map(|&i| &self.files[i])
    }

    pub fn is_loading(&self) -> bool {
        self.in_flight.is_some()
    }

    /// Another session (or none): everything resets at once, and an answer still on its way is dropped.
    pub fn reset(&mut self, session: &str) {
        let request_id = self.request_id + 1;
        *self = Diffs { session: session.to_string(), request_id, ..Default::default() };
    }

    /// `fetchDiffs`: the request to make, or `None` when there is no session or (`join`) one is already on its way, which will
    /// answer for this caller too. Without `join` (after an edit or a revert) a new request always goes out, so its answer
    /// can't predate the change.
    pub fn begin(&mut self, join: bool) -> Option<Ticket> {
        self.begin_as(join, false)
    }

    fn begin_as(&mut self, join: bool, refresh: bool) -> Option<Ticket> {
        if self.session.is_empty() {
            return None;
        }
        if join && self.in_flight.is_some() {
            return None;
        }
        self.request_id += 1;
        self.in_flight = Some(self.request_id);
        self.error = None;
        Some(Ticket { session: self.session.clone(), id: self.request_id, refresh })
    }

    /// An answer, or the failure to get one. Returns the request that follows when edits came in during a refresh.
    pub fn finish(&mut self, ticket: &Ticket, answer: Result<(u16, Value), String>) -> Option<Ticket> {
        let current = ticket.session == self.session && ticket.id == self.request_id;
        if current {
            match answer {
                Ok((status, body)) if (200..300).contains(&status) => self.apply(&body),
                Ok((status, _)) => self.fail(format!("HTTP {status}")),
                Err(message) => self.fail(message),
            }
            self.in_flight = None;
            self.has_loaded = true;
        }
        // The refresh loop ends on any answer, even one that was dropped.
        if ticket.refresh && ticket.session == self.session {
            self.refreshing = false;
            if std::mem::take(&mut self.refresh_again) {
                return self.refresh();
            }
        }
        None
    }

    /// The request could not be sent at all.
    fn abandon(&mut self, ticket: &Ticket) {
        if ticket.session == self.session && ticket.id == self.request_id {
            self.in_flight = None;
        }
        if ticket.refresh && ticket.session == self.session {
            self.refreshing = false;
        }
    }

    fn fail(&mut self, message: String) {
        self.available = false;
        self.base = None;
        self.error = Some(message);
    }

    /// The answer's wrapper object, or a bare array from an old server (which counts as available).
    pub fn apply(&mut self, body: &Value) {
        let items: Vec<FileDiff> = match body {
            Value::Array(a) => a.iter().filter_map(parse_item).collect(),
            _ => body.get("diffs").and_then(Value::as_array).map(|a| a.iter().filter_map(parse_item).collect()).unwrap_or_default(),
        };
        // The list is replaced only when a file, a status or a count changed, so what derives from it doesn't redo on every edit.
        let same = items.len() == self.files.len()
            && items
                .iter()
                .zip(&self.files)
                .all(|(a, b)| (&a.file, a.status, a.additions, a.deletions) == (&b.file, b.status, b.additions, b.deletions));
        if !same {
            self.index = items.iter().enumerate().map(|(i, d)| (d.file.clone(), i)).collect();
            self.files = items;
        }
        let object = !body.is_array();
        self.available = !object || body.get("available").and_then(Value::as_bool).unwrap_or(true);
        self.refusal = body
            .get("refusal")
            .filter(|r| r.is_object())
            .map(|r| Refusal { message: text(r, "message").unwrap_or_default(), fix_command: text(r, "fixCommand").unwrap_or_default() });
        let next = body.get("base").and_then(Base::parse);
        if Base::key(next.as_ref()) != Base::key(self.base.as_ref()) {
            self.base = next;
        }
        self.is_stale = false;
        self.error = None;
    }

    pub fn mark_stale(&mut self) {
        if !self.session.is_empty() {
            self.is_stale = true;
        }
    }

    /// `turn.ended` for this session: ask at once.
    pub fn on_turn_ended(&mut self) -> Option<Ticket> {
        self.begin(false)
    }

    /// `files.changed` for this session. `wanted`: something shows the diffs. Returns true when this edit opens the window
    /// (the caller ends it after `EDIT_WINDOW`); edits inside an open window join it rather than pushing it back.
    pub fn on_files_changed(&mut self, wanted: bool) -> bool {
        if self.session.is_empty() {
            return false;
        }
        if !wanted {
            self.mark_stale();
            return false;
        }
        !std::mem::replace(&mut self.window_open, true)
    }

    /// The window ended: one refresh at a time.
    pub fn window_ended(&mut self, session: &str) -> Option<Ticket> {
        if session != self.session || !std::mem::take(&mut self.window_open) {
            return None;
        }
        self.refresh()
    }

    /// One refresh at a time: one still running is followed by exactly one more, not joined by a second.
    fn refresh(&mut self) -> Option<Ticket> {
        if self.refreshing {
            self.refresh_again = true;
            return None;
        }
        let ticket = self.begin_as(false, true)?;
        self.refreshing = true;
        Some(ticket)
    }

    /// Whatever shows the diffs opened (`isWanted` turned true): a stale list loads unless a load is on its way.
    pub fn on_wanted(&mut self) -> Option<Ticket> {
        if self.is_stale && self.in_flight.is_none() { self.begin(false) } else { None }
    }
}

impl State {
    /// Copy command (`copyFixCommand`): nothing to copy, nothing happens; "Copied" shows once the clipboard has the command.
    pub fn copy_fix_command(&mut self, tx: &mut rune::Tx) {
        let Some(command) = self.diffs.refusal.as_ref().map(|r| r.fix_command.clone()).filter(|c| !c.is_empty()) else { return };
        tx.copy(command);
        self.diffs.copied_fix = true;
    }

    /// Try again: ask for the list again (`retry`); it does nothing while a request is on its way.
    pub fn retry_diffs(&mut self) {
        if self.diffs.is_loading() {
            return;
        }
        self.refetch_diffs();
    }

    /// Whether anything shows the diffs (`isWanted`). On the desktop the panel is always wanted: `SessionsV2RightPanel` sits
    /// beside the route in `AppShell`, not under it, so it never gets the route's instance (the one with `isWanted`) and runs
    /// its own, which has none. A folded panel still learns of the first change, and that is what opens an empty session's.
    fn diffs_wanted(&self) -> bool {
        true
    }

    /// A session opened (`useDiffs` mounting): another session starts from nothing; the page and the panel both asking shares
    /// one request. Nothing for an invented session or with no server.
    pub fn diffs_open(&mut self, session: &str) {
        if self.diffs.session() != session {
            self.diffs.reset(session);
        } else if self.diffs.has_loaded && !self.diffs.is_stale {
            return;
        }
        let ticket = self.diffs.begin(true);
        self.send_diffs(ticket);
    }

    /// Asks again after an edit made here (a revert), never joining an answer that may predate it.
    pub fn refetch_diffs(&mut self) {
        let ticket = self.diffs.begin(false);
        self.send_diffs(ticket);
    }

    /// The panel opened or closed: opening catches a stale list up.
    pub fn diffs_wanted_changed(&mut self) {
        if self.diffs_wanted() {
            let ticket = self.diffs.on_wanted();
            self.send_diffs(ticket);
            self.review_sync();
        }
    }

    /// `turn.ended` and `files.changed` on the open session's topic.
    pub fn on_diffs_event(&mut self, topic: &Topic, event: &Event) {
        let Topic::Session(open) = topic else { return };
        if self.diffs.session() != open {
            return;
        }
        match event {
            Event::TurnEnded => {
                let ticket = self.diffs.on_turn_ended();
                self.send_diffs(ticket);
            }
            Event::Other(Kind::FilesChanged, payload) if payload.get("sessionId").and_then(Value::as_str) == Some(open) => {
                let wanted = self.diffs_wanted();
                if self.diffs.on_files_changed(wanted) {
                    self.end_window_later(open);
                }
            }
            _ => {}
        }
    }

    fn end_window_later(&self, session: &str) {
        let Some(link) = self.link_for(session) else { return };
        let id = session.to_string();
        link.off_thread(
            |_| std::thread::sleep(EDIT_WINDOW),
            move |s, _, ()| {
                let ticket = s.diffs.window_ended(&id);
                s.send_diffs(ticket);
            },
        );
    }

    /// Runs a request off the UI thread and hands its answer to `finish`; a follow-up request goes out the same way.
    fn send_diffs(&mut self, ticket: Option<Ticket>) {
        let Some(ticket) = ticket else { return };
        let Some(link) = self.link_for(&ticket.session) else {
            // No server to ask (an invented session): nothing is loading, and nothing was learned.
            self.diffs.abandon(&ticket);
            return;
        };
        let id = ticket.session.clone();
        link.off_thread(
            move |base| http::diffs(base, &id),
            move |s, _, answer| {
                let next = s.diffs.finish(&ticket, answer);
                s.send_diffs(next);
                s.review_sync();
            },
        );
    }
}
