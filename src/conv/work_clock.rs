//! What the background strip shows and when it changes (`BackgroundStrip.vue`, `BackgroundWorkRow.vue`,
//! `composables/use-running-work.ts`, `lib/running-work.ts`). No Rune in here.
//!
//! The strip lists a session's running work and what ended in the last 30 seconds, counted from when this window first
//! saw it end (not from the server's `endedAt`, so a server clock that runs ahead or behind can't hide or pin a row).
use super::time::{MONTHS, civil, clock};
use super::working::format_duration;
use crate::fleet::work::{RunningWorkItem, WorkKind, is_running, parse_iso_ms, work_elapsed_ms};
use std::collections::{HashMap, HashSet};

/// `FINISHED_VISIBLE_MS`: how long a finished item stays with its result before it drops off.
pub const FINISHED_VISIBLE_MS: i64 = 30_000;

/// What the strip remembers between frames: whether it is collapsed (`weave:background-strip-collapsed`; kept for the
/// run, not yet across restarts: checklist row 4.4) and when each item was seen to end.
#[derive(Default, Debug)]
pub struct WorkStrip {
    pub collapsed: bool,
    /// The strip's measured width, for its narrow layout; 0 until its first layout.
    pub width: f32,
    /// Items whose Stop is on its way, by Fleet's item id (`stopping`): shared by the strip and the status bar's list.
    pub stopping: HashSet<String>,
    /// Why a Stop was refused, under the row, until the next press (`stopError`).
    pub stop_errors: HashMap<String, String>,
    /// Rows whose output is open, with what each has read (piece L).
    pub outputs: HashMap<String, super::work_output::Reader>,
    /// Rows whose Details are open.
    pub details: HashSet<String>,
    /// The session Open left, for the back link of a sub-agent's own session (row 4.31).
    pub opened_from: Option<String>,
    pub next_epoch: u64,
    seen_running: HashSet<String>,
    ended_seen_at: HashMap<String, i64>,
    /// What the last frame showed, so the poll knows when a redraw is due.
    shown: Vec<String>,
}

impl WorkStrip {
    /// `noteEnds`: an item that ends while this window watches gets its result shown from now. Work that had already
    /// ended when first seen keeps the server's own end time.
    pub fn observe(&mut self, items: &[RunningWorkItem], now: i64) {
        for item in items {
            if is_running(item) {
                self.seen_running.insert(item.id.clone());
            } else if self.seen_running.contains(&item.id) {
                self.ended_seen_at.entry(item.id.clone()).or_insert(now);
            }
        }
    }

    /// `visibleUntil`: when a finished item drops off. `None` when its end can't be read and was never seen.
    fn visible_until(&self, item: &RunningWorkItem) -> Option<i64> {
        let seen = self.ended_seen_at.get(&item.id).copied().or_else(|| item.ended_at.as_deref().and_then(parse_iso_ms))?;
        Some(seen + FINISHED_VISIBLE_MS)
    }

    /// `isRecentlyEnded`.
    fn recently_ended(&self, item: &RunningWorkItem, now: i64) -> bool {
        !is_running(item) && self.visible_until(item).is_some_and(|until| until > now)
    }

    /// `visible`: what is running, then what just ended; each in the list's order (oldest start first).
    pub fn visible<'a>(&self, items: &'a [RunningWorkItem], now: i64) -> Vec<&'a RunningWorkItem> {
        let running = items.iter().filter(|i| is_running(i));
        let finished = items.iter().filter(|i| self.recently_ended(i, now));
        running.chain(finished).collect()
    }

    /// The header's words: `2 running · 1 finished`.
    pub fn summary(&self, items: &[RunningWorkItem], now: i64) -> String {
        let running = items.iter().filter(|i| is_running(i)).count();
        let finished = items.iter().filter(|i| self.recently_ended(i, now)).count();
        let mut parts = Vec::new();
        if running > 0 {
            parts.push(format!("{running} running"));
        }
        if finished > 0 {
            parts.push(format!("{finished} finished"));
        }
        parts.join(" \u{b7} ")
    }

    /// How long until something the strip shows reads differently: a second in a running item's first minute, then the
    /// next whole minute, and the moment a finished item drops. `None` while nothing shows, so the clock rests.
    pub fn next_change_ms(&self, items: &[RunningWorkItem], now: i64) -> Option<i64> {
        self.visible(items, now)
            .into_iter()
            .filter_map(|item| {
                if is_running(item) {
                    let elapsed = work_elapsed_ms(item, now);
                    let step = if elapsed < 60_000 { 1_000 } else { 60_000 };
                    Some(step - elapsed % step)
                } else {
                    self.visible_until(item).map(|until| until - now)
                }
            })
            .min()
    }

    /// Records what the strip shows now; true when it differs from the last frame.
    pub fn changed(&mut self, items: &[RunningWorkItem], now: i64) -> bool {
        let mut shown = vec![self.collapsed.to_string()];
        shown.extend(self.visible(items, now).into_iter().map(|i| format!("{}|{}|{}", i.id, is_running(i), time_text(i, now))));
        std::mem::replace(&mut self.shown, shown) != self.shown
    }
}

/// The text of a row's "what" (`what`): a shell's command; "agent · what it was asked" for a sub-agent or task; the
/// label else the title for the rest.
pub fn what_text(item: &RunningWorkItem) -> String {
    let label = item.label.as_deref();
    match item.kind {
        WorkKind::Shell => label.unwrap_or(&item.title).to_string(),
        WorkKind::Subagent | WorkKind::Task => match label {
            Some(l) if l != item.title => format!("{} \u{b7} {l}", item.title),
            _ => label.unwrap_or(&item.title).to_string(),
        },
        WorkKind::Monitor => label.unwrap_or(&item.title).to_string(),
    }
}

/// The row's right edge: how long it has run, or how long ago it ended (`time`).
pub fn time_text(item: &RunningWorkItem, now: i64) -> String {
    if is_running(item) {
        return format_duration(work_elapsed_ms(item, now));
    }
    match item.ended_at.as_deref().and_then(parse_iso_ms) {
        Some(ended) => relative(now - ended),
        None => String::new(),
    }
}

/// `formatRelative` in words, for what ended a moment ago: "just now", "4 minutes ago", "3 hours ago".
fn relative(diff_ms: i64) -> String {
    let span = diff_ms.abs();
    let (value, unit) = match span {
        ..60_000 => return "just now".to_string(),
        60_000..3_600_000 => (span / 60_000, "minute"),
        3_600_000..86_400_000 => (span / 3_600_000, "hour"),
        _ => (span / 86_400_000, "day"),
    };
    let words = format!("{value} {unit}{}", if value == 1 { "" } else { "s" });
    if diff_ms >= 0 { format!("{words} ago") } else { format!("in {words}") }
}

/// `formatDuration(ms)` with seconds, for the tooltip: "45 s", "3 min 20 s", "1 h 05 min".
fn exact(ms: i64) -> String {
    let seconds = ms.max(0) / 1000;
    if (60..3600).contains(&seconds) {
        let (m, s) = (seconds / 60, seconds % 60);
        return if s == 0 { format!("{m} min") } else { format!("{m} min {s:02} s") };
    }
    format_duration(ms)
}

/// `formatDateTime(at, { seconds: true })` in en-US: "Oct 4, 10:03:12 AM", with the year when it isn't this one.
fn date_time(at: i64, now: i64, offset_secs: i64) -> String {
    let (y, mo, d, h, mi) = civil(at, offset_secs);
    let secs = (at + offset_secs * 1000).div_euclid(1000).rem_euclid(60);
    let year = if y == civil(now, offset_secs).0 { String::new() } else { format!(", {y}") };
    let time = clock(h, mi).replacen(&format!(":{mi:02}"), &format!(":{mi:02}:{secs:02}"), 1);
    format!("{} {d}{year}, {time}", MONTHS[mo])
}

/// The tooltip on the time (`timeTitle`).
pub fn time_title(item: &RunningWorkItem, now: i64, offset_secs: i64) -> String {
    let started = parse_iso_ms(&item.started_at).map(|at| date_time(at, now, offset_secs)).unwrap_or_default();
    if is_running(item) { format!("Started {started}") } else { format!("Ran for {}, from {started}", exact(work_elapsed_ms(item, now))) }
}
