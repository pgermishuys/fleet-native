//! The calls and stores behind Settings → Updates & about (`use-update-status.ts`, `use-release-notes.ts`,
//! `use-fleet-updates.ts`). The stores are plain structs, tested without a server; the `State` methods send the requests
//! on threads and hand the answers back. Every request goes to the machine Settings shows, except the rollout, the
//! restart and the rule, which Fleet sends to home whatever the switch says (the plan's notes).
use super::notes::{ReleaseNote, Releases};
use super::{
    Kind, LastSeen, MachineInput, MachineKind, Rollout, RuleChoice, Status, Tone, UPDATE_RULE_DEFAULT_KEY, UPDATE_RULE_OVERRIDES_KEY,
    UpdateRow, UpdateRule, UpdateState, UpdateStatus, busy_sessions, describe_machine_update, describe_rollout_step, effective_rule,
    os_label, overrides_json, parse_chosen_rule, parse_rule_overrides, restart_offer, with_override,
};
use crate::fleet::events::{Event, Kind as EventKind};
use crate::fleet::http;
use crate::fleet::machines::{Machines, read_error};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::settings_nav::Page;
use crate::fleet::state::Topic;
use crate::{Rail, State, Tx};
use serde_json::{Value, json};
use std::collections::HashMap;

/// While a download is under way the status is read this often, in seconds (`POLL_INTERVAL_DOWNLOADING_MS`).
pub const DOWNLOAD_POLL_SECS: f64 = 1.5;
/// The server checks GitHub every few hours; asking it for its state is local and cheap, so the page hears soon after
/// (`REFRESH_INTERVAL_MS`).
pub const STATUS_REFRESH_SECS: f64 = 15.0 * 60.0;
/// While a rollout runs the page reads it this often as well as taking what the Fleet pushes (a push can be missed across a
/// reconnect), and while a check runs it reads the machines this often (`ROLLOUT_POLL_MS`, `BUSY_REFRESH_MS`).
pub const ROLLOUT_POLL_SECS: f64 = 3.0;
/// After this Fleet restarts itself, the page asks whether it is back this often, for at most this long.
pub const COME_BACK_POLL_SECS: f64 = 1.5;
pub const COME_BACK_LIMIT_SECS: f64 = 3.0 * 60.0;

/// One machine's update status, read and kept fresh (`createUpdateStatus`).
#[derive(Debug, Default)]
pub struct StatusStore {
    pub status: Option<UpdateStatus>,
    /// Until the first answer, good or not.
    pub loading: bool,
    request: u64,
    in_flight: bool,
    next_read: Option<f64>,
}

impl StatusStore {
    /// A read is about to be sent: its number, to tell an answer that has been overtaken.
    pub fn begin(&mut self) -> u64 {
        self.request += 1;
        self.in_flight = true;
        if self.status.is_none() {
            self.loading = true;
        }
        self.request
    }

    /// Takes an answer (None: the read failed, which is ignored, so a failing check doesn't break the page). An answer that
    /// a later read has overtaken is dropped. While a download runs it asks again in 1.5 s, else in 15 minutes.
    pub fn adopt(&mut self, request: u64, answer: Option<UpdateStatus>, now: f64) {
        if request != self.request {
            return;
        }
        self.in_flight = false;
        self.loading = false;
        if let Some(status) = answer {
            let downloading = status.status == Status::Downloading;
            self.status = Some(status);
            self.next_read = Some(now + if downloading { DOWNLOAD_POLL_SECS } else { STATUS_REFRESH_SECS });
        } else {
            self.next_read = Some(now + STATUS_REFRESH_SECS);
        }
    }

    /// Whether a read should go out now: none under way, and none ever made or one is due.
    pub fn due(&self, now: f64) -> bool {
        !self.in_flight && self.next_read.is_none_or(|at| now >= at)
    }
}

/// One machine's release notes, read once and kept (`createReleaseNotes`).
#[derive(Debug, Default)]
pub struct ReleasesStore {
    pub releases: Vec<ReleaseNote>,
    pub fetched_at: Option<String>,
    pub error: Option<String>,
    pub loading: bool,
    pub loaded: bool,
}

pub const RELEASES_FAILED: &str = "Fleet couldn't load the release notes.";

impl ReleasesStore {
    pub fn adopt(&mut self, answer: Option<Releases>) {
        self.loading = false;
        self.loaded = true;
        match answer {
            Some(got) => {
                self.releases = got.releases;
                self.fetched_at = got.fetched_at;
                self.error = got.error;
            }
            None => self.error = Some(RELEASES_FAILED.into()),
        }
    }
}

/// Waiting for this Fleet to come back after it restarted itself (`restartSelf`): it has to go down and answer again, within
/// three minutes.
#[derive(Clone, Debug, PartialEq)]
pub struct ComeBack {
    pub from: Option<String>,
    pub to: Option<String>,
    started: f64,
    went_down: bool,
    next_ask: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComeBackStep {
    Waiting,
    /// It answered after being down: read everything again.
    Back,
    GaveUp,
}

pub const DIDNT_COME_BACK: &str = "This Fleet didn't come back within 3 minutes.";

impl ComeBack {
    pub fn new(from: Option<String>, to: Option<String>, now: f64) -> ComeBack {
        ComeBack { from, to, started: now, went_down: false, next_ask: now + COME_BACK_POLL_SECS }
    }

    /// Whether to ask the Fleet whether it is back (`GET /api/machine`), at the poll's pace.
    pub fn should_ask(&self, now: f64) -> bool {
        now >= self.next_ask
    }

    /// What an answer (or the lack of one) means. An answer before it ever went down is the Fleet not having stopped yet.
    pub fn answered(&mut self, answered: bool, now: f64) -> ComeBackStep {
        self.next_ask = now + COME_BACK_POLL_SECS;
        if !answered {
            self.went_down = true;
        }
        if answered && self.went_down {
            ComeBackStep::Back
        } else if now - self.started > COME_BACK_LIMIT_SECS {
            ComeBackStep::GaveUp
        } else {
            ComeBackStep::Waiting
        }
    }
}

/// What a refusal says: the server's `{error}` or the status (`postOnHome`).
pub fn refusal(status: u16, body: &str) -> String {
    read_error(body, format!("The Fleet answered {status}."))
}

pub const UNREACHABLE: &str = "Couldn't reach this Fleet.";

/// Everything Updates & about keeps between frames.
#[derive(Debug, Default)]
pub struct UpdatesFeed {
    pub status: HashMap<String, StatusStore>,
    pub releases: HashMap<String, ReleasesStore>,
    /// The rollout the home Fleet is running, or its last; the Fleet pushes each change.
    pub rollout: Option<Rollout>,
    pub action_error: Option<String>,
    /// Set from the moment this Fleet is asked to restart until it answers again.
    pub restarting: Option<ComeBack>,
    /// "Check now" is under way.
    pub checking: bool,
    /// What each machine last reported to this device.
    pub last_seen: LastSeen,
    /// The rollout has been read and is followed, while the page is open.
    following: bool,
    next_rollout_read: f64,
    ask_in_flight: bool,
}

/// What `machine_row` needs besides the machine list.
pub struct RowContext<'a> {
    pub rollout: Option<&'a Rollout>,
    pub restarting: Option<&'a ComeBack>,
    pub chosen: Option<UpdateRule>,
    pub overrides: &'a std::collections::BTreeMap<String, UpdateRule>,
    pub last_seen: &'a LastSeen,
}

impl Machines {
    /// Home, then each listed machine, as Updates & about rows (`useFleetUpdates` `machineRow`). The desktop app's own row is
    /// piece V's and never among them.
    pub fn update_rows(&self, cx: &RowContext) -> Vec<UpdateRow> {
        let current = self.current_key().to_string();
        self.entries()
            .into_iter()
            .map(|entry| {
                let info = entry.info.as_ref();
                let reachable = entry.is_home || self.others.get(&entry.key).is_none_or(|o| o.error.is_none());
                let version = info
                    .map(|i| i.version.clone())
                    .filter(|v| !v.is_empty())
                    .or_else(|| if reachable { None } else { cx.last_seen.get(&entry.key).map(str::to_string) });
                let working = entry.capabilities().map_or(0, |c| busy_sessions(c.working, c.needs_you));
                let update = info.and_then(|i| i.update.as_ref());
                let described = if reachable && info.is_none() {
                    UpdateState { kind: Kind::Checking, label: "checking…".into(), tone: Tone::Muted, target: None }
                } else {
                    describe_machine_update(&MachineInput { version: version.as_deref(), update, working, reachable })
                };
                let step = cx.rollout.filter(|r| r.running()).and_then(|r| r.steps.iter().find(|s| s.machine_id == entry.key));
                let shown = match (&cx.restarting, step) {
                    (Some(back), _) if entry.is_home => UpdateState {
                        kind: Kind::Restarting,
                        label: "restarting · this page reconnects when it's back".into(),
                        tone: Tone::Info,
                        target: back.to.clone(),
                    },
                    // A machine that can't be reached says so, even while a rollout waits to get to it (it will skip it).
                    (_, Some(step)) if described.kind != Kind::Unreachable => describe_rollout_step(step),
                    _ => described.clone(),
                };
                let kind = if info.and_then(|i| i.web_app) == Some(false) { MachineKind::Node } else { MachineKind::Fleet };
                let overridden = cx.overrides.get(&entry.key).copied();
                UpdateRow {
                    name: entry.name.clone(),
                    os: os_label(entry.os.as_deref().unwrap_or("")),
                    // The home Fleet reports its rule; a Fleet older than the rule can't take one.
                    rule: update
                        .and_then(|u| u.rule)
                        .map(|_| RuleChoice { effective: effective_rule(overridden, cx.chosen, kind), overridden }),
                    version,
                    latest: update.and_then(|u| u.latest_version.clone()),
                    where_: if entry.is_home {
                        "this machine".into()
                    } else {
                        entry.machine.as_ref().map(|m| m.base().to_string()).unwrap_or_default()
                    },
                    is_live: entry.key == current,
                    state: shown,
                    restart: if step.is_some() || cx.restarting.is_some() {
                        None
                    } else {
                        restart_offer(&described, update, entry.is_home)
                    },
                    is_home: entry.is_home,
                    working: Some(working),
                    kind,
                    key: entry.key,
                }
            })
            .collect()
    }
}

fn parse_body<T>(result: Result<(u16, String), String>, read: impl Fn(&Value) -> Option<T>) -> Option<T> {
    let (status, body) = result.ok()?;
    if !(200..300).contains(&status) {
        return None;
    }
    read(&serde_json::from_str::<Value>(&body).ok()?)
}

impl State {
    fn updates_page_open(&self) -> bool {
        self.rail == Rail::Settings && self.settings.page == Page::Updates
    }

    /// The rows of the Machines group, from the machine list and what this device knows.
    pub fn update_rows(&self) -> Vec<UpdateRow> {
        let prefs = &self.pickers.prefs;
        let overrides = parse_rule_overrides(prefs.get(UPDATE_RULE_OVERRIDES_KEY));
        let cx = RowContext {
            rollout: self.updates.rollout.as_ref(),
            restarting: self.updates.restarting.as_ref(),
            chosen: parse_chosen_rule(prefs.get(UPDATE_RULE_DEFAULT_KEY)),
            overrides: &overrides,
            last_seen: &self.updates.last_seen,
        };
        self.machines.update_rows(&cx)
    }

    /// Notes what each machine reported, so one that can't be reached still says what it ran. True when the note changed and
    /// is worth writing to the settings file.
    pub fn note_machine_versions(&mut self) -> bool {
        let reported: Vec<(String, String)> = self.machines.entries().into_iter().filter_map(|e| Some((e.key, e.info?.version))).collect();
        self.updates.last_seen.note(reported.iter().map(|(k, v)| (k.as_str(), v.as_str())))
    }

    /// Called from `poll`: while Updates & about is open, keeps the shown machine's status and notes fresh, reads the rollout
    /// and follows it. A restart of this Fleet is followed wherever the page is.
    pub fn keep_updates(&mut self, now: f64, tx: &mut Tx) -> bool {
        let mut changed = self.keep_come_back(now);
        if !self.updates_page_open() {
            self.updates.following = false;
            return changed;
        }
        let key = self.settings.choice.key(&self.machines).to_string();
        if self.updates.status.entry(key.clone()).or_default().due(now) {
            self.load_update_status(&key, now);
        }
        let notes = self.updates.releases.entry(key.clone()).or_default();
        if !notes.loaded && !notes.loading {
            self.load_releases(&key);
        }
        if !self.updates.following {
            self.updates.following = true;
            self.load_rollout();
            self.updates.next_rollout_read = now + ROLLOUT_POLL_SECS;
        } else if self.updates.rollout.as_ref().is_some_and(Rollout::running) && now >= self.updates.next_rollout_read {
            self.updates.next_rollout_read = now + ROLLOUT_POLL_SECS;
            self.load_rollout();
            if let Some(link) = &self.link {
                link.refresh_machines();
            }
        }
        if self.note_machine_versions() {
            changed = true;
        }
        let _ = tx;
        changed
    }

    /// `GET /api/update/status` on a machine.
    pub fn load_update_status(&mut self, key: &str, now: f64) {
        let request = self.updates.status.entry(key.to_string()).or_default().begin();
        let Some(link) = self.link_for_machine(key) else {
            self.updates.status.entry(key.to_string()).or_default().adopt(request, None, now);
            return;
        };
        let key = key.to_string();
        link.off_thread(
            |base| parse_body(http::text_with("GET", base, "/api/update/status", None), UpdateStatus::from_json),
            move |s, _, got| {
                let at = s.clock;
                s.updates.status.entry(key).or_default().adopt(request, got, at);
            },
        );
    }

    /// `GET /api/update/releases` on a machine, once.
    pub fn load_releases(&mut self, key: &str) {
        self.updates.releases.entry(key.to_string()).or_default().loading = true;
        let Some(link) = self.link_for_machine(key) else {
            self.updates.releases.entry(key.to_string()).or_default().adopt(None);
            return;
        };
        let key = key.to_string();
        link.off_thread(
            |base| parse_body(http::text_with("GET", base, "/api/update/releases", None), Releases::from_json),
            move |s, _, got| s.updates.releases.entry(key).or_default().adopt(got),
        );
    }

    /// `GET /api/update/rollout` on home: a 200 is the rollout, anything else leaves what is shown.
    pub fn load_rollout(&mut self) {
        let Some(link) = &self.link else { return };
        link.off_thread(
            |base| parse_body(http::text_with("GET", base, "/api/update/rollout", None), Rollout::from_json),
            |s, _, got| {
                if let Some(rollout) = got {
                    s.updates.rollout = Some(rollout);
                }
            },
        );
    }

    /// `update.rollout` on the home sessions hub: its payload is the whole rollout.
    pub fn on_update_event(&mut self, topic: &Topic, event: &Event) {
        let Event::Other(EventKind::UpdateRollout, payload) = event else { return };
        if *topic != Topic::Sessions {
            return;
        }
        if let Some(rollout) = Rollout::from_json(payload) {
            self.updates.rollout = Some(rollout);
        }
    }

    /// "Check now" (`checkNow`): every reachable machine looks for a release, then they are read again. The desktop app's own
    /// check is piece V's.
    pub fn check_updates_now(&mut self) {
        if self.updates.checking {
            return;
        }
        let machines: Vec<String> = self
            .machines
            .entries()
            .into_iter()
            .filter(|e| e.info.is_some() && (e.is_home || self.machines.others.get(&e.key).is_none_or(|o| o.error.is_none())))
            .map(|e| e.key)
            .collect();
        let links: Vec<_> = machines.iter().filter_map(|k| self.link_for_machine(k)).collect();
        self.updates.checking = true;
        let Some(home) = self.link.clone() else {
            self.updates.checking = false;
            return;
        };
        let post = home.post();
        std::thread::spawn(move || {
            // Every check goes out at once; one that fails doesn't stop the others (`Promise.allSettled`).
            std::thread::scope(|scope| {
                for link in &links {
                    scope.spawn(|| {
                        let _ = http::text_with("POST", &link.base, "/api/update/check", None);
                    });
                }
            });
            post.update(|s, _| {
                s.updates.checking = false;
                if let Some(link) = &s.link {
                    link.refresh_machines();
                }
            });
        });
    }

    /// `POST /api/update/{check,download}` on one machine, then its status is read again (`checkForUpdate`, `downloadUpdate`).
    pub fn update_machine(&mut self, key: &str, path: &'static str) {
        let Some(link) = self.link_for_machine(key) else { return };
        let (key, now) = (key.to_string(), self.clock);
        link.off_thread(
            move |base| {
                let _ = http::text_with("POST", base, path, None);
            },
            move |s, _, ()| s.load_update_status(&key, now),
        );
    }

    /// A call on home that changes something: its refusal is shown, its answer handed on (`postOnHome`).
    fn post_on_home(
        &mut self,
        path: &'static str,
        body: Option<Value>,
        done: impl FnOnce(&mut State, &mut Tx, Result<String, String>) + Send + 'static,
    ) {
        self.updates.action_error = None;
        let Some(link) = &self.link else {
            self.updates.action_error = Some(UNREACHABLE.into());
            return;
        };
        link.off_thread(
            move |base| match http::text_with("POST", base, path, body.as_ref()) {
                Ok((status, text)) if (200..300).contains(&status) => Ok(text),
                Ok((status, text)) => Err(refusal(status, &text)),
                Err(_) => Err(UNREACHABLE.to_string()),
            },
            done,
        );
    }

    /// `startRollout`: restarts the machines (all that can, or just `machine_ids`) one at a time, from the home Fleet.
    pub fn start_rollout(&mut self, machine_ids: Vec<String>) {
        self.post_on_home("/api/update/rollout", Some(json!({ "machineIds": machine_ids })), |s, _, result| match result {
            Ok(text) => {
                if let Some(rollout) = serde_json::from_str::<Value>(&text).ok().as_ref().and_then(Rollout::from_json) {
                    s.updates.rollout = Some(rollout);
                }
            }
            Err(why) => s.updates.action_error = Some(why),
        });
    }

    /// `stopRollout`: it finishes the machine it is on and leaves the rest alone.
    pub fn stop_rollout(&mut self) {
        let Some(link) = &self.link else { return };
        link.off_thread(
            |base| {
                let _ = http::text_with("DELETE", base, "/api/update/rollout", None);
            },
            |_, _, ()| {},
        );
    }

    /// The button on a row: this machine restarts itself; any other is a rollout of that one machine (`restart`).
    pub fn restart_machine(&mut self, key: &str, from: Option<String>, to: Option<String>) {
        if key != HOME_MACHINE_KEY {
            return self.start_rollout(vec![key.to_string()]);
        }
        let now = self.clock;
        self.post_on_home("/api/update/apply", None, move |s, _, result| match result {
            Ok(_) => s.updates.restarting = Some(ComeBack::new(from, to, now)),
            Err(why) => s.updates.action_error = Some(why),
        });
    }

    /// While this Fleet restarts, asks whether it is back, at 1.5 s, for three minutes. When it is, everything is read again
    /// (the browser reloads; here the stores are cleared and the page reads).
    fn keep_come_back(&mut self, now: f64) -> bool {
        let Some(back) = &self.updates.restarting else { return false };
        if self.updates.ask_in_flight || !back.should_ask(now) {
            return false;
        }
        let Some(link) = &self.link else { return false };
        self.updates.ask_in_flight = true;
        link.off_thread(
            |base| http::text_with("GET", base, "/api/machine", None).is_ok_and(|(status, _)| (200..300).contains(&status)),
            |s, _, answered| {
                s.updates.ask_in_flight = false;
                let at = s.clock;
                let Some(back) = s.updates.restarting.as_mut() else { return };
                match back.answered(answered, at) {
                    ComeBackStep::Waiting => {}
                    ComeBackStep::Back => {
                        s.updates.restarting = None;
                        s.updates.status.clear();
                        s.updates.releases.clear();
                        s.updates.rollout = None;
                        s.updates.following = false;
                        if let Some(link) = &s.link {
                            link.refresh_machines();
                        }
                    }
                    ComeBackStep::GaveUp => {
                        s.updates.restarting = None;
                        s.updates.action_error = Some(DIDNT_COME_BACK.into());
                    }
                }
            },
        );
        false
    }

    /// `chooseRule`: the rule for every machine that has none of its own (None: none chosen). Home gives it to them within a minute.
    pub fn choose_update_rule(&mut self, rule: Option<UpdateRule>) {
        let before = self.pickers.prefs.get(UPDATE_RULE_DEFAULT_KEY).map(str::to_string);
        self.put_preference(UPDATE_RULE_DEFAULT_KEY, rule.map_or("", UpdateRule::word), before);
    }

    /// `overrideRule`: one machine's own rule, or (None) back on the chosen one.
    pub fn override_update_rule(&mut self, key: &str, rule: Option<UpdateRule>) {
        let before = self.pickers.prefs.get(UPDATE_RULE_OVERRIDES_KEY).map(str::to_string);
        let next = with_override(&parse_rule_overrides(before.as_deref()), key, rule);
        self.put_preference(UPDATE_RULE_OVERRIDES_KEY, &overrides_json(&next), before);
    }
}
