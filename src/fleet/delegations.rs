//! A session's sub-agents ("delegations") and the status they help derive: weave-fleet's
//! `client/src/lib/delegation-state.ts` and the delegation parts of `domain-event-reducer.ts`.
//! Pure: no Rune, no I/O.
use super::model::{Explicit, StreamStatus};
use serde_json::Value;

/// `DelegationDto["status"]`. Anything the server sends that isn't one of these reads as pending.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelegationStatus {
    Pending,
    Running,
    Completed,
    Error,
    Cancelled,
}

impl DelegationStatus {
    /// `toDelegationStatus`: unknown, missing or not text is `Pending`.
    pub fn from_wire(status: Option<&str>) -> Self {
        match status {
            Some("running") => Self::Running,
            Some("completed") => Self::Completed,
            Some("error") => Self::Error,
            Some("cancelled") => Self::Cancelled,
            _ => Self::Pending,
        }
    }

    /// Pending and running are the active ones; the rest are over.
    pub fn active(self) -> bool {
        matches!(self, Self::Pending | Self::Running)
    }
}

/// `DelegationDto`.
#[derive(Clone, Debug, PartialEq)]
pub struct Delegation {
    pub delegation_id: String,
    pub parent_tool_call_id: Option<String>,
    pub child_session_id: Option<String>,
    pub title: String,
    pub status: DelegationStatus,
    pub created_at: Option<String>,
    /// What the sub-agent's own session shows (`activity_status` on the `sessions` topic).
    pub child_activity_status: Option<String>,
    /// Once in the background a sub-agent stays there: its call returned, so it isn't the session's work.
    pub background: bool,
}

/// What a `delegation.created` / `.updated` / `.completed` event says (`DelegationEvent`). A field the
/// event doesn't carry is `None`, so an update can keep what it had.
#[derive(Clone, Debug, PartialEq)]
pub struct DelegationEvent {
    pub delegation_id: String,
    pub parent_tool_call_id: Option<String>,
    pub child_session_id: Option<String>,
    pub title: Option<String>,
    /// `None` when the event leaves it out: an update then keeps the delegation's own.
    pub status: Option<DelegationStatus>,
    pub created_at: Option<String>,
    pub background: bool,
}

impl DelegationEvent {
    /// `mapDelegationEvent`. `None` when there's no `delegationId`: nothing to key it by.
    pub fn from_wire(props: &Value) -> Option<Self> {
        let text = |k: &str| props.get(k).and_then(Value::as_str).map(str::to_string);
        Some(DelegationEvent {
            delegation_id: text("delegationId")?,
            parent_tool_call_id: text("parentToolCallId"),
            child_session_id: text("childSessionId"),
            title: text("title"),
            status: props.get("status").and_then(Value::as_str).map(|s| DelegationStatus::from_wire(Some(s))),
            created_at: text("createdAt"),
            background: props.get("background") == Some(&Value::Bool(true)),
        })
    }
}

impl Delegation {
    /// `mapSnapshotDelegation`.
    pub fn from_snapshot(v: &Value) -> Option<Self> {
        let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        Some(Delegation {
            delegation_id: text("delegationId")?,
            parent_tool_call_id: text("parentToolCallId"),
            child_session_id: text("childSessionId"),
            title: text("title").unwrap_or_default(),
            status: DelegationStatus::from_wire(v.get("status").and_then(Value::as_str)),
            created_at: text("createdAt"),
            child_activity_status: text("childActivityStatus"),
            background: v.get("background") == Some(&Value::Bool(true)),
        })
    }
}

/// `applyDelegationCreated`: merged over a delegation with the same id, else added at the end.
pub fn apply_created(list: &mut Vec<Delegation>, ev: &DelegationEvent) {
    let next = |old: Option<&Delegation>| Delegation {
        delegation_id: ev.delegation_id.clone(),
        parent_tool_call_id: ev.parent_tool_call_id.clone(),
        child_session_id: ev.child_session_id.clone(),
        title: ev.title.clone().unwrap_or_default(),
        status: ev.status.unwrap_or(DelegationStatus::Pending),
        created_at: ev.created_at.clone(),
        // What the event doesn't carry stays, as with the object spread in Fleet.
        child_activity_status: old.and_then(|d| d.child_activity_status.clone()),
        background: ev.background || old.is_some_and(|d| d.background),
    };
    match list.iter().position(|d| d.delegation_id == ev.delegation_id) {
        Some(i) => list[i] = next(Some(&list[i])),
        None => list.push(next(None)),
    }
}

/// `applyDelegationUpdated`: keeps the fields the event doesn't carry and never takes `background`
/// away. Returns whether the list changed, and does nothing for an id it doesn't know.
pub fn apply_updated(list: &mut [Delegation], ev: &DelegationEvent) -> bool {
    let Some(old) = list.iter_mut().find(|d| d.delegation_id == ev.delegation_id) else { return false };
    let next = Delegation {
        delegation_id: old.delegation_id.clone(),
        parent_tool_call_id: ev.parent_tool_call_id.clone().or_else(|| old.parent_tool_call_id.clone()),
        child_session_id: ev.child_session_id.clone().or_else(|| old.child_session_id.clone()),
        title: ev.title.clone().unwrap_or_else(|| old.title.clone()),
        status: ev.status.unwrap_or(old.status),
        created_at: ev.created_at.clone().or_else(|| old.created_at.clone()),
        child_activity_status: old.child_activity_status.clone(),
        background: ev.background || old.background,
    };
    let changed = *old != next;
    *old = next;
    changed
}

/// `upsertDelegation`, for `delegation.updated` and `.completed`: an id the list doesn't have yet
/// (the created event was missed) is created.
pub fn upsert(list: &mut Vec<Delegation>, ev: &DelegationEvent) {
    if !list.iter().any(|d| d.delegation_id == ev.delegation_id) {
        apply_created(list, ev);
    } else {
        apply_updated(list, ev);
    }
}

/// `isDelegationWaiting`: still at work, and stopped on a question.
pub fn is_waiting(d: &Delegation) -> bool {
    d.status.active() && d.child_activity_status.as_deref() == Some("waiting_input")
}

fn has_waiting(list: &[Delegation]) -> bool {
    list.iter().any(is_waiting)
}

/// Whether a sub-agent is doing the session's work. One in the background isn't: the session is free
/// for the next prompt and wakes by itself when it finishes. Its question still counts (`is_waiting`).
fn has_active(list: &[Delegation]) -> bool {
    list.iter().any(|d| d.status.active() && !d.background)
}

/// `deriveSessionStatus`: a sub-agent waiting on a question outranks the session's own work.
pub fn derive_status(explicit: Explicit, list: &[Delegation]) -> StreamStatus {
    if has_waiting(list) {
        return StreamStatus::WaitingInput;
    }
    match explicit {
        Explicit::Busy => StreamStatus::Busy,
        Explicit::Retry => StreamStatus::Retry,
        Explicit::Idle if has_active(list) => StreamStatus::Delegating,
        Explicit::Idle => StreamStatus::Idle,
    }
}

/// `deriveSnapshotSessionStatus`: a snapshot can land while the parent still says busy for its
/// turn, but an active sub-agent has to show at once.
pub fn derive_snapshot_status(explicit: Explicit, list: &[Delegation]) -> StreamStatus {
    if has_active(list) && !has_waiting(list) {
        return StreamStatus::Delegating;
    }
    derive_status(explicit, list)
}

/// `withChildActivity`: records what a sub-agent's session shows now. Returns whether anything
/// changed; an event for any other session leaves the list as it was.
pub fn set_child_activity(list: &mut [Delegation], session_id: &str, activity: &str) -> bool {
    if session_id.is_empty() || activity.is_empty() {
        return false;
    }
    let mut changed = false;
    for d in list.iter_mut().filter(|d| d.child_session_id.as_deref() == Some(session_id)) {
        if d.child_activity_status.as_deref() != Some(activity) {
            d.child_activity_status = Some(activity.to_string());
            changed = true;
        }
    }
    changed
}
