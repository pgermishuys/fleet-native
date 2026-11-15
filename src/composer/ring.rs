//! The context ring's maths and wording, with nothing of Rune in it: how full the window is, the tone,
//! how many turns fit before the harness compacts, and the texts the ring and its tooltip use. Ports
//! `lib/context-usage.ts` (`contextPercent`, `contextTone`, `turnsBeforeCompaction`) and the computed
//! values of `ContextRing.vue` (`stateLabel`, `summary`, `ringTitle`, `fill`, `amber`).
use super::format::format_tokens;
use crate::fleet::context::{ContextTurn, ContextUsage};

/// Under 75% the meter stays quiet; from 75% it's getting full; from 90% nearly full.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Ok,
    Warn,
    Danger,
}

pub const WARN_PERCENT: u64 = 75;
pub const DANGER_PERCENT: u64 = 90;

/// `contextPercent`: how full the context is, 0 to 100, or `None` when the size or the window isn't known.
pub fn percent(context: &ContextUsage) -> Option<u64> {
    let (used, limit) = (context.used?, context.limit?);
    // JavaScript's Math.round rounds halves up; f64::round does the same for positive numbers.
    Some(((used as f64 / limit as f64 * 100.0).round() as u64).min(100))
}

/// `contextTone`.
pub fn tone(percent: Option<u64>) -> Tone {
    match percent {
        Some(p) if p >= DANGER_PERCENT => Tone::Danger,
        Some(p) if p >= WARN_PERCENT => Tone::Warn,
        _ => Tone::Ok,
    }
}

/// Amber from above 80%, strictly; the red of "nearly full" still wins at 90%.
pub fn amber(percent: Option<u64>) -> bool {
    percent.is_some_and(|p| p > 80)
}

/// The ring's arc, 0 to 1. An unknown size (right after a compaction) draws an empty ring, an unknown window a full one.
pub fn fill(context: &ContextUsage) -> f32 {
    match (context.used, context.limit) {
        (None, _) => 0.0,
        (Some(_), None) => 1.0,
        (Some(used), Some(limit)) => (used as f64 / limit as f64).min(1.0) as f32,
    }
}

/// Where the harness compacts, as a share of the window, for the tick on the bar.
pub fn compacts_at_percent(context: &ContextUsage) -> Option<f64> {
    Some((context.compacts_at? as f64 / context.limit? as f64 * 100.0).min(100.0))
}

/// `turnsBeforeCompaction`: about how many more turns fit before the harness compacts, from how much the context
/// grew over the last few turns since the last compaction. `None` when it can't tell.
pub fn turns_before_compaction(context: &ContextUsage, sample: usize) -> Option<u64> {
    let (used, compacts_at) = (context.used?, context.compacts_at?);
    let since = &context.turns[last_compaction_index(&context.turns)..];
    let recent = &since[since.len().saturating_sub(sample)..];
    if recent.len() < 2 {
        return None;
    }
    let growth = (recent[recent.len() - 1].used as f64 - recent[0].used as f64) / (recent.len() - 1) as f64;
    if growth <= 0.0 {
        return None;
    }
    Some(((compacts_at as f64 - used as f64) / growth).floor().max(0.0) as u64)
}

fn last_compaction_index(turns: &[ContextTurn]) -> usize {
    turns.iter().rposition(|t| t.after_compaction).unwrap_or(0)
}

/// The six cases the ring is drawn in (row 13.6), in the order the label tests them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    Compacting,
    /// `used` unknown: right after a compaction, or before it was measured.
    Unmeasured,
    /// Used known, window not.
    WindowUnknown,
    Danger,
    Warn,
    Ok,
}

/// `compacting` is the context's own flag or a compact request of ours in flight (piece H).
pub fn case(context: &ContextUsage, compacting: bool) -> Case {
    if compacting || context.compacting {
        return Case::Compacting;
    }
    if context.used.is_none() {
        return Case::Unmeasured;
    }
    let p = percent(context);
    if p.is_none() {
        return Case::WindowUnknown;
    }
    match tone(p) {
        Tone::Danger => Case::Danger,
        Tone::Warn => Case::Warn,
        Tone::Ok => Case::Ok,
    }
}

/// `stateLabel`.
pub fn state_label(context: &ContextUsage, compacting: bool) -> &'static str {
    match case(context, compacting) {
        Case::Compacting => "Compacting\u{2026}",
        Case::Unmeasured if context.compacted_at.is_some() => "Compacted",
        Case::Unmeasured => "Not measured yet",
        Case::WindowUnknown => "Window unknown",
        Case::Danger => "Nearly full",
        Case::Warn => "Getting full",
        Case::Ok => "Plenty of room",
    }
}

/// `summary`: what the ring says to a screen reader, and on hover when the window isn't known.
pub fn summary(context: &ContextUsage, compacting: bool) -> String {
    match case(context, compacting) {
        Case::Compacting => "Compacting the context\u{2026}".into(),
        Case::Unmeasured => "Context compacted. Its size shows after the next reply.".into(),
        Case::WindowUnknown => {
            format!("{} tokens in the context. The model's window isn't known.", format_tokens(context.used.map(|u| u as f64)))
        }
        _ => format!("{}% of the context window used", percent(context).unwrap_or(0)),
    }
}

/// `ringTitle`: the hover text, "Context: 38% of 200k tokens".
pub fn title(context: &ContextUsage, compacting: bool) -> String {
    match (percent(context), context.limit) {
        (Some(p), Some(limit)) => format!("Context: {p}% of {} tokens", format_tokens(Some(limit as f64))),
        _ => format!("{} \u{b7} details", summary(context, compacting)),
    }
}

/// The ring's `aria-label`.
pub fn label(context: &ContextUsage, compacting: bool) -> String {
    format!("{}. Details", summary(context, compacting))
}
