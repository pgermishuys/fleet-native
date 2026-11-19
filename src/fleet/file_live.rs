//! Keeping open files in step with the disk (`composables/use-file-live-updates.ts`, `refreshBuffer`'s caller). An open file is read
//! again when the agent changes it (`files.changed`), when the agent stops (`turn.ended` or `session.idled`: shell edits send no
//! file event), when its tab comes to the front and when the window gets focus back. Events wait 500 ms for a burst to settle and
//! read each file once; the tab and focus cases read at once. `Live` is pure (the queue and the flash); the events, the timer
//! thread and the tab pulse are in the `State` half at the bottom.
use super::events::{Event, Kind};
use super::file_buffers::{DiskUpdate, Kind as Read, Request};
use super::state::Topic;
use crate::State;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

/// The agent edits a file several times in one turn; wait for the burst to settle, as Changes does.
pub const FILE_EVENT_DEBOUNCE: Duration = Duration::from_millis(500);

/// How long the lines the agent just changed flash (`cm-agent-flash`, 1.6 s ease-out from `--accent` 28 % to nothing).
pub const FLASH_SECONDS: f64 = 1.6;

/// Whether a path from a `files.changed` event is an open file. The agent's edits arrive as absolute paths (OpenCode sends them
/// that way), saves as session-relative ones; backslashes count as slashes (`isSameFile`).
pub fn is_same_file(event_path: &str, open_path: &str) -> bool {
    let event = event_path.replace('\\', "/");
    event == open_path || event.ends_with(&format!("/{open_path}"))
}

/// What queueing paths asks of the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Queued {
    Nothing,
    /// Read these now.
    Now(Vec<String>),
    /// Wait the delay, then call `fire` with this generation: a later queue makes it stale (the burst isn't over).
    Later(u64),
}

/// Lines the agent just changed, from when (`flashLines`).
#[derive(Clone, Debug, PartialEq)]
pub struct Flash {
    /// 1-based lines of the new text, first to last.
    pub lines: (usize, usize),
    /// 0 when it starts, 1 when it has faded to nothing; the view eases it (the flash is `--accent` 28 % × (1 − eased)).
    pub progress: f64,
}

type Key = (String, String, String);

#[derive(Debug, Default)]
pub struct Live {
    /// The session the pending paths are in; another session starts the queue afresh.
    session: String,
    pending: BTreeSet<String>,
    generation: u64,
    flashes: HashMap<Key, ((usize, usize), f64)>,
    /// The file tab in front, so a tab coming forward is seen once.
    front: Option<(String, String)>,
}

impl Live {
    /// Adds paths to the wait and restarts it (`queue`). A delay of nothing reads everything pending now.
    pub fn queue(&mut self, session: &str, paths: Vec<String>, delay: Duration) -> Queued {
        if self.session != session {
            self.session = session.to_string();
            self.pending.clear();
        }
        if paths.is_empty() {
            return Queued::Nothing;
        }
        self.pending.extend(paths);
        self.generation += 1;
        if delay.is_zero() {
            return Queued::Now(std::mem::take(&mut self.pending).into_iter().collect());
        }
        Queued::Later(self.generation)
    }

    /// The wait ended: what to read, or nothing when a later queue replaced it (`flush`).
    pub fn fire(&mut self, session: &str, generation: u64) -> Vec<String> {
        if self.session != session || self.generation != generation {
            return Vec::new();
        }
        std::mem::take(&mut self.pending).into_iter().collect()
    }

    /// The topic went away (the session changed): what waited is dropped.
    pub fn clear(&mut self) {
        self.pending.clear();
        self.generation += 1;
    }

    /// A clean buffer took the agent's change: its lines flash from `now`.
    pub fn flash(&mut self, machine: &str, session: &str, path: &str, lines: (usize, usize), now: f64) {
        self.flashes.insert((machine.into(), session.into(), path.into()), (lines, now));
    }

    /// The lines flashing now, and how far along, or `None` once it is over.
    pub fn flashing(&self, machine: &str, session: &str, path: &str, now: f64) -> Option<Flash> {
        let (lines, from) = self.flashes.get(&(machine.into(), session.into(), path.into()))?;
        let elapsed = now - from;
        (0.0..FLASH_SECONDS).contains(&elapsed).then(|| Flash { lines: *lines, progress: elapsed / FLASH_SECONDS })
    }

    pub fn forget(&mut self, machine: &str, session: &str, path: &str) {
        self.flashes.remove(&(machine.into(), session.into(), path.into()));
    }

    /// Whether `path` just came to the front of `session`'s tabs: seen once per coming forward.
    pub fn came_forward(&mut self, session: &str, path: Option<&str>) -> bool {
        let now = path.map(|p| (session.to_string(), p.to_string()));
        let forward = now.is_some() && now != self.front;
        self.front = now;
        forward
    }
}

/// The paths a `files.changed` event names (`payload.files[].path`).
fn event_paths(payload: &Value) -> Vec<String> {
    let files = payload.get("files").and_then(Value::as_array);
    files.into_iter().flatten().filter_map(|f| f.get("path").and_then(Value::as_str)).map(str::to_string).collect()
}

impl State {
    /// `files.changed`, `turn.ended` and `session.idled` on the open session's own topic. Relayed turn events carry the harness's
    /// own session id, not Fleet's, so only the topic is checked.
    pub fn on_file_live_event(&mut self, topic: &Topic, event: &Event) {
        let Topic::Session(session) = topic else { return };
        if self.open_id().as_deref() != Some(session) {
            return;
        }
        let open = self.open_file_paths(session);
        let paths = match event {
            Event::Other(Kind::FilesChanged, payload) => {
                let changed = event_paths(payload);
                open.into_iter().filter(|p| changed.iter().any(|c| is_same_file(c, p))).collect()
            }
            // The agent stopped: pooled OpenCode says so with session.idled, not turn.ended.
            Event::TurnEnded | Event::SessionIdled => open,
            _ => return,
        };
        self.queue_file_reads(session, paths, FILE_EVENT_DEBOUNCE);
    }

    /// The window has the keyboard again: every open file of the open session is checked at once.
    pub fn file_window_focus(&mut self) {
        let Some(session) = self.open_id() else { return };
        let paths = self.open_file_paths(&session);
        self.queue_file_reads(&session, paths, Duration::ZERO);
    }

    /// A file tab that has just come to the front is checked at once (`watch` of the active tab's path). Called every frame; only a
    /// change does anything.
    pub fn keep_file_front(&mut self) {
        let Some(session) = self.open_id() else {
            self.file_live.came_forward("", None);
            return;
        };
        let active = self.canvases.session_active_id(&session);
        let path = active.as_deref().and_then(|id| id.strip_prefix("file:"));
        if self.file_live.came_forward(&session, path)
            && let Some(path) = path
            && self.open_file_paths(&session).iter().any(|p| p == path)
        {
            self.queue_file_reads(&session, vec![path.to_string()], Duration::ZERO);
        }
    }

    /// Keeps frames coming while the open file's changed lines flash (`cm-agent-flash`); true while one does.
    pub fn keep_file_flash(&mut self, tx: &mut rune::Tx) -> bool {
        let Some(session) = self.open_id() else { return false };
        let active = self.canvases.session_active_id(&session);
        let Some(path) = active.as_deref().and_then(|id| id.strip_prefix("file:")) else { return false };
        let machine = self.machine_key_of(&session).to_string();
        let on = self.file_live.flashing(&machine, &session, path, self.clock).is_some();
        if on {
            tx.wake_after(Duration::from_millis(16));
        }
        on
    }

    fn open_file_paths(&self, session: &str) -> Vec<String> {
        self.file_buffers.open_paths(self.machine_key_of(session), session)
    }

    fn queue_file_reads(&mut self, session: &str, paths: Vec<String>, delay: Duration) {
        match self.file_live.queue(session, paths, delay) {
            Queued::Nothing => {}
            Queued::Now(paths) => self.read_files_again(session, paths),
            Queued::Later(generation) => {
                let Some(link) = self.link_for(session) else { return };
                let id = session.to_string();
                link.off_thread(
                    move |_| std::thread::sleep(delay),
                    move |s, _, ()| {
                        let paths = s.file_live.fire(&id, generation);
                        s.read_files_again(&id, paths);
                    },
                );
            }
        }
    }

    fn read_files_again(&mut self, session: &str, paths: Vec<String>) {
        for path in paths {
            self.refresh_file_buffer(session, &path);
        }
    }

    /// What a read of an open file brought: the tab pulses when the file changed under it, and the lines the agent changed flash
    /// (`refreshBuffer`'s caller marks the tab on `updated` and `conflict`).
    pub(crate) fn after_file_read(&mut self, request: &Request, update: Option<DiskUpdate>) {
        if !matches!(request.kind, Read::Refresh { .. }) {
            return;
        }
        match update {
            Some(DiskUpdate::Updated(lines)) => {
                if let Some(lines) = lines {
                    self.file_live.flash(&request.machine, &request.session, &request.path, lines, self.clock);
                }
            }
            Some(DiskUpdate::Conflict(_)) => {}
            _ => return,
        }
        self.canvases.mark_updated(&super::canvases::file_id(&request.path), self.clock);
    }
}
