//! A document or page the agent rendered, read from a tool's output (`lib/visual-payload.ts`, and the "Visual canvases" part of
//! `ActivityStream.vue`). Plain logic, no Rune: the Document tab and the tool card's box draw from it.
use crate::fleet::canvases::{VisualPayload, VisualType};
use crate::view::conversation::{Message, Part};
use serde_json::Value;

/// A visual that arrives this close to the session opening still counts as live (`LIVE_TOLERANCE_MS`).
pub const LIVE_TOLERANCE_MS: i64 = 5_000;

/// `parseVisualPayload`: the output is a JSON object with `$type` `html` or `markdown` and a text `content`; a `title` is kept
/// when it is text. Any other key is dropped, which also covers `__proto__` and `constructor`, as Fleet strips them.
pub fn parse_visual_payload(raw: &str) -> Option<VisualPayload> {
    // Most output isn't JSON; look at its first character before paying for a parse.
    if !raw.trim_start().starts_with('{') {
        return None;
    }
    let Value::Object(object) = serde_json::from_str::<Value>(raw).ok()? else { return None };
    let kind = match object.get("$type")?.as_str()? {
        "html" => VisualType::Html,
        "markdown" => VisualType::Markdown,
        _ => return None,
    };
    let content = object.get("content")?.as_str()?.to_string();
    let title = object.get("title").and_then(Value::as_str).map(str::to_string);
    Some(VisualPayload { kind, title, source_path: None, content })
}

/// The word after a Document's title (`KIND_LABELS`).
pub fn kind_label(kind: VisualType) -> &'static str {
    match kind {
        VisualType::Markdown => "Document",
        VisualType::Html => "HTML",
    }
}

/// One visual of the conversation, with the call it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationVisual {
    pub tool_id: String,
    pub created_at: Option<i64>,
    pub payload: VisualPayload,
}

/// Every visual in the conversation's tool outputs, oldest first, as (tool id, when the message was made, payload) (`conversationVisuals`).
pub fn visual_refs(messages: &[Message]) -> impl Iterator<Item = (&str, Option<i64>, &VisualPayload)> {
    messages.iter().flat_map(|m| {
        m.parts.iter().filter_map(|p| match p {
            Part::Tool { id, body, .. } => body.visual.as_ref().map(|v| (id.as_str(), m.meta.created_at, v)),
            _ => None,
        })
    })
}

pub fn conversation_visuals(messages: &[Message]) -> Vec<ConversationVisual> {
    visual_refs(messages)
        .map(|(tool_id, created_at, payload)| ConversationVisual { tool_id: tool_id.to_string(), created_at, payload: payload.clone() })
        .collect()
}

/// Whether a visual opens as a tab by itself: it was made after the conversation opened here, give or take 5 s. History
/// loaded with the session never does (`isLive`).
pub fn is_live(created_at: Option<i64>, mounted_at: i64) -> bool {
    created_at.unwrap_or(0) >= mounted_at - LIVE_TOLERANCE_MS
}
