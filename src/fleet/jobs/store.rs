//! The queue and each machine's seats (component 23, piece F; `stores/jobs.ts`). They belong to the home Fleet, which does
//! the matching, so every request goes there whichever machine is live. Nothing asks until something watches, which only the
//! Jobs page and Settings → Machines do, and only with Jobs on. While anything watches the queue is read at once and then
//! every 3 s (`JOBS_POLL_MS`); several watchers share one timer and the last one to stop stops it.
//!
//! Every request runs on a thread and is handed back on the UI thread. A read is numbered: an answer a newer read overtook
//! is dropped. A failed read keeps the last list. No view here: G and H read these fields.
use super::{Job, JobMachine, JobRepository, NewJob, parse_jobs, parse_machine, parse_repositories};
use crate::conv::export_history::encode_component;
use crate::fleet::http;
use crate::fleet::labs::Feature;
use crate::fleet::workflows::runs::error_from;
use crate::{Rail, State};
use rune::Tx;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::time::Duration;

/// How often the open screen asks again (`JOBS_POLL_MS`).
pub const POLL_SECS: f64 = 3.0;

/// A watcher of the queue; give it back to `Jobs::unwatch` to stop watching. Stopping twice is harmless.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Watch(u64);

/// An action on its way, or the words of why its last try failed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Op {
    pub busy: bool,
    pub error: Option<String>,
}

/// New job's repository list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Repositories {
    pub list: Option<Vec<JobRepository>>,
    pub loading: bool,
    pub error: Option<String>,
}

#[derive(Debug, Default)]
pub struct Jobs {
    pub jobs: Vec<Job>,
    pub machines: Vec<JobMachine>,
    /// A read has answered (the first good answer; a refusal never sets it).
    pub is_loaded: bool,
    pub error: Option<String>,
    pub post: Op,
    /// Cancel, and the job it is for.
    pub cancel: Op,
    pub cancelling: Option<String>,
    /// Save of a machine's seats or labels, and the machine it is for.
    pub save: Op,
    pub saving: Option<String>,
    pub repositories: Repositories,
    /// New job's dialog, while it is open (piece H).
    pub new: Option<super::new_job::NewJobForm>,
    /// Each job machine's seat and label fields on its card in Settings → Machines, by machine id.
    pub seat_fields: std::collections::HashMap<String, super::new_job::SeatFields>,
    /// The seat field last seen focused: leaving it saves it.
    pub seat_focus: Option<(String, super::new_job::SeatField)>,
    /// Settings → Machines has been shown since the last time it wasn't (the queue is read once per visit).
    pub seats_visit: bool,
    /// Counts the reads of the queue sent, for a test.
    pub asked: u32,
    watchers: HashSet<u64>,
    next_watch: u64,
    /// The Jobs page's own watch, held while it is shown.
    page: Option<Watch>,
    due: Option<f64>,
    read: u64,
}

impl Jobs {
    /// `watch`: the queue is read at the next look and every 3 s until the last watcher stops.
    pub fn watch(&mut self) -> Watch {
        self.next_watch += 1;
        self.watchers.insert(self.next_watch);
        self.due = Some(0.0);
        Watch(self.next_watch)
    }

    pub fn unwatch(&mut self, watch: Watch) {
        self.watchers.remove(&watch.0);
        if self.watchers.is_empty() {
            self.due = None;
        }
    }

    pub fn watching(&self) -> bool {
        !self.watchers.is_empty()
    }

    /// A read's answer (`load`): the lists replace the held ones; a refusal says what the server said and keeps them.
    fn took(&mut self, read: u64, got: Result<(Vec<Job>, Vec<JobMachine>), String>) {
        if read != self.read {
            return;
        }
        match got {
            Ok((jobs, machines)) => {
                self.jobs = jobs;
                self.machines = machines;
                self.error = None;
                self.is_loaded = true;
            }
            Err(words) => self.error = Some(words),
        }
    }

    fn next_read(&mut self) -> u64 {
        self.read += 1;
        self.asked += 1;
        self.read
    }
}

/// `GET /api/jobs`: the jobs and the machines' seats. A refusal says what the server said, else "Couldn't load the jobs.";
/// no answer (or one that isn't the lists) is "Couldn't reach Fleet.".
fn load(base: &str) -> Result<(Vec<Job>, Vec<JobMachine>), String> {
    let (status, body) = http::text_with("GET", base, "/api/jobs", None).map_err(|_| "Couldn't reach Fleet.".to_string())?;
    if !(200..300).contains(&status) {
        return Err(error_from(&body, "Couldn't load the jobs."));
    }
    serde_json::from_str::<Value>(&body).ok().and_then(|v| parse_jobs(&v)).ok_or_else(|| "Couldn't reach Fleet.".to_string())
}

/// `send`: a request whose refusal is the server's words, else `fallback`. A request that got no answer is `fallback` too.
fn send(base: &str, method: &str, path: &str, body: Option<&Value>, fallback: &str) -> Result<String, String> {
    let (status, text) = http::text_with(method, base, path, body).map_err(|_| fallback.to_string())?;
    if (200..300).contains(&status) { Ok(text) } else { Err(error_from(&text, fallback)) }
}

impl State {
    /// The home Fleet's address: where every jobs request goes. None with no server (scenes, tests with no fake).
    pub(super) fn jobs_base(&self) -> Option<String> {
        self.link.as_ref().map(|l| l.base.clone())
    }

    /// The Jobs page is shown: its view of the rail, with Jobs on.
    fn jobs_page_shown(&self) -> bool {
        self.rail == Rail::Jobs && self.labs_feature(Feature::Jobs) == Some(true)
    }

    /// Watches while the Jobs page is shown, reads when the timer falls due. True when something changed.
    pub(crate) fn keep_jobs(&mut self, now: f64, tx: &mut Tx) -> bool {
        self.keep_jobs_queue(now, tx) | self.keep_job_seats(tx)
    }

    fn keep_jobs_queue(&mut self, now: f64, tx: &mut Tx) -> bool {
        let shown = self.jobs_page_shown();
        match (shown, self.jobs.page) {
            (true, None) => self.jobs.page = Some(self.jobs.watch()),
            (false, Some(watch)) => {
                self.jobs.page = None;
                self.jobs.unwatch(watch);
            }
            _ => {}
        }
        if !self.jobs.watching() {
            return false;
        }
        let Some(base) = self.jobs_base() else { return false };
        tx.wake_after(Duration::from_secs_f64(self.jobs.due.map_or(POLL_SECS, |d| (d - now).clamp(0.02, POLL_SECS))));
        if self.jobs.due.is_some_and(|due| now < due) {
            return false;
        }
        self.jobs.due = Some(now + POLL_SECS);
        self.jobs_load(&base);
        true
    }

    /// `load`, off the UI thread.
    pub(super) fn jobs_load(&mut self, base: &str) {
        let (Some(post), read) = (self.link.as_ref().map(|l| l.post()), self.jobs.next_read()) else { return };
        let base = base.to_string();
        std::thread::spawn(move || {
            let got = load(&base);
            post.update(move |s, _| s.jobs.took(read, got));
        });
    }

    /// `post`: sends the job, then reads the queue again. A refusal says what the server said, else "Couldn't post the job.".
    pub fn jobs_post(&mut self, job: &NewJob) {
        let (Some(base), Some(post)) = (self.jobs_base(), self.link.as_ref().map(|l| l.post())) else { return };
        self.jobs.post = Op { busy: true, error: None };
        let (body, read) = (job.body(), self.jobs.next_read());
        std::thread::spawn(move || {
            let sent = send(&base, "POST", "/api/jobs", Some(&body), "Couldn't post the job.");
            let loaded = sent.is_ok().then(|| load(&base));
            post.update(move |s, _| {
                s.jobs.post = Op { busy: false, error: sent.err() };
                if let Some(got) = loaded {
                    s.jobs.took(read, got);
                }
                s.new_job_posted();
            });
        });
    }

    /// `cancel`, for a waiting job only (one that started is handled in its session): `DELETE /api/jobs/{id}`, then the
    /// queue again. A refusal is "Couldn't cancel the job." unless the server said more.
    pub fn jobs_cancel(&mut self, id: &str) {
        let (Some(base), Some(post)) = (self.jobs_base(), self.link.as_ref().map(|l| l.post())) else { return };
        self.jobs.cancel = Op { busy: true, error: None };
        self.jobs.cancelling = Some(id.to_string());
        let (path, read) = (format!("/api/jobs/{}", encode_component(id)), self.jobs.next_read());
        std::thread::spawn(move || {
            let sent = send(&base, "DELETE", &path, None, "Couldn't cancel the job.");
            let loaded = sent.is_ok().then(|| load(&base));
            post.update(move |s, _| {
                s.jobs.cancel = Op { busy: false, error: sent.as_ref().err().cloned() };
                // A good Cancel goes back to the bare `/jobs`.
                if sent.is_ok() {
                    s.labs.job = None;
                }
                if let Some(got) = loaded {
                    s.jobs.took(read, got);
                }
            });
        });
    }

    /// `saveMachine`: how many jobs the machine runs at once, and its labels; a field left `None` stays as it is. The
    /// answer replaces that machine in the list. A refusal is "Couldn't save the seats." unless the server said more.
    pub fn jobs_save_machine(&mut self, id: &str, seats: Option<u32>, labels: Option<Vec<String>>) {
        let (Some(base), Some(post)) = (self.jobs_base(), self.link.as_ref().map(|l| l.post())) else { return };
        self.jobs.save = Op { busy: true, error: None };
        self.jobs.saving = Some(id.to_string());
        let mut body = serde_json::Map::new();
        if let Some(seats) = seats {
            body.insert("seats".into(), json!(seats));
        }
        if let Some(labels) = labels {
            body.insert("labels".into(), json!(labels));
        }
        let (path, body) = (format!("/api/jobs/machines/{}", encode_component(id)), Value::Object(body));
        std::thread::spawn(move || {
            let saved = send(&base, "PUT", &path, Some(&body), "Couldn't save the seats.").and_then(|text| {
                serde_json::from_str::<Value>(&text)
                    .ok()
                    .as_ref()
                    .and_then(parse_machine)
                    .ok_or_else(|| "Couldn't save the seats.".to_string())
            });
            post.update(move |s, _| {
                s.jobs.save = Op { busy: false, error: saved.as_ref().err().cloned() };
                if let Ok(machine) = saved {
                    for held in s.jobs.machines.iter_mut().filter(|m| m.id == machine.id) {
                        *held = machine.clone();
                    }
                }
            });
        });
    }

    /// `loadRepositories`: what New job can pick. A refusal is "Couldn't list the repositories." unless the server said more.
    pub fn jobs_load_repositories(&mut self) {
        let (Some(base), Some(post)) = (self.jobs_base(), self.link.as_ref().map(|l| l.post())) else { return };
        self.jobs.repositories.loading = true;
        self.jobs.repositories.error = None;
        std::thread::spawn(move || {
            let got = send(&base, "GET", "/api/jobs/repositories", None, "Couldn't list the repositories.").and_then(|text| {
                serde_json::from_str::<Value>(&text)
                    .ok()
                    .as_ref()
                    .and_then(parse_repositories)
                    .ok_or_else(|| "Couldn't list the repositories.".to_string())
            });
            post.update(move |s, _| {
                s.jobs.repositories.loading = false;
                match got {
                    Ok(list) => s.jobs.repositories.list = Some(list),
                    Err(words) => s.jobs.repositories.error = Some(words),
                }
                s.new_job_repositories_arrived();
            });
        });
    }
}
