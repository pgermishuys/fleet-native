//! A changed file's contents at what the session's changes are compared with (`composables/use-diff-base.ts`,
//! `GET /api/sessions/{id}/diffs/file?path=` → `before`): the "git base" the changed-lines stripe and Diff read. `Bases` is pure:
//! it says which request to make and takes the answer; the thread is in the `State` half at the bottom. It shares the request
//! with Changes (`http::diff_file`) but keeps its own answers, kept by machine, session, what the changes are compared with and
//! path, so a rebase onto a newer main reads each file again.
use super::canvases::is_image_path;
use super::diffs::Base;
use super::http;
use crate::State;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};

/// Open files are few; keep the bases of the most recent ones.
pub const MAX_CACHED: usize = 50;

type Key = (String, String, String, String);

fn key(machine: &str, session: &str, base: &str, path: &str) -> Key {
    (machine.into(), session.into(), base.into(), path.into())
}

/// A read to make: its answer goes to `finish`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub machine: String,
    pub session: String,
    /// `Base::key` of what the changes were compared with when asked.
    pub base: String,
    pub path: String,
}

/// What asking for a base found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// Here already, or on its way (a second ask waits for the first).
    Known,
    Ask(Request),
}

#[derive(Debug, Default)]
pub struct Bases {
    /// `None` while the read is on its way, then the contents: "" for a file the session added.
    items: HashMap<Key, Option<String>>,
    /// Oldest first; a use moves a key to the end.
    order: VecDeque<Key>,
}

impl Bases {
    /// The base once it has loaded; `None` until then and when it can't be read.
    pub fn get(&self, machine: &str, session: &str, base: &str, path: &str) -> Option<&str> {
        self.items.get(&key(machine, session, base, path))?.as_deref()
    }

    /// Asks for a file's base: the read to make, or `Known` when it is kept or already being read (`fetchDiffBase`).
    pub fn fetch(&mut self, machine: &str, session: &str, base: &str, path: &str) -> Found {
        let k = key(machine, session, base, path);
        if self.items.contains_key(&k) {
            self.order.retain(|o| o != &k);
            self.order.push_back(k);
            return Found::Known;
        }
        self.items.insert(k.clone(), None);
        self.order.push_back(k);
        while self.order.len() > MAX_CACHED {
            if let Some(oldest) = self.order.pop_front() {
                self.items.remove(&oldest);
            }
        }
        Found::Ask(Request { machine: machine.into(), session: session.into(), base: base.into(), path: path.into() })
    }

    /// An answer, or the failure to get one. A failed read isn't kept, so the next ask reads again. Returns whether a base is
    /// now held for the request.
    pub fn finish(&mut self, request: &Request, answer: Result<(u16, Value), String>) -> bool {
        let k = key(&request.machine, &request.session, &request.base, &request.path);
        let before = match answer {
            Ok((status, body)) if (200..300).contains(&status) => {
                Some(body.get("before").and_then(Value::as_str).unwrap_or("").to_string())
            }
            _ => None,
        };
        // Evicted while it was being read: the answer is for nobody.
        let Some(slot) = self.items.get_mut(&k) else { return false };
        match before {
            Some(text) => {
                *slot = Some(text);
                true
            }
            None => {
                self.items.remove(&k);
                self.order.retain(|o| o != &k);
                false
            }
        }
    }

    /// Forgets every base.
    pub fn clear(&mut self) {
        self.items.clear();
        self.order.clear();
    }
}

impl State {
    /// A file may have a git base only while it is among the session's changes and isn't an image (`inDiff`).
    pub fn file_in_changes(&self, session: &str, path: &str) -> bool {
        self.diffs.session() == session && !is_image_path(path) && self.diffs.by_file(path).is_some()
    }

    /// Whether the file has changes: among the session's changes, or (slice 2) unsaved (`hasChanges`).
    pub fn file_has_changes(&self, session: &str, path: &str) -> bool {
        let machine = self.machine_key_of(session);
        self.file_in_changes(session, path) || self.file_buffers.is_dirty(machine, session, path)
    }

    /// Reads a changed file's base unless it is kept (`useDiffBase` watching its inputs): nothing while the file isn't among the
    /// changes, and nothing with no server. Moving what the changes are compared with asks again, under the new key.
    pub fn load_file_base(&mut self, session: &str, path: &str) {
        if !self.file_in_changes(session, path) {
            return;
        }
        let machine = self.machine_key_of(session).to_string();
        let Some(link) = self.link_for(session) else { return };
        let base = Base::key(self.diffs.base.as_ref());
        let Found::Ask(request) = self.file_bases.fetch(&machine, session, &base, path) else { return };
        let (id, file) = (session.to_string(), path.to_string());
        link.off_thread(
            move |url| http::diff_file(url, &id, &file),
            move |s, _, answer| {
                s.file_bases.finish(&request, answer);
            },
        );
    }

    /// The base of a file while it is among the session's changes; `None` otherwise, or until it has loaded.
    pub fn file_base(&self, session: &str, path: &str) -> Option<&str> {
        if !self.file_in_changes(session, path) {
            return None;
        }
        let base = Base::key(self.diffs.base.as_ref());
        self.file_bases.get(self.machine_key_of(session), session, &base, path)
    }
}
