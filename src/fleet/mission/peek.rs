//! The peek's "just now" (`lib/mission-peek.ts`): the last few things an agent did, read from its latest messages.
use crate::fleet::model::{Msg, PartData};
use crate::view::conversation::Role;
use serde_json::Value;

/// How many events the peek shows.
pub const PEEK_COUNT: usize = 6;

const TARGET_FIELDS: [&str; 8] = ["command", "pattern", "filePath", "file_path", "path", "url", "description", "query"];
const MAX_LENGTH: usize = 90;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeekEvent {
    /// `<message id>:<part index>`.
    pub key: String,
    /// When the message holding it was written (ms), if it said.
    pub at: Option<i64>,
    pub text: String,
}

/// The first line, trimmed, cut to 90 characters with a `…`.
fn first_line(text: &str) -> String {
    let line = text.split('\n').next().unwrap_or("").trim();
    if line.chars().count() <= MAX_LENGTH {
        return line.to_string();
    }
    let kept: String = line.chars().take(MAX_LENGTH - 1).collect();
    format!("{kept}…")
}

fn tool_line(name: &str, input: &Value) -> String {
    let field = TARGET_FIELDS.iter().find_map(|f| input.get(*f).and_then(Value::as_str).filter(|s| !s.trim().is_empty()));
    let name = name.to_lowercase();
    match field.map(first_line).filter(|t| !t.is_empty()) {
        Some(target) => format!("{name} · {target}"),
        None => name,
    }
}

/// `peekEvents`: the assistant's tool calls and replies, oldest first, the newest `count`. The user's words are left out.
pub fn peek_events(messages: &[Msg], count: usize) -> Vec<PeekEvent> {
    let mut events = Vec::new();
    for message in messages.iter().filter(|m| m.role == Role::Assistant) {
        for (index, part) in message.parts.iter().enumerate() {
            let key = format!("{}:{index}", message.id);
            let text = match part {
                PartData::Tool { tool, state, .. } if !tool.is_empty() => tool_line(tool, state.get("input").unwrap_or(&Value::Null)),
                PartData::Text { text, .. } if !text.trim().is_empty() => first_line(text),
                _ => continue,
            };
            events.push(PeekEvent { key, at: message.created_at, text });
        }
    }
    let from = events.len().saturating_sub(count);
    events.split_off(from)
}
