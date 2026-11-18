//! One file's contents in Changes (`composables/use-file-diff-contents.ts`, `GET /api/sessions/{id}/diffs/file?path=`): the
//! before and after of each file, fetched one file at a time as its section opens. `Contents` is pure: it says which requests to
//! make and takes their answers; the threads are in the `State` half at the bottom. Git is asked about that file only
//! (weave-fleet #942), so a long list costs a request per file, never one for the whole diff.
use super::diffs::{FileDiff, FileStatus};
use super::http;
use crate::State;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};

/// The newest answers kept, by machine, session and path.
pub const MAX_CACHED: usize = 200;

/// File requests asked of one machine at a time. A long list (find loads every file) fired at once would hold up what else the
/// app asks (switching session, sending a message).
pub const MAX_IN_FLIGHT: usize = 4;

/// `FileDiffEntry`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Loading,
    Ready(FileDiff),
    /// `HTTP <status>` or the failure's own words.
    Error(String),
}

/// The version a file's contents are asked for: its status and counts. They move when the agent edits the file again.
pub fn version_of(file: &FileDiff) -> String {
    let status = match file.status {
        FileStatus::Added => "added",
        FileStatus::Deleted => "deleted",
        FileStatus::Modified => "modified",
    };
    format!("{status}:{}:{}", file.additions, file.deletions)
}

/// A request to make: its answer goes to `finish`, which drops it when the file has moved on to a newer version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub machine: String,
    pub session: String,
    pub path: String,
    version: String,
}

/// The last `MAX_CACHED` answers, oldest out first.
#[derive(Debug, Default)]
struct Cache {
    items: HashMap<(String, String, String), FileDiff>,
    order: VecDeque<(String, String, String)>,
}

impl Cache {
    fn key(machine: &str, session: &str, path: &str) -> (String, String, String) {
        (machine.to_string(), session.to_string(), path.to_string())
    }

    fn get(&self, machine: &str, session: &str, path: &str) -> Option<&FileDiff> {
        self.items.get(&Self::key(machine, session, path))
    }

    /// A newer answer for a key moves it to the newest end.
    fn remember(&mut self, machine: &str, session: &str, path: &str, item: FileDiff) {
        let key = Self::key(machine, session, path);
        self.order.retain(|k| k != &key);
        self.order.push_back(key.clone());
        self.items.insert(key, item);
        while self.order.len() > MAX_CACHED {
            if let Some(oldest) = self.order.pop_front() {
                self.items.remove(&oldest);
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct Contents {
    session: String,
    /// The open session's files. Another session starts from nothing, so one session's file never shows in another.
    entries: HashMap<String, Entry>,
    versions: HashMap<String, String>,
    reloads: u64,
    cache: Cache,
    in_flight: HashMap<String, usize>,
    queued: HashMap<String, VecDeque<Request>>,
}

impl Contents {
    pub fn entry(&self, path: &str) -> Option<&Entry> {
        self.entries.get(path)
    }

    /// Holds an answer as if it had come (scenes and tests draw states no server was asked for).
    pub fn put(&mut self, session: &str, path: &str, entry: Entry) {
        self.open(session);
        self.entries.insert(path.to_string(), entry);
    }

    /// `put` as an answer to this version of the file: `load` leaves it alone, as it does what a request brought.
    pub fn put_at(&mut self, session: &str, path: &str, version: &str, entry: Entry) {
        self.put(session, path, entry);
        self.versions.insert(path.to_string(), version.to_string());
    }

    /// How many requests are on their way to a machine.
    pub fn in_flight(&self, machine: &str) -> usize {
        self.in_flight.get(machine).copied().unwrap_or(0)
    }

    /// How many wait for a free place at a machine.
    pub fn queued(&self, machine: &str) -> usize {
        self.queued.get(machine).map_or(0, VecDeque::len)
    }

    /// Another session: its files start empty. What was asked for the old one is dropped when it answers (its slot is freed).
    fn open(&mut self, session: &str) {
        if self.session != session {
            self.session = session.to_string();
            self.entries.clear();
            self.versions.clear();
        }
    }

    /// `load`: does nothing when this version of the file is already there.
    pub fn load(&mut self, machine: &str, session: &str, path: &str, version: &str) -> Option<Request> {
        self.open(session);
        if self.versions.get(path).map(String::as_str) == Some(version) && self.entries.contains_key(path) {
            return None;
        }
        self.fetch(machine, session, path, version.to_string())
    }

    /// `reload`: always asks again.
    pub fn reload(&mut self, machine: &str, session: &str, path: &str, version: &str) -> Option<Request> {
        self.open(session);
        self.reloads += 1;
        let version = format!("{version}#{}", self.reloads);
        self.fetch(machine, session, path, version)
    }

    fn fetch(&mut self, machine: &str, session: &str, path: &str, version: String) -> Option<Request> {
        self.versions.insert(path.to_string(), version.clone());
        // What is drawn stays drawn while it is asked again; a file not yet drawn shows what was cached, or loading.
        if !matches!(self.entries.get(path), Some(Entry::Ready(_))) {
            let shown = match self.cache.get(machine, session, path) {
                Some(item) => Entry::Ready(item.clone()),
                None => Entry::Loading,
            };
            self.entries.insert(path.to_string(), shown);
        }
        let request = Request { machine: machine.to_string(), session: session.to_string(), path: path.to_string(), version };
        if self.in_flight(machine) >= MAX_IN_FLIGHT {
            self.queued.entry(machine.to_string()).or_default().push_back(request);
            return None;
        }
        *self.in_flight.entry(machine.to_string()).or_default() += 1;
        Some(request)
    }

    /// Replaced while it waited (the agent edited the file again, or another session opened): the newer request asks instead.
    fn is_current(&self, request: &Request) -> bool {
        request.session == self.session && self.versions.get(&request.path) == Some(&request.version)
    }

    /// An answer, or the failure to get one. Returns the request that takes the place it frees.
    pub fn finish(&mut self, request: &Request, answer: Result<(u16, Value), String>) -> Option<Request> {
        let next = self.release(&request.machine);
        if self.is_current(request) || request.session != self.session {
            match answer {
                Ok((status, body)) if (200..300).contains(&status) => {
                    let item = super::diffs::parse_file(&body, &request.path);
                    self.cache.remember(&request.machine, &request.session, &request.path, item.clone());
                    if self.is_current(request) {
                        self.entries.insert(request.path.clone(), Entry::Ready(item));
                    }
                }
                Ok((status, _)) if self.is_current(request) => {
                    self.entries.insert(request.path.clone(), Entry::Error(format!("HTTP {status}")));
                }
                Err(message) if self.is_current(request) => {
                    self.entries.insert(request.path.clone(), Entry::Error(message));
                }
                _ => {}
            }
        }
        next
    }

    /// A place freed at a machine goes to the next request still wanted; those replaced while they waited are skipped.
    fn release(&mut self, machine: &str) -> Option<Request> {
        while let Some(next) = self.queued.get_mut(machine).and_then(VecDeque::pop_front) {
            if self.is_current(&next) {
                return Some(next);
            }
        }
        if let Some(n) = self.in_flight.get_mut(machine) {
            *n = n.saturating_sub(1);
        }
        None
    }
}

impl State {
    /// A section opened (`load`): asks for the file's contents unless this version is already there. Nothing with no server.
    pub fn load_diff_file(&mut self, session: &str, file: &FileDiff) {
        let machine = self.machine_key_of(session).to_string();
        if self.link_for(session).is_none() {
            return;
        }
        let request = self.diff_contents.load(&machine, session, &file.file, &version_of(file));
        self.send_diff_file(request);
    }

    /// Try again, or after a revert: asks even when this version is there.
    pub fn reload_diff_file(&mut self, session: &str, file: &FileDiff) {
        let machine = self.machine_key_of(session).to_string();
        if self.link_for(session).is_none() {
            return;
        }
        let request = self.diff_contents.reload(&machine, session, &file.file, &version_of(file));
        self.send_diff_file(request);
    }

    fn send_diff_file(&mut self, request: Option<Request>) {
        let Some(request) = request else { return };
        let Some(link) = self.link_for(&request.session) else { return };
        let (session, path) = (request.session.clone(), request.path.clone());
        link.off_thread(
            move |base| http::diff_file(base, &session, &path),
            move |s, _, answer| {
                let next = s.diff_contents.finish(&request, answer);
                s.send_diff_file(next);
            },
        );
    }
}
