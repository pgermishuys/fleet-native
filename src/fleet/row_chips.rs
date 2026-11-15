//! What a session's row shows besides its title and word (`SessionItem.vue`, `ProgressRing.vue`, `PrBadge.vue`,
//! `lib/session-progress.ts`): the progress summary and when it shows, the running chip's words and the pull request
//! badge's state. Pure: no Rune, no I/O. The badge's data (the smart-links store) is the session header's; until that
//! exists a row carries a `PrBadge` only from mock data.
use super::row_status::Tone;
use serde_json::Value;

/// `SessionProgressSummary`: how far along a session is, as the server counts it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Progress {
    /// What the counts count: "todos" today.
    pub kind: String,
    pub done: u32,
    pub total: u32,
    /// The item being worked on, or the next one.
    pub current: Option<String>,
}

/// A non-negative whole number, as JSON `isCount` reads it (`1.5`, `-1` and strings are not counts).
fn count(v: &Value, key: &str) -> Option<u32> {
    v.get(key)?.as_u64().map(|n| n.min(u32::MAX as u64) as u32)
}

/// `parseProgressSummary`: the session's id and its summary, or `None` for one that lacks an id or a kind or has a count
/// that isn't a whole number from zero.
pub fn parse_summary(v: &Value) -> Option<(String, Progress)> {
    let id = v.get("sessionId")?.as_str().filter(|s| !s.is_empty())?;
    Some((id.to_string(), parse_row_progress(v)?))
}

/// The summary a list item carries in its `progress` field: the same shape, whose id is the item's own.
pub fn parse_row_progress(v: &Value) -> Option<Progress> {
    let kind = v.get("kind")?.as_str()?.to_string();
    Some(Progress {
        kind,
        done: count(v, "done")?,
        total: count(v, "total")?,
        current: v.get("current").and_then(Value::as_str).map(str::to_string),
    })
}

/// `sameProgressSummary`: whether two summaries draw the same row.
pub fn same_progress(a: Option<&Progress>, b: Option<&Progress>) -> bool {
    a == b
}

/// What a row does with its progress (`progress`, `showProgressCount`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgressView<'a> {
    pub progress: &'a Progress,
    /// Only a working row's count replaces its age; a quiet row with unfinished items keeps the ring beside its age.
    pub count: bool,
}

/// `None` when the row shows no progress: no items, or a quiet session that finished its list (its age comes back).
pub fn progress_view(progress: Option<&Progress>, tone: Tone) -> Option<ProgressView<'_>> {
    let progress = progress.filter(|p| p.total > 0)?;
    if tone == Tone::Quiet && progress.done >= progress.total {
        return None;
    }
    Some(ProgressView { progress, count: tone == Tone::Working })
}

/// The ring's tooltip: `3 of 7 done`, then `. Now: <current>` when the summary names one.
pub fn progress_description(p: &Progress) -> String {
    let counts = format!("{} of {} done", p.done, p.total);
    match p.current.as_deref().filter(|c| !c.is_empty()) {
        Some(current) => format!("{counts}. Now: {current}"),
        None => counts,
    }
}

/// `ProgressRing`'s numbers: the share filled (0 when there is nothing to do) and whether everything is done.
pub fn ring_state(done: u32, total: u32) -> (f32, bool) {
    let ratio = if total > 0 { (done as f32 / total as f32).min(1.0) } else { 0.0 };
    (ratio, total > 0 && done >= total)
}

/// `runningChipLabel`: the chip's tooltip, one thing or several.
pub fn running_label(count: u32) -> String {
    if count == 1 { "1 thing running in the background".into() } else { format!("{count} things running in the background") }
}

/// `PrState` (`lib/pr-state.ts`): what colour the badge takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrState {
    Open,
    Draft,
    Blocked,
    Merged,
    Closed,
}

/// `ChecksState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Checks {
    None,
    Passing,
    Failing,
    Pending,
}

/// `PrBadge`'s props: the pull request a session opened, once GitHub has answered for it.
#[derive(Clone, Debug, PartialEq)]
pub struct PrBadge {
    pub number: u32,
    pub state: PrState,
    pub checks: Checks,
    /// The tooltip, e.g. `Pull request #187 · 1 failing`.
    pub description: String,
}

impl PrBadge {
    /// The checks dot shows while the pull request is still alive (`showChecks`).
    pub fn shows_checks(&self) -> bool {
        self.checks != Checks::None && matches!(self.state, PrState::Open | PrState::Blocked | PrState::Draft)
    }
}
