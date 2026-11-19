//! Open files' buffers, per machine, session and path (`stores/file-buffers.ts`, `lib/code-editor/buffers.ts`,
//! `lib/code-editor/file-buffer.ts`): what reading a file brings, why a file can't be shown, and what a new version of the
//! file on disk does to a buffer. `FileBuffers` is pure: it says which read to make and takes the answer; the thread is in
//! the `State` half at the bottom. Reads of one file are shared: a second open, or a refresh, while one is on its way asks
//! nothing. The text is the file's lines, split on its own break only; the editing half (typing, undo) is slice 2.
use super::http;
use crate::State;
use crate::conv::minimal_change::minimal_change;
use crate::conv::text_format::{TextFormat, join_lines, read_text_format, same_text_format, split_lines, to_disk_text};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub const TOO_LARGE_MESSAGE: &str = "Too large to edit here (over 512 KB).";
pub const NOT_TEXT_MESSAGE: &str = "This file is binary or not UTF-8 text, so it can't be edited here.";
pub const GONE_MESSAGE: &str = "This file isn't in the session's folder any more.";
pub const UNREADABLE_MESSAGE: &str = "The file couldn't be read.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Loading,
    Ready,
    Unavailable,
    Error,
}

/// The file changed on disk while the buffer had unsaved edits, or a save lost the race.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    /// What's on disk now (the agent's version); `None` when it's no longer text.
    pub disk_text: Option<String>,
    pub hash: String,
}

/// What the tab and bar show (`FileBufferInfo`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub status: Status,
    /// Why the file can't be shown, or why it failed to load.
    pub message: Option<String>,
    pub dirty: bool,
    pub conflict: Option<Conflict>,
    pub saving: bool,
}

impl Default for Info {
    fn default() -> Self {
        Info { status: Status::Loading, message: None, dirty: false, conflict: None, saving: false }
    }
}

/// A change to [`Info`]: only the fields that are set.
#[derive(Clone, Debug, Default)]
pub struct Patch {
    pub status: Option<Status>,
    pub message: Option<Option<String>>,
    pub dirty: Option<bool>,
    pub conflict: Option<Option<Conflict>>,
    pub saving: Option<bool>,
}

/// A file as the read endpoint returns it (`DiskFile`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiskFile {
    pub content: Option<String>,
    pub hash: Option<String>,
    pub is_binary: bool,
    pub is_truncated: bool,
}

impl DiskFile {
    pub fn new(content: Option<&str>, hash: &str) -> DiskFile {
        DiskFile { content: content.map(str::to_string), hash: Some(hash.to_string()), is_binary: false, is_truncated: false }
    }

    fn parse(body: &str) -> DiskFile {
        let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
        let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let flag = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
        DiskFile { content: text("content"), hash: text("hash"), is_binary: flag("isBinary"), is_truncated: flag("isTruncated") }
    }
}

/// Why a file can't be shown here, or `None` when it can (`unavailableReason`).
pub fn unavailable_reason(file: &DiskFile) -> Option<&'static str> {
    if file.is_truncated {
        Some(TOO_LARGE_MESSAGE)
    } else if file.is_binary || file.content.is_none() || file.hash.as_deref().is_none_or(str::is_empty) {
        Some(NOT_TEXT_MESSAGE)
    } else {
        None
    }
}

/// What a new version of the file did (`DiskUpdate`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiskUpdate {
    Unchanged,
    /// The lines the change covers, or `None` when the buffer started afresh.
    Updated(Option<(usize, usize)>),
    Conflict(Conflict),
    Unavailable(String),
}

/// One open file.
#[derive(Clone, Debug, Default)]
pub struct Buffer {
    info: Info,
    /// The text as lines, split on the file's own break only; empty until the file has been read.
    pub(crate) lines: Vec<String>,
    /// Whether the file was read (Fleet's `state`): a buffer that was only made is not.
    read: bool,
    /// The lines as last read from or saved to disk; the buffer is dirty when they differ.
    saved: Vec<String>,
    /// Hash of the disk version the buffer is based on; a save sends it.
    base_hash: Option<String>,
    format: Option<TextFormat>,
    /// Counts every change to `lines`, so what is worked out from the text (its colours) knows when it is stale.
    rev: u64,
}

impl Buffer {
    pub fn rev(&self) -> u64 {
        self.rev
    }

    pub fn info(&self) -> &Info {
        &self.info
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn format(&self) -> Option<&TextFormat> {
        self.format.as_ref()
    }

    pub fn base_hash(&self) -> Option<&str> {
        self.base_hash.as_deref()
    }

    pub fn is_dirty(&self) -> bool {
        self.read && self.lines != self.saved
    }

    /// The bytes to save, as the text the server writes (`diskTextOf`).
    pub fn disk_text(&self) -> String {
        match &self.format {
            Some(format) if self.read => to_disk_text(&join_lines(&self.lines, format), format),
            _ => String::new(),
        }
    }

    /// Fills the buffer from a disk read. Returns why it can't be shown, or `None` when it's ready (`loadBuffer`).
    fn load(&mut self, file: &DiskFile) -> Option<&'static str> {
        if let Some(reason) = unavailable_reason(file) {
            return Some(reason);
        }
        self.start_afresh(file);
        None
    }

    fn start_afresh(&mut self, file: &DiskFile) {
        let (text, format) = read_text_format(file.content.as_deref().unwrap_or(""));
        self.lines = split_lines(&text, &format);
        self.rev += 1;
        self.saved = self.lines.clone();
        self.format = Some(format);
        self.base_hash = file.hash.clone();
        self.read = true;
    }

    /// The file on disk may have changed (`applyDiskFile`). The same hash changes nothing. A buffer without unsaved edits takes
    /// the new text as one smallest replacement; one with unsaved edits is never touched: the caller shows the conflict.
    fn apply_disk_file(&mut self, file: &DiskFile) -> DiskUpdate {
        if file.hash.is_some() && file.hash == self.base_hash {
            return DiskUpdate::Unchanged;
        }
        let reason = unavailable_reason(file);
        if self.is_dirty() {
            let disk_text = if reason.is_some() { None } else { file.content.clone() };
            return DiskUpdate::Conflict(Conflict { disk_text, hash: file.hash.clone().unwrap_or_default() });
        }
        if let Some(reason) = reason {
            return DiskUpdate::Unavailable(reason.to_string());
        }
        let (text, format) = read_text_format(file.content.as_deref().unwrap_or(""));
        let Some(old) = self.format.filter(|old| self.read && same_text_format(old, &format)) else {
            // A new line-break style or BOM can't be applied as an edit; start from a fresh buffer.
            self.start_afresh(file);
            return DiskUpdate::Updated(None);
        };
        let next = split_lines(&text, &old);
        self.base_hash = file.hash.clone();
        let Some(change) = minimal_change(&self.lines, &next) else {
            self.saved = self.lines.clone();
            return DiskUpdate::Unchanged;
        };
        self.lines = change.apply(&self.lines);
        self.rev += 1;
        self.saved = self.lines.clone();
        DiskUpdate::Updated(Some(change.lines))
    }
}

/// Which read an answer is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Opening the file: the first read, or another after a failed one.
    Open,
    /// Reading an open file again; `unavailable` is what it was when asked.
    Refresh { unavailable: bool },
}

/// A read to make: its answer goes to `finish`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub machine: String,
    pub session: String,
    pub path: String,
    pub kind: Kind,
}

type Key = (String, String, String);

fn key(machine: &str, session: &str, path: &str) -> Key {
    (machine.to_string(), session.to_string(), path.to_string())
}

#[derive(Debug, Default)]
pub struct FileBuffers {
    buffers: HashMap<Key, Buffer>,
    /// The files with a read on its way: a second ask for one waits for it (`refreshing`).
    reading: HashSet<Key>,
}

impl FileBuffers {
    pub fn buffer(&self, machine: &str, session: &str, path: &str) -> Option<&Buffer> {
        self.buffers.get(&key(machine, session, path))
    }

    pub fn info(&self, machine: &str, session: &str, path: &str) -> Option<&Info> {
        self.buffer(machine, session, path).map(Buffer::info)
    }

    /// The buffer for a file, made (loading) if it isn't open yet (`ensure`).
    pub fn ensure(&mut self, machine: &str, session: &str, path: &str) -> &Buffer {
        self.buffers.entry(key(machine, session, path)).or_default()
    }

    /// Changes what is set; `false` when nothing was different, so the info stays as it was (`patch`).
    pub fn patch(&mut self, machine: &str, session: &str, path: &str, next: Patch) -> bool {
        let Some(buffer) = self.buffers.get_mut(&key(machine, session, path)) else { return false };
        let before = buffer.info.clone();
        let info = &mut buffer.info;
        if let Some(v) = next.status {
            info.status = v;
        }
        if let Some(v) = next.message {
            info.message = v;
        }
        if let Some(v) = next.dirty {
            info.dirty = v;
        }
        if let Some(v) = next.conflict {
            info.conflict = v;
        }
        if let Some(v) = next.saving {
            info.saving = v;
        }
        *info != before
    }

    /// A buffer leaves when its tab closes, a preview it was is replaced, or the session goes (`remove`).
    pub fn remove(&mut self, machine: &str, session: &str, path: &str) {
        self.buffers.remove(&key(machine, session, path));
        self.reading.remove(&key(machine, session, path));
    }

    pub fn is_dirty(&self, machine: &str, session: &str, path: &str) -> bool {
        self.info(machine, session, path).is_some_and(|i| i.dirty)
    }

    /// The paths open in a session, for re-checking them when the agent changes files (`openPaths`).
    pub fn open_paths(&self, machine: &str, session: &str) -> Vec<String> {
        let mut paths: Vec<String> =
            self.buffers.keys().filter(|(m, s, _)| m == machine && s == session).map(|(_, _, p)| p.clone()).collect();
        paths.sort();
        paths
    }

    /// Files with unsaved changes, across sessions: `(machine, session, path)`.
    pub fn unsaved(&self) -> Vec<(String, String, String)> {
        let mut files: Vec<Key> = self.buffers.iter().filter(|(_, b)| b.info.dirty).map(|(k, _)| k.clone()).collect();
        files.sort();
        files
    }

    /// Tests write into a buffer's lines as typing will (slice 2).
    #[cfg(test)]
    pub(crate) fn buffers_for_test(&mut self, machine: &str, session: &str, path: &str) -> &mut Buffer {
        self.buffers.get_mut(&key(machine, session, path)).unwrap()
    }

    pub fn is_reading(&self, machine: &str, session: &str, path: &str) -> bool {
        self.reading.contains(&key(machine, session, path))
    }

    /// Opens a file: the read to make, or `None` when the file is there already, can't be shown (an unavailable file is not
    /// read again by opening it), or a read of it is on its way (`openBuffer`).
    pub fn open(&mut self, machine: &str, session: &str, path: &str) -> Option<Request> {
        let k = key(machine, session, path);
        let buffer = self.buffers.entry(k.clone()).or_default();
        if buffer.read || buffer.info.status == Status::Unavailable || self.reading.contains(&k) {
            return None;
        }
        buffer.info.status = Status::Loading;
        buffer.info.message = None;
        self.reading.insert(k);
        Some(Request { machine: machine.into(), session: session.into(), path: path.into(), kind: Kind::Open })
    }

    /// Reads an open file again (`refreshBuffer`). Skipped while a save is in flight (that save's own `files.changed` would
    /// otherwise look like the agent's change), for a file that isn't open or isn't ready or unavailable, and while a read is
    /// on its way (overlapping calls share one read).
    pub fn refresh(&mut self, machine: &str, session: &str, path: &str) -> Option<Request> {
        let k = key(machine, session, path);
        let info = &self.buffers.get(&k)?.info;
        if info.saving || !matches!(info.status, Status::Ready | Status::Unavailable) || self.reading.contains(&k) {
            return None;
        }
        let unavailable = info.status == Status::Unavailable;
        self.reading.insert(k);
        Some(Request { machine: machine.into(), session: session.into(), path: path.into(), kind: Kind::Refresh { unavailable } })
    }

    /// A read that was never sent.
    pub fn cancel(&mut self, request: &Request) {
        self.reading.remove(&key(&request.machine, &request.session, &request.path));
    }

    /// An answer, or the failure to get one. What a refresh did to the buffer comes back; an open's result is in the info.
    /// An answer for a file that was closed meanwhile is dropped.
    pub fn finish(&mut self, request: &Request, answer: Result<(u16, String), String>) -> Option<DiskUpdate> {
        let (m, s, p) = (request.machine.as_str(), request.session.as_str(), request.path.as_str());
        let k = key(m, s, p);
        self.reading.remove(&k);
        let read = answer.and_then(|(status, body)| {
            if (200..300).contains(&status) {
                Ok(DiskFile::parse(&body))
            } else {
                Err(format!("Failed to read file: {}", super::revert::reason_of(status)))
            }
        });
        self.buffers.get(&k)?;
        match request.kind {
            Kind::Open => {
                let patch = match read {
                    Ok(file) => {
                        let reason = self.buffers.get_mut(&k)?.load(&file);
                        match reason {
                            Some(reason) => {
                                Patch { status: Some(Status::Unavailable), message: Some(Some(reason.into())), ..Patch::default() }
                            }
                            None => Patch {
                                status: Some(Status::Ready),
                                message: Some(None),
                                dirty: Some(false),
                                conflict: Some(None),
                                ..Patch::default()
                            },
                        }
                    }
                    Err(why) => {
                        let message = if why.contains("404") { GONE_MESSAGE } else { UNREADABLE_MESSAGE };
                        Patch { status: Some(Status::Error), message: Some(Some(message.into())), ..Patch::default() }
                    }
                };
                self.patch(m, s, p, patch);
                None
            }
            Kind::Refresh { unavailable } => {
                let file = read.ok()?;
                if self.info(m, s, p)?.saving {
                    return None;
                }
                if unavailable {
                    let reason = self.buffers.get_mut(&k)?.load(&file);
                    return Some(match reason {
                        Some(reason) => {
                            self.patch(m, s, p, Patch { message: Some(Some(reason.into())), ..Patch::default() });
                            DiskUpdate::Unavailable(reason.into())
                        }
                        None => {
                            let ready = Patch { status: Some(Status::Ready), message: Some(None), dirty: Some(false), ..Patch::default() };
                            self.patch(m, s, p, ready);
                            DiskUpdate::Updated(None)
                        }
                    });
                }
                let result = self.buffers.get_mut(&k)?.apply_disk_file(&file);
                match &result {
                    DiskUpdate::Updated(_) => {
                        self.patch(m, s, p, Patch { dirty: Some(false), conflict: Some(None), ..Patch::default() });
                    }
                    DiskUpdate::Conflict(conflict) => {
                        if self.info(m, s, p)?.conflict.as_ref().map(|c| &c.hash) != Some(&conflict.hash) {
                            self.patch(m, s, p, Patch { conflict: Some(Some(conflict.clone())), ..Patch::default() });
                        }
                    }
                    DiskUpdate::Unavailable(message) => {
                        let gone = Patch {
                            status: Some(Status::Unavailable),
                            message: Some(Some(message.clone())),
                            dirty: Some(false),
                            conflict: Some(None),
                            ..Patch::default()
                        };
                        self.patch(m, s, p, gone);
                    }
                    DiskUpdate::Unchanged => {}
                }
                Some(result)
            }
        }
    }
}

impl State {
    /// Opens a file's buffer, reading it the first time; asking again reads nothing more. Nothing with no server.
    pub fn open_file_buffer(&mut self, session: &str, path: &str) {
        let machine = self.machine_key_of(session).to_string();
        let request = self.file_buffers.open(&machine, session, path);
        self.send_file_read(request);
    }

    /// Reads an open file again (the agent changed files).
    pub fn refresh_file_buffer(&mut self, session: &str, path: &str) {
        let machine = self.machine_key_of(session).to_string();
        let request = self.file_buffers.refresh(&machine, session, path);
        self.send_file_read(request);
    }

    /// The buffer leaves with its tab.
    pub fn close_file_buffer(&mut self, session: &str, path: &str) {
        let machine = self.machine_key_of(session).to_string();
        self.file_buffers.remove(&machine, session, path);
        self.session_images.forget(&machine, session, path);
        self.file_live.forget(&machine, session, path);
    }

    fn send_file_read(&mut self, request: Option<Request>) {
        let Some(request) = request else { return };
        let Some(link) = self.link_for(&request.session) else {
            // No server: nothing will answer, so the read isn't on its way and the buffer stays loading.
            self.file_buffers.cancel(&request);
            return;
        };
        let (session, path) = (request.session.clone(), request.path.clone());
        link.off_thread(
            move |base| http::file_content(base, &session, &path),
            move |s, _, answer| {
                let update = s.file_buffers.finish(&request, answer);
                s.after_file_read(&request, update);
            },
        );
    }
}
