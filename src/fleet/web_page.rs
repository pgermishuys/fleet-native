//! The page canvas's page (component 14, pieces A and C; rows 14.20, 14.21 and 14.34). Ports `lib/server-canvas.ts`
//! (`shownPage`, `pageAddress`) and the bar half of `lib/page-choice.ts` (`pageChoiceBar`).
use super::pages::{ChoiceStatus, PageChoice, read_page_choice};
use crate::conv::image_sniff::encode_component;
use serde_json::Value;

/// The page an agent showed: an HTML file Fleet copied and serves at `/pages/{pageId}/{entry}`. `shown_at` changes every
/// time the agent shows it again, so the tab loads it again.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShownPage {
    pub page_id: String,
    pub entry: String,
    pub source: String,
    pub shown_at: String,
    pub files: u64,
    pub bytes: u64,
    /// Links in the page that Fleet couldn't copy, in the words Fleet wrote them.
    pub warnings: Vec<String>,
    /// Shown in the bar instead of the source, for a page Fleet wrote itself.
    pub label: Option<String>,
    pub choice: Option<PageChoice>,
}

impl ShownPage {
    /// `fileName`: the last part of the source (either slash), else the entry.
    pub fn file_name(&self) -> &str {
        self.source.rsplit(['/', '\\']).next().filter(|n| !n.is_empty()).unwrap_or(&self.entry)
    }
}

/// `shownPage`: a page canvas's `state`, with anything missing or of the wrong type empty.
pub fn shown_page(state: &Value) -> ShownPage {
    let text = |key: &str| state.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
    let count = |key: &str| state.get(key).and_then(Value::as_f64).filter(|n| n.is_finite() && *n >= 0.0).map_or(0, |n| n as u64);
    let warnings = state
        .get("warnings")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |all| all.iter().filter_map(Value::as_str).map(str::to_string).collect());
    ShownPage {
        page_id: text("pageId"),
        entry: text("entry"),
        source: text("source"),
        shown_at: text("shownAt"),
        files: count("files"),
        bytes: count("bytes"),
        warnings,
        label: Some(text("label")).filter(|l| !l.is_empty()),
        choice: state.get("choice").and_then(read_page_choice),
    }
}

/// `pageAddress`: the page's path on Fleet, the id and each part of the file name escaped (`encodeURIComponent`).
pub fn page_address(page: &ShownPage) -> String {
    let entry: Vec<String> = page.entry.split('/').map(encode_component).collect();
    format!("/pages/{}/{}", encode_component(&page.page_id), entry.join("/"))
}

/// The page's whole address on the Fleet at `base` (`apiUrlOn`), or `None` for a canvas that names no page.
pub fn page_url(base: &str, page: &ShownPage) -> Option<String> {
    if page.page_id.is_empty() || page.entry.is_empty() {
        return None;
    }
    Some(format!("{}{}", base.trim_end_matches('/'), page_address(page)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarKind {
    Waiting,
    Proposed,
    /// The page sent a pick Fleet can't take; `text` says why and Send stays off.
    Refused,
    Answered,
    NotAnswered,
}

/// What the bar under the page says (`PageChoiceBarState`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceBar {
    pub kind: BarKind,
    pub text: String,
    /// The bold part, e.g. the picked summary.
    pub strong: Option<String>,
    pub after: String,
    /// ISO time, for the answered and unanswered bars.
    pub at: Option<String>,
}

/// A summary shortened with an ellipsis (`page_bridge`) ends the sentence already.
fn full_stop(summary: &str) -> &'static str {
    if summary.ends_with('\u{2026}') { "" } else { "." }
}

/// `pageChoiceBar`: from the canvas's choice, whether it is live, what the page proposed and why Fleet refused what the
/// page last sent, if it did.
pub fn page_choice_bar(choice: &PageChoice, live: bool, proposal: Option<&(String, String)>, refusal: &str) -> ChoiceBar {
    let bar = |kind, text: &str, strong: Option<&str>, after: &str, at: Option<&str>| ChoiceBar {
        kind,
        text: text.to_string(),
        strong: strong.map(str::to_string),
        after: after.to_string(),
        at: at.map(str::to_string),
    };
    if choice.status == ChoiceStatus::Answered {
        let strong = choice.summary.as_deref().or(choice.value.as_deref()).unwrap_or_default();
        return bar(BarKind::Answered, "Sent: ", Some(strong), full_stop(strong), Some(&choice.at));
    }
    if choice.status == ChoiceStatus::Waiting && live {
        if let Some((_, summary)) = proposal {
            return bar(BarKind::Proposed, "You picked ", Some(summary), full_stop(summary), None);
        }
        return if refusal.is_empty() {
            bar(BarKind::Waiting, "The agent is waiting for your pick.", None, "", None)
        } else {
            bar(BarKind::Refused, refusal, None, "", None)
        };
    }
    let words = match (choice.status, choice.reason.as_deref()) {
        (ChoiceStatus::NotAnswered, Some("skipped")) => "Not answered. You skipped it.",
        (ChoiceStatus::NotAnswered, Some("wrote instead")) => "Not answered. You wrote instead.",
        (ChoiceStatus::NotAnswered, Some("stopped")) => "Not answered. The turn stopped.",
        (ChoiceStatus::NotAnswered, Some("Fleet restarted")) => "Not answered. Fleet restarted.",
        (ChoiceStatus::NotAnswered, Some("timed out")) => "Not answered after 24 hours.",
        _ => "Not answered.",
    };
    bar(BarKind::NotAnswered, words, None, "", Some(&choice.at))
}

/// `pageChoiceRowTitle`: the row's title for a `fleet_page_show` call that waits for a pick, or `None` for any other call.
pub fn page_choice_row_title(input: &Value, status: Option<&str>, metadata: &Value) -> Option<String> {
    if input.get("waitForChoice").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let word = |v: Option<&Value>| v.and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
    match status {
        Some("error") => Some("Not answered".into()),
        Some("completed") => {
            let choice = metadata.get("choice").unwrap_or(&Value::Null);
            if choice.get("answered").and_then(Value::as_bool) == Some(true) {
                let picked = word(choice.get("summary")).or_else(|| match choice.get("value") {
                    Some(Value::String(s)) => Some(s.clone()),
                    Some(Value::Null) | None => Some(String::new()),
                    Some(other) => Some(other.to_string()),
                });
                return Some(format!("Picked {}", picked.unwrap_or_default()).trim().to_string());
            }
            let reason = word(choice.get("reason")).unwrap_or_default();
            let why = if reason == "skipped" { "you skipped it".to_string() } else { reason };
            Some(if why.is_empty() { "Not picked".into() } else { format!("Not picked \u{b7} {why}") })
        }
        _ => Some(match word(input.get("title")) {
            Some(title) => format!("Waiting for your pick \u{b7} {title}"),
            None => "Waiting for your pick".into(),
        }),
    }
}
