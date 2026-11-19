//! `HarnessSetupRows.vue` (`components/harness-setup`): the harnesses step's rows, with what each shows (rows C13.1-C13.49) and
//! what its Install and Sign in buttons do (C13.52-C13.73). The dialog is `view::harness_setup`, the rows `view::harness_setup_rows`.
//! The setup terminal's own widget is component 13's: the server side is real here (the shell is asked for and ended, the harness
//! list is checked every 3 s while it is open, the found banner appears), the body shows the command in its place.
use super::harness_setup::Step;
use super::http;
use super::new_session::harness_choice::{HarnessEntry, Harnesses};
use super::settings::HOME_MACHINE_KEY;
use crate::{State, Tx};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// How often the harnesses are checked while the setup terminal is open (`CHECK_EVERY_MS`).
pub const CHECK_EVERY_SECS: f64 = 3.0;
/// How often a download that waits or runs is checked (`HARNESS_UPDATE_POLL_MS`).
pub const UPDATE_POLL_SECS: f64 = 2.0;
/// "Copied" shows this long.
pub const COPIED_SECS: f64 = 1.5;
/// The shell's size when it is asked for (`createSetupTerminal(machine, 100, 14)`).
pub const TERMINAL_COLS: u32 = 100;
pub const TERMINAL_ROWS: u32 = 14;

/// A folder an install uses (`HarnessFolder`): "Program", "Settings", "Sessions" and where.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Folder {
    pub label: String,
    pub path: String,
}

impl Folder {
    fn parse_all(v: &Value) -> Vec<Folder> {
        let text = |f: &Value, k: &str| f.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        v.get("folders")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|f| Folder { label: text(f, "label"), path: text(f, "path") })
            .collect()
    }
}

/// One place a harness can be installed (`HarnessInstallChoice`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Choice {
    pub id: String,
    pub label: String,
    pub description: String,
    pub command: String,
    pub recommended: bool,
    pub folders: Vec<Folder>,
}

/// What the server says about setting a harness up (`HarnessInfo.setup`, `presentation`, and the bits of `update` the rows read).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Setup {
    pub install_command: Option<String>,
    pub sign_in_command: Option<String>,
    pub download_url: Option<String>,
    pub install_choices: Vec<Choice>,
    pub notes: Vec<String>,
    /// `mode`: how it is installed here, in a few words ("In its own folder"); the Settings install panel shows for one that has it.
    pub mode: Option<String>,
    /// The folders the install uses (program, settings, data).
    pub folders: Vec<Folder>,
    /// `presentation.icon`: `terminal`, `hexagon`, `infinity` or `plug`.
    pub icon: Option<String>,
    /// `presentation.pitch`: the line shown while it isn't installed.
    pub pitch: Option<String>,
    /// `update.installPlan`: set for a harness Fleet downloads itself (GitHub Copilot).
    pub install_plan: Option<String>,
    /// `update.job.phase`.
    pub update_phase: Option<String>,
}

impl Setup {
    pub fn parse(h: &Value) -> Setup {
        let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
        let setup = h.get("setup").unwrap_or(&Value::Null);
        Setup {
            install_command: text(setup, "installCommand"),
            sign_in_command: text(setup, "signInCommand"),
            download_url: text(setup, "downloadUrl"),
            install_choices: setup
                .get("installChoices")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|c| {
                    Some(Choice {
                        id: text(c, "id")?,
                        label: text(c, "label").unwrap_or_default(),
                        description: text(c, "description").unwrap_or_default(),
                        command: text(c, "command").unwrap_or_default(),
                        recommended: c.get("recommended").and_then(Value::as_bool) == Some(true),
                        folders: Folder::parse_all(c),
                    })
                })
                .collect(),
            notes: setup
                .get("notes")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|n| n.as_str().map(str::to_string))
                .collect(),
            mode: text(setup, "mode"),
            folders: Folder::parse_all(setup),
            icon: h.pointer("/presentation/icon").and_then(Value::as_str).map(str::to_string),
            pitch: h.pointer("/presentation/pitch").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string),
            install_plan: h.pointer("/update/installPlan").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string),
            update_phase: h.pointer("/update/job/phase").and_then(Value::as_str).map(str::to_string),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Install,
    SignIn,
}

impl HarnessEntry {
    /// `harnessState`: the server's word, else `ready` when it is available, else `not-working`.
    pub fn setup_state(&self) -> &str {
        if !self.state.is_empty() {
            &self.state
        } else if self.available {
            "ready"
        } else {
            "not-working"
        }
    }

    /// One Fleet downloads itself (GitHub Copilot): no command to type, no download link, but an install plan (C13.6).
    pub fn fleet_installs(&self) -> bool {
        self.setup.install_command.is_none() && self.setup.download_url.is_none() && self.setup.install_plan.is_some()
    }

    /// What the row's button does next (`actionFor`).
    pub fn setup_action(&self) -> Option<Action> {
        match self.setup_state() {
            "ready" => None,
            "sign-in-required" => self.setup.sign_in_command.is_some().then_some(Action::SignIn),
            _ => Some(Action::Install),
        }
    }

    /// `version · executablePath`, whichever exist (`harnessLocation`).
    pub fn location(&self) -> Option<String> {
        let parts: Vec<&str> =
            [self.version.as_deref(), self.executable_path.as_deref()].into_iter().flatten().filter(|p| !p.is_empty()).collect();
        (!parts.is_empty()).then(|| parts.join(" · "))
    }

    /// The row's second line (C13.13-C13.17).
    pub fn subtitle(&self) -> String {
        let name = &self.display_name;
        match self.setup_state() {
            "ready" => self.location().unwrap_or_else(|| "Ready.".into()),
            "sign-in-required" => "Installed. Sign in once so sessions can use your account.".into(),
            "not-working" => self.reason.clone().unwrap_or_else(|| format!("{name} isn't working.")),
            "update-needed" => self.reason.clone().unwrap_or_else(|| format!("{name} is too old for Fleet.")),
            _ => self.setup.pitch.clone().or_else(|| self.reason.clone()).unwrap_or_else(|| format!("{name} isn't installed.")),
        }
    }

    /// The download to open by hand: it needs installing and there's no installer to type (`downloadFor`).
    pub fn download(&self) -> Option<&str> {
        if self.setup_action() != Some(Action::Install) || self.setup.install_command.is_some() {
            return None;
        }
        self.setup.download_url.as_deref()
    }

    /// What to know before installing; gone once it is ready (`notesFor`).
    pub fn notes(&self) -> &[String] {
        if self.setup_state() == "ready" { &[] } else { &self.setup.notes }
    }

    /// The places to pick from: more than one, and only while it needs installing (`choicesFor`).
    pub fn choices(&self) -> &[Choice] {
        if self.setup.install_choices.len() > 1 && self.setup_action() == Some(Action::Install) { &self.setup.install_choices } else { &[] }
    }

    pub fn action_label(&self) -> String {
        if self.setup_action() == Some(Action::SignIn) {
            format!("Sign in to {}", self.display_name)
        } else if self.setup_state() == "update-needed" {
            format!("Update {}", self.display_name)
        } else {
            format!("Install {}", self.display_name)
        }
    }
}

/// The harnesses Fleet has an installer or a download for, the default one first (`rows`).
pub fn rows(h: &Harnesses) -> Vec<&HarnessEntry> {
    let default = h.default_type();
    let mut list: Vec<&HarnessEntry> =
        h.all.iter().filter(|e| e.setup.install_command.is_some() || e.setup.download_url.is_some() || e.fleet_installs()).collect();
    list.sort_by_key(|e| e.kind != default);
    list
}

/// The terminal under a row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Active {
    pub kind: String,
    pub action: Action,
    pub command: String,
    /// `None` until the server has started the shell.
    pub terminal_id: Option<String>,
}

/// Why the terminal couldn't open, with the command to run elsewhere.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub kind: String,
    pub message: String,
    pub command: String,
}

/// The rows' own state (`HarnessSetupRows.vue`'s refs), kept in `Setup` so it ends with the wizard.
#[derive(Debug, Default)]
pub struct Rows {
    /// The install place picked per harness type; the recommended one until another is picked.
    picked: HashMap<String, String>,
    pub active: Option<Active>,
    pub failure: Option<Failure>,
    pub copied_until: Option<f64>,
    /// Counts starts and closes, so a shell that was asked for and not wanted any more is ended when it arrives.
    seq: u64,
    pub(crate) check_due: Option<f64>,
    pub(crate) update_due: Option<f64>,
}

/// What asking for the shell came to.
enum Outcome {
    Started(String),
    Off,
    Failed(String),
}

impl Rows {
    pub fn pick(&mut self, kind: &str, choice: &str) {
        self.picked.insert(kind.to_string(), choice.to_string());
    }

    /// The user's pick, else the recommended place, else the first (`pickedChoice`).
    pub fn picked_choice<'a>(&self, h: &'a HarnessEntry) -> Option<&'a Choice> {
        let choices = h.choices();
        choices
            .iter()
            .find(|c| self.picked.get(&h.kind) == Some(&c.id))
            .or_else(|| choices.iter().find(|c| c.recommended))
            .or_else(|| choices.first())
    }

    /// What is typed into the terminal (`commandFor`).
    pub fn command_for(&self, h: &HarnessEntry, action: Action) -> Option<String> {
        match action {
            Action::SignIn => h.setup.sign_in_command.clone(),
            Action::Install => self.picked_choice(h).map(|c| c.command.clone()).or_else(|| h.setup.install_command.clone()),
        }
    }

    /// Whether the terminal under this harness's row is open (its button and radios are off while it is).
    pub fn is_active(&self, kind: &str) -> bool {
        self.active.as_ref().is_some_and(|a| a.kind == kind)
    }

    /// The banner under the terminal once the harness it was opened for is found (`found`).
    pub fn found(&self, list: &Harnesses) -> Option<String> {
        let active = self.active.as_ref()?;
        let h = list.all.iter().find(|h| h.kind == active.kind)?;
        let state = h.setup_state();
        if active.action == Action::SignIn {
            return (state == "ready").then(|| format!("{} is signed in and ready.", h.display_name));
        }
        if state != "ready" && state != "sign-in-required" {
            return None;
        }
        let version = h.version.as_deref().filter(|v| !v.is_empty()).map(|v| format!(" {v}")).unwrap_or_default();
        let at = h.executable_path.as_deref().filter(|p| !p.is_empty()).map(|p| format!(" at {p}")).unwrap_or_default();
        Some(format!("Found {}{version}{at}. You don't need to restart Fleet.", h.display_name))
    }
}

impl State {
    /// `start(harness, action)`: types nothing, runs nothing, only opens the terminal and watches for the result. Does nothing
    /// when there is no command to show or no server.
    pub fn setup_rows_start(&mut self, kind: &str, action: Action) {
        let list = self.setup_harnesses();
        let Some(h) = list.all.iter().find(|h| h.kind == kind) else { return };
        let Some(command) = self.harness_setup.rows.command_for(h, action) else { return };
        if self.feed_base(HOME_MACHINE_KEY).is_none() {
            return;
        }
        let turn_on = !h.user_enabled;
        self.close_setup_terminal();
        let rows = &mut self.harness_setup.rows;
        rows.failure = None;
        let seq = rows.seq;
        rows.active = Some(Active { kind: kind.to_string(), action, command: command.clone(), terminal_id: None });
        // Setting up a harness here means wanting to use it: turn it on (Claude Code starts off), then read the list again.
        if turn_on {
            let key = format!("{kind}.enabled");
            self.pickers.prefs.values.insert(key.clone(), "true".into());
            self.feed_call(
                HOME_MACHINE_KEY,
                move |base| http::put_preference(base, &key, "true"),
                |s, _, _| s.read_harnesses(HOME_MACHINE_KEY, true),
            );
        }
        let (kind, shown) = (kind.to_string(), command);
        self.feed_call(
            HOME_MACHINE_KEY,
            |base| {
                let on = http::client_config(base).ok().and_then(|c| c.get("terminalEnabled").and_then(Value::as_bool)).unwrap_or(false);
                if !on {
                    return Outcome::Off;
                }
                match http::setup_terminal_create(base, TERMINAL_COLS, TERMINAL_ROWS) {
                    Ok(id) => Outcome::Started(id),
                    Err(message) => Outcome::Failed(message),
                }
            },
            move |s, tx, outcome| s.setup_terminal_answered(seq, &kind, shown, outcome, tx),
        );
    }

    fn setup_terminal_answered(&mut self, seq: u64, kind: &str, command: String, outcome: Outcome, tx: &mut Tx) {
        let rows = &mut self.harness_setup.rows;
        let wanted = rows.seq == seq && rows.is_active(kind);
        match outcome {
            Outcome::Started(id) if wanted => {
                if let Some(active) = rows.active.as_mut() {
                    active.terminal_id = Some(id.clone());
                }
                // `startChecking`: the list is read again every 3 s from now.
                rows.check_due = Some(self.clock + CHECK_EVERY_SECS);
                tx.wake_after(Duration::from_secs_f64(CHECK_EVERY_SECS));
                self.attach_setup_terminal(&id, &command, tx);
            }
            // Another harness's terminal was opened (or this one closed) meanwhile: end the one just made (C13.53).
            Outcome::Started(id) => {
                self.feed_call(
                    HOME_MACHINE_KEY,
                    move |base| {
                        let _ = http::setup_terminal_close(base, &id);
                    },
                    |_, _, _| {},
                );
            }
            Outcome::Off if wanted => {
                rows.active = None;
                rows.failure = Some(Failure { kind: kind.to_string(), message: "Terminals are turned off in this Fleet.".into(), command });
            }
            Outcome::Failed(message) if wanted => {
                rows.active = None;
                rows.failure = Some(Failure { kind: kind.to_string(), message, command });
            }
            _ => {}
        }
    }

    /// A row's button (`startNext`).
    pub fn setup_rows_next(&mut self, kind: &str) {
        let action = self.setup_harnesses().all.iter().find(|h| h.kind == kind).and_then(HarnessEntry::setup_action);
        if let Some(action) = action {
            self.setup_rows_start(kind, action);
        }
    }

    /// `closeTerminal`: stop checking, drop the terminal, and if the server had made one, check once more and end it (a 404 or
    /// any other answer is ignored).
    pub fn close_setup_terminal(&mut self) {
        self.release_setup_terminal();
        let rows = &mut self.harness_setup.rows;
        rows.check_due = None;
        rows.seq += 1;
        let Some(id) = rows.active.take().and_then(|a| a.terminal_id) else { return };
        self.read_harnesses(HOME_MACHINE_KEY, true);
        self.feed_call(
            HOME_MACHINE_KEY,
            move |base| {
                let _ = http::setup_terminal_close(base, &id);
            },
            |_, _, _| {},
        );
    }

    /// The copy button of a failure: the command on the clipboard, then "Copied" for 1.5 s.
    pub fn copy_setup_command(&mut self, command: &str, tx: &mut Tx) {
        tx.copy(command.to_string());
        self.harness_setup.rows.copied_until = Some(self.clock + COPIED_SECS);
        tx.wake_after(Duration::from_millis(100));
    }

    /// The timers of the rows, run from `poll`: "Copied" ends, the harnesses are read again every 3 s while a terminal is open
    /// (until a sign-in is found), and every 2 s while a download waits or runs. They only run while the step shows (the rows are
    /// mounted). True when something drawn changed.
    pub fn keep_setup_rows(&mut self, now: f64, tx: &mut Tx) -> bool {
        if !self.harness_setup.is_open || self.harness_setup.step != Step::Harnesses {
            return false;
        }
        let list = self.setup_harnesses();
        let rows = &mut self.harness_setup.rows;
        let mut changed = false;
        if let Some(until) = rows.copied_until {
            if now >= until {
                rows.copied_until = None;
                changed = true;
            } else {
                tx.wake_after(Duration::from_millis(100));
            }
        }
        let signed_in = rows.active.as_ref().is_some_and(|a| a.action == Action::SignIn) && rows.found(&list).is_some();
        let checking = rows.active.as_ref().is_some_and(|a| a.terminal_id.is_some()) && !signed_in;
        let updating = list.all.iter().any(|h| matches!(h.setup.update_phase.as_deref(), Some("waiting" | "running")));
        let mut read = false;
        for (wanted, due, every) in [(checking, &mut rows.check_due, CHECK_EVERY_SECS), (updating, &mut rows.update_due, UPDATE_POLL_SECS)]
        {
            if !wanted {
                *due = None;
            } else if due.is_none_or(|d| now >= d) {
                // The first call only starts the wait.
                read |= due.is_some();
                *due = Some(now + every);
                tx.wake_after(Duration::from_secs_f64(every));
            } else if let Some(d) = *due {
                tx.wake_after(Duration::from_secs_f64((d - now).max(0.01)));
            }
        }
        if read {
            self.read_harnesses(HOME_MACHINE_KEY, true);
        }
        changed
    }
}
