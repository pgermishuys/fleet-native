//! What each Fleet you run says about its own version and update (Settings → Updates & about; weave-fleet's
//! `lib/fleet-updates.ts`, `lib/release-notes.ts`, `composables/use-fleet-updates.ts`, `use-update-status.ts`,
//! `use-release-notes.ts`). Pure mapping from what a machine reported (`update` in `GET /api/machine`, `GET
//! /api/update/status`, the rollout) to the rows a page draws, so every state has one test. No Rune and no I/O here
//! except in [`feed`], which makes the calls on threads. The page is a later piece (V, W, X).
//!
//! The desktop app's own row is not here: native keeps its own updater (`src/update/`, decision 4 of the plan) and
//! piece V draws that row from it.
pub mod after_update;
pub mod feed;
pub mod notes;
pub mod rows;
pub mod strip;
pub mod whats_new;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_feed;
#[cfg(test)]
mod tests_notes;
#[cfg(test)]
mod tests_whats_new;

pub use rows::*;
pub use strip::*;

use serde_json::Value;
use std::cmp::Ordering;
use std::collections::BTreeMap;

/// Home's preference for the rule every machine follows unless it has its own.
pub const UPDATE_RULE_DEFAULT_KEY: &str = "FleetUpdateRule";
/// A JSON object: machine key (`home` for this one) to the rule that machine follows.
pub const UPDATE_RULE_OVERRIDES_KEY: &str = "FleetUpdateRules";
/// Set once the user has been told a machine updated itself under a rule they never picked.
pub const UPDATE_RULE_NOTICE_KEY: &str = "FleetUpdateRuleNotice";
/// The switch "Check for updates" on the Harnesses group: on unless the value is the literal "false". It is the shown
/// machine's own preference.
pub const HARNESS_UPDATE_CHECK: crate::fleet::settings::switches::Switch =
    crate::fleet::settings::switches::Switch { key: "harnessUpdates.check", default_on: true };

/// When a Fleet restarts itself into an update it has downloaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateRule {
    Ask,
    Idle,
    Overnight,
}

pub struct RuleInfo {
    pub value: UpdateRule,
    pub label: &'static str,
    pub short: &'static str,
    pub detail: &'static str,
}

pub const UPDATE_RULES: [RuleInfo; 3] = [
    RuleInfo { value: UpdateRule::Ask, label: "Ask me first", short: "Ask me", detail: "Each Fleet waits here until you restart it." },
    RuleInfo {
        value: UpdateRule::Idle,
        label: "Update each machine when nothing is in use",
        short: "When nothing is in use",
        detail: "It waits until no turn has run, no question has waited and no prompt or answer has come from any device for 15 minutes, then restarts and tells you after. Clicking Restart is immediate. Desktop apps still ask, because you're using them.",
    },
    RuleInfo {
        value: UpdateRule::Overnight,
        label: "Only overnight, 02:00–05:00 on that machine's clock",
        short: "Overnight",
        detail: "For machines that run long jobs during the day. It waits for 2 quiet minutes inside the window.",
    },
];

impl UpdateRule {
    pub fn parse(text: &str) -> Option<UpdateRule> {
        match text {
            "ask" => Some(UpdateRule::Ask),
            "idle" => Some(UpdateRule::Idle),
            "overnight" => Some(UpdateRule::Overnight),
            _ => None,
        }
    }

    /// The word the preference and the server keep.
    pub fn word(self) -> &'static str {
        match self {
            UpdateRule::Ask => "ask",
            UpdateRule::Idle => "idle",
            UpdateRule::Overnight => "overnight",
        }
    }

    pub fn info(self) -> &'static RuleInfo {
        UPDATE_RULES.iter().find(|r| r.value == self).expect("every rule is listed")
    }
}

/// One choice of "Machines update:" on the release strip (`RULE_OPTIONS`): none chosen, then the three rules.
pub struct RuleOption {
    pub value: Option<UpdateRule>,
    pub label: &'static str,
    pub detail: &'static str,
}

pub const RULE_OPTIONS: [RuleOption; 4] = [
    RuleOption {
        value: None,
        label: "Default: by machine, when idle or ask",
        detail: "Until you choose, a machine with no web app (started with fleet node) updates when nothing has been in use for 15 minutes, and one with the web app asks first.",
    },
    RuleOption { value: Some(UpdateRule::Ask), label: "Ask me first, every machine", detail: UPDATE_RULES[0].detail },
    RuleOption { value: Some(UpdateRule::Idle), label: "When nothing is in use, every machine", detail: UPDATE_RULES[1].detail },
    RuleOption { value: Some(UpdateRule::Overnight), label: "Only overnight, 02:00–05:00", detail: UPDATE_RULES[2].detail },
];

/// The line under the strip: what the chosen rule does, and that Update machines now ignores it (`updates-rule-hint`).
pub fn rule_hint(chosen: Option<UpdateRule>) -> String {
    let detail = RULE_OPTIONS.iter().find(|o| o.value == chosen).map_or("", |o| o.detail);
    format!("{detail} Update machines now doesn't wait for the rule.")
}

/// What "Default (…)" says on a machine's card: the short name of the rule it would follow without its own.
pub fn default_rule_label(chosen: Option<UpdateRule>, kind: MachineKind) -> &'static str {
    effective_rule(None, chosen, kind).info().short
}

/// The rule chosen for every machine (`FleetUpdateRule`): none for "" or anything unknown.
pub fn parse_chosen_rule(value: Option<&str>) -> Option<UpdateRule> {
    value.and_then(UpdateRule::parse)
}

/// The overrides as the preference keeps them; anything that isn't a known rule is dropped (`parseRuleOverrides`).
pub fn parse_rule_overrides(json: Option<&str>) -> BTreeMap<String, UpdateRule> {
    let parsed: Option<Value> = serde_json::from_str(json.filter(|j| !j.is_empty()).unwrap_or("{}")).ok();
    parsed
        .as_ref()
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(k, v)| Some((k.clone(), UpdateRule::parse(v.as_str()?)?)))
        .collect()
}

/// The overrides written back, for `PUT /api/preferences/FleetUpdateRules`.
pub fn overrides_json(overrides: &BTreeMap<String, UpdateRule>) -> String {
    serde_json::to_string(&overrides.iter().map(|(k, v)| (k.as_str(), v.word())).collect::<BTreeMap<_, _>>())
        .unwrap_or_else(|_| "{}".into())
}

/// `overrideRule`: one machine's own rule, or (None) back on the chosen one.
pub fn with_override(overrides: &BTreeMap<String, UpdateRule>, key: &str, rule: Option<UpdateRule>) -> BTreeMap<String, UpdateRule> {
    let mut next = overrides.clone();
    match rule {
        Some(rule) => next.insert(key.to_string(), rule),
        None => next.remove(key),
    };
    next
}

/// What a machine is: a full Fleet, or a node (`fleet node`, no web app).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineKind {
    Fleet,
    Node,
}

/// The rule a machine follows: its own, else the one chosen for all, else what suits its kind (`effectiveRule`).
pub fn effective_rule(overridden: Option<UpdateRule>, chosen: Option<UpdateRule>, kind: MachineKind) -> UpdateRule {
    overridden.or(chosen).unwrap_or(if kind == MachineKind::Node { UpdateRule::Idle } else { UpdateRule::Ask })
}

/// What started the restart a Fleet last made into an update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    Idle,
    Overnight,
    Manual,
    Rollout,
}

/// The update a Fleet last restarted into.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LastUpdate {
    pub from: String,
    pub to: String,
    pub at: String,
    pub trigger: Trigger,
}

impl LastUpdate {
    fn from_json(v: &Value) -> Option<LastUpdate> {
        let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let trigger = match v.get("trigger").and_then(Value::as_str)? {
            "idle" => Trigger::Idle,
            "overnight" => Trigger::Overnight,
            "manual" => Trigger::Manual,
            "rollout" => Trigger::Rollout,
            _ => return None,
        };
        Some(LastUpdate { from: text("from")?, to: text("to")?, at: text("at")?, trigger })
    }
}

/// Where a machine is with updating: the words `GET /api/update/status` and `update` in `GET /api/machine` share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Unknown,
    UpToDate,
    Available,
    Downloading,
    Staged,
    Error,
    /// The desktop app updates Fleet; the server doesn't check.
    Managed,
}

impl Status {
    fn parse(text: &str) -> Status {
        match text {
            "uptodate" => Status::UpToDate,
            "available" => Status::Available,
            "downloading" => Status::Downloading,
            "staged" => Status::Staged,
            "error" => Status::Error,
            "managed" => Status::Managed,
            _ => Status::Unknown,
        }
    }
}

/// An update that failed to start on a machine and was undone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RolledBack {
    pub from: String,
    pub to: String,
    pub reason: String,
    pub at: String,
}

/// What `update` in `GET /api/machine` says (`MachineUpdate`). `None` there is a Fleet older than the field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineUpdate {
    pub status: Status,
    pub latest_version: Option<String>,
    /// The version downloaded and waiting for a restart.
    pub staged_version: Option<String>,
    pub checked_at: Option<String>,
    /// Missing on a Fleet older than the restart.
    pub can_restart: Option<bool>,
    pub restart_unavailable_reason: Option<String>,
    pub rolled_back: Option<RolledBack>,
    /// Missing on a Fleet older than the rule.
    pub rule: Option<UpdateRule>,
    pub rule_set: bool,
    pub last_update: Option<LastUpdate>,
}

impl MachineUpdate {
    pub fn from_json(v: &Value) -> Option<MachineUpdate> {
        v.as_object()?;
        let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let rolled_back = v.get("rolledBack").filter(|r| r.is_object()).and_then(|r| {
            let t = |k: &str| r.get(k).and_then(Value::as_str).map(str::to_string);
            Some(RolledBack { from: t("from")?, to: t("to")?, reason: t("reason").unwrap_or_default(), at: t("at").unwrap_or_default() })
        });
        Some(MachineUpdate {
            status: Status::parse(v.get("status").and_then(Value::as_str).unwrap_or("unknown")),
            latest_version: text("latestVersion"),
            staged_version: text("stagedVersion"),
            checked_at: text("checkedAt"),
            can_restart: v.get("canRestart").and_then(Value::as_bool),
            restart_unavailable_reason: text("restartUnavailableReason"),
            rolled_back,
            rule: v.get("rule").and_then(Value::as_str).and_then(UpdateRule::parse),
            rule_set: v.get("ruleSource").and_then(Value::as_str) == Some("set"),
            last_update: v.get("lastUpdate").and_then(LastUpdate::from_json),
        })
    }
}

/// `GET /api/update/status` (`UpdateStatus`): this Fleet's version and where its update is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateStatus {
    pub current_version: String,
    pub status: Status,
    pub latest_version: Option<String>,
    pub checked_at: Option<String>,
    pub error: Option<String>,
    pub download_bytes_received: Option<u64>,
    pub download_bytes_total: Option<u64>,
}

impl UpdateStatus {
    pub fn from_json(v: &Value) -> Option<UpdateStatus> {
        v.as_object()?;
        let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        Some(UpdateStatus {
            current_version: text("currentVersion")?,
            status: Status::parse(v.get("status").and_then(Value::as_str).unwrap_or("unknown")),
            latest_version: text("latestVersion"),
            checked_at: text("checkedAt"),
            error: text("error"),
            download_bytes_received: v.get("downloadBytesReceived").and_then(Value::as_u64),
            download_bytes_total: v.get("downloadBytesTotal").and_then(Value::as_u64),
        })
    }

    /// `isUpdateAvailable`: an update is out or on its way.
    pub fn is_available(&self) -> bool {
        matches!(self.status, Status::Available | Status::Downloading)
    }

    /// `isUpdateStaged`: it is downloaded and waits for a restart.
    pub fn is_staged(&self) -> bool {
        self.status == Status::Staged
    }
}

/// One machine's part of a rollout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Pending,
    Checking,
    Waiting,
    Restarting,
    Done,
    RolledBack,
    NoReturn,
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RolloutStep {
    pub machine_id: String,
    pub name: String,
    pub state: StepState,
    pub detail: Option<String>,
    pub from_version: Option<String>,
    pub to_version: Option<String>,
    pub waiting_sessions: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RolloutStatus {
    Running,
    Done,
    Stopped,
    Cancelled,
}

/// A rollout, as `GET /api/update/rollout` and the `update.rollout` event say it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rollout {
    pub id: String,
    pub status: RolloutStatus,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub steps: Vec<RolloutStep>,
    pub summary: Option<String>,
}

impl Rollout {
    pub fn from_json(v: &Value) -> Option<Rollout> {
        let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let status = match v.get("status").and_then(Value::as_str)? {
            "running" => RolloutStatus::Running,
            "done" => RolloutStatus::Done,
            "stopped" => RolloutStatus::Stopped,
            "cancelled" => RolloutStatus::Cancelled,
            _ => return None,
        };
        let steps = v
            .get("steps")
            .and_then(Value::as_array)?
            .iter()
            .filter_map(|s| {
                let t = |k: &str| s.get(k).and_then(Value::as_str).map(str::to_string);
                let state = match s.get("state").and_then(Value::as_str)? {
                    "pending" => StepState::Pending,
                    "checking" => StepState::Checking,
                    "waiting" => StepState::Waiting,
                    "restarting" => StepState::Restarting,
                    "done" => StepState::Done,
                    "rolled-back" => StepState::RolledBack,
                    "no-return" => StepState::NoReturn,
                    "skipped" => StepState::Skipped,
                    _ => return None,
                };
                Some(RolloutStep {
                    machine_id: t("machineId")?,
                    name: t("name").unwrap_or_default(),
                    state,
                    detail: t("detail"),
                    from_version: t("fromVersion"),
                    to_version: t("toVersion"),
                    waiting_sessions: s.get("waitingSessions").and_then(Value::as_u64).unwrap_or(0).min(u32::MAX as u64) as u32,
                })
            })
            .collect();
        Some(Rollout {
            id: text("id")?,
            status,
            started_at: text("startedAt").unwrap_or_default(),
            finished_at: text("finishedAt"),
            steps,
            summary: text("summary"),
        })
    }

    pub fn running(&self) -> bool {
        self.status == RolloutStatus::Running
    }
}

/// The leading digits of a part, as JavaScript's `parseInt(part, 10) || 0` reads it.
fn leading_number(part: &str) -> u64 {
    part.chars().take_while(char::is_ascii_digit).collect::<String>().parse().unwrap_or(0)
}

/// Compares dotted versions numerically on `major.minor.patch`; a leading `v` is dropped and a tag ignored. Fleet keeps three
/// copies of this that differ in small ways (the plan's notes); one covers all of them.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let parts = |v: &str| -> Vec<u64> { v.strip_prefix('v').unwrap_or(v).split(['.', '+', '-']).take(3).map(leading_number).collect() };
    let (pa, pb) = (parts(a), parts(b));
    (0..3).map(|i| pa.get(i).unwrap_or(&0).cmp(pb.get(i).unwrap_or(&0))).find(|o| o.is_ne()).unwrap_or(Ordering::Equal)
}

/// True when `a` is a later release than `b`; false when either is missing (`isNewerVersion`).
pub fn is_newer_version(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) if !a.is_empty() && !b.is_empty() => compare_versions(a, b) == Ordering::Greater,
        _ => false,
    }
}

/// Where a version's notes are on GitHub (`releaseNotesUrl`).
pub fn release_notes_url(version: &str) -> String {
    format!("https://github.com/pgermishuys/weave-fleet/releases/tag/v{}", version.trim_start_matches('v'))
}

/// Sessions a restart would cut off: working, or stopped on a question or a permission ask (still in their turn).
pub fn busy_sessions(working: u32, needs_you: u32) -> u32 {
    working + needs_you
}

/// The version each machine last reported to this device, so an unreachable machine still says what it ran
/// (`fleet.machineVersions`). The settings file keeps it as JSON.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LastSeen(pub BTreeMap<String, String>);

impl LastSeen {
    /// Anything that isn't a text entry is dropped (`readLastSeenVersions`).
    pub fn parse(json: &str) -> LastSeen {
        let parsed: Option<Value> = serde_json::from_str(json).ok();
        LastSeen(
            parsed
                .as_ref()
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .collect(),
        )
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.0).unwrap_or_else(|_| "{}".into())
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    /// Notes what machines reported now; true when something changed and the file should be written again.
    pub fn note<'a>(&mut self, reported: impl IntoIterator<Item = (&'a str, &'a str)>) -> bool {
        let mut changed = false;
        for (key, version) in reported.into_iter().filter(|(_, v)| !v.is_empty()) {
            if self.get(key) != Some(version) {
                self.0.insert(key.to_string(), version.to_string());
                changed = true;
            }
        }
        changed
    }
}
