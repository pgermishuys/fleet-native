//! What a session's row in the list knows beyond its glyph, and how the `sessions` topic changes it
//! (`use-session-activity-updates.ts`, `use-session-token-updates.ts`, `use-session-branch-updates.ts`,
//! `use-session-checkout-updates.ts`, `use-session-terminal-updates.ts`, `lib/session-status.ts`). Pure: no Rune, no
//! I/O. How a row looks is component 6; this keeps the data it will draw.
use super::row_chips::{self, PrBadge, Progress};
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

/// `capabilities` of a list item or a detail (`SessionActionCapabilities`): what the harness lets the header offer.
/// Each is `None` when the server didn't say, so the page's fallbacks apply (`header::Actions`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Capabilities {
    pub can_abort: Option<bool>,
    pub can_archive: Option<bool>,
    pub can_fork: Option<bool>,
    pub fork_disabled_reason: Option<String>,
    pub can_delete: Option<bool>,
}

/// `origin`: where an imported or linked session came from (`SessionOriginDto`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Origin {
    pub source_type: String,
    pub title: Option<String>,
    pub resource_url: Option<String>,
    pub resource_id: Option<String>,
    pub provider_id: String,
}

impl Origin {
    pub fn from_wire(v: &Value) -> Option<Origin> {
        v.is_object().then(|| Origin {
            source_type: text(v, "sourceType").unwrap_or_default(),
            title: text(v, "title"),
            resource_url: text(v, "resourceUrl"),
            resource_id: text(v, "resourceId"),
            provider_id: text(v, "providerId").unwrap_or_default(),
        })
    }
}

impl Capabilities {
    pub fn from_wire(v: Option<&Value>) -> Capabilities {
        let Some(v) = v else { return Capabilities::default() };
        let flag = |k: &str| v.get(k).and_then(Value::as_bool);
        Capabilities {
            can_abort: flag("canAbort"),
            can_archive: flag("canArchive"),
            can_fork: flag("canFork"),
            fork_disabled_reason: text(v, "forkDisabledReason").filter(|r| !r.is_empty()),
            can_delete: flag("canDelete"),
        }
    }
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
    /// `isolationStrategy` ("existing", "worktree", ...): the Context tab's Workspace section names it unless it is "existing".
    pub isolation: Option<String>,
    pub checkout_restoring: bool,
    /// `retentionStatus == "archived"`: the composer takes no prompt (row 1.9).
    pub archived: bool,
    /// `capabilities.canPrompt`, when the row has capabilities.
    pub can_prompt: Option<bool>,
    /// `lifecycleStatus`: with no `capabilities`, the composer takes a prompt only while it is "running" (`canSend`).
    /// `None` for the invented sessions, which count as running.
    pub lifecycle_status: Option<String>,
    /// `capabilities.promptDisabledReason`: why the session takes no prompt; the sub-agent line's tooltip.
    pub prompt_disabled_reason: Option<String>,
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
    /// `session.time.updated` in milliseconds (0 when the row has none): the `@` list orders sessions by it.
    pub updated: i64,
    /// `session.time.created` in milliseconds (0 when the row has none): the age falls back to it.
    pub created: i64,
    /// `runningWorkCount`: what the session's agent has running (sub-agents, background shells, monitors).
    pub running_work: u32,
    /// `progress`: the todo list's counts.
    pub progress: Option<Progress>,
    /// `tags`, in the order the server keeps them.
    pub tags: Vec<String>,
    /// `workspaceDirectory`: the worktree's folder for a worktree session (the header's path).
    pub directory: String,
    /// `totalTokens`: `None` while the server has no count (the analytics popover says "—").
    pub total_tokens: Option<u64>,
    /// `totalCost`: `None` while the server has none (`cost` above is the same number as 0 when absent).
    pub total_cost: Option<f64>,
    pub origin: Option<Origin>,
    pub capabilities: Capabilities,
    /// `harnessProfileName`: the profile the session started with. Only the detail carries it.
    pub harness_profile_name: Option<String>,
    /// `inTerminal`: a terminal has this Claude Code session open now (the In terminal chip, #1099). Where it started
    /// (`spawnKind: "terminal"`) is the listing's `spawn_kind`.
    pub in_terminal: bool,
    /// The pull request the session opened. Only mock data fills it until the header component reads the smart links.
    pub pr: Option<PrBadge>,
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
            tags: item
                .get("tags")
                .and_then(Value::as_array)
                .map(|t| t.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default(),
            directory: text(item, "workspaceDirectory").unwrap_or_default(),
            total_tokens: item.get("totalTokens").and_then(Value::as_u64),
            total_cost: item.get("totalCost").and_then(Value::as_f64),
            origin: item.get("origin").and_then(Origin::from_wire),
            capabilities: Capabilities::from_wire(item.get("capabilities")),
            harness_profile_name: text(item, "harnessProfileName").filter(|n| !n.is_empty()),
            checkout_removed_at: text(item, "checkoutRemovedAt"),
            isolation: text(item, "isolationStrategy").filter(|n| !n.is_empty()),
            checkout_restoring: false,
            archived: text(item, "retentionStatus").as_deref() == Some("archived"),
            can_prompt: item.get("capabilities").and_then(|c| c.get("canPrompt")).and_then(Value::as_bool),
            lifecycle_status: text(item, "lifecycleStatus"),
            prompt_disabled_reason: item
                .get("capabilities")
                .and_then(|c| c.get("promptDisabledReason"))
                .and_then(Value::as_str)
                .filter(|r| !r.is_empty())
                .map(str::to_string),
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
            updated: 0,
            created: 0,
            // The server sends the count as a number or a string.
            running_work: item
                .get("runningWorkCount")
                .and_then(|n| n.as_u64().or_else(|| n.as_str().and_then(|s| s.trim().parse().ok())))
                .unwrap_or(0)
                .min(u32::MAX as u64) as u32,
            progress: item.get("progress").and_then(row_chips::parse_row_progress),
            in_terminal: item.get("inTerminal") == Some(&Value::Bool(true)),
            pr: None,
        }
    }
}

/// One `sessions` event that changes rows.
#[derive(Clone, Debug, PartialEq)]
pub enum RowPatch {
    Activity {
        session: String,
        activity: String,
        retrying: Retrying,
    },
    ScheduledRetry {
        session: String,
        retry: Option<ScheduledRetry>,
    },
    Pages {
        session: String,
        count: u32,
        last_shown_at: Option<String>,
    },
    Interrupted {
        session: String,
        interruption: Option<Interruption>,
    },
    Failure {
        session: String,
        failure: Option<Failure>,
    },
    Tokens {
        session: String,
        tokens: u32,
        cost: f64,
    },
    Progress {
        session: String,
        progress: Progress,
    },
    Branch {
        workspace: String,
        branch: String,
    },
    Checkout {
        workspace: String,
        state: String,
        removed_at: Option<String>,
    },
    /// `session_terminal`: a terminal opened the session or let go of it (`use-session-terminal-updates.ts`).
    Terminal {
        session: String,
        in_terminal: bool,
    },
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
            "session_progress" => {
                let (session, progress) = row_chips::parse_summary(props)?;
                RowPatch::Progress { session, progress }
            }
            "session_branch" => RowPatch::Branch { workspace: text(props, "workspaceId")?, branch: text(props, "branch")? },
            "session_terminal" => RowPatch::Terminal { session: id()?, in_terminal: props.get("inTerminal")?.as_bool()? },
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
                        s.touch(super::link::now_ms());
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
            RowPatch::Progress { session, progress } => {
                rows.filter(|s| &s.id == session).for_each(|s| s.row.progress = Some(progress.clone()))
            }
            RowPatch::Terminal { session, in_terminal } => rows.filter(|s| &s.id == session).for_each(|s| s.row.in_terminal = *in_terminal),
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
