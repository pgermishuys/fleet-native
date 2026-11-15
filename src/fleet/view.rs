//! Boils the reducer's messages down to what the conversation draws (`view::conversation`).
use super::model::{Msg, PartData};
use crate::conv::card::tool_body;
use crate::conv::tools::{ToolStatus, get_tool, is_shell_row, is_write_call, tool_duration_ms};
use crate::view::conversation::{Message, Part, ToolState};
use serde_json::Value;

pub fn messages(from: &[Msg]) -> Vec<Message> {
    from.iter().filter(|m| !m.compaction_summary).map(message).collect()
}

/// What `Message::version` would say of `m` once boiled down, without copying its text: the way to
/// tell which of two thousand messages an event batch touched.
pub fn version(m: &Msg) -> (usize, usize) {
    let weight = |p: &PartData| match p {
        PartData::Text { text, .. } | PartData::Reasoning { text, .. } => text.len(),
        other => part(other).weight(),
    };
    (
        m.parts.len() + usize::from(turn_failure(m).is_some()),
        m.parts.iter().map(weight).sum::<usize>() + turn_failure(m).map_or(0, str::len),
    )
}

fn turn_failure(m: &Msg) -> Option<&str> {
    m.turn_error.as_ref()?.get("message")?.as_str()
}

pub fn message(m: &Msg) -> Message {
    let mut parts: Vec<Part> = m.parts.iter().map(part).collect();
    // The conversation has no failure row yet (the parts lane draws one); until then say it in words.
    if let Some(reason) = turn_failure(m) {
        parts.push(Part::text(format!("{}-failed", m.id), format!("The turn failed: {reason}")));
    }
    Message::new(m.id.clone(), m.role, parts)
}

fn part(p: &PartData) -> Part {
    match p {
        PartData::Text { id, text } => Part::Text { id: id.clone(), text: text.clone() },
        PartData::Reasoning { id, text, summary } => Part::Reasoning { id: id.clone(), text: text.clone(), summary: summary.clone() },
        PartData::File { id, mime, filename, url } => {
            Part::File { id: id.clone(), mime: mime.clone(), filename: filename.clone(), url: url.clone() }
        }
        PartData::Compaction { id, tokens_before, tokens_after, .. } => {
            Part::Compaction { id: id.clone(), tokens_before: *tokens_before, tokens_after: *tokens_after }
        }
        PartData::Tool { id, tool, call_id, state } => Part::Tool {
            id: id.clone(),
            tool: tool.clone(),
            call_id: call_id.clone(),
            detail: title(tool, state),
            status: ToolStatus::from_word(state.get("status").and_then(Value::as_str)),
            state: tool_state(tool, state),
            body: Box::new(tool_body(tool, &title(tool, state), state)),
        },
    }
}

/// The call's title (`toToolCardItem`): Fleet's own answer for its own tools, else the registry's one-line label of the
/// input, else the tool's name. One line, as the row clips it.
fn title(tool: &str, state: &Value) -> String {
    let known = get_tool(tool);
    let answer = if known.titled_from_answer { state.get("title").and_then(Value::as_str).filter(|t| !t.is_empty()) } else { None };
    let label = answer.map(str::to_string).unwrap_or_else(|| known.label(state.get("input").unwrap_or(&Value::Null)));
    let one_line = label.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.is_empty() { tool.to_string() } else { one_line }
}

/// What a finished call shows on the right of its row: lines changed for a file write, how long a shell command took.
fn tool_state(tool: &str, state: &Value) -> ToolState {
    if state.get("status").and_then(Value::as_str) != Some("completed") {
        return ToolState::Plain;
    }
    let count = |k: &str| state.pointer(&format!("/metadata/filediff/{k}")).and_then(Value::as_u64).map(|n| n as u32);
    if is_write_call(tool) {
        count("additions").zip(count("deletions")).map_or(ToolState::Plain, |(a, d)| ToolState::Diff(a, d))
    } else if is_shell_row(tool) {
        tool_duration_ms(state).map_or(ToolState::Plain, |ms| ToolState::Took((ms / 1000.0) as f32))
    } else {
        ToolState::Plain
    }
}
