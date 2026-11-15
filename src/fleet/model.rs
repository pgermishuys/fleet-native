//! The conversation as the reducer accumulates it: weave-fleet's `AccumulatedMessage`
//! (`client/src/lib/client-types.ts`). Tool state stays raw JSON here, because the reducer merges
//! it key by key; `view.rs` boils it down for drawing.
use super::context::ContextUsage;
use super::delegations::Delegation;
use super::work::RunningWorkItem;
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
    /// The slash command a prompt came from (`SlashCommand`); user messages only.
    pub command: Option<SlashCommand>,
    /// The mod that sent or rewrote a prompt (`ModMark`); user messages only.
    pub mod_mark: Option<ModMark>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlashCommand {
    pub name: String,
    /// Left out (`None`) when the command had none.
    pub arguments: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModKind {
    Sent,
    Rewritten,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModMark {
    pub kind: ModKind,
    pub name: String,
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
            command: None,
            mod_mark: None,
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
    /// What the agent left running (`work.*`); background work isn't the session's own, so it doesn't
    /// touch the status.
    pub running_work: Vec<RunningWorkItem>,
    /// How full the context window is: the snapshot's `context`, replaced whole by each `context.updated`.
    pub context: Option<ContextUsage>,
    pub explicit_status: Explicit,
    pub session_status: StreamStatus,
    pub last_event_id: Option<i64>,
    /// The snapshot says the harness was unavailable and this is cached data (`isPartial`).
    pub is_partial: bool,
    /// There are older messages than the ones held (`hasMore`), and the cursor to ask for the page before them.
    pub has_more: bool,
    pub cursor: Option<String>,
    /// An older page is on its way, or the last one failed and the cursor was kept for another try.
    pub is_loading_older: bool,
    pub load_older_failed: bool,
}
