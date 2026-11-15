//! Where a harness compacted the conversation, as the conversation shows it: a divider ("Context compacted · 181k → 34k
//! tokens") with the summary the model goes on from behind "Show summary". Ports `lib/compaction.ts` and `formatTokens`
//! (`lib/format/numbers.ts`). No Rune in here.
use super::output::thousands;
use crate::fleet::model::{Msg, PartData};
use std::collections::{HashMap, HashSet};

/// A token count, short: "950", "17.1k", "200k", "1.2M". One decimal under 100 of a unit, none from 100; a count that
/// would round up to the next unit moves to it ("1M", never "1,000k").
pub fn format_tokens(n: f64) -> String {
    if !n.is_finite() {
        return String::new();
    }
    let sign = if n < 0.0 { "-" } else { "" };
    let abs = n.abs();
    if abs.round() < 1000.0 {
        return format!("{sign}{}", abs.round());
    }
    for (size, suffix) in [(1e3, "k"), (1e6, "M"), (1e9, "B")] {
        let value = abs / size;
        let digits = usize::from(value < 99.95);
        let rounded: f64 = format!("{value:.digits$}").parse().unwrap_or(value);
        if rounded < 1000.0 || suffix == "B" {
            // `{}` prints the shortest form, so a trailing ".0" never appears.
            let whole = rounded.trunc();
            let shown = if rounded == whole { thousands(whole as usize) } else { format!("{rounded}") };
            return format!("{sign}{shown}{suffix}");
        }
    }
    String::new()
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompactionView {
    /// "auto" or "manual", when the harness says.
    pub trigger: Option<String>,
    pub tokens_before: Option<u32>,
    pub tokens_after: Option<u32>,
    pub summary: Option<String>,
}

/// "Context compacted · 181k → 34k tokens", or just "Context compacted" when the harness gives no sizes.
pub fn describe_compaction(c: &CompactionView) -> String {
    match (c.tokens_before, c.tokens_after) {
        (Some(before), Some(after)) => {
            format!("Context compacted \u{b7} {} \u{2192} {} tokens", format_tokens(f64::from(before)), format_tokens(f64::from(after)))
        }
        (Some(before), None) => format!("Context compacted \u{b7} {} tokens before", format_tokens(f64::from(before))),
        _ => "Context compacted".to_string(),
    }
}

/// The divider's tooltip, by what started the compaction.
pub fn compaction_title(trigger: Option<&str>) -> &'static str {
    match trigger {
        Some("auto") => "The harness compacted the conversation because its context was nearly full. The model goes on from the summary.",
        Some("manual") => "Compacted on request. The model goes on from the summary.",
        _ => "The model goes on from a summary of the conversation before this point.",
    }
}

fn compaction_of(m: &Msg) -> Option<&PartData> {
    m.parts.iter().find(|p| matches!(p, PartData::Compaction { .. }))
}

/// A summary message's text: what the compaction wrote.
fn summary_text(m: &Msg) -> String {
    let texts: Vec<&str> =
        m.parts.iter().filter_map(|p| if let PartData::Text { text, .. } = p { Some(text.as_str()) } else { None }).collect();
    texts.join("\n\n").trim().to_string()
}

/// What `fold_compaction_summaries` found.
#[derive(Debug, Default)]
pub struct Folded {
    /// The summary each divider gets, by the divider's message id.
    pub summaries: HashMap<String, String>,
    /// The summary messages, which aren't shown.
    pub hidden: HashSet<String>,
}

/// Folds the summaries a harness wrote as messages of their own into the divider before them, so the conversation shows
/// the compaction once. A summary with no divider before it (the divider is on an older page) stays a message.
pub fn fold_compaction_summaries(messages: &[Msg]) -> Folded {
    let mut folded = Folded::default();
    // The divider the next summary message belongs to: its id and whether it has a summary of its own.
    let mut divider: Option<(&str, bool)> = None;
    for m in messages {
        if let Some(PartData::Compaction { summary, .. }) = compaction_of(m) {
            divider = Some((&m.id, summary.is_some()));
            continue;
        }
        let Some((id, has_summary)) = divider.filter(|_| m.compaction_summary) else { continue };
        folded.hidden.insert(m.id.clone());
        let text = summary_text(m);
        if !has_summary && !text.is_empty() {
            folded.summaries.insert(id.to_string(), text);
        }
        divider = None;
    }
    folded
}
