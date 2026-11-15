//! Boils the reducer's messages down to what the conversation draws (`view::conversation`).
use super::model::{Msg, PartData};
use crate::conv::card::tool_body;
use crate::conv::compaction::fold_compaction_summaries;
use crate::conv::shell::shell_command_of;
use crate::conv::tools::{ToolStatus, get_tool, is_shell_row, is_write_call, tool_duration_ms};
use crate::view::conversation::{Message, Part, Role, ToolState};
use serde_json::Value;

/// The conversation's messages: a compaction's summary message is folded into its divider (`foldCompactionSummaries`).
pub fn messages(from: &[Msg]) -> Vec<Message> {
    let folded = fold_compaction_summaries(from);
    from.iter().filter(|m| !folded.hidden.contains(&m.id)).map(|m| message(m, folded.summaries.get(&m.id).map(String::as_str))).collect()
}

/// What `Message::version` would say of `m` once boiled down, without copying its text: the way to
/// tell which of two thousand messages an event batch touched.
pub fn version(m: &Msg, folded: Option<&str>) -> (usize, usize) {
    let weight = |p: &PartData| match p {
        PartData::Text { text, .. } | PartData::Reasoning { text, .. } => text.len(),
        other => part(other, m.role, folded).weight(),
    };
    (
        m.parts.len() + usize::from(turn_failure(m).is_some()),
        m.parts.iter().map(weight).sum::<usize>() + turn_failure(m).map_or(0, str::len),
    )
}

fn turn_failure(m: &Msg) -> Option<&str> {
    m.turn_error.as_ref()?.get("message")?.as_str()
}

/// `folded` is the summary a compaction's own message gave this one, when it is a divider without one of its own.
pub fn message(m: &Msg, folded: Option<&str>) -> Message {
    let mut parts: Vec<Part> = m.parts.iter().map(|p| part(p, m.role, folded)).collect();
    // The conversation has no failure row yet (the parts lane draws one); until then say it in words.
    if let Some(reason) = turn_failure(m) {
        parts.push(Part::text(format!("{}-failed", m.id), format!("The turn failed: {reason}")));
    }
    Message::new(m.id.clone(), m.role, parts)
}

fn part(p: &PartData, role: Role, folded: Option<&str>) -> Part {
    match p {
        // A command the user ran is its own block, not a tool row.
        PartData::Tool { id, state, .. } if role == Role::Shell => Part::Shell(shell_command_of(id, state)),
        PartData::Text { id, text } => Part::Text { id: id.clone(), text: text.clone() },
        PartData::Reasoning { id, text, summary } => Part::Reasoning { id: id.clone(), text: text.clone(), summary: summary.clone() },
        PartData::File { id, mime, filename, url } => {
            Part::File { id: id.clone(), mime: mime.clone(), filename: filename.clone(), url: url.clone() }
        }
        PartData::Compaction { id, trigger, tokens_before, tokens_after, summary } => Part::Compaction {
            id: id.clone(),
            trigger: trigger.clone(),
            tokens_before: *tokens_before,
            tokens_after: *tokens_after,
            summary: summary.clone().or_else(|| folded.map(str::to_string)),
        },
        PartData::Tool { id, tool, call_id, state } => {
            let body = tool_body(tool, &title(tool, state), state);
            Part::Tool {
                id: id.clone(),
                tool: tool.clone(),
                call_id: call_id.clone(),
                detail: title(tool, state),
                status: ToolStatus::from_word(state.get("status").and_then(Value::as_str)),
                state: tool_state(tool, state, &body.diff),
                body: Box::new(body),
            }
        }
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
fn tool_state(tool: &str, state: &Value, diff: &[crate::conv::diff::DiffLine]) -> ToolState {
    if state.get("status").and_then(Value::as_str) != Some("completed") {
        return ToolState::Plain;
    }
    let count = |k: &str| state.pointer(&format!("/metadata/filediff/{k}")).and_then(Value::as_u64).map(|n| n as u32);
    if is_write_call(tool) {
        // The session's own count when Fleet kept one, else the lines the card draws (`diffStats`).
        let counted = || (!diff.is_empty()).then(|| crate::conv::diff::count_changes(diff));
        count("additions")
            .zip(count("deletions"))
            .filter(|&(a, d)| a + d > 0)
            .or_else(counted)
            .map_or(ToolState::Plain, |(a, d)| ToolState::Diff(a, d))
    } else if is_shell_row(tool) {
        tool_duration_ms(state).map_or(ToolState::Plain, |ms| ToolState::Took((ms / 1000.0) as f32))
    } else {
        ToolState::Plain
    }
}
