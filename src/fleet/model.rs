//! The conversation as the reducer accumulates it: weave-fleet's `AccumulatedMessage`
//! (`client/src/lib/client-types.ts`). Tool state stays raw JSON here, because the reducer merges
//! it key by key; `view.rs` boils it down for drawing.
use super::delegations::Delegation;
use crate::view::conversation::Role;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub enum PartData {
    Text { id: String, text: String },
    Reasoning { id: String, text: String, summary: Option<String> },
    Tool { id: String, tool: String, call_id: String, state: Value },
    File { id: String, mime: String, filename: Option<String>, url: String },
    Compaction { id: String, trigger: Option<String>, tokens_before: Option<u32>, tokens_after: Option<u32>, summary: Option<String> },
}

impl PartData {
    pub fn id(&self) -> &str {
        match self {
            PartData::Text { id, .. }
            | PartData::Reasoning { id, .. }
            | PartData::Tool { id, .. }
            | PartData::File { id, .. }
            | PartData::Compaction { id, .. } => id,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tokens {
    pub input: i64,
    pub output: i64,
    pub reasoning: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Msg {
    pub id: String,
    pub session_id: String,
    pub role: Role,
    pub parts: Vec<PartData>,
    pub cost: Option<f64>,
    pub tokens: Option<Tokens>,
    pub created_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub agent: Option<String>,
    pub model_id: Option<String>,
    pub parent_id: Option<String>,
    /// The failure that ended the turn that produced this message (a `TurnError`).
    pub turn_error: Option<Value>,
    pub finish: Option<String>,
    pub steered: bool,
    /// The summary a compaction wrote as a message of its own; shown behind the divider, not as a reply.
    pub compaction_summary: bool,
}

impl Msg {
    pub fn new(id: impl Into<String>, session_id: impl Into<String>, role: Role) -> Self {
        Msg {
            id: id.into(),
            session_id: session_id.into(),
            role,
            parts: Vec::new(),
            cost: None,
            tokens: None,
            created_at: None,
            completed_at: None,
            agent: None,
            model_id: None,
            parent_id: None,
            turn_error: None,
            finish: None,
            steered: false,
            compaction_summary: false,
        }
    }
}

/// What the session's own turn events say (`SessionStreamExplicitStatus`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Explicit {
    Idle,
    Busy,
    Retry,
}

/// The derived status (`SessionStreamStatus`): the session's own status and its sub-agents together
/// (`delegations::derive_status`). `WaitingInput` is a sub-agent stopped on a question.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamStatus {
    Idle,
    Busy,
    Retry,
    Delegating,
    WaitingInput,
}

impl StreamStatus {
    /// Whether the conversation shows the agent at work: anything but idle.
    pub fn working(self) -> bool {
        self != StreamStatus::Idle
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StreamState {
    pub messages: Vec<Msg>,
    pub delegations: Vec<Delegation>,
    pub explicit_status: Explicit,
    pub session_status: StreamStatus,
    pub last_event_id: Option<i64>,
}
