//! What the context ring's popover says, with nothing of Rune in it: the headline, the size line, the meter, the
//! chart's bars, the rows, the usage limits, the error and the foot. Ports the computed values of `ContextRing.vue`
//! (`headline`, `sizeLine`, `bars`, `chartCaption`, `splitCall`, `cost`, `canCompact`, `compactTitle`, `compactHint`,
//! `error`, `limits`), so the view only draws them and a test can read them.
use super::format::{format_count, format_money, format_tokens};
use super::ring::{self, Case, Tone};
use crate::conv::retry::format_when;
use crate::fleet::context::{ContextTurn, ContextUsage};
use crate::fleet::usage::{Status, Window, is_near_limit, percent_label, window_label};

/// What the popover needs besides the context: the session's row, the Compact now request and the harness's windows.
#[derive(Clone, Debug, Default)]
pub struct Facts<'a> {
    /// `totalCost` of the session's row.
    pub cost: f64,
    /// `capabilities.canCompact`; only `Some(false)` turns the button off.
    pub can_compact: Option<bool>,
    pub compact_disabled_reason: Option<&'a str>,
    /// A Compact now Fleet hasn't answered yet.
    pub requesting: bool,
    /// Why the last Compact now was refused.
    pub request_error: Option<&'a str>,
    /// The harness's windows, those that haven't reset (`currentWindows`).
    pub windows: Vec<Window>,
    pub now: i64,
    pub offset_secs: i64,
    /// The bar the pointer is over.
    pub hovered: Option<usize>,
}

/// The size bar under the headline (with a window).
#[derive(Clone, Debug, PartialEq)]
pub struct Meter {
    /// 0 to 100.
    pub fill: f64,
    /// Where the harness compacts, 0 to 100.
    pub tick: Option<f64>,
    /// The legend's right end: "compacts at ~84%" or the window.
    pub legend: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    /// 4 to 100, of the chart's height.
    pub height: f64,
    pub last: bool,
    pub after_compaction: bool,
    /// "Turn 2 · 76k": the caption while the bar is hovered, and its label.
    pub describe: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chart {
    pub caption: String,
    pub bars: Vec<Bar>,
    /// The dashed line, where the harness compacts, 0 to 100.
    pub limit_line: Option<f64>,
}

/// One line of the rows grid: the label, the value and the muted words after it.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub label: &'static str,
    pub value: String,
    pub muted: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Limit {
    pub label: String,
    pub value: String,
    /// 0 to 100.
    pub fill: i64,
    pub tone: LimitTone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitTone {
    Ok,
    Warn,
    Danger,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Details {
    pub tone: Tone,
    pub case: Case,
    pub compacting: bool,
    pub headline: String,
    /// The small "%" after the headline.
    pub percent_sign: bool,
    pub state_label: &'static str,
    pub size_line: String,
    pub meter: Option<Meter>,
    pub chart: Option<Chart>,
    pub lines: Vec<Line>,
    pub limits: Vec<Limit>,
    pub error: Option<String>,
    pub hint: &'static str,
    pub can_compact: bool,
    pub compact_title: String,
    pub compact_label: &'static str,
}

/// "Turn 2 · 76k · after a compaction".
pub fn describe_turn(turn: &ContextTurn, index: usize) -> String {
    format!(
        "Turn {} \u{b7} {}{}",
        index + 1,
        format_tokens(Some(turn.used as f64)),
        if turn.after_compaction { " \u{b7} after a compaction" } else { "" }
    )
}

/// `sizeLine`.
pub fn size_line(c: &ContextUsage) -> String {
    let model = c.model_id.as_ref().map(|m| format!(" \u{b7} {m}")).unwrap_or_default();
    let n = |v: u64| format_count(Some(v as f64));
    match (c.used, c.limit) {
        (None, None) => format!("Window unknown{model}"),
        (None, Some(limit)) => format!("{} token window{model}", n(limit)),
        (Some(used), None) => format!("{} tokens{model}", n(used)),
        (Some(used), Some(limit)) => format!("{} of {} tokens{model}", n(used), n(limit)),
    }
}

/// `headline`: the percent, the token count when the window isn't known, "\u{2014}" before the size is.
pub fn headline(c: &ContextUsage) -> String {
    if c.used.is_none() {
        return "\u{2014}".into();
    }
    match ring::percent(c) {
        Some(p) => p.to_string(),
        None => format_tokens(c.used.map(|u| u as f64)),
    }
}

/// `bars`, scaled to the window, or to the largest turn when the window isn't known.
pub fn chart(c: &ContextUsage, hovered: Option<usize>) -> Option<Chart> {
    if c.turns.is_empty() {
        return None;
    }
    let scale = c.limit.unwrap_or_else(|| c.turns.iter().map(|t| t.used).max().unwrap_or(1).max(1)) as f64;
    let last = c.turns.len() - 1;
    let bars: Vec<Bar> = c
        .turns
        .iter()
        .enumerate()
        .map(|(i, t)| Bar {
            height: (t.used as f64 / scale * 100.0).clamp(4.0, 100.0),
            last: i == last,
            after_compaction: t.after_compaction,
            describe: describe_turn(t, i),
        })
        .collect();
    let caption = match hovered.and_then(|h| bars.get(h)) {
        Some(bar) => bar.describe.clone(),
        None => format!("{} {}", bars.len(), if bars.len() == 1 { "turn" } else { "turns" }),
    };
    Some(Chart { caption, bars, limit_line: ring::compacts_at_percent(c) })
}

/// The rows: the last call's split, how long before it compacts, what the session cost.
pub fn lines(c: &ContextUsage, cost: f64) -> Vec<Line> {
    let n = |v: u64| format_count(Some(v as f64));
    let mut out = Vec::new();
    if let Some(call) = c.last_call.as_ref().filter(|call| !call.total_only) {
        out.push(Line { label: "Read from cache", value: n(call.cache_read), muted: None });
        out.push(Line { label: "New input", value: n(call.input + call.cache_write), muted: None });
        out.push(Line {
            label: "Output",
            value: n(call.output),
            muted: (call.reasoning > 0).then(|| format!("\u{b7} {} reasoning", n(call.reasoning))),
        });
    }
    if let Some(left) = ring::turns_before_compaction(c, 5) {
        let (value, muted) = match left {
            0 => ("Probably next turn".to_string(), None),
            l if l > 25 => ("25+ turns".to_string(), Some("at this pace".to_string())),
            l => (format!("About {l} {}", if l == 1 { "turn" } else { "turns" }), Some("at this pace".to_string())),
        };
        out.push(Line { label: "Before it compacts", value, muted });
    }
    if cost > 0.0 {
        out.push(Line { label: "Estimated spend", value: format_money(Some(cost)), muted: None });
    }
    out
}

/// `limits`: the harness's windows under the context.
pub fn limits(windows: &[Window], now: i64, offset_secs: i64) -> Vec<Limit> {
    windows
        .iter()
        .map(|w| {
            let reset = w.resets_at.map(|at| format!("resets {}", format_when(at, now, offset_secs)));
            Limit {
                label: window_label(&w.window),
                value: [percent_label(w), reset].into_iter().flatten().collect::<Vec<_>>().join(" \u{b7} "),
                fill: if w.status == Status::Rejected { 100 } else { (w.utilization.unwrap_or(0.0) * 100.0).round() as i64 },
                tone: if w.status == Status::Rejected {
                    LimitTone::Danger
                } else if is_near_limit(w) {
                    LimitTone::Warn
                } else {
                    LimitTone::Ok
                },
            }
        })
        .collect()
}

pub fn details(c: &ContextUsage, f: &Facts) -> Details {
    let compacting = c.compacting || f.requesting;
    let percent = ring::percent(c);
    let tone = ring::tone(percent);
    let meter = c.limit.map(|limit| {
        let tick = ring::compacts_at_percent(c);
        Meter {
            fill: percent.unwrap_or(0) as f64,
            tick,
            legend: tick.map_or_else(|| format_tokens(Some(limit as f64)), |t| format!("compacts at ~{}%", t.round())),
        }
    });
    let can_compact = f.can_compact != Some(false) && !compacting;
    Details {
        tone,
        case: ring::case(c, f.requesting),
        compacting,
        headline: headline(c),
        percent_sign: percent.is_some(),
        state_label: ring::state_label(c, f.requesting),
        size_line: size_line(c),
        meter,
        chart: chart(c, f.hovered),
        lines: lines(c, f.cost),
        limits: limits(&f.windows, f.now, f.offset_secs),
        error: f.request_error.map(str::to_string).or_else(|| c.compaction_error.clone()),
        hint: if tone == Tone::Ok || percent.is_none() {
            "The agent compacts on its own when it gets close."
        } else {
            "Compacting summarises earlier turns so the agent can keep going."
        },
        can_compact,
        compact_title: if compacting {
            "The context is being compacted.".into()
        } else {
            f.compact_disabled_reason.unwrap_or("Summarise the conversation so far, so the agent can keep going.").into()
        },
        compact_label: if compacting { "Compacting\u{2026}" } else { "Compact now" },
    }
}
