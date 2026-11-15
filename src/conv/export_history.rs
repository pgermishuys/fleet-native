//! Loading a session's whole history for an export (`lib/export-history.ts`, `convertFleetMessageToAccumulated` in
//! `lib/pagination-utils.ts`). The conversation only holds the pages it has scrolled to, so export pages
//! `GET /api/sessions/{id}/messages` back to the first message, newest page first, over REST as Fleet does. The request
//! itself is the caller's (`fetch`), so this is plain logic with no Rune and no network in it.
use crate::fleet::model::{Msg, PartData, Tokens};
use crate::fleet::work::parse_iso_ms;
use crate::view::conversation::Role;
use serde_json::{Map, Value};
use std::collections::HashSet;

const PAGE_SIZE: usize = 200;
/// A history longer than this is a loop, not a conversation.
const MAX_PAGES: usize = 500;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistoryError {
    /// The dialog was closed or reopened while a page was on its way: nothing to show.
    Aborted,
    /// What to tell the user (`ExportHistoryError`). A page that fails stops the load, so a file is never made from half a
    /// history.
    Failed(String),
}

/// What asking for one page came to: the status and body of an answer, or why there was none.
pub type Fetched = Result<(u16, String), String>;

/// `encodeURIComponent`.
pub fn encode_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `loadWholeHistory`: every message of the session, oldest first. `progress` hears the count after each page.
pub fn load_whole_history(
    session_id: &str,
    fetch: &mut dyn FnMut(&str) -> Fetched,
    progress: &mut dyn FnMut(usize),
    aborted: &dyn Fn() -> bool,
) -> Result<Vec<Msg>, HistoryError> {
    let mut all: Vec<Msg> = Vec::new();
    let mut before: Option<String> = None;
    for _ in 0..MAX_PAGES {
        if aborted() {
            return Err(HistoryError::Aborted);
        }
        let mut path = format!("/api/sessions/{}/messages?limit={PAGE_SIZE}", encode_component(session_id));
        if let Some(id) = &before {
            path.push_str(&format!("&before={}", encode_component(id)));
        }
        let (status, body) = match fetch(&path) {
            Ok(answer) => answer,
            Err(_) if aborted() => return Err(HistoryError::Aborted),
            Err(_) => {
                return Err(HistoryError::Failed(format!(
                    "Could not reach Fleet to load the conversation (after {} messages).",
                    all.len()
                )));
            }
        };
        if !(200..300).contains(&status) {
            return Err(HistoryError::Failed(format!(
                "Fleet could not load the conversation (error {status} after {} messages).",
                all.len()
            )));
        }
        let Ok(body) = serde_json::from_str::<Value>(&body) else {
            return Err(HistoryError::Failed(format!(
                "Fleet sent a page of the conversation that could not be read (after {} messages).",
                all.len()
            )));
        };
        let known: HashSet<String> = all.iter().map(|m| m.id.clone()).collect();
        let mut older: Vec<Msg> = body
            .get("messages")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|m| convert_fleet_message(m, session_id))
            .filter(|m| !known.contains(&m.id))
            .collect();
        older.append(&mut all);
        all = older;
        progress(all.len());

        let pagination = body.get("pagination");
        let oldest = pagination.and_then(|p| p.get("oldestMessageId")).and_then(Value::as_str).map(str::to_string);
        if pagination.and_then(|p| p.get("hasMore")).and_then(Value::as_bool) != Some(true) {
            return Ok(all);
        }
        // A cursor that doesn't move would ask for the same page forever.
        if oldest.is_none() || oldest == before {
            return Err(HistoryError::Failed(format!("Fleet did not say where the older messages start (after {} messages).", all.len())));
        }
        before = oldest;
    }
    Err(HistoryError::Failed("The conversation is longer than Fleet can export.".into()))
}

fn str_field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

/// `convertFleetMessageToAccumulated`: a message as `GET /api/sessions/{id}/messages` writes it, which is flatter than the
/// hub's. The parts keep their order; a `tool-result` part is folded into the call it answers.
pub fn convert_fleet_message(message: &Value, session_id: &str) -> Option<Msg> {
    let id = str_field(message, "id")?.to_string();
    let role = match str_field(message, "role") {
        Some("user") => Role::User,
        Some("shell") => Role::Shell,
        _ => Role::Assistant,
    };
    let mut msg = Msg::new(id.clone(), session_id, role);
    let parts: &[Value] = message.get("parts").and_then(Value::as_array).map_or(&[], Vec::as_slice);
    let results: Vec<(&str, &str, bool)> = parts
        .iter()
        .filter(|p| str_field(p, "type") == Some("tool-result"))
        .filter_map(|p| {
            let call = str_field(p, "toolCallId").filter(|c| !c.is_empty())?;
            Some((call, str_field(p, "content").unwrap_or(""), p.get("isError").and_then(Value::as_bool).unwrap_or(false)))
        })
        .collect();
    let (mut cost, mut input, mut output, mut reasoning) = (0.0, 0, 0, 0);
    for part in parts {
        let n = msg.parts.len();
        match str_field(part, "type") {
            Some("text") => {
                msg.parts.push(PartData::Text { id: format!("{id}-text-{n}"), text: str_field(part, "text").unwrap_or("").into() })
            }
            Some("reasoning") => msg.parts.push(PartData::Reasoning {
                id: format!("{id}-reasoning-{n}"),
                text: str_field(part, "text").unwrap_or("").into(),
                summary: str_field(part, "summary").map(str::to_string),
            }),
            Some("tool") => {
                let call = str_field(part, "toolCallId").unwrap_or("");
                let result = results.iter().rev().find(|(c, ..)| *c == call).copied();
                msg.parts.push(PartData::Tool {
                    id: if call.is_empty() { format!("{id}-tool-{n}") } else { call.to_string() },
                    tool: str_field(part, "toolName").unwrap_or("").into(),
                    call_id: call.into(),
                    state: with_own_result(map_tool_state(part, result), part),
                });
            }
            Some("file") => msg.parts.push(PartData::File {
                id: str_field(part, "partId").map_or_else(|| format!("{id}-file-{n}"), str::to_string),
                mime: str_field(part, "mime").unwrap_or("").into(),
                filename: str_field(part, "filename").map(str::to_string),
                url: str_field(part, "url").unwrap_or("").into(),
            }),
            Some("step-finish") => {
                cost += part.get("cost").and_then(Value::as_f64).unwrap_or(0.0);
                input += part.get("tokensInput").and_then(Value::as_i64).unwrap_or(0);
                output += part.get("tokensOutput").and_then(Value::as_i64).unwrap_or(0);
                reasoning += part.get("tokensReasoning").and_then(Value::as_i64).unwrap_or(0);
            }
            _ => {}
        }
    }
    msg.created_at = str_field(message, "timestamp").and_then(parse_iso_ms);
    msg.agent = str_field(message, "agent").map(str::to_string);
    msg.model_id = str_field(message, "modelId").map(str::to_string);
    msg.cost = (cost != 0.0).then_some(cost);
    msg.tokens = (input != 0 || output != 0 || reasoning != 0).then_some(Tokens { input, output, reasoning });
    Some(msg)
}

/// `mapToolState`: `ToolUseState` 0 to 3 as a status; the part's metadata spread over it; the arguments as `input`; what
/// a `tool-result` part returned as `output` (read as JSON when it is, else the text).
fn map_tool_state(part: &Value, result: Option<(&str, &str, bool)>) -> Map<String, Value> {
    let status = match part.get("state").and_then(Value::as_i64) {
        Some(1) => "running",
        Some(2) => "completed",
        Some(3) => "error",
        _ => "pending",
    };
    let mut state = Map::new();
    state.insert("status".into(), status.into());
    if let Some(Value::Object(extra)) = part.get("metadata") {
        state.extend(extra.clone());
    }
    if let Some(arguments) = part.get("arguments").filter(|a| !a.is_null()) {
        state.insert("input".into(), arguments.clone());
    }
    if let Some((_, content, _)) = result {
        state.insert("output".into(), serde_json::from_str(content).unwrap_or_else(|_| content.into()));
    }
    state
}

/// `withOwnResult`: the call's own `output` and `error`, when no `tool-result` part gave them (OpenCode 2 and Claude Code
/// keep them on the part).
fn with_own_result(mut state: Map<String, Value>, part: &Value) -> Value {
    if !state.contains_key("output")
        && let Some(output) = part.get("output").filter(|o| !o.is_null())
    {
        state.insert("output".into(), output.clone());
    }
    if !state.contains_key("error")
        && let Some(error) = str_field(part, "error").filter(|e| !e.is_empty())
    {
        state.insert("error".into(), error.into());
    }
    Value::Object(state)
}
