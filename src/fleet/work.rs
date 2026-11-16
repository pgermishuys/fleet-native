//! Running work: what a session's agent left running (background shells, sub-agents, monitors,
//! tasks). weave-fleet's `client/src/lib/running-work.ts`. One item per piece of work, the same shape
//! in the snapshot's `runningWork` and in `work.started` / `work.updated` / `work.ended`. Every event
//! carries the whole item as it is now, so applying one is a replace by id. Pure: no Rune, no I/O.
//!
//! The strip that draws these is component 9; its state and the helpers it and the status bar use
//! are here.
use super::model::Msg;
use crate::view::conversation::Role;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkKind {
    Subagent,
    Shell,
    Monitor,
    Task,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkStatus {
    Pending,
    Running,
    Completed,
    Error,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndedReason {
    Completed,
    Error,
    Cancelled,
    Lost,
}

/// `RunningWorkItem`. Times stay the server's ISO text, as in Fleet; `started_ms` and the elapsed
/// helpers read them.
#[derive(Clone, Debug, PartialEq)]
pub struct RunningWorkItem {
    /// Fleet's id for the item; stop and output take this.
    pub id: String,
    /// The session whose agent started it.
    pub session_id: String,
    /// The harness's own handle (a shell id, a task id, a sub-agent's call id).
    pub work_id: String,
    pub kind: WorkKind,
    pub title: String,
    pub label: Option<String>,
    pub status: WorkStatus,
    /// The call that started it returned while the work runs on.
    pub background: bool,
    pub child_session_id: Option<String>,
    pub tool_call_id: Option<String>,
    pub can_stop: bool,
    pub can_read_output: bool,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub ended_reason: Option<EndedReason>,
    /// Its result in a few words, e.g. `exit 0`.
    pub detail: Option<String>,
}

fn non_empty(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

/// `toRunningWorkItem`: the item a payload describes, or `None` when it isn't one. The server leaves
/// nulls (and false) out of the JSON, so missing fields get their empty values; a kind or status this
/// client doesn't know reads as a task, running.
pub fn to_item(value: &Value) -> Option<RunningWorkItem> {
    let text = |k: &str| value.get(k).and_then(Value::as_str);
    let (id, session_id) = (text("id").filter(|s| !s.is_empty())?, text("sessionId").filter(|s| !s.is_empty())?);
    let kind = match text("kind") {
        Some("subagent") => WorkKind::Subagent,
        Some("shell") => WorkKind::Shell,
        Some("monitor") => WorkKind::Monitor,
        _ => WorkKind::Task,
    };
    let status = match text("status") {
        Some("pending") => WorkStatus::Pending,
        Some("completed") => WorkStatus::Completed,
        Some("error") => WorkStatus::Error,
        Some("cancelled") => WorkStatus::Cancelled,
        _ => WorkStatus::Running,
    };
    let ended_reason = match text("endedReason") {
        Some("completed") => Some(EndedReason::Completed),
        Some("error") => Some(EndedReason::Error),
        Some("cancelled") => Some(EndedReason::Cancelled),
        Some("lost") => Some(EndedReason::Lost),
        _ => None,
    };
    let flag = |k: &str| value.get(k) == Some(&Value::Bool(true));
    Some(RunningWorkItem {
        id: id.to_string(),
        session_id: session_id.to_string(),
        work_id: text("workId").unwrap_or_default().to_string(),
        kind,
        title: text("title").map_or_else(|| kind.wire().to_string(), str::to_string),
        label: non_empty(value, "label"),
        status,
        background: flag("background"),
        child_session_id: non_empty(value, "childSessionId"),
        tool_call_id: non_empty(value, "toolCallId"),
        can_stop: flag("canStop"),
        can_read_output: flag("canReadOutput"),
        started_at: text("startedAt").map_or_else(|| "1970-01-01T00:00:00.000Z".to_string(), str::to_string),
        ended_at: non_empty(value, "endedAt"),
        ended_reason,
        detail: non_empty(value, "detail"),
    })
}

/// `toRunningWorkItems`: the items a list payload holds, oldest first; anything that isn't one is dropped.
pub fn to_items(values: Option<&Value>) -> Vec<RunningWorkItem> {
    let mut items: Vec<RunningWorkItem> = values.and_then(Value::as_array).into_iter().flatten().filter_map(to_item).collect();
    sort(&mut items);
    items
}

impl WorkKind {
    pub fn wire(self) -> &'static str {
        match self {
            WorkKind::Subagent => "subagent",
            WorkKind::Shell => "shell",
            WorkKind::Monitor => "monitor",
            WorkKind::Task => "task",
        }
    }

    /// What the row calls the kind: Shell, Subagent, Monitor, Task (`workKindLabel`).
    pub fn label(self) -> &'static str {
        match self {
            WorkKind::Subagent => "Subagent",
            WorkKind::Shell => "Shell",
            WorkKind::Monitor => "Monitor",
            WorkKind::Task => "Task",
        }
    }
}

/// Whether the work is still going: pending or running, and not ended (`isWorkRunning`).
pub fn is_running(item: &RunningWorkItem) -> bool {
    matches!(item.status, WorkStatus::Pending | WorkStatus::Running) && item.ended_at.is_none()
}

/// Oldest first, then by id. A start time that can't be read sorts as `NaN` does in Fleet's
/// `Date.parse(a) - Date.parse(b) || …`: the comparison is 0, so the id decides.
pub fn sort(items: &mut [RunningWorkItem]) {
    items.sort_by(|a, b| match (started_ms(a), started_ms(b)) {
        (Some(x), Some(y)) if x != y => x.cmp(&y),
        _ => a.id.cmp(&b.id),
    });
}

/// `applyWorkItem`: the list with `item` in it, replacing the item with its id. Returns whether the
/// list changed. Work that ended stays ended: a late event that still says running (one sent before
/// the end, delivered after it) doesn't bring it back, as on the server.
pub fn apply_item(items: &mut Vec<RunningWorkItem>, item: RunningWorkItem) -> bool {
    let Some(at) = items.iter().position(|c| c.id == item.id) else {
        items.push(item);
        sort(items);
        return true;
    };
    if !is_running(&items[at]) && is_running(&item) || items[at] == item {
        return false;
    }
    items[at] = item;
    true
}

// ─── time ───────────────────────────────────────────────────────────────────

/// `Date.parse` for the ISO-8601 text the server writes (`2026-10-04T10:00:00Z`, with fraction or a
/// `+hh:mm` offset), in milliseconds since the epoch. `None` where Fleet gets `NaN`.
pub fn parse_iso_ms(text: &str) -> Option<i64> {
    let (date, rest) = text.split_once(['T', ' '])?;
    let mut d = date.splitn(3, '-');
    let (y, m, day): (i64, i64, i64) = (d.next()?.parse().ok()?, d.next()?.parse().ok()?, d.next()?.parse().ok()?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) {
        return None;
    }
    let (clock, offset_min) = match rest.find(['Z', 'z', '+']).or_else(|| rest.rfind('-')) {
        Some(i) if matches!(rest.as_bytes()[i], b'Z' | b'z') => (&rest[..i], 0),
        Some(i) => {
            let sign = if rest.as_bytes()[i] == b'+' { 1 } else { -1 };
            let (h, mi) = rest[i + 1..].split_once(':').unwrap_or((&rest[i + 1..], "0"));
            (&rest[..i], sign * (h.parse::<i64>().ok()? * 60 + mi.parse::<i64>().ok()?))
        }
        None => (rest, 0),
    };
    let mut c = clock.splitn(3, ':');
    let (h, mi): (i64, i64) = (c.next()?.parse().ok()?, c.next()?.parse().ok()?);
    let secs = c.next().unwrap_or("0");
    let (whole, frac) = secs.split_once('.').unwrap_or((secs, ""));
    let s: i64 = whole.parse().ok()?;
    let ms: i64 = format!("{frac:0<3}")[..3].parse().ok()?;
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 24 + h) * 60 + mi - offset_min) * 60_000 + s * 1000 + ms)
}

fn started_ms(item: &RunningWorkItem) -> Option<i64> {
    parse_iso_ms(&item.started_at)
}

/// A short duration: `58s` in the first minute, then whole minutes, `4m`, `1h 12m` (`formatElapsed`).
pub fn format_elapsed(ms: i64) -> String {
    let seconds = (ms / 1000).max(0);
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = minutes / 60;
    if minutes % 60 == 0 { format!("{hours}h") } else { format!("{hours}h {}m", minutes % 60) }
}

/// How long ago something ended: `just now` in the first minute, then `4m ago` (`formatAgo`).
pub fn format_ago(ms: i64) -> String {
    if ms < 60_000 { "just now".to_string() } else { format!("{} ago", format_elapsed(ms)) }
}

/// How often an elapsed time of `ms` needs to tick to stay right (`elapsedTickMs`).
pub fn elapsed_tick_ms(ms: i64) -> i64 {
    if ms < 60_000 { 1_000 } else { 60_000 }
}

/// How long it has run, or ran: from its start to its end, or to `now` (`workElapsedMs`).
pub fn work_elapsed_ms(item: &RunningWorkItem, now: i64) -> i64 {
    let Some(started) = started_ms(item) else { return 0 };
    let ended = item.ended_at.as_deref().and_then(parse_iso_ms).unwrap_or(now);
    (ended - started).max(0)
}

/// How the work ended, in a few words: its own detail (`exit 0`), else what ended it. `None` while it
/// runs (`workResult`).
pub fn work_result(item: &RunningWorkItem) -> Option<String> {
    if is_running(item) {
        return None;
    }
    if let Some(detail) = &item.detail {
        return Some(detail.clone());
    }
    let word = match item.ended_reason {
        Some(EndedReason::Error) => "failed",
        Some(EndedReason::Cancelled) => "stopped",
        Some(EndedReason::Lost) => "lost",
        Some(EndedReason::Completed) => "done",
        None => match item.status {
            WorkStatus::Error => "failed",
            WorkStatus::Cancelled => "stopped",
            _ => "done",
        },
    };
    Some(word.to_string())
}

/// Whether the work ended badly: failed, or lost when the harness went away (`workFailed`).
pub fn work_failed(item: &RunningWorkItem) -> bool {
    !is_running(item) && (item.status == WorkStatus::Error || matches!(item.ended_reason, Some(EndedReason::Error | EndedReason::Lost)))
}

/// When the running turn began: your last prompt that started it. A prompt sent into the running turn
/// (steered) is part of that turn, so it doesn't restart the clock (`turnStartedAt`).
pub fn turn_started_at(messages: &[Msg]) -> Option<i64> {
    messages.iter().rev().find(|m| m.role == Role::User && !m.steered).and_then(|m| m.created_at)
}
