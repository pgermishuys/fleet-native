//! The calls that fill `machines::Machines`, each on a thread of its own (`stores/machines.ts`: `syncFromServer`,
//! `loadHome`, `refreshMachine`, `startPolling`). Never on the UI thread: the answers are handed to the state with
//! `Post::update`.
//!
//! One thread reads home's list and home's own identity at start and then every 15 s while the window is on screen
//! (`refresh::Refresh`, the timer the session list uses). Each listed machine is read on a thread of its own, one at a
//! time per machine, so a machine that is slow never holds the others.
use super::http;
use super::link::{Post, now_ms};
use super::machines::{Answer, Info, ReadJob, parse_machine_list, read_machine};
use super::refresh::Refresh;
use crate::State;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

pub enum Cmd {
    /// Read everything again now: a machine was added, forgotten, renamed or given a new token, or the window is back.
    Now,
    /// The window went off screen or came back.
    Visible(bool),
}

/// Starts the poll for the Fleet at `base`. Returns the way to talk to it; the thread ends when that is dropped.
pub fn start(base: String, post: Post) -> Sender<Cmd> {
    let (tx, rx) = channel();
    std::thread::spawn(move || run(&base, &rx, &post));
    tx
}

fn run(base: &str, rx: &Receiver<Cmd>, post: &Post) {
    let mut timer = Refresh::new(Instant::now());
    loop {
        read_home(base, post);
        loop {
            // A second's granularity is plenty for a 15 s poll.
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(Cmd::Now) => break,
                Ok(Cmd::Visible(visible)) => timer.set_visible(visible, Instant::now()),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            if timer.due(Instant::now()) {
                break;
            }
        }
    }
}

/// `GET /api/machines` and `GET /api/machine` on home, then a read of every machine listed. When home can't be reached at
/// all the last list stays: the machines were not forgotten, home is only away.
fn read_home(base: &str, post: &Post) {
    let Ok((status, body)) = http::get_page(base, "/api/machines") else { return };
    let listed = parse_machine_list(status, &body);
    // An older home has no identity; it stays "This machine".
    let home = http::get(base, "/api/machine").ok().as_ref().and_then(Info::from_json);
    let again = post.clone();
    post.update(move |s, _| {
        s.machines.apply_list(listed);
        if let Some(home) = home {
            s.machines.home = Some(home);
        }
        if !std::mem::replace(&mut s.machine_schemes_told, true) && s.machines.has_machines() {
            eprintln!("fleet-native: machines reached over {}", s.machines.address_schemes().join(", "));
        }
        s.save_machines();
        for job in s.machines.begin_reads() {
            spawn_read(job, again.clone());
        }
    });
}

/// `readMachine`: the machine's sessions, projects and identity in one go, each call with the machine's own token.
pub(crate) fn read_one(job: &ReadJob) -> Result<super::machines::Read, String> {
    let get = |path: &str| -> Answer { http::get_page(&job.base, path) };
    let (sessions, projects, identity) = std::thread::scope(|scope| {
        let projects = scope.spawn(|| get("/api/projects"));
        let identity = scope.spawn(|| get("/api/machine"));
        let sessions = get(&format!("/api/sessions?limit={}&offset=0", super::store::PAGE_SIZE));
        let nothing = || Err("no answer".to_string());
        (sessions, projects.join().unwrap_or_else(|_| nothing()), identity.join().unwrap_or_else(|_| nothing()))
    });
    read_machine(&job.name, sessions, projects, identity, now_ms())
}

pub fn spawn_read(job: ReadJob, post: Post) {
    std::thread::spawn(move || {
        let result = read_one(&job);
        post.update(move |s, _| {
            // One line when something changes (first list, back, away, another reason), not one per poll.
            match result {
                Ok(read) => {
                    let first = s.machines.others.get(&job.key).is_none_or(|o| o.loaded_at.is_none());
                    let count = read.sessions.len();
                    let back = s.machines.apply_read(&job.key, read, now_ms());
                    if first || back {
                        eprintln!("fleet-native: machine {} listed {count} session(s)", job.name);
                    }
                    s.save_machines();
                }
                Err(why) => {
                    if s.machines.others.get(&job.key).and_then(|o| o.error.as_ref()) != Some(&why) {
                        eprintln!("fleet-native: machine {}: {why}", job.name);
                    }
                    s.machines.mark_unreachable(&job.key, why);
                }
            }
        });
    });
}

impl State {
    /// Writes the machines' last lists to the app's data folder, off the UI thread. Nothing when there is no folder.
    pub fn save_machines(&self) {
        if let Some(dir) = &self.machine_dir {
            super::machine_cache::spawn_save(dir.clone(), super::machine_cache::to_json(&self.machines));
        }
    }

    /// The cache file was read (off the UI thread): what a machine last listed shows before its first read comes back.
    pub fn adopt_machine_cache(&mut self, dir: std::path::PathBuf, cached: super::machine_cache::Cached) {
        self.machine_dir = Some(dir);
        self.machines.adopt_cache(cached);
    }
}
