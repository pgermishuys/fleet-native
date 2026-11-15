//! What a session's row in the list knows beyond its glyph, and how the `sessions` topic changes it
//! (`use-session-activity-updates.ts`, `use-session-token-updates.ts`, `use-session-branch-updates.ts`,
//! `use-session-checkout-updates.ts`, `lib/session-status.ts`). Pure: no Rune, no I/O. How a row looks is
//! component 6; this keeps the data it will draw.
use crate::mock::{Project, Session, Status};
use serde_json::Value;

/// When Fleet tries a turn a model provider's limit stopped again (`toScheduledRetry`).
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduledRetry {
    pub due_at: String,
    pub attempt: i64,
    pub kind: String,
    pub reason: String,
    pub provider_said: bool,
}

/// A turn a Fleet restart cut off (`toSessionInterruption`).
#[derive(Clone, Debug, PartialEq)]
pub struct Interruption {
    pub at: String,
    pub waiting: bool,
    pub last_call: Option<String>,
    pub changed_files: Option<i64>,
    pub auto: bool,
}

/// A turn that stopped on a failure (`toSessionFailure`).
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub at: String,
    pub message: String,
}

/// A retrying session's wording, from `activity_status` (`retryAttempt` and the rest).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Retrying {
    pub attempt: Option<i64>,
    pub max_attempts: Option<i64>,
    pub message: Option<String>,
    pub next: Option<String>,
}

/// The fields of a `SessionListItem` the glyph doesn't carry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    /// `sessionStatus`: `idle`, `active`, `waiting_input`, or a lifecycle state (`stopped`, `completed`, `error`,
    /// `disconnected`). Empty for the invented sessions.
    pub lifecycle: String,
    /// `activityStatus`: `idle`, `busy`, `delegating`, `retry`, `waiting_input`.
    pub activity: String,
    pub workspace_id: String,
    /// Set only while `activity` is `retry`.
    pub retrying: Retrying,
    pub scheduled_retry: Option<ScheduledRetry>,
    pub interrupted: Option<Interruption>,
    pub failed: Option<Failure>,
    pub page_count: u32,
    pub last_page_shown_at: Option<String>,
    pub cost: f64,
    pub checkout_removed_at: Option<String>,
    pub checkout_restoring: bool,
    /// `retentionStatus == "archived"`: the composer takes no prompt (row 1.9).
    pub archived: bool,
    /// `capabilities.canPrompt`, when the row has capabilities.
    pub can_prompt: Option<bool>,
    /// `capabilities.canCompact`: only an explicit `false` turns Compact now off.
    pub can_compact: Option<bool>,
    /// `capabilities.compactDisabledReason`: what the Compact now button says when it can't.
    pub compact_disabled_reason: Option<String>,
    /// `selectedAgent`: the agent a prompt that names none goes to; `None` is the harness's default.
    pub selected_agent: Option<String>,
    /// `selectedModel`: the model a prompt that names none gets.
    pub selected_model: Option<crate::composer::choice::ModelReference>,
    /// `harnessType`: which entry of `GET /api/harnesses` is this session's.
    pub harness_type: Option<String>,
    /// `lastAssistantModelId`: the model that answered last, which "Default" names when nothing else does.
    pub last_model_id: Option<String>,
}

/// Whether the server's own lifecycle state is the session's status, whatever it is doing (`deriveSessionStatus`).
fn is_lifecycle(status: &str) -> bool {
    matches!(status, "stopped" | "completed" | "error" | "disconnected")
}

/// `deriveSessionStatus`: the session's status after an activity change. A lifecycle state stays.
pub fn derive_session_status(activity: &str, current: &str) -> String {
    if is_lifecycle(current) {
        return current.to_string();
    }
    match activity {
        "idle" => "idle",
        "waiting_input" => "waiting_input",
        // Busy, delegating, retry, and anything a newer Fleet adds.
        _ => "active",
    }
    .to_string()
}

/// The glyph a row gets (`session-row-status.ts`): working for an active session, the amber diamond for one
/// waiting on the user, nothing for the rest (a lifecycle state is words, not a glyph, and is component 6's).
pub fn glyph_of(lifecycle: &str, activity: &str) -> Status {
    if is_lifecycle(lifecycle) {
        return Status::Idle;
    }
    // A list item from a Fleet that sends no activity: the session's own status says what it is doing.
    super::sessions::status_of(if activity.is_empty() { if lifecycle == "active" { "busy" } else { lifecycle } } else { activity })
}

fn text(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_string)
}

/// Vue checks a time with `Date.parse`; only its shape is checked here: `2026-11-15` and then anything.
fn is_time(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_str).is_some_and(|t| {
        let b = t.as_bytes();
        b.len() >= 10 && b[4] == b'-' && b[7] == b'-' && b[..4].iter().all(u8::is_ascii_digit)
    })
}

impl ScheduledRetry {
    pub fn from_wire(v: &Value) -> Option<ScheduledRetry> {
        if !v.is_object() || !is_time(v, "dueAt") {
            return None;
        }
        let kind = text(v, "kind").filter(|k| matches!(k.as_str(), "rate_limit" | "usage_limit" | "overloaded"))?;
        Some(ScheduledRetry {
            due_at: text(v, "dueAt")?,
            attempt: v.get("attempt").and_then(Value::as_i64).unwrap_or(1),
            kind,
            reason: text(v, "reason").unwrap_or_default(),
            provider_said: v.get("providerSaid") == Some(&Value::Bool(true)),
        })
    }
}

impl Interruption {
    pub fn from_wire(v: &Value) -> Option<Interruption> {
        if !v.is_object() || !is_time(v, "at") {
            return None;
        }
        Some(Interruption {
            at: text(v, "at")?,
            waiting: v.get("waiting") == Some(&Value::Bool(true)),
            last_call: text(v, "lastCall").filter(|c| !c.trim().is_empty()),
            changed_files: v.get("changedFiles").and_then(Value::as_i64),
            auto: v.get("auto") == Some(&Value::Bool(true)),
        })
    }
}

impl Failure {
    pub fn from_wire(v: &Value) -> Option<Failure> {
        if !v.is_object() || !is_time(v, "at") {
            return None;
        }
        Some(Failure { at: text(v, "at")?, message: text(v, "message").unwrap_or_default() })
    }
}

impl Row {
    /// The row fields of one `GET /api/sessions` item.
    pub fn from_list_item(item: &Value) -> Row {
        let retrying = item.get("activityStatus").and_then(Value::as_str) == Some("retry");
        Row {
            lifecycle: text(item, "sessionStatus").unwrap_or_default(),
            activity: text(item, "activityStatus").unwrap_or_default(),
            workspace_id: text(item, "workspaceId").unwrap_or_default(),
            retrying: if retrying {
                Retrying {
                    attempt: item.get("retryAttempt").and_then(Value::as_i64),
                    max_attempts: item.get("retryMaxAttempts").and_then(Value::as_i64),
                    message: text(item, "retryMessage"),
                    next: text(item, "retryNext"),
                }
            } else {
                Retrying::default()
            },
            scheduled_retry: item.get("scheduledRetry").and_then(ScheduledRetry::from_wire),
            interrupted: item.get("interrupted").and_then(Interruption::from_wire),
            failed: item.get("failed").and_then(Failure::from_wire),
            page_count: item.get("pageCount").and_then(Value::as_u64).unwrap_or(0).min(u32::MAX as u64) as u32,
            last_page_shown_at: text(item, "lastPageShownAt"),
            cost: item.get("totalCost").and_then(Value::as_f64).unwrap_or(0.0),
            checkout_removed_at: text(item, "checkoutRemovedAt"),
            checkout_restoring: false,
            archived: text(item, "retentionStatus").as_deref() == Some("archived"),
            can_prompt: item.get("capabilities").and_then(|c| c.get("canPrompt")).and_then(Value::as_bool),
            can_compact: item.get("capabilities").and_then(|c| c.get("canCompact")).and_then(Value::as_bool),
            compact_disabled_reason: item
                .get("capabilities")
                .and_then(|c| c.get("compactDisabledReason"))
                .and_then(Value::as_str)
                .filter(|r| !r.is_empty())
                .map(str::to_string),
            selected_agent: text(item, "selectedAgent").filter(|a| !a.is_empty()),
            selected_model: item.get("selectedModel").and_then(crate::fleet::catalog::model_reference),
            harness_type: text(item, "harnessType").filter(|h| !h.is_empty()),
            last_model_id: text(item, "lastAssistantModelId").filter(|m| !m.is_empty()),
        }
    }
}

/// One `sessions` event that changes rows.
#[derive(Clone, Debug, PartialEq)]
pub enum RowPatch {
    Activity { session: String, activity: String, retrying: Retrying },
    ScheduledRetry { session: String, retry: Option<ScheduledRetry> },
    Pages { session: String, count: u32, last_shown_at: Option<String> },
    Interrupted { session: String, interruption: Option<Interruption> },
    Failure { session: String, failure: Option<Failure> },
    Tokens { session: String, tokens: u32, cost: f64 },
    Branch { workspace: String, branch: String },
    Checkout { workspace: String, state: String, removed_at: Option<String> },
}

impl RowPatch {
    /// Reads a frame of the `sessions` topic. `None` for a type that doesn't patch a row, and for one that lacks
    /// the id Vue's handler needs (it ignores those too).
    pub fn from_wire(data: &Value) -> Option<RowPatch> {
        let props = data.get("properties")?;
        let id = || text(props, "sessionId").filter(|s| !s.is_empty());
        // A null payload field is "none", a present one that isn't an object is read as none too (`toX` returns null).
        let nested = |k: &str| props.get(k).cloned().unwrap_or(Value::Null);
        Some(match data.get("type")?.as_str()? {
            "activity_status" => {
                let activity = text(props, "activityStatus").filter(|s| !s.is_empty())?;
                let retrying = if activity == "retry" {
                    Retrying {
                        attempt: props.get("attempt").and_then(Value::as_i64),
                        max_attempts: props.get("maxAttempts").and_then(Value::as_i64),
                        message: text(props, "message"),
                        next: text(props, "next"),
                    }
                } else {
                    Retrying::default()
                };
                RowPatch::Activity { session: id()?, activity, retrying }
            }
            "session.retry" => RowPatch::ScheduledRetry { session: id()?, retry: ScheduledRetry::from_wire(&nested("retry")) },
            "session.pages" => RowPatch::Pages {
                session: id()?,
                count: props.get("pageCount").and_then(Value::as_u64).unwrap_or(0).min(u32::MAX as u64) as u32,
                last_shown_at: text(props, "lastPageShownAt"),
            },
            "session.interrupted" => {
                RowPatch::Interrupted { session: id()?, interruption: Interruption::from_wire(&nested("interruption")) }
            }
            "session.failure" => RowPatch::Failure { session: id()?, failure: Failure::from_wire(&nested("failure")) },
            "session_tokens" => RowPatch::Tokens {
                session: id()?,
                tokens: props.get("totalTokens")?.as_u64()?.min(u32::MAX as u64) as u32,
                cost: props.get("totalCost")?.as_f64()?,
            },
            "session_branch" => RowPatch::Branch { workspace: text(props, "workspaceId")?, branch: text(props, "branch")? },
            "session_checkout" => RowPatch::Checkout {
                workspace: text(props, "workspaceId")?,
                state: text(props, "state")?,
                removed_at: text(props, "removedAt"),
            },
            _ => return None,
        })
    }

    /// Changes the rows this patch is about. A session that isn't in the list is ignored (`patchSession`).
    pub fn apply(&self, projects: &mut [Project]) {
        let rows = projects.iter_mut().flat_map(|p| p.sessions.iter_mut());
        match self {
            RowPatch::Activity { session, activity, retrying } => {
                for s in rows.filter(|s| &s.id == session) {
                    let lifecycle = derive_session_status(activity, &s.row.lifecycle);
                    let status = glyph_of(&lifecycle, activity);
                    if status == Status::Idle && s.status != Status::Idle {
                        s.age = "now".into();
                    }
                    s.status = status;
                    s.row.lifecycle = lifecycle;
                    s.row.activity = activity.clone();
                    s.row.retrying = retrying.clone();
                }
            }
            RowPatch::ScheduledRetry { session, retry } => {
                rows.filter(|s| &s.id == session).for_each(|s| s.row.scheduled_retry = retry.clone())
            }
            RowPatch::Pages { session, count, last_shown_at } => rows.filter(|s| &s.id == session).for_each(|s| {
                s.row.page_count = *count;
                s.row.last_page_shown_at = last_shown_at.clone();
            }),
            RowPatch::Interrupted { session, interruption } => {
                rows.filter(|s| &s.id == session).for_each(|s| s.row.interrupted = interruption.clone())
            }
            RowPatch::Failure { session, failure } => rows.filter(|s| &s.id == session).for_each(|s| s.row.failed = failure.clone()),
            RowPatch::Tokens { session, tokens, cost } => rows.filter(|s| &s.id == session).for_each(|s| {
                s.tokens = *tokens;
                s.row.cost = *cost;
            }),
            RowPatch::Branch { workspace, branch } => rows.filter(|s| in_workspace(s, workspace)).for_each(|s| s.branch = branch.clone()),
            RowPatch::Checkout { workspace, state, removed_at } => {
                rows.filter(|s| in_workspace(s, workspace)).for_each(|s| match state.as_str() {
                    "removed" => {
                        s.row.checkout_removed_at = removed_at.clone();
                        s.row.checkout_restoring = false;
                    }
                    "restoring" => s.row.checkout_restoring = true,
                    _ => {
                        s.row.checkout_removed_at = None;
                        s.row.checkout_restoring = false;
                    }
                })
            }
        }
    }
}

fn in_workspace(s: &Session, workspace: &str) -> bool {
    !s.row.workspace_id.is_empty() && s.row.workspace_id == workspace
}
