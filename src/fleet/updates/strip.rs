//! The strip at the top of Updates & about › Machines (`lib/fleet-updates.ts`): the release line while nothing rolls out, and the
//! narrative while a rollout runs. Pure.
use super::rows::{active_step, plural};
use super::{Rollout, StepState, Summary};
use crate::fleet::row_status::format_date;
use crate::fleet::work::parse_iso_ms;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseKind {
    Available,
    Current,
    None,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseLine {
    pub kind: ReleaseKind,
    /// The bold start of the line; empty when there is none.
    pub lead: String,
    pub text: String,
}

/// "4 minutes ago", "3 hours ago", "yesterday", "last week", then the date: `formatRelative(at, { now })` in words. Fleet gets the
/// words from `Intl.RelativeTimeFormat` with `numeric: "auto"`; English is all this app has.
pub fn format_relative_words(at: i64, now: i64, offset_secs: i64) -> String {
    const MINUTE: i64 = 60_000;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    const WEEK: i64 = 7 * DAY;
    let diff = now - at;
    let span = diff.abs();
    if span < MINUTE {
        return "just now".into();
    }
    if span >= 8 * WEEK {
        return format_date(at, now, offset_secs);
    }
    let (value, unit) = if span < HOUR {
        (span / MINUTE, "minute")
    } else if span < DAY {
        (span / HOUR, "hour")
    } else if span < WEEK {
        (span / DAY, "day")
    } else {
        (span / WEEK, "week")
    };
    match (diff >= 0, value, unit) {
        (true, 1, "day") => "yesterday".into(),
        (false, 1, "day") => "tomorrow".into(),
        (true, 1, "week") => "last week".into(),
        (false, 1, "week") => "next week".into(),
        (past, n, unit) => {
            let words = format!("{n} {unit}{}", if n == 1 { "" } else { "s" });
            if past { format!("{words} ago") } else { format!("in {words}") }
        }
    }
}

/// The strip at the top of Updates & about › Machines when no rollout runs (`describeReleaseLine`). `checked_at` is the newest
/// check any machine made.
pub fn describe_release_line(summary: &Summary, checked_at: Option<&str>, now_ms: i64, offset_secs: i64) -> ReleaseLine {
    let Some(latest) = &summary.latest else {
        return ReleaseLine { kind: ReleaseKind::None, lead: String::new(), text: "Fleet hasn't found a release yet.".into() };
    };
    let (waiting, downloaded) = (summary.waiting, summary.downloaded);
    if waiting == 0 {
        let checked = checked_at
            .and_then(parse_iso_ms)
            .map_or(String::new(), |at| format!(" Checked {}.", format_relative_words(at, now_ms, offset_secs)));
        return ReleaseLine {
            kind: ReleaseKind::Current,
            lead: String::new(),
            text: format!("Every machine that can be reached is on Fleet {latest}.{checked}"),
        };
    }
    let lead = format!("Fleet {latest} is out.");
    let machines = if waiting == 1 { "machine" } else { "machines" };
    let text = if downloaded == 0 {
        format!("{waiting} {machines} can update; {} downloaded yet.", if waiting == 1 { "it isn't" } else { "they aren't" })
    } else if downloaded == waiting {
        if waiting == 1 {
            "1 machine has it downloaded and switches when it restarts.".into()
        } else {
            format!("{waiting} machines have it downloaded and switch when they restart.")
        }
    } else {
        format!(
            "{downloaded} of {waiting} machines that can update {} it downloaded and switch when they restart.",
            if downloaded == 1 { "has" } else { "have" }
        )
    };
    ReleaseLine { kind: ReleaseKind::Available, lead, text }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Narrative {
    pub title: String,
    /// Who is being handled now (for "Stop after falcon").
    pub active: Option<String>,
    pub text: String,
}

/// The strip while a rollout runs: its headline, who is being handled, and a sentence on each machine
/// (`describeRolloutNarrative`). `unreachable` holds the machines that can't be reached now: the rollout will skip them.
pub fn describe_rollout_narrative(rollout: &Rollout, unreachable: &HashSet<String>) -> Narrative {
    let to = rollout.steps.iter().find_map(|s| s.to_version.as_deref());
    let title = format!("Updating machines{}, one at a time.", to.map_or(String::new(), |t| format!(" to {t}")));
    let mut sentences: Vec<String> = Vec::new();
    let mut live: Vec<String> = Vec::new();
    let mut first_pending = true;
    let flush = |live: &mut Vec<String>, sentences: &mut Vec<String>| {
        if !live.is_empty() {
            sentences.push(format!("{}.", live.join("; ")));
            live.clear();
        }
    };
    for step in &rollout.steps {
        let name = &step.name;
        match step.state {
            StepState::Waiting => live.push(if step.waiting_sessions > 0 {
                format!("{name} is waiting for {}", plural(step.waiting_sessions, "session", "sessions"))
            } else {
                format!("{name} is waiting for its sessions")
            }),
            StepState::Checking => live.push(format!("{name} is being checked")),
            StepState::Restarting => live.push(format!("{name} is restarting")),
            StepState::Pending if unreachable.contains(&step.machine_id) => {
                flush(&mut live, &mut sentences);
                sentences.push(format!("{name} can't be reached, so it'll be skipped."));
            }
            StepState::Pending => {
                live.push(format!("{name} is {}", if first_pending { "next" } else { "queued" }));
                first_pending = false;
            }
            StepState::Done => {
                flush(&mut live, &mut sentences);
                sentences.push(format!("{name} is done."));
            }
            StepState::Skipped => {
                flush(&mut live, &mut sentences);
                sentences.push(match step.detail.as_deref() {
                    Some(d) if d.to_lowercase().contains("reach") => format!("{name} can't be reached, so it's skipped."),
                    Some(d) if !d.is_empty() => format!("{name} is skipped: {d}."),
                    _ => format!("{name} is skipped."),
                });
            }
            StepState::RolledBack => {
                flush(&mut live, &mut sentences);
                sentences.push(format!("{name} went back to its old version."));
            }
            StepState::NoReturn => {
                flush(&mut live, &mut sentences);
                sentences.push(format!("{name} didn't come back."));
            }
        }
    }
    flush(&mut live, &mut sentences);
    Narrative { title, active: active_step(Some(rollout)).map(|s| s.name.clone()), text: sentences.join(" ") }
}
