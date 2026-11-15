//! A permission ask as the server sends it, and the reader that turns JSON into one. Ports `PermissionAsk`,
//! `HandoffAsk` and `toPermissionAsk` of `composables/use-session-permissions.ts` (rows 3.17).
use crate::conv::tools::PermissionKind;
use serde_json::Value;

/// An agent's request to do something the session's permission level doesn't allow.
#[derive(Clone, Debug, PartialEq)]
pub struct PermissionAsk {
    pub id: String,
    /// The session whose harness asked, which takes the answer.
    pub session_id: String,
    pub kind: PermissionKind,
    /// The harness's name for the tool: `bash`, `edit`, `WebFetch`.
    pub tool: String,
    /// What it touches, in one line: the command, the file, the address.
    pub title: Option<String>,
    /// More to look at before answering: a diff, a whole script.
    pub detail: Option<String>,
    pub directory: Option<String>,
    /// What "Don't ask again" would allow, such as `git push *`; empty when it is the whole tool.
    pub always: Vec<String>,
    pub call_id: Option<String>,
    /// Set when a sub-agent asked rather than the session's own agent.
    pub subagent: Option<String>,
    pub asked_at: String,
    /// Set on Fleet's own ask: an agent wants to send work to another machine set to Ask.
    pub handoff: Option<HandoffAsk>,
}

/// What an agent wants to send to another machine (`HandoffAskDetails`).
#[derive(Clone, Debug, PartialEq)]
pub struct HandoffAsk {
    pub machine: String,
    pub os: Option<String>,
    pub folder: Option<String>,
    pub branch: Option<String>,
    pub harness: Option<String>,
    /// The first line of the task, or of the message.
    pub task: Option<String>,
    /// The session there that it messages; none for a new session.
    pub session: Option<String>,
    /// The permission level the work runs under there; none from a Fleet too old to say.
    pub level: Option<String>,
}

impl PermissionAsk {
    /// A bare ask: the other fields are none, for scenes and tests to fill in.
    pub fn new(kind: PermissionKind, tool: &str) -> PermissionAsk {
        PermissionAsk {
            id: "per_1".into(),
            session_id: "s1".into(),
            kind,
            tool: tool.into(),
            title: None,
            detail: None,
            directory: None,
            always: Vec::new(),
            call_id: None,
            subagent: None,
            asked_at: "2026-10-01T10:00:00Z".into(),
            handoff: None,
        }
    }
}

/// An empty string reads as none.
fn optional(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

fn handoff(value: Option<&Value>) -> Option<HandoffAsk> {
    let raw = value?.as_object()?;
    let machine = raw.get("machine")?.as_str()?.to_string();
    let field = |name: &str| optional(raw.get(name));
    Some(HandoffAsk {
        machine,
        os: field("os"),
        folder: field("folder"),
        branch: field("branch"),
        harness: field("harness"),
        task: field("task"),
        session: field("session"),
        level: field("level"),
    })
}

/// The server's word for what an ask is for; a word this client doesn't know reads as "other".
pub fn kind_of(word: Option<&str>) -> PermissionKind {
    match word {
        Some("read") => PermissionKind::Read,
        Some("edit") => PermissionKind::Edit,
        Some("shell") => PermissionKind::Shell,
        Some("web") => PermissionKind::Web,
        _ => PermissionKind::Other,
    }
}

/// `toPermissionAsk`: an ask missing its `id`, `sessionId` or `tool` is ignored.
pub fn to_permission_ask(value: &Value) -> Option<PermissionAsk> {
    let raw = value.as_object()?;
    let id = raw.get("id")?.as_str()?.to_string();
    let session_id = raw.get("sessionId")?.as_str()?.to_string();
    let tool = raw.get("tool")?.as_str()?.to_string();
    let always = raw
        .get("always")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string).collect())
        .unwrap_or_default();
    Some(PermissionAsk {
        id,
        session_id,
        kind: kind_of(raw.get("kind").and_then(Value::as_str)),
        tool,
        title: optional(raw.get("title")),
        detail: optional(raw.get("detail")),
        directory: optional(raw.get("directory")),
        always,
        call_id: optional(raw.get("callId")),
        subagent: optional(raw.get("subagent")),
        // Fleet stamps the time of arrival when the server sent none; a card never shows it, so keep it empty.
        asked_at: raw.get("askedAt").and_then(Value::as_str).unwrap_or_default().to_string(),
        handoff: handoff(raw.get("handoff")),
    })
}
