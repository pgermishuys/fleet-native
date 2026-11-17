//! A harness's update strip (`HarnessUpdateStrip.vue`) as plain rules: when it shows, which words and buttons it has in each phase
//! of the job, and the requests it sends. It sits on Updates & about and in the setup rows; the Harnesses page doesn't draw it from
//! v0.72 on. `view::settings::harness_update` draws it.
use super::super::new_session::feeds::extract_error;
use super::super::new_session::harness_choice::HarnessEntry;
use super::harnesses::Status;
use crate::fleet::http;
use crate::{State, Tx};
use serde_json::Value;
use std::time::Duration;

/// How often a harness update that waits or runs is checked (`HARNESS_UPDATE_POLL_MS`).
pub const POLL_SECS: f64 = 2.0;
/// "Copied" shows this long.
pub const COPIED_SECS: f64 = 1.5;

/// `HarnessUpdateJob`: the update Fleet is running (or has just run) for one harness.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Job {
    /// `waiting | running | succeeded | failed`.
    pub phase: String,
    pub working_sessions: u32,
    pub message: Option<String>,
    pub from_version: Option<String>,
    pub output: Option<String>,
}

/// `HarnessUpdateInfo`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Update {
    pub available: bool,
    pub latest_version: Option<String>,
    /// The update command as you'd type it.
    pub command: Option<String>,
    /// Fleet downloads this harness itself and it isn't installed: it can be installed from here.
    pub can_install: bool,
    /// What Fleet's own install or update downloads, and where it goes.
    pub install_plan: Option<String>,
    pub job: Option<Job>,
}

impl Update {
    pub fn parse(h: &Value) -> Update {
        let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
        let Some(u) = h.get("update").filter(|u| u.is_object()) else { return Update::default() };
        let job = u.get("job").filter(|j| j.is_object()).map(|j| Job {
            phase: text(j, "phase").unwrap_or_default(),
            working_sessions: j.get("workingSessions").and_then(Value::as_u64).unwrap_or(0) as u32,
            message: text(j, "message"),
            from_version: text(j, "fromVersion"),
            output: text(j, "output"),
        });
        Update {
            available: u.get("updateAvailable").and_then(Value::as_bool) == Some(true),
            latest_version: text(u, "latestVersion"),
            command: text(u, "command"),
            can_install: u.get("canInstall").and_then(Value::as_bool) == Some(true),
            install_plan: text(u, "installPlan"),
            job,
        }
    }
}

/// The box's look (`harness-update--warn`, `--ok`, `--error`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Warn,
    Ok,
    Error,
}

/// A piece of the muted line under the lead: words, or a command in the mono face.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    Words(String),
    Code(String),
}

/// What the strip says and offers for one harness right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Strip {
    pub tone: Tone,
    /// The bold first words.
    pub lead: String,
    /// Plain words that follow the lead on its own line ("You have 1.18.20.").
    pub after_lead: Option<String>,
    /// The muted line under it.
    pub rest: Vec<Piece>,
    /// A spinner: the update waits or runs.
    pub busy: bool,
    pub cancel: bool,
    /// "Done" (succeeded) or "Dismiss" (failed).
    pub dismiss: Option<&'static str>,
    /// The primary button: "Install", "Update to {latest}", "Update" or "Try again".
    pub primary: Option<String>,
    /// After a failure: the command to run by hand, and the output.
    pub command: Option<String>,
    pub output: Option<String>,
}

impl Strip {
    /// A strip with nothing but its box: what an error alone shows (`v-if="job || offer || error"`).
    pub fn empty() -> Strip {
        Strip {
            tone: Tone::Plain,
            lead: String::new(),
            after_lead: None,
            rest: Vec::new(),
            busy: false,
            cancel: false,
            dismiss: None,
            primary: None,
            command: None,
            output: None,
        }
    }
}

fn words(w: impl Into<String>) -> Piece {
    Piece::Words(w.into())
}

/// The strip for a harness, or none: nothing is drawn unless a job exists or an install or update is offered (`v-if="job || offer
/// || error"`; the error is the strip's own state, not the harness's).
pub fn strip(h: &HarnessEntry) -> Option<Strip> {
    let u = &h.card.update;
    let name = &h.display_name;
    let too_old = Status::of(h, true) == Status::UpdateNeeded;
    let job = u.job.as_ref();
    let offer = job.is_none() && (u.can_install || too_old || u.available);
    if job.is_none() && !offer {
        return None;
    }
    let mut s = Strip { tone: if offer && too_old { Tone::Warn } else { Tone::Plain }, ..Strip::empty() };
    if let Some(job) = job {
        match job.phase.as_str() {
            "waiting" => {
                let n = job.working_sessions;
                s.lead = format!("Waiting for {n} working {} to finish.", if n == 1 { "session" } else { "sessions" });
                s.rest = vec![words("The update starts as soon as they're idle.")];
                s.busy = true;
                s.cancel = true;
            }
            "running" => {
                s.lead = job.message.clone().unwrap_or_else(|| format!("Updating {name}\u{2026}"));
                s.rest = vec![words("It stops after 5 minutes if it hasn't finished.")];
                s.busy = true;
            }
            phase => {
                s.lead = job.message.clone().unwrap_or_default();
                if phase == "succeeded" {
                    s.tone = Tone::Ok;
                    s.dismiss = Some("Done");
                    s.rest = vec![words(if job.from_version.is_some() {
                        format!("Idle {name} processes restarted. New sessions use the new version.")
                    } else {
                        format!("New sessions can use {name} now.")
                    })];
                } else if phase == "failed" {
                    s.tone = Tone::Error;
                    s.dismiss = Some("Dismiss");
                    s.command = u.command.clone();
                    s.output = job.output.clone();
                }
            }
        }
        if job.phase == "failed" {
            s.primary = Some("Try again".into());
        }
        return Some(s);
    }
    if offer && u.can_install {
        s.lead = format!("{name} isn't installed.");
        s.rest = vec![words(u.install_plan.clone().unwrap_or_default())];
    } else if too_old {
        s.lead = h.reason.clone().unwrap_or_default();
        s.rest = vec![words(match &u.latest_version {
            Some(latest) => format!("{latest} is the latest. New sessions can't use {name} until you update."),
            None => format!("New sessions can't use {name} until you update."),
        })];
    } else {
        s.lead = format!("{name} {} is available.", u.latest_version.clone().unwrap_or_default());
        s.after_lead = Some(format!("You have {}.", h.version.clone().unwrap_or_default()));
        if let Some(command) = &u.command {
            s.rest = vec![words("Fleet runs"), Piece::Code(command.clone()), words("once no session is working.")];
        } else if let Some(plan) = &u.install_plan {
            s.rest = vec![words(plan.clone())];
        }
    }
    if u.command.is_some() || u.install_plan.is_some() {
        s.primary = Some(if u.can_install {
            "Install".into()
        } else {
            match &u.latest_version {
                Some(latest) => format!("Update to {latest}"),
                None => "Update".into(),
            }
        });
    }
    Some(s)
}

/// Whether any harness's update waits or runs: the list is read again every [`POLL_SECS`] so what shows it follows the update.
pub fn updating(list: &[HarnessEntry]) -> bool {
    list.iter().any(|h| matches!(h.card.update.job.as_ref().map(|j| j.phase.as_str()), Some("waiting" | "running")))
}

/// What the strips keep: the request that is out, the error it came back with, the command just copied.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Strips {
    pub busy: bool,
    pub error: Option<(String, String)>,
    /// The command just copied and when "Copied" ends (on `clock`).
    pub copied: Option<(String, f64)>,
    /// The next 2-second read while an update waits or runs.
    pub poll_due: Option<f64>,
}

impl State {
    /// `send`: `POST` starts, `DELETE` cancels or dismisses; the harnesses are read again after, whatever came of it. `machine` is
    /// the one the strip's page shows.
    pub fn send_harness_update(&mut self, machine: &str, kind: &str, method: &'static str) {
        if self.settings.harnesses.strips.busy {
            return;
        }
        self.settings.harnesses.strips.busy = true;
        self.settings.harnesses.strips.error = None;
        let (path, key, from) = (format!("/api/harnesses/{}/update", http::escape(kind)), kind.to_string(), machine.to_string());
        let ok = self.feed_call(
            machine,
            move |base| http::text_with(method, base, &path, None),
            move |s, _, answer| {
                s.settings.harnesses.strips.busy = false;
                match answer {
                    Ok((status, _)) if (200..300).contains(&status) => {}
                    Ok((status, body)) => {
                        s.settings.harnesses.strips.error =
                            Some((key, extract_error(&body, &format!("The request failed (HTTP {status})."))))
                    }
                    Err(why) => s.settings.harnesses.strips.error = Some((key, why)),
                }
                s.read_harnesses(&from, true);
            },
        );
        if !ok {
            self.settings.harnesses.strips.busy = false;
        }
    }

    /// The copy button of a failed update: the command on the clipboard, then "Copied" for 1.5 s.
    pub fn copy_harness_command(&mut self, command: &str, tx: &mut Tx) {
        tx.copy(command.to_string());
        self.settings.harnesses.strips.copied = Some((command.to_string(), self.clock + COPIED_SECS));
        tx.wake_after(Duration::from_millis(100));
    }

    /// Whether this command's button says "Copied".
    pub fn harness_copied(&self, command: &str) -> bool {
        self.settings.harnesses.strips.copied.as_ref().is_some_and(|(c, _)| c == command)
    }

    /// The timers: "Copied" ends. True when something drawn changed.
    pub fn keep_harness_copied(&mut self, now: f64, tx: &mut Tx) -> bool {
        match self.settings.harnesses.strips.copied {
            Some((_, until)) if now >= until => {
                self.settings.harnesses.strips.copied = None;
                true
            }
            Some(_) => {
                tx.wake_after(Duration::from_millis(100));
                false
            }
            None => false,
        }
    }

    /// While a harness update waits or runs, the list of `machine` is read again every 2 s (`useHarnessUpdatePoll`). `shown` says
    /// whether something that draws it is on screen. True when a read was sent.
    pub fn keep_update_poll(&mut self, machine: &str, shown: bool, now: f64, tx: &mut Tx) -> bool {
        let list = self.harness_choice(machine).all;
        let due = &mut self.settings.harnesses.strips.poll_due;
        if !shown || !updating(&list) {
            *due = None;
            return false;
        }
        match *due {
            Some(at) if now < at => {
                tx.wake_after(Duration::from_secs_f64((at - now).max(0.01)));
                false
            }
            before => {
                *due = Some(now + POLL_SECS);
                tx.wake_after(Duration::from_secs_f64(POLL_SECS));
                // The first look only starts the wait.
                if before.is_some() {
                    self.read_harnesses(machine, true);
                }
                before.is_some()
            }
        }
    }
}

impl State {
    /// The run of the strips' timers, from `poll`: "Copied" ends, and the Harnesses page follows an update that waits or runs.
    pub fn keep_harness_strips(&mut self, now: f64, tx: &mut Tx) -> bool {
        let on_page = self.rail == crate::Rail::Settings && self.settings.page == crate::fleet::settings_nav::Page::Harnesses;
        let machine = self.harness_machine();
        self.keep_harness_copied(now, tx) | self.keep_update_poll(&machine, on_page, now, tx)
    }
}
