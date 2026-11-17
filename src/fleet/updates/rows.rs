//! The words and rows of Settings → Updates & about (`lib/fleet-updates.ts`): what each state is called, what a machine card
//! says along its bottom, the release strip, the rollout's narrative and outcome, the status bar chip and the notice for a
//! Fleet that updated itself. Every function is pure; the page (pieces V, W) only draws what these return.
use super::{
    LastUpdate, MachineKind, MachineUpdate, Rollout, RolloutStatus, RolloutStep, Status, StepState, Trigger, UpdateRule, is_newer_version,
};
use crate::conv::time::{civil, clock};
use crate::fleet::work::parse_iso_ms;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Ok,
    Warn,
    Info,
    Bad,
    Muted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Current,
    Available,
    Downloading,
    Ready,
    Busy,
    Managed,
    Unreachable,
    Error,
    Unchecked,
    Unsupported,
    Checking,
    /// An update that didn't start there and was undone.
    RolledBack,
    /// Part of a rollout under way.
    Pending,
    Waiting,
    Restarting,
    Updated,
    Skipped,
    NoReturn,
    /// The desktop app can't install it itself: it links to the download.
    Manual,
}

/// What a row says about its Fleet's update (`FleetUpdateState`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateState {
    pub kind: Kind,
    pub label: String,
    pub tone: Tone,
    /// The version it would move to, when an update is waiting or on its way.
    pub target: Option<String>,
}

fn state(kind: Kind, label: impl Into<String>, tone: Tone, target: Option<String>) -> UpdateState {
    UpdateState { kind, label: label.into(), tone, target }
}

/// What `describe_machine_update` reads of a machine (`FleetUpdateInput`).
pub struct MachineInput<'a> {
    pub version: Option<&'a str>,
    /// None from a Fleet older than the field.
    pub update: Option<&'a MachineUpdate>,
    /// Sessions in a turn on that machine, including ones waiting for an answer (`busy_sessions`).
    pub working: u32,
    pub reachable: bool,
}

pub(super) fn plural(count: u32, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// The state of a Fleet reached through `GET /api/machine` (`describeMachineUpdate`).
pub fn describe_machine_update(input: &MachineInput) -> UpdateState {
    let MachineInput { version, update, working, reachable } = *input;
    if !reachable {
        return state(Kind::Unreachable, "can't be reached", Tone::Muted, None);
    }
    let Some(update) = update else { return state(Kind::Unsupported, "too old to report updates", Tone::Muted, None) };
    if update.status == Status::Managed {
        return state(Kind::Managed, "managed by the desktop app", Tone::Muted, None);
    }
    let newer = |v: &Option<String>| v.clone().filter(|v| is_newer_version(Some(v), version));
    let (staged, latest) = (newer(&update.staged_version), newer(&update.latest_version));

    // Its launcher put the old version back; it won't fetch that release again, so nothing is waiting.
    if let Some(undone) = update.rolled_back.as_ref().filter(|u| staged.is_none() && is_newer_version(Some(&u.to), version)) {
        let label = format!("{} didn't start here ({}) · went back to {}", undone.to, undone.reason, version.unwrap_or(&undone.from));
        return state(Kind::RolledBack, label, Tone::Bad, None);
    }
    if update.status == Status::Downloading {
        let label = format!("downloading {}…", latest.as_deref().unwrap_or("the update"));
        return state(Kind::Downloading, label, Tone::Info, latest);
    }
    if staged.is_some() || update.status == Status::Staged {
        let target = staged.or(latest);
        let ready = format!("{} downloaded", target.as_deref().unwrap_or("the update"));
        return if working > 0 {
            let sessions = format!("{} working or waiting for you", plural(working, "session", "sessions"));
            state(Kind::Busy, format!("{ready} · busy: {sessions}"), Tone::Warn, target)
        } else {
            state(Kind::Ready, format!("{ready} · waiting for a restart"), Tone::Warn, target)
        };
    }
    if let (Status::Available, Some(latest)) = (update.status, &latest) {
        return state(Kind::Available, format!("{latest} is out, not downloaded yet"), Tone::Warn, Some(latest.clone()));
    }
    match update.status {
        Status::Error => state(Kind::Error, "couldn't check for updates", Tone::Bad, None),
        Status::Unknown => state(Kind::Unchecked, "not checked yet", Tone::Muted, None),
        _ => state(Kind::Current, "up to date", Tone::Ok, None),
    }
}

/// Whether an update is on its way or waiting for this Fleet: what the nav badge counts (`hasUpdateWaiting`).
pub fn has_update_waiting(s: &UpdateState) -> bool {
    matches!(s.kind, Kind::Available | Kind::Downloading | Kind::Ready | Kind::Busy | Kind::Manual)
}

/// What a row offers for restarting into the update it has: a button, or the reason it can't (`RestartOffer`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestartOffer {
    Button { label: String, disabled: bool },
    Unavailable { reason: String },
}

/// `canRestart` and its reason come from the machine; a Fleet older than the restart sends neither. A busy remote machine
/// still offers it: the rollout waits for its sessions. This machine can't wait, so its button waits for them to end.
pub fn restart_offer(s: &UpdateState, update: Option<&MachineUpdate>, is_home: bool) -> Option<RestartOffer> {
    if !matches!(s.kind, Kind::Ready | Kind::Busy) {
        return None;
    }
    let update = update?;
    let Some(can) = update.can_restart else {
        return Some(RestartOffer::Unavailable { reason: "too old to restart itself · restart it by hand".into() });
    };
    if !can {
        return Some(RestartOffer::Unavailable {
            reason: update.restart_unavailable_reason.clone().unwrap_or_else(|| "can't restart itself".into()),
        });
    }
    let into = format!("Restart into {}", s.target.as_deref().unwrap_or("the update"));
    Some(match (is_home, s.kind == Kind::Busy) {
        (true, busy) => RestartOffer::Button { label: into, disabled: busy },
        (false, true) => RestartOffer::Button { label: "Restart when idle".into(), disabled: false },
        (false, false) => RestartOffer::Button { label: into, disabled: false },
    })
}

/// How a rule is chosen for a row, and what it falls back to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleChoice {
    pub effective: UpdateRule,
    pub overridden: Option<UpdateRule>,
}

/// One machine in Updates & about (`FleetUpdateRow`; the desktop app's row is piece V's).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateRow {
    pub key: String,
    pub name: String,
    pub kind: MachineKind,
    pub os: Option<String>,
    /// The version it runs; None when it won't say.
    pub version: Option<String>,
    /// The newest release it knows of.
    pub latest: Option<String>,
    /// "this machine" or its address.
    pub where_: String,
    pub is_live: bool,
    pub state: UpdateState,
    pub restart: Option<RestartOffer>,
    /// Home is the Fleet this page belongs to; it restarts only from its own row.
    pub is_home: bool,
    /// None where there is none to set: a Fleet too old to take one.
    pub rule: Option<RuleChoice>,
    pub working: Option<u32>,
}

/// "Linux", "macOS", "Windows" from what `GET /api/machine` calls them (`osLabel`).
pub fn os_label(os: &str) -> Option<String> {
    match os {
        "" => None,
        "linux" => Some("Linux".into()),
        "macos" => Some("macOS".into()),
        "windows" => Some("Windows".into()),
        other => Some(other.into()),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub latest: Option<String>,
    pub waiting: usize,
    pub downloaded: usize,
    pub unreachable: usize,
}

/// The newest release any Fleet knows of, and how many have an update waiting or already downloaded (`summarizeRows`).
pub fn summarize_rows(rows: &[UpdateRow]) -> Summary {
    let mut latest: Option<String> = None;
    for row in rows {
        for candidate in [&row.latest, &row.state.target].into_iter().flatten() {
            if latest.as_deref().is_none_or(|l| is_newer_version(Some(candidate), Some(l))) {
                latest = Some(candidate.clone());
            }
        }
    }
    let count = |f: &dyn Fn(&UpdateRow) -> bool| rows.iter().filter(|r| f(r)).count();
    Summary {
        latest,
        waiting: count(&|r| has_update_waiting(&r.state)),
        downloaded: count(&|r| matches!(r.state.kind, Kind::Ready | Kind::Busy)),
        unreachable: count(&|r| r.state.kind == Kind::Unreachable),
    }
}

/// The state a row shows while a rollout has it in hand (`describeRolloutStep`).
pub fn describe_rollout_step(step: &RolloutStep) -> UpdateState {
    let to = step.to_version.clone();
    let detail = |fallback: &str| step.detail.clone().unwrap_or_else(|| fallback.to_string());
    match step.state {
        StepState::Checking => state(Kind::Checking, "checking…", Tone::Info, to),
        StepState::Waiting => state(Kind::Waiting, detail("waiting for its sessions to finish"), Tone::Warn, to),
        StepState::Restarting => state(Kind::Restarting, detail("restarting…"), Tone::Info, to),
        StepState::Done => {
            state(Kind::Updated, format!("updated to {} · healthy", to.as_deref().unwrap_or("the new version")), Tone::Ok, None)
        }
        StepState::RolledBack => {
            state(Kind::RolledBack, format!("{} · the rollout stopped here", detail("went back to its old version")), Tone::Bad, None)
        }
        StepState::NoReturn => state(Kind::NoReturn, format!("{} · the rollout stopped here", detail("didn't come back")), Tone::Bad, None),
        StepState::Skipped => state(Kind::Skipped, format!("skipped · {}", detail("left alone")), Tone::Muted, None),
        StepState::Pending => state(Kind::Pending, "waiting its turn", Tone::Muted, to),
    }
}

/// The line under the buttons once a rollout has ended: what it did, or why it stopped. None while it runs.
pub fn describe_rollout_outcome(rollout: Option<&Rollout>) -> Option<(String, Tone)> {
    let rollout = rollout.filter(|r| !r.running())?;
    let names = |s: StepState| -> Vec<&str> { rollout.steps.iter().filter(|x| x.state == s).map(|x| x.name.as_str()).collect() };
    let (done, skipped, left) = (names(StepState::Done), names(StepState::Skipped), names(StepState::Pending));
    if rollout.status == RolloutStatus::Done {
        let text = if done.is_empty() { "Nothing needed updating.".to_string() } else { format!("Updated {}.", done.join(" and ")) };
        return Some((if skipped.is_empty() { text } else { format!("{text} Skipped {}.", skipped.join(", ")) }, Tone::Ok));
    }
    let why = rollout.summary.clone().unwrap_or_else(|| "The rollout stopped.".into());
    let text = if left.is_empty() {
        why
    } else {
        format!("{why} {} {} left alone.", left.join(" and "), if left.len() == 1 { "was" } else { "were" })
    };
    Some((text, if rollout.status == RolloutStatus::Stopped { Tone::Bad } else { Tone::Muted }))
}

/// How long after a Fleet updated itself the home page still says so.
const UPDATED_NOTICE_MS: i64 = 3 * 86_400_000;

/// The notice for a Fleet that has updated (`describeUpdatedNotice`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdatedNotice {
    pub id: String,
    pub title: String,
    pub body: String,
    pub version: String,
    pub trigger: Trigger,
}

/// "falcon updated to 0.59.1", once per machine and version. None when it isn't news. `offset_secs` is the local zone, for the clock.
pub fn describe_updated_notice(
    machine_key: &str,
    name: &str,
    running: Option<&str>,
    last: Option<&LastUpdate>,
    now_ms: i64,
    offset_secs: i64,
) -> Option<UpdatedNotice> {
    let (last, running) = (last?, running?);
    if last.to.trim_start_matches('v') != running.trim_start_matches('v') {
        return None;
    }
    let at = parse_iso_ms(&last.at)?;
    if now_ms - at > UPDATED_NOTICE_MS {
        return None;
    }
    let how = match last.trigger {
        Trigger::Idle => "when nothing was working",
        Trigger::Overnight => "overnight",
        Trigger::Manual => "when you restarted it",
        Trigger::Rollout => "by Update machines now",
    };
    let (_, _, _, h, m) = civil(at, offset_secs);
    Some(UpdatedNotice {
        id: format!("fleet-updated:{machine_key}:{}", last.to),
        title: format!("{name} updated to {}", last.to),
        body: format!("At {}, {how}.", clock(h, m)),
        version: last.to.clone(),
        trigger: last.trigger,
    })
}

fn finished(s: StepState) -> bool {
    matches!(s, StepState::Done | StepState::RolledBack | StepState::NoReturn | StepState::Skipped)
}

pub(super) fn active(s: StepState) -> bool {
    matches!(s, StepState::Checking | StepState::Waiting | StepState::Restarting)
}

/// The one status bar chip for machine updates: how many machines can update, or where a rollout is. None when neither
/// (`describeUpdateChip`). Counts machines only; the desktop app's own update has its own notice.
pub fn describe_update_chip(waiting: usize, rollout: Option<&Rollout>) -> Option<String> {
    if let Some(r) = rollout.filter(|r| r.running() && !r.steps.is_empty()) {
        let done = r.steps.iter().filter(|s| finished(s.state)).count();
        return Some(format!("Updating machines · {} of {}", (done + 1).min(r.steps.len()), r.steps.len()));
    }
    (waiting > 0).then(|| format!("↑ {waiting} {} can update", if waiting == 1 { "machine" } else { "machines" }))
}

pub(super) fn active_step(rollout: Option<&Rollout>) -> Option<&RolloutStep> {
    rollout.filter(|r| r.running())?.steps.iter().find(|s| active(s.state))
}

/// What the machine card's bottom line says (`VersionLine`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionLine {
    /// "Fleet 0.59.0", or "Fleet" when the machine won't say.
    pub version: String,
    /// The version it would move to; None when it isn't moving.
    pub target: Option<String>,
    pub text: String,
    pub tone: Tone,
    /// Whether the per-machine rule menu belongs on the line: the machine reports a rule and can be reached.
    pub rule_menu: bool,
}

/// What a machine that downloaded an update does by itself under its rule.
fn self_update_text(rule: UpdateRule, working: u32, s: &UpdateState) -> String {
    match rule {
        UpdateRule::Ask => "waiting for you to restart it".into(),
        UpdateRule::Overnight => "updates itself tonight, 02:00–05:00".into(),
        UpdateRule::Idle if s.kind == Kind::Busy && working > 0 => {
            format!(
                "updates itself when {}",
                if working == 1 { "its session finishes".to_string() } else { format!("its {working} sessions finish") }
            )
        }
        UpdateRule::Idle => "updates itself in a minute".into(),
    }
}

/// The line along the bottom of a machine card in Updates & about › Machines (`describeVersionLine`).
pub fn describe_version_line(row: &UpdateRow, rollout: Option<&Rollout>) -> VersionLine {
    let s = &row.state;
    let running = rollout.filter(|r| r.running());
    let step = running.and_then(|r| r.steps.iter().find(|c| c.machine_id == row.key));
    let base = |version: Option<&str>, target: Option<String>, text: String, tone: Tone| VersionLine {
        version: version.map_or("Fleet".to_string(), |v| format!("Fleet {v}")),
        target,
        text,
        tone,
        rule_menu: row.rule.is_some() && s.kind != Kind::Unreachable,
    };
    let version = row.version.as_deref();

    if s.kind == Kind::Unreachable {
        let text = version.map_or("can't be reached".to_string(), |v| format!("can't be reached · last seen on {v}"));
        return base(version, None, text, Tone::Muted);
    }
    if let (Some(step), Some(rollout)) = (step, running) {
        let target = step.to_version.clone().or_else(|| s.target.clone());
        return match step.state {
            StepState::Waiting => {
                let detail = step.detail.clone().unwrap_or_else(|| {
                    if step.waiting_sessions > 0 {
                        format!("waiting for {} to finish", plural(step.waiting_sessions, "session", "sessions"))
                    } else {
                        "waiting for its sessions to finish".into()
                    }
                });
                base(version, target, format!("Updating: {detail}"), Tone::Warn)
            }
            StepState::Restarting => {
                base(version, target, format!("Updating: {}", step.detail.as_deref().unwrap_or("restarting…")), Tone::Info)
            }
            StepState::Checking => base(version, target, "Updating: checking…".into(), Tone::Info),
            StepState::Pending => {
                // The machine before it in the queue: the one being handled now, or another still waiting its turn.
                let at = rollout.steps.iter().position(|c| std::ptr::eq(c, step)).unwrap_or(0);
                let before = at.checked_sub(1).map(|i| &rollout.steps[i]);
                let ahead = before.filter(|b| b.state == StepState::Pending || active(b.state)).or_else(|| active_step(Some(rollout)));
                let text = ahead.map_or("Updating: waiting its turn".to_string(), |a| format!("Updating: next, after {}", a.name));
                base(version, target, text, Tone::Info)
            }
            StepState::Done => {
                let shown = describe_rollout_step(step);
                base(step.to_version.as_deref().or(version), None, shown.label, Tone::Ok)
            }
            _ => {
                let shown = describe_rollout_step(step);
                base(version, None, shown.label, shown.tone)
            }
        };
    }
    if matches!(s.kind, Kind::Ready | Kind::Busy) {
        let downloaded = format!("{} downloaded", s.target.as_deref().unwrap_or("the update"));
        if let Some(RestartOffer::Unavailable { reason }) = &row.restart {
            return base(version, s.target.clone(), format!("{downloaded} · {reason}"), s.tone);
        }
        return match row.rule {
            None => base(version, s.target.clone(), s.label.clone(), s.tone),
            Some(rule) => base(
                version,
                s.target.clone(),
                format!("{downloaded} · {}", self_update_text(rule.effective, row.working.unwrap_or(0), s)),
                s.tone,
            ),
        };
    }
    base(version, s.target.clone(), s.label.clone(), s.tone)
}

/// The newest `checkedAt` any machine reported, or None when none has checked (`newestCheckedAt`).
pub fn newest_checked_at<'a>(times: impl IntoIterator<Item = Option<&'a str>>) -> Option<String> {
    times.into_iter().flatten().filter_map(|t| Some((parse_iso_ms(t)?, t))).max_by_key(|(ms, _)| *ms).map(|(_, t)| t.to_string())
}

/// Machines other than home that can restart into what they downloaded (`readyMachines`).
pub fn ready_machines(rows: &[UpdateRow]) -> usize {
    rows.iter().filter(|r| !r.is_home && matches!(r.restart, Some(RestartOffer::Button { .. }))).count()
}
