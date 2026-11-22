//! Work an agent moved into the background, and the sub-agent rows that point at its sessions. Ports
//! `lib/background-work.ts` (the notice Fleet shows when the work finished, the handle a backgrounded call carries, how
//! each handle ended), the `subagentKind` / `subagentTask` / `withDelegation` helpers of `activity-stream-tool-card.ts`
//! and `use-activity-messages.ts`, `backgroundStatus` of `toToolCardItem`, `withEnd`, and what `AgentTaskRow.vue` says
//! (its status word, glyph and tooltip). No Rune in here.
use super::message_list::render_message_body;
use super::tools::ToolStatus;
use crate::fleet::delegations::{Delegation, DelegationStatus, is_waiting};
use crate::fleet::model::{Msg, PartData};
use crate::fleet::work::{EndedReason, RunningWorkItem};
use serde_json::Value;
use std::collections::HashMap;

/// How background work ended, as its notice says (`BackgroundState`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundState {
    Completed,
    Error,
    Cancelled,
}

impl BackgroundState {
    fn of(word: &str) -> Self {
        match word {
            "error" => Self::Error,
            "cancelled" => Self::Cancelled,
            _ => Self::Completed,
        }
    }

    /// The word the chip shows: "stopped" is something you did, not a failure.
    pub fn word(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Error => "error",
            Self::Cancelled => "stopped",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundKind {
    Shell,
    Subagent,
}

impl BackgroundKind {
    /// The chip's label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Shell => "Background command",
            Self::Subagent => "Background helper",
        }
    }
}

/// `BackgroundNotice`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundNotice {
    pub kind: BackgroundKind,
    /// The shell id or child session the notice is about, which matches the call that started it.
    pub id: String,
    pub state: BackgroundState,
    /// The command, or what the sub-agent was asked to do.
    pub label: String,
    /// The output, or what the sub-agent said.
    pub text: String,
}

/// How each piece of background work ended, by handle; work still running isn't in it.
pub type Finished = HashMap<String, BackgroundState>;

/// `^<open …>\n…\n</close>\s*$`, as the regexes of `background-work.ts` read a notice: `open` is what the tag starts with
/// up to the id's quote, then ` state="…"` and ` label_attr="…"` follow. The label is matched lazily (the first `">\n`
/// after which the notice closes), the text greedily.
fn parse_tag(body: &str, open: &str, close: &str, id_attr: &str, label_attr: &str) -> Option<(String, BackgroundState, String, String)> {
    let rest = body.strip_prefix(&format!("<{open}{id_attr}=\""))?;
    let (id, rest) = rest.split_once('"')?;
    let rest = rest.strip_prefix(" state=\"")?;
    let (state, rest) = rest.split_once('"')?;
    let rest = rest.strip_prefix(&format!(" {label_attr}=\""))?;
    // The closing tag sits on its own line, after the text.
    let inside = rest.trim_end().strip_suffix(&format!("</{close}>"))?.strip_suffix('\n')?;
    // The label ends at the first `">\n`: the text follows, and `">\n</tag>` (no text line) is no notice.
    let at = inside.find("\">\n")?;
    Some((id.to_string(), BackgroundState::of(state), inside[..at].to_string(), inside[at + 3..].to_string()))
}

/// `looksLikeBackgroundNotice`: whether a message body may be a notice, before parsing it.
pub fn looks_like_background_notice(body: &str) -> bool {
    body.starts_with("<shell ") || body.starts_with("<subagent ") || body.starts_with("<background-result ")
}

/// `parseBackgroundNotice`: the background work a message says finished, or `None` for anything else. Newer OpenCode 2
/// writes `<background-result kind="shell|subagent" …>` closed by `</background-result>`; both shapes are read. Image,
/// video and codemode notices stay text.
pub fn parse_background_notice(body: &str) -> Option<BackgroundNotice> {
    // Nearly every message is neither: leave at the first byte.
    if !looks_like_background_notice(body) {
        return None;
    }
    let shell = parse_tag(body, "shell ", "shell", "id", "command")
        .or_else(|| parse_tag(body, "background-result kind=\"shell\" ", "background-result", "id", "command"));
    if let Some((id, state, label, text)) = shell {
        return Some(BackgroundNotice { kind: BackgroundKind::Shell, id, state, label, text });
    }
    let (id, state, label, text) = parse_tag(body, "subagent ", "subagent", "sessionID", "description")
        .or_else(|| parse_tag(body, "background-result kind=\"subagent\" ", "background-result", "sessionID", "description"))?;
    Some(BackgroundNotice { kind: BackgroundKind::Subagent, id, state, label, text })
}

/// `backgroundWorkId`: the handle a call's state carries when its work went to the background, or `None`. Only a call
/// that has returned: an ordinary running sub-agent reports its child the same way.
pub fn background_work_id(state: &Value) -> Option<String> {
    if state.get("background") != Some(&Value::Bool(true)) {
        return None;
    }
    let metadata = state.get("metadata").filter(|m| m.is_object())?;
    ["shellID", "sessionID"].iter().find_map(|k| metadata.get(k).and_then(Value::as_str).filter(|v| !v.is_empty())).map(str::to_string)
}

/// `finishedBackgroundWork`.
pub fn finished_background_work<'a>(bodies: impl IntoIterator<Item = &'a str>) -> Finished {
    bodies.into_iter().filter_map(parse_background_notice).map(|n| (n.id, n.state)).collect()
}

/// `backgroundStatus`: what a call that went to the background shows, "Background" while its work runs, then how it
/// ended. `None` for any other call.
pub fn background_status(state: &Value, finished: &Finished) -> Option<ToolStatus> {
    let id = background_work_id(state)?;
    Some(match finished.get(&id) {
        None => ToolStatus::Background,
        Some(BackgroundState::Completed) => ToolStatus::Completed,
        Some(BackgroundState::Error) => ToolStatus::Error,
        // One Stop, one word: the strip says stopped too.
        Some(BackgroundState::Cancelled) => ToolStatus::Stopped,
    })
}

/// `workStatus` of `toToolCardItem`: what a call shows from the running-work item it started. "Background" while that
/// work runs on after the call returned (a Claude Code call reports itself done at once), "Stopped" once Fleet stopped
/// it; `None` leaves the call's own state to say. Not yet read by the list: it needs the work item of each call
/// (`workByCall`) in `ListContext`.
pub fn work_status(work: Option<&RunningWorkItem>) -> Option<ToolStatus> {
    let work = work?;
    if crate::fleet::work::is_running(work) {
        return work.background.then_some(ToolStatus::Background);
    }
    (work.ended_reason == Some(EndedReason::Cancelled)).then_some(ToolStatus::Stopped)
}

/// `withEnd`: a notice with how its work really ended. Work Fleet stopped reads stopped; the error the harness reported
/// for it is the stop's doing (a removed shell is "not found"), not the work's, so it isn't shown.
pub fn with_end(mut notice: BackgroundNotice, finished: &Finished) -> BackgroundNotice {
    if let Some(&state) = finished.get(&notice.id)
        && state != notice.state
    {
        if notice.state == BackgroundState::Error {
            notice.text.clear();
        }
        notice.state = state;
    }
    notice
}

/// How the session's background work stands (`use-background-work.ts`): by its notices, and work Fleet stopped ended
/// stopped whatever the notice says. `older` is work the stream no longer holds, asked of the server when a notice needs it.
pub fn finished_of(messages: &[Msg], running: &[RunningWorkItem], older: &[RunningWorkItem]) -> Finished {
    let bodies: Vec<String> = messages.iter().map(|m| render_message_body(&m.parts)).collect();
    let mut finished = finished_background_work(bodies.iter().map(String::as_str));
    let work = work_by_call(running, older);
    for (handle, call) in background_calls(messages) {
        if work.get(call.as_str()).is_some_and(|w| w.ended_reason == Some(EndedReason::Cancelled)) {
            finished.insert(handle, BackgroundState::Cancelled);
        }
    }
    finished
}

fn work_by_call<'a>(running: &'a [RunningWorkItem], older: &'a [RunningWorkItem]) -> HashMap<&'a str, &'a RunningWorkItem> {
    older.iter().chain(running).filter_map(|w| Some((w.tool_call_id.as_deref()?, w))).collect()
}

/// The call each piece of background work came from, by its handle (`backgroundCalls`).
fn background_calls(messages: &[Msg]) -> HashMap<String, String> {
    let mut calls = HashMap::new();
    for part in messages.iter().flat_map(|m| &m.parts) {
        if let PartData::Tool { call_id, state, .. } = part
            && !call_id.is_empty()
            && let Some(handle) = background_work_id(state)
        {
            calls.insert(handle, call_id.clone());
        }
    }
    calls
}

/// Whether a notice says its work failed while the stream holds no item for its call: the older work is asked of the
/// server once, so work Fleet stopped still reads stopped when the conversation is opened later.
pub fn needs_older_work(messages: &[Msg], running: &[RunningWorkItem], older: &[RunningWorkItem]) -> bool {
    let bodies: Vec<String> = messages.iter().map(|m| render_message_body(&m.parts)).collect();
    let notices = finished_background_work(bodies.iter().map(String::as_str));
    let work = work_by_call(running, older);
    let calls = background_calls(messages);
    notices
        .iter()
        .any(|(handle, state)| *state == BackgroundState::Error && calls.get(handle).is_some_and(|c| !work.contains_key(c.as_str())))
}

fn input_text(state: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|k| state.pointer(&format!("/input/{k}")).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()))
        .unwrap_or_default()
        .to_string()
}

/// `subagentKind`: the kind of sub-agent a call asked for: OpenCode names it `subagent_type`, OpenCode 2 `agent`.
pub fn subagent_kind(state: &Value) -> String {
    input_text(state, &["subagent_type", "agent"])
}

/// `subagentTask`: what the call asked the sub-agent to do, in its own words.
pub fn subagent_task(state: &Value) -> String {
    input_text(state, &["description"])
}

/// `withDelegation`'s match: the delegation a sub-agent call started, once it has a session.
pub fn delegation_of<'a>(list: &'a [Delegation], call_id: &str, part_id: &str) -> Option<&'a Delegation> {
    list.iter().find(|d| {
        (d.parent_tool_call_id.as_deref() == Some(call_id) || d.parent_tool_call_id.as_deref() == Some(part_id))
            && d.child_session_id.is_some()
    })
}

/// What a sub-agent call's input says it is: the kind of agent and its task, empty when the call doesn't say.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentCall {
    pub kind: String,
    pub task: String,
}

impl AgentCall {
    pub fn of(state: &Value) -> AgentCall {
        AgentCall { kind: subagent_kind(state), task: subagent_task(state) }
    }
}

/// What `AgentTaskRow.vue` shows of one sub-agent call.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentRow {
    pub agent: String,
    pub task: String,
    pub status: DelegationStatus,
    pub needs_input: bool,
    pub background: bool,
    pub child_session_id: String,
}

impl AgentRow {
    /// `withDelegation` for a call: its kind and task from the input (the delegation's title when the call has none),
    /// its progress from the delegation.
    pub fn of(call: &AgentCall, d: &Delegation) -> Option<AgentRow> {
        Some(AgentRow {
            agent: call.kind.clone(),
            task: if call.task.is_empty() { d.title.clone() } else { call.task.clone() },
            status: d.status,
            needs_input: is_waiting(d),
            background: d.background,
            child_session_id: d.child_session_id.clone()?,
        })
    }

    /// Working is pending or running.
    pub fn working(&self) -> bool {
        self.status.active()
    }

    /// The sub-agent's question holds it up, and its session is where you answer; a stale one after it finished is ignored.
    pub fn asks(&self) -> bool {
        self.working() && self.needs_input
    }

    /// Its call returned: the session is free while it works, and hears when it's done.
    pub fn in_background(&self) -> bool {
        self.working() && self.background
    }

    pub fn status_word(&self) -> &'static str {
        if self.asks() {
            return "Needs input";
        }
        if self.in_background() {
            return "Running in the background";
        }
        match self.status {
            DelegationStatus::Completed => "Done",
            DelegationStatus::Error => "Failed",
            DelegationStatus::Cancelled => "Stopped",
            DelegationStatus::Pending => "Starting",
            DelegationStatus::Running => "Working",
        }
    }

    pub fn tip(&self) -> String {
        if self.asks() { format!("Open {} to answer its question", self.task) } else { format!("Open {}", self.task) }
    }
}
