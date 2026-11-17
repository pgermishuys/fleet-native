//! What a page tab can say to the app (component 14, piece B; rows 14.30 and 14.31). Ports `lib/page-bridge.ts`: the
//! messages read from a page, the refusals the user hears, and the state kept for a page between its loads. Plain Rust.
use crate::composer::format::format_count;
use serde_json::{Value, json};
use std::collections::VecDeque;

pub const MAX_REPLY_LENGTH: usize = 64 * 1024;
/// Room for a page of many questions sending every answer at once; the server takes as much.
pub const MAX_CHOICE_LENGTH: usize = 16_000;
/// The longest summary the bar shows and the agent gets; a longer one is shortened, not refused.
pub const MAX_CHOICE_SUMMARY_LENGTH: usize = 200;
pub const MAX_STATE_LENGTH: usize = 256 * 1024;
const MAX_KEPT_PAGES: usize = 50;
/// The tallest a page in the conversation grows; past it, the page scrolls inside its frame.
pub const MAX_PAGE_HEIGHT: u32 = 2000;

#[derive(Clone, Debug, PartialEq)]
pub enum PageMessage {
    /// Text for the session's composer. Fleet never sends it: the user does.
    Reply { text: String },
    /// The page's own state, kept for the page and given back on its next hello.
    State { state: Value },
    /// The page has loaded; the answer is `fleet:page-state` with what was kept, or null.
    Hello,
    /// A pick for a page that waits for one (Fleet's bar sends it, the page never does).
    Choice { value: String, summary: String },
    /// A pick the app can't take, with why in plain words.
    ChoiceRefused { reason: String },
}

impl PageMessage {
    /// The message as it goes to a page.
    pub fn to_json(&self) -> Value {
        match self {
            Self::Reply { text } => json!({ "type": "fleet:page-reply", "text": text }),
            Self::State { state } => json!({ "type": "fleet:page-state", "state": state }),
            Self::Hello => json!({ "type": "fleet:page-hello" }),
            Self::Choice { value, summary } => json!({ "type": "fleet:page-choice", "value": value, "summary": summary }),
            Self::ChoiceRefused { reason } => json!({ "type": "fleet:page-choice-refused", "reason": reason }),
        }
    }
}

/// `readPageMessage`. Lengths count UTF-16 units, as JavaScript's `length` does.
pub fn read_page_message(data: &Value) -> Option<PageMessage> {
    let message = data.as_object()?;
    match message.get("type")?.as_str()? {
        "fleet:page-reply" => {
            let text = message.get("text")?.as_str()?;
            (!text.trim().is_empty() && utf16_len(text) <= MAX_REPLY_LENGTH).then(|| PageMessage::Reply { text: text.to_string() })
        }
        "fleet:page-state" => {
            let state = message.get("state")?;
            (state.to_string().encode_utf16().count() <= MAX_STATE_LENGTH).then(|| PageMessage::State { state: state.clone() })
        }
        "fleet:page-hello" => Some(PageMessage::Hello),
        "fleet:page-choice" => Some(read_choice(message.get("value"), message.get("summary"))),
        _ => None,
    }
}

fn read_choice(value: Option<&Value>, summary: Option<&Value>) -> PageMessage {
    let refused = |reason: String| PageMessage::ChoiceRefused { reason };
    let blank = match value {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => s.trim().is_empty(),
        _ => false,
    };
    if blank {
        return refused("The page's pick was empty, so there's nothing to send.".into());
    }
    let summary_ok = matches!(summary, None | Some(Value::Null) | Some(Value::String(_)));
    let Some(value) = value.and_then(Value::as_str).filter(|_| summary_ok) else {
        return refused("The page sent a pick Fleet can't read.".into());
    };
    let len = utf16_len(value);
    if len > MAX_CHOICE_LENGTH {
        return refused(format!(
            "The page's pick was too long to send: {} characters, and the most is {}.",
            format_count(Some(len as f64)),
            format_count(Some(MAX_CHOICE_LENGTH as f64))
        ));
    }
    let own = summary.and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    PageMessage::Choice { value: value.to_string(), summary: shorten_summary(own.unwrap_or_else(|| value.trim())) }
}

/// A page with many questions sums up every answer: past the limit, the summary ends in an ellipsis.
fn shorten_summary(summary: &str) -> String {
    let units: Vec<u16> = summary.encode_utf16().collect();
    if units.len() <= MAX_CHOICE_SUMMARY_LENGTH {
        return summary.to_string();
    }
    format!("{}…", String::from_utf16_lossy(&units[..MAX_CHOICE_SUMMARY_LENGTH - 1]))
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `readPageSize`: the height in a page's size report (MCP Apps' `ui/notifications/size-changed`), or `None`.
pub fn read_page_size(data: &Value) -> Option<u32> {
    let message = data.as_object()?;
    if message.get("jsonrpc")?.as_str()? != "2.0" || message.get("method")?.as_str()? != "ui/notifications/size-changed" {
        return None;
    }
    let height = message.get("params")?.get("height")?.as_f64().filter(|h| h.is_finite() && *h >= 0.0)?;
    Some(height.ceil().min(MAX_PAGE_HEIGHT as f64) as u32)
}

/// The state pages saved, kept for as long as the app is open so a page shown again (or reloaded) gets its answers
/// back. The oldest pages are dropped first once 50 are kept.
#[derive(Debug, Default)]
pub struct KeptStates {
    pages: VecDeque<(String, Value)>,
}

impl KeptStates {
    pub fn keep(&mut self, page_id: &str, state: Value) {
        self.pages.retain(|(id, _)| id != page_id);
        self.pages.push_back((page_id.to_string(), state));
        while self.pages.len() > MAX_KEPT_PAGES {
            self.pages.pop_front();
        }
    }

    pub fn kept(&self, page_id: &str) -> Value {
        self.pages.iter().find(|(id, _)| id == page_id).map(|(_, s)| s.clone()).unwrap_or(Value::Null)
    }

    /// `pageStateMessage`: the answer to a hello.
    pub fn state_message(&self, page_id: &str) -> PageMessage {
        PageMessage::State { state: self.kept(page_id) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(v: Value) -> Option<PageMessage> {
        read_page_message(&v)
    }

    #[test]
    fn reads_the_three_messages_a_page_can_send() {
        assert_eq!(read(json!({"type": "fleet:page-hello"})), Some(PageMessage::Hello));
        assert_eq!(read(json!({"type": "fleet:page-reply", "text": "# Re: Plan"})), Some(PageMessage::Reply { text: "# Re: Plan".into() }));
        let state = json!({"answers": {"a": 1}});
        assert_eq!(read(json!({"type": "fleet:page-state", "state": state})), Some(PageMessage::State { state }));
    }

    #[test]
    fn ignores_anything_else() {
        for data in [
            json!(null),
            json!("fleet:page-hello"),
            json!(42),
            json!({}),
            json!({"type": "hello", "fleet": 1}),
            json!({"type": "fleet:page-send", "text": "x"}),
        ] {
            assert_eq!(read(data.clone()), None, "{data}");
        }
    }

    #[test]
    fn needs_a_reply_with_text_within_the_limit() {
        assert_eq!(read(json!({"type": "fleet:page-reply"})), None);
        assert_eq!(read(json!({"type": "fleet:page-reply", "text": 7})), None);
        assert_eq!(read(json!({"type": "fleet:page-reply", "text": "  \n "})), None);
        assert_eq!(read(json!({"type": "fleet:page-reply", "text": "x".repeat(MAX_REPLY_LENGTH + 1)})), None);
        assert!(read(json!({"type": "fleet:page-reply", "text": "x".repeat(MAX_REPLY_LENGTH)})).is_some());
    }

    #[test]
    fn needs_state_that_is_json_within_the_limit() {
        // A cyclic or function state can't arrive over IPC at all (the page's JSON.stringify refuses it); the limit and null can.
        assert_eq!(read(json!({"type": "fleet:page-state", "state": "x".repeat(MAX_STATE_LENGTH)})), None);
        assert_eq!(read(json!({"type": "fleet:page-state", "state": null})), Some(PageMessage::State { state: Value::Null }));
        assert_eq!(read(json!({"type": "fleet:page-state"})), None);
    }

    fn choice(value: &str, summary: &str) -> Option<PageMessage> {
        Some(PageMessage::Choice { value: value.into(), summary: summary.into() })
    }

    fn refused(reason: &str) -> Option<PageMessage> {
        Some(PageMessage::ChoiceRefused { reason: reason.into() })
    }

    #[test]
    fn reads_a_pick_defaulting_the_summary_to_the_value() {
        assert_eq!(read(json!({"type": "fleet:page-choice", "value": "a", "summary": " Layout A "})), choice("a", "Layout A"));
        assert_eq!(read(json!({"type": "fleet:page-choice", "value": "b"})), choice("b", "b"));
        let Some(PageMessage::Choice { summary, .. }) = read(json!({"type": "fleet:page-choice", "value": "x".repeat(900), "summary": ""}))
        else {
            panic!("a pick")
        };
        assert_eq!(summary.encode_utf16().count(), 200);
    }

    #[test]
    fn reads_every_answer_on_a_page_of_many_questions_shortening_a_long_summary() {
        let answers: serde_json::Map<String, Value> =
            (1..=10).map(|i| (format!("question-{i}"), json!(format!("The answer to question {i}")))).collect();
        let value = Value::Object(answers.clone()).to_string();
        let summary = answers.iter().map(|(q, a)| format!("{q}: {}", a.as_str().unwrap())).collect::<Vec<_>>().join("; ");
        assert!(value.len() > 400 && summary.len() > MAX_CHOICE_SUMMARY_LENGTH);
        let Some(PageMessage::Choice { value: got, summary: short }) =
            read(json!({"type": "fleet:page-choice", "value": value, "summary": summary}))
        else {
            panic!("a pick")
        };
        assert_eq!(got, value);
        assert_eq!(short.chars().count(), MAX_CHOICE_SUMMARY_LENGTH);
        assert_eq!(short, format!("{}…", &summary[..MAX_CHOICE_SUMMARY_LENGTH - 1]));
    }

    #[test]
    fn takes_a_pick_of_several_thousand_characters() {
        let value = "x".repeat(5000);
        assert!(
            matches!(read(json!({"type": "fleet:page-choice", "value": value})), Some(PageMessage::Choice { value: v, .. }) if v == value)
        );
    }

    #[test]
    fn refuses_an_oversized_pick_saying_how_long_it_was_and_how_long_it_may_be() {
        assert_eq!(
            read(json!({"type": "fleet:page-choice", "value": "x".repeat(MAX_CHOICE_LENGTH + 2345)})),
            refused("The page's pick was too long to send: 18,345 characters, and the most is 16,000.")
        );
        assert!(matches!(
            read(json!({"type": "fleet:page-choice", "value": "x".repeat(MAX_CHOICE_LENGTH)})),
            Some(PageMessage::Choice { .. })
        ));
    }

    #[test]
    fn refuses_an_empty_or_mistyped_pick_in_plain_words_never_silently() {
        let empty = "The page's pick was empty, so there's nothing to send.";
        assert_eq!(read(json!({"type": "fleet:page-choice", "value": "  "})), refused(empty));
        assert_eq!(read(json!({"type": "fleet:page-choice"})), refused(empty));
        for data in [json!({"type": "fleet:page-choice", "value": 3}), json!({"type": "fleet:page-choice", "value": "a", "summary": 4})] {
            assert_eq!(read(data), refused("The page sent a pick Fleet can't read."));
        }
    }

    #[test]
    fn doesnt_take_a_refusal_from_the_page_itself() {
        assert_eq!(read(json!({"type": "fleet:page-choice-refused", "reason": "Pick Layout B"})), None);
    }

    #[test]
    fn gives_back_the_last_state_a_page_saved_and_null_for_a_page_with_none() {
        let mut kept = KeptStates::default();
        kept.keep("pg_a", json!({"answers": {"one": "1"}}));
        kept.keep("pg_a", json!({"answers": {"one": "2"}}));
        assert_eq!(kept.kept("pg_a"), json!({"answers": {"one": "2"}}));
        assert_eq!(kept.state_message("pg_b"), PageMessage::State { state: Value::Null });
    }

    #[test]
    fn drops_the_pages_saved_longest_ago_once_50_are_kept() {
        let mut kept = KeptStates::default();
        for i in 0..51 {
            kept.keep(&format!("pg_{i}"), json!(i));
        }
        assert_eq!(kept.kept("pg_0"), Value::Null);
        assert_eq!(kept.kept("pg_1"), json!(1));
        assert_eq!(kept.kept("pg_50"), json!(50));
    }

    fn size(params: Value) -> Value {
        json!({"jsonrpc": "2.0", "method": "ui/notifications/size-changed", "params": params})
    }

    #[test]
    fn reads_the_height_a_page_reports_rounded_up() {
        assert_eq!(read_page_size(&size(json!({"height": 312.4}))), Some(313));
        assert_eq!(read_page_size(&size(json!({"height": 0}))), Some(0));
    }

    #[test]
    fn caps_a_page_that_reports_more_than_the_tallest_frame() {
        assert_eq!(read_page_size(&size(json!({"height": 99_999}))), Some(MAX_PAGE_HEIGHT));
    }

    #[test]
    fn page_size_ignores_anything_else() {
        assert_eq!(read_page_size(&json!({"type": "fleet:page-hello"})), None);
        assert_eq!(read_page_size(&json!({"method": "ui/notifications/size-changed", "params": {"height": 10}})), None);
        for height in [json!("10"), json!(-1), json!(null)] {
            assert_eq!(read_page_size(&size(json!({ "height": height }))), None);
        }
        assert_eq!(read_page_size(&size(json!(null))), None);
        assert_eq!(read_page_size(&json!(null)), None);
    }
}
