//! Boils the reducer's messages down to what the conversation draws (`view::conversation`).
use super::model::{Msg, PartData};
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
            tool: display_name(tool),
            call_id: call_id.clone(),
            detail: detail(state),
            state: tool_state(tool, state),
        },
    }
}

/// `bash` as "Bash", the way the summary row names it.
fn display_name(tool: &str) -> String {
    let mut chars = tool.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// What the call ran or touched: the command, the file, the pattern.
fn detail(state: &Value) -> String {
    let input = state.get("input");
    ["command", "filePath", "path", "pattern", "url", "query", "description"]
        .iter()
        .find_map(|k| input.and_then(|i| i.get(*k)).and_then(Value::as_str))
        .or_else(|| state.get("title").and_then(Value::as_str))
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("")
        .to_string()
}

fn tool_state(tool: &str, state: &Value) -> ToolState {
    match state.get("status").and_then(Value::as_str) {
        Some("pending" | "running") => ToolState::Running,
        Some("completed") => {
            let count = |k: &str| state.pointer(&format!("/metadata/filediff/{k}")).and_then(Value::as_u64).map(|n| n as u32);
            let took = || {
                let (start, end) = (state.pointer("/time/start")?.as_f64()?, state.pointer("/time/end")?.as_f64()?);
                Some(((end - start) / 1000.0) as f32)
            };
            match tool {
                "edit" | "write" | "patch" => {
                    count("additions").zip(count("deletions")).map_or(ToolState::Plain, |(a, d)| ToolState::Diff(a, d))
                }
                "bash" => took().map_or(ToolState::Plain, ToolState::Took),
                _ => ToolState::Plain,
            }
        }
        _ => ToolState::Plain,
    }
}
