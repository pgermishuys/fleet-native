//! Every event the hub sends, read (`client/src/lib/domain-events.ts`, the `DomainEvent` union of
//! 64 `type` strings). The reducer applies fourteen of them (`Event` variants with fields); the rest
//! are kept whole as `Event::Other(kind, properties)` for the component that owns them, and a
//! type this build has never heard of (a newer Fleet) is `Event::Unknown`. Nothing is dropped.
use super::delegations::DelegationEvent;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Mutex;

/// The events the reducer applies, and the rest of the union.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    MessageCreated(Lifecycle),
    MessageUpdated(Lifecycle),
    /// `user.prompt.committed`: the server has the prompt. Applied as a message update; the correlation
    /// id and the event's id confirm the prompt the person sent (`prompts.rs`).
    PromptCommitted {
        life: Lifecycle,
        correlation_id: Option<String>,
        event_id: Option<i64>,
    },
    PartUpdated(Value),
    TextDelta {
        session_id: String,
        message_id: String,
        part_id: String,
        delta: String,
        offset: Option<i64>,
    },
    TurnStarted,
    TurnEnded,
    TurnFailed {
        session_id: String,
        message_id: Option<String>,
        error: Value,
    },
    SessionIdled,
    /// `delegation.created`, `.updated` or `.completed` (`kind` says which; the last two upsert).
    Delegation {
        kind: Kind,
        event: DelegationEvent,
    },
    /// `work.started`, `.updated` or `.ended` (`kind` says which; all three replace the item by id). The
    /// payload is kept as sent: a payload that isn't an item changes nothing.
    Work {
        kind: Kind,
        item: Value,
    },
    /// `context.updated`: the whole usage, as sent.
    Context(Value),
    /// `session.recap`: what Fleet wrote for the session while nobody looked; `None` when the next prompt cleared it.
    Recap(Option<String>),
    /// A session's activity on the `sessions` topic. The stream reads it for its sub-agents' sessions;
    /// the fields are empty when the frame lacks them, and the reducer then ignores it.
    ActivityStatus {
        session_id: String,
        activity_status: String,
    },
    /// A type in the union that the reducer doesn't apply (yet): its `properties`, untouched. Also a
    /// `message.part.delta.streamed` for a field other than `text`.
    Other(Kind, Value),
    /// A type this build doesn't know. Logged once, never an error.
    Unknown {
        kind: String,
        properties: Value,
    },
    /// The hub's event id of the event just before this one in the batch (`getDomainEventCursor`):
    /// it moves `last_event_id`.
    Cursor(i64),
}

/// A message's `info`, and its parts when the event carries them (the harness's own
/// `message.updated` has none).
#[derive(Clone, Debug, PartialEq)]
pub struct Lifecycle {
    pub info: Value,
    pub parts: Option<Vec<Value>>,
}

macro_rules! kinds {
    ($($variant:ident = $wire:literal,)*) => {
        /// A type of the union. The fourteen the reducer applies have their own `Event` variants and
        /// appear here too, so one table lists all 64.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Kind { $($variant,)* }

        impl Kind {
            pub const ALL: &'static [Kind] = &[$(Kind::$variant,)*];

            pub fn wire(self) -> &'static str {
                match self { $(Kind::$variant => $wire,)* }
            }

            pub fn from_wire(name: &str) -> Option<Kind> {
                match name { $($wire => Some(Kind::$variant),)* _ => None }
            }
        }
    };
}

kinds! {
    SessionStarted = "session.started",
    SessionIdled = "session.idled",
    SessionDeleted = "session.deleted",
    SessionArchived = "session.archived",
    TurnStarted = "turn.started",
    TurnEnded = "turn.ended",
    TurnFailed = "turn.failed",
    MessageCreated = "message.created",
    MessageUpdated = "message.updated",
    UserPromptCommitted = "user.prompt.committed",
    MessagePartUpdated = "message.part.updated",
    MessagePartDeltaStreamed = "message.part.delta.streamed",
    DelegationCreated = "delegation.created",
    DelegationUpdated = "delegation.updated",
    DelegationCompleted = "delegation.completed",
    WorkStarted = "work.started",
    WorkUpdated = "work.updated",
    WorkEnded = "work.ended",
    ContextUpdated = "context.updated",
    ActivityStatus = "activity_status",
    FilesChanged = "files.changed",
    CanvasUpdated = "canvas.updated",
    CanvasClosed = "canvas.closed",
    CanvasFocused = "canvas.focused",
    TerminalOpened = "terminal.opened",
    TerminalClosed = "terminal.closed",
    AppUpdated = "app.updated",
    BrowserStep = "browser.step",
    SessionRecap = "session.recap",
    SessionNotification = "session_notification",
    SessionQueue = "session.queue",
    SessionPermissionRules = "session.permission-rules",
    SessionRetry = "session.retry",
    SessionPages = "session.pages",
    SessionInterrupted = "session.interrupted",
    SessionFailure = "session.failure",
    PermissionAsked = "permission.asked",
    PermissionReplied = "permission.replied",
    SessionMessaged = "session.messaged",
    SessionReported = "session.reported",
    TodosReported = "todos.reported",
    FilesWritten = "files.written",
    SessionStatus = "session.status",
    SessionCreated = "session_created",
    SessionArchivedRow = "session_archived",
    SessionUnarchived = "session_unarchived",
    SessionDeletedRow = "session_deleted",
    SessionProgress = "session_progress",
    SessionCheckout = "session_checkout",
    SessionBranch = "session_branch",
    ProgressUpdated = "progress.updated",
    SessionTokens = "session_tokens",
    SmartLinkUpdated = "smart_link.updated",
    UpdateRollout = "update.rollout",
    MemorySaved = "memory.saved",
    MemoryDreamed = "memory.dreamed",
    WorkflowRun = "workflow_run",
    HarnessUsage = "harness.usage",
    UsageLimits = "usage.limits",
    HarnessCatalogChanged = "harness.catalog_changed",
    ModsChanged = "mods.changed",
    ModsRuntime = "mods.runtime",
    ModUi = "mod.ui",
    ModPane = "mod.pane",
}

impl Event {
    /// An event as the hub sends it: `data` is `{type, eventId?, properties}` (`toDomainEvent`: the
    /// wire calls the payload `properties`). `None` only for a frame that can't be read at all: no
    /// `type`, or a type the reducer applies with the fields it needs missing. The caller skips that
    /// one event and carries on with the rest (`applySafely`).
    pub fn from_wire(data: &Value) -> Option<Event> {
        let kind = data.get("type")?.as_str()?;
        let props = data.get("properties").unwrap_or(&Value::Null);
        let text = |k: &str| props.get(k).and_then(Value::as_str).map(str::to_string);
        let Some(known) = Kind::from_wire(kind) else {
            log_unknown(kind);
            return Some(Event::Unknown { kind: kind.to_string(), properties: props.clone() });
        };
        Some(match known {
            Kind::MessageCreated | Kind::MessageUpdated | Kind::UserPromptCommitted => {
                let life = Lifecycle { info: props.get("info")?.clone(), parts: props.get("parts").and_then(Value::as_array).cloned() };
                match known {
                    Kind::MessageCreated => Event::MessageCreated(life),
                    Kind::UserPromptCommitted => Event::PromptCommitted {
                        life,
                        correlation_id: text("correlationId").filter(|c| !c.is_empty()),
                        event_id: data.get("eventId").and_then(Value::as_i64),
                    },
                    _ => Event::MessageUpdated(life),
                }
            }
            Kind::MessagePartUpdated => Event::PartUpdated(props.get("part")?.clone()),
            Kind::MessagePartDeltaStreamed if text("field")? == "text" => Event::TextDelta {
                session_id: text("sessionID")?,
                message_id: text("messageID")?,
                part_id: text("partID")?,
                delta: text("delta")?,
                offset: props.get("offset").and_then(Value::as_i64),
            },
            Kind::TurnStarted => Event::TurnStarted,
            Kind::TurnEnded => Event::TurnEnded,
            Kind::TurnFailed => {
                Event::TurnFailed { session_id: text("sessionID")?, message_id: text("messageID"), error: props.get("error")?.clone() }
            }
            Kind::SessionIdled => Event::SessionIdled,
            Kind::DelegationCreated | Kind::DelegationUpdated | Kind::DelegationCompleted => {
                Event::Delegation { kind: known, event: DelegationEvent::from_wire(props)? }
            }
            Kind::WorkStarted | Kind::WorkUpdated | Kind::WorkEnded => Event::Work { kind: known, item: props.clone() },
            Kind::ContextUpdated => Event::Context(props.clone()),
            Kind::SessionRecap => Event::Recap(text("text").filter(|t| !t.is_empty())),
            Kind::ActivityStatus => Event::ActivityStatus {
                session_id: props.get("sessionId").and_then(Value::as_str).unwrap_or_default().to_string(),
                activity_status: props.get("activityStatus").and_then(Value::as_str).unwrap_or_default().to_string(),
            },
            other => Event::Other(other, props.clone()),
        })
    }

    /// The type string, for the variants that have one.
    pub fn kind(&self) -> Option<Kind> {
        Some(match self {
            Event::MessageCreated(_) => Kind::MessageCreated,
            Event::MessageUpdated(_) => Kind::MessageUpdated,
            Event::PromptCommitted { .. } => Kind::UserPromptCommitted,
            Event::PartUpdated(_) => Kind::MessagePartUpdated,
            Event::TextDelta { .. } => Kind::MessagePartDeltaStreamed,
            Event::TurnStarted => Kind::TurnStarted,
            Event::TurnEnded => Kind::TurnEnded,
            Event::TurnFailed { .. } => Kind::TurnFailed,
            Event::SessionIdled => Kind::SessionIdled,
            Event::Delegation { kind, .. } => *kind,
            Event::Work { kind, .. } => *kind,
            Event::Context(_) => Kind::ContextUpdated,
            Event::Recap(_) => Kind::SessionRecap,
            Event::ActivityStatus { .. } => Kind::ActivityStatus,
            Event::Other(kind, _) => *kind,
            Event::Unknown { .. } | Event::Cursor(_) => return None,
        })
    }

    /// Whether the reducer changes state for this event. The others go to subscribers (`State::subscribe`).
    pub fn applied(&self) -> bool {
        !matches!(self, Event::Other(..) | Event::Unknown { .. })
    }
}

/// Says once per type that a newer Fleet sent something this build doesn't read.
fn log_unknown(kind: &str) {
    static SEEN: Mutex<Option<HashSet<String>>> = Mutex::new(None);
    if let Ok(mut seen) = SEEN.lock()
        && seen.get_or_insert_with(HashSet::new).insert(kind.to_string())
    {
        eprintln!("fleet: an event of a type this build doesn't know, kept unread: {kind}");
    }
}
