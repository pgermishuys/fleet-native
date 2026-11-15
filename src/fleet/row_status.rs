//! What a session row says about its session: the word on the right, its tone, the glyph, how far a quiet row fades and
//! the compact age (`lib/session-row-status.ts`, `lib/format/time.ts`, `lib/retry-status.ts`,
//! `lib/session-interruption.ts`). Pure: no Rune, no I/O. Times are written as Fleet's en-US locale writes them
//! (`Oct 12, 2:07 PM`), as `conv::time` does.
use super::rows::{Failure, Interruption, Retrying, ScheduledRetry};
use super::work::parse_iso_ms;
use crate::conv::retry::{format_countdown, format_when};
use crate::conv::time::{DAY, MONTHS, civil, clock};
use crate::mock::{Session, Status};

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const WEEK: i64 = 7 * DAY;
/// From eight weeks on a compact age is a date, never `119w`.
const DATE_AFTER: i64 = 8 * WEEK;

/// How often the list re-reads the clock (`TICK_INTERVAL_MS` in `use-relative-time.ts`), in seconds.
pub const TICK_SECS: f64 = 30.0;

/// `toTimestamp` for text: digits are unix ms; a bare date is that day's local midnight; a string with no zone is UTC,
/// as SQLite writes it. `None` where Fleet gets `NaN`.
pub fn to_timestamp(text: &str, offset_secs: i64) -> Option<i64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if text.bytes().all(|b| b.is_ascii_digit()) {
        return text.parse::<i64>().ok().filter(|&ms| ms > 0);
    }
    let b = text.as_bytes();
    if b.len() == 10 && b[4] == b'-' && b[7] == b'-' {
        // Midnight UTC of that day, moved to the local midnight.
        return parse_iso_ms(&format!("{text}T00:00:00Z")).map(|ms| ms - offset_secs * 1000);
    }
    parse_iso_ms(text)
}

/// A unix-ms number as a time: zero and negative are "no time".
pub fn from_ms(ms: i64) -> Option<i64> {
    (ms > 0).then_some(ms)
}

/// "Oct 12" (`Oct 12, 2025` when it isn't this year): `formatDate`.
pub fn format_date(at: i64, now: i64, offset_secs: i64) -> String {
    let (y, mo, d, ..) = civil(at, offset_secs);
    let (ny, ..) = civil(now, offset_secs);
    if y == ny { format!("{} {d}", MONTHS[mo]) } else { format!("{} {d}, {y}", MONTHS[mo]) }
}

/// "Oct 12, 2:07 PM" (with the year when it isn't this year): `formatDateTime`, the tooltip of every relative time.
pub fn format_date_time(at: i64, now: i64, offset_secs: i64) -> String {
    let (y, mo, d, h, mi) = civil(at, offset_secs);
    let (ny, ..) = civil(now, offset_secs);
    if y == ny { format!("{} {d}, {}", MONTHS[mo], clock(h, mi)) } else { format!("{} {d}, {y}, {}", MONTHS[mo], clock(h, mi)) }
}

/// `formatRelative(at, { compact: true })`: `now`, `4m`, `3h`, `5d`, `2w`, then a date from eight weeks; a time ahead
/// reads `in 4m`. Whole units rounded down.
pub fn format_relative(at: i64, now: i64, offset_secs: i64) -> String {
    let diff = now - at;
    let span = diff.abs();
    if span < MINUTE {
        return "now".into();
    }
    if span >= DATE_AFTER {
        return format_date(at, now, offset_secs);
    }
    let (value, letter) = if span < HOUR {
        (span / MINUTE, 'm')
    } else if span < DAY {
        (span / HOUR, 'h')
    } else if span < WEEK {
        (span / DAY, 'd')
    } else {
        (span / WEEK, 'w')
    };
    if diff >= 0 { format!("{value}{letter}") } else { format!("in {value}{letter}") }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Attention,
    Working,
    Retry,
    Error,
    Quiet,
}

/// `SessionRowStatus`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RowStatus {
    /// Short text on the right of the row. Empty while the session works.
    pub label: String,
    pub tone: Tone,
    /// The state in words, for the glyph's tooltip.
    pub description: String,
    /// The label's tooltip: when it is a time, the exact one.
    pub title: Option<String>,
}

/// How far a quiet session's row fades back: 0 at full contrast, 1 after a day, 2 after three (`SessionRowDim`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dim {
    None,
    One,
    Two,
}

/// The glyph in a row's 8px slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Glyph {
    /// Nothing live: the empty slot that keeps titles on one edge.
    Quiet,
    /// A restart cut the turn off: the slot as a filled amber dot.
    Cut,
    Working,
    /// Working, and retrying: the same dots in amber.
    Retry,
    Waiting,
    Error,
}

/// What `sessionRowStatus` reads of a list item.
#[derive(Clone, Copy)]
pub struct RowItem<'a> {
    /// `sessionStatus`.
    pub status: &'a str,
    /// `activityStatus`.
    pub activity: &'a str,
    pub retrying: &'a Retrying,
    pub scheduled_retry: Option<&'a ScheduledRetry>,
    pub interrupted: Option<&'a Interruption>,
    pub failed: Option<&'a Failure>,
    /// `session.time.updated` and `created`, as unix ms.
    pub updated: Option<i64>,
    pub created: Option<i64>,
}

impl<'a> RowItem<'a> {
    /// The row of a session in the list. The glyph state of the open conversation outranks the list's own word (the
    /// stream is quicker than the poll); a lifecycle state (`stopped`, `completed`, `error`, `disconnected`) stands.
    pub fn of(session: &'a Session, now: i64) -> RowItem<'a> {
        let row = &session.row;
        let lifecycle = matches!(row.lifecycle.as_str(), "stopped" | "completed" | "error" | "disconnected");
        let status = if lifecycle {
            row.lifecycle.as_str()
        } else {
            match session.status {
                Status::Working => "active",
                Status::NeedsInput => "waiting_input",
                Status::Idle => "idle",
            }
        };
        let (updated, created) = match (from_ms(row.updated), session.ago_secs) {
            (Some(ms), _) => (Some(ms), from_ms(row.created)),
            (None, Some(ago)) => (Some(now - ago * 1000), None),
            (None, None) => (None, from_ms(row.created)),
        };
        RowItem {
            status,
            activity: if status == "active" { row.activity.as_str() } else { "" },
            retrying: &row.retrying,
            scheduled_retry: row.scheduled_retry.as_ref(),
            interrupted: row.interrupted.as_ref(),
            failed: row.failed.as_ref(),
            updated,
            created,
        }
    }
}

/// `LIVE_STATUSES`: where something is going on. Only these get a glyph on the row.
pub fn is_live(status: &str) -> bool {
    matches!(status, "active" | "waiting_input" | "error")
}

/// `sessionRowDim`: live sessions never dim; the rest fade by their last activity, from `updated`, else `created`.
pub fn row_dim(item: &RowItem, now: i64) -> Dim {
    if is_live(item.status) {
        return Dim::None;
    }
    let Some(at) = item.updated.or(item.created) else { return Dim::None };
    let age = now - at;
    if age >= 3 * DAY {
        Dim::Two
    } else if age >= DAY {
        Dim::One
    } else {
        Dim::None
    }
}

/// The glyph a row draws (`isLive`, `StatusGlyph`, `session-glyph-slot--cut`).
pub fn glyph(item: &RowItem) -> Glyph {
    match item.status {
        "active" if item.activity == "retry" => Glyph::Retry,
        "active" => Glyph::Working,
        "waiting_input" => Glyph::Waiting,
        "error" => Glyph::Error,
        _ if item.interrupted.is_some() => Glyph::Cut,
        _ => Glyph::Quiet,
    }
}

/// `retryShortLabel`: `Retry 3/10`, `Retry 3` or `Retrying`.
pub fn retry_short_label(retry: &Retrying) -> String {
    match (retry.attempt.filter(|&a| a != 0), retry.max_attempts.filter(|&m| m != 0)) {
        (None, _) => "Retrying".into(),
        (Some(a), Some(m)) => format!("Retry {a}/{m}"),
        (Some(a), None) => format!("Retry {a}"),
    }
}

/// `describeRetry`: `Retrying · attempt 3 of 10 · in 12 s · API overloaded (529)`, leaving out what the harness didn't say.
pub fn describe_retry(retry: &Retrying, now: i64) -> String {
    let mut parts = vec!["Retrying".to_string()];
    match (retry.attempt.filter(|&a| a != 0), retry.max_attempts.filter(|&m| m != 0)) {
        (Some(a), Some(m)) => parts.push(format!("attempt {a} of {m}")),
        (Some(a), None) => parts.push(format!("attempt {a}")),
        _ => {}
    }
    if let Some(next) = retry.next.as_deref().and_then(parse_iso_ms) {
        parts.push(format_countdown(next - now));
    }
    if let Some(message) = retry.message.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
        parts.push(message.to_string());
    }
    parts.join(" · ")
}

/// `limitName`.
fn limit_name(kind: &str) -> &'static str {
    match kind {
        "usage_limit" => "a usage limit",
        "rate_limit" => "a rate limit",
        _ => "an overloaded provider",
    }
}

/// `sessionRowStatus(item, now, machineAnswering)`.
pub fn row_status(item: &RowItem, now: i64, offset_secs: i64, machine_answering: bool) -> RowStatus {
    let said =
        |label: &str, tone: Tone, description: &str| RowStatus { label: label.into(), tone, description: description.into(), title: None };
    if !machine_answering && item.status == "active" {
        return said("Not answering", Tone::Quiet, "Its machine isn't answering. It was working when last heard.");
    }
    match item.status {
        "waiting_input" => said("Needs input", Tone::Attention, "Needs input"),
        "error" => said("Error", Tone::Error, "Error"),
        "active" if item.activity == "retry" => RowStatus {
            label: retry_short_label(item.retrying),
            tone: Tone::Retry,
            description: describe_retry(item.retrying, now),
            title: None,
        },
        "active" => said("", Tone::Working, if item.activity == "delegating" { "Delegating" } else { "Working" }),
        "completed" => said("Done", Tone::Quiet, "Done"),
        "disconnected" => said("Offline", Tone::Quiet, "Offline"),
        _ => {
            // A restart cut the last turn off, and nobody has carried on yet.
            if let Some(cut) = item.interrupted {
                let label = if cut.waiting { "Cut off · question lost" } else { "Cut off by a restart" };
                return said(label, Tone::Retry, label);
            }
            // A limit stopped its last turn and Fleet tries again by itself.
            if let Some(retry) = item.scheduled_retry {
                let due = to_timestamp(&retry.due_at, offset_secs);
                let clock = due.map(|d| format_when(d, now, offset_secs)).unwrap_or_default();
                let left = due.map(|d| format_countdown(d - now)).filter(|c| !c.is_empty()).unwrap_or_else(|| "now".into());
                return RowStatus {
                    label: format!("Retry {clock}"),
                    tone: Tone::Retry,
                    description: format!("Stopped by {}. Fleet tries again at {clock} ({left})", limit_name(&retry.kind)),
                    title: None,
                };
            }
            // The last turn stopped on an error and nobody has carried on: not the same as a quiet idle.
            if let Some(failed) = item.failed {
                let reason = failed.message.trim();
                let description = if reason.is_empty() { "Last turn failed".to_string() } else { format!("Last turn failed: {reason}") };
                return RowStatus { label: "Failed".into(), tone: Tone::Error, description, title: None };
            }
            let at = item.updated.or(item.created);
            RowStatus {
                label: at.map(|at| format_relative(at, now, offset_secs)).unwrap_or_default(),
                tone: Tone::Quiet,
                description: "Idle".into(),
                title: at.map(|at| format_date_time(at, now, offset_secs)).filter(|t| !t.is_empty()),
            }
        }
    }
}

/// `label || session.title?.trim() || "Untitled session"` (`displayTitle`): a workflow step's name wins over the title.
pub fn display_title(label: Option<&str>, title: &str) -> String {
    label.filter(|l| !l.is_empty()).map(str::to_string).unwrap_or_else(|| {
        let title = title.trim();
        if title.is_empty() { "Untitled session".into() } else { title.to_string() }
    })
}

/// The glyph's spoken label and tooltip (`StatusGlyph`'s `statusLabel`), for a status the row passes no words for.
pub fn glyph_label(status: &str) -> String {
    match status {
        "active" => "Working",
        "idle" => "Idle",
        "running" => "Running",
        "completed" => "Completed",
        "error" => "Error",
        "waiting_input" => "Waiting for input",
        "stopped" => "Stopped",
        "disconnected" => "Disconnected",
        other => other,
    }
    .into()
}
