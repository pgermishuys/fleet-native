//! The Agents tab's data (`lib/session-lineage.ts` from "The Agents tab", `AgentsCanvas.vue`, `AgentsCanvasRow.vue`,
//! `AgentDetail.vue`, `use-session-lineage.ts`): what the session's agent runs, what it started, what ended, the words a row
//! and its detail say, and the history of ended work the tab keeps per session. Plain logic and `State` glue; no Rune views.
use super::groups::{LineageKind, lineage_of};
use super::model::{Msg, PartData};
use super::work::{
    EndedReason, RunningWorkItem, WorkKind, WorkStatus, is_running, parse_iso_ms, work_elapsed_ms, work_failed, work_result,
};
use crate::State;
use crate::composer::choice::ModelOption;
use crate::composer::format::format_tokens;
use crate::composer::status::HarnessInfo;
use crate::conv::tools::get_tool;
use crate::conv::working::format_duration;
use crate::mock::Session;
use crate::view::conversation::Role;
use std::collections::HashMap;

/// Where a row stands: working, waiting on the user, or how it ended (`AgentRowState`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentRowState {
    Running,
    Waiting,
    Idle,
    Done,
    Failed,
    Stopped,
}

impl AgentRowState {
    /// `data-state`.
    pub fn word(self) -> &'static str {
        match self {
            AgentRowState::Running => "running",
            AgentRowState::Waiting => "waiting",
            AgentRowState::Idle => "idle",
            AgentRowState::Done => "done",
            AgentRowState::Failed => "failed",
            AgentRowState::Stopped => "stopped",
        }
    }

    /// Running and waiting rows are what "Running now" holds.
    pub fn is_active(self) -> bool {
        matches!(self, AgentRowState::Running | AgentRowState::Waiting)
    }
}

/// What a row is: work the agent ran (a sub-agent or a task), or a session it forked or started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentKind {
    Subagent,
    Task,
    Fork,
    Started,
}

/// `kindLabel` of the row: "subagent", "fork", "started" and "task" (the session list's has no "task").
pub fn kind_label(kind: AgentKind) -> &'static str {
    match kind {
        AgentKind::Subagent => "subagent",
        AgentKind::Task => "task",
        AgentKind::Fork => "fork",
        AgentKind::Started => "started",
    }
}

fn kind_of(kind: LineageKind) -> AgentKind {
    match kind {
        LineageKind::Subagent => AgentKind::Subagent,
        LineageKind::Fork => AgentKind::Fork,
        LineageKind::Started => AgentKind::Started,
    }
}

/// What a row reads of the session list's item: enough to name where it runs and to open it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListedSession {
    pub id: String,
    pub instance_id: Option<String>,
    pub project_name: Option<String>,
    pub harness_type: Option<String>,
    /// `selectedModel.modelID ?? lastAssistantModelId`.
    pub model_id: Option<String>,
}

impl ListedSession {
    pub fn of(session: &Session) -> ListedSession {
        ListedSession {
            id: session.id.clone(),
            instance_id: session.listing.instance_id.clone(),
            project_name: session.listing.project_name.clone(),
            harness_type: session.row.harness_type.clone(),
            model_id: session.row.selected_model.as_ref().map(|m| m.model_id.clone()).or_else(|| session.row.last_model_id.clone()),
        }
    }
}

/// `AgentRow`.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentRow {
    /// `work:{id}` or `session:{id}`.
    pub key: String,
    pub kind: AgentKind,
    pub name: String,
    /// What it was asked to do, when that is more than its name.
    pub task: Option<String>,
    pub state: AgentRowState,
    /// The Fleet session it runs in, to open; none when it has none.
    pub session_id: Option<String>,
    pub work: Option<RunningWorkItem>,
    pub session: Option<ListedSession>,
}

/// `AgentsLineage`: the tab's three sections.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgentsLineage {
    pub running: Vec<AgentRow>,
    /// Sessions this one forked or started that aren't working now.
    pub started: Vec<AgentRow>,
    /// Sub-agents and tasks that ended, newest first.
    pub earlier: Vec<AgentRow>,
}

impl AgentsLineage {
    /// `activeCount`: what the tab's badge counts, and what makes a collapsed panel open by itself (row 5.50).
    pub fn active_count(&self) -> usize {
        self.running.len()
    }

    /// Nothing to show: no parent and all three sections empty (the empty state of row 5.9).
    pub fn is_empty(&self, has_parent: bool) -> bool {
        !has_parent && self.running.is_empty() && self.started.is_empty() && self.earlier.is_empty()
    }

    /// The head's words after "Lineage": "1 running · 1 waiting for you"; none with nothing running or waiting.
    pub fn summary(&self) -> Option<String> {
        let count = |state| self.running.iter().filter(|r| r.state == state).count();
        let (running, waiting) = (count(AgentRowState::Running), count(AgentRowState::Waiting));
        let mut parts = Vec::new();
        if running > 0 {
            parts.push(format!("{running} running"));
        }
        if waiting > 0 {
            parts.push(format!("{waiting} waiting for you"));
        }
        (!parts.is_empty()).then(|| parts.join(" · "))
    }

    /// The Earlier fold's button: "Earlier agents · 2".
    pub fn earlier_label(&self) -> String {
        format!("Earlier agents · {}", self.earlier.len())
    }

    /// Every row the sections can show, in the order they show.
    pub fn rows(&self) -> impl Iterator<Item = &AgentRow> {
        self.running.iter().chain(&self.started).chain(&self.earlier)
    }
}

/// `workState`.
pub fn work_state(item: &RunningWorkItem) -> AgentRowState {
    if is_running(item) {
        AgentRowState::Running
    } else if work_failed(item) {
        AgentRowState::Failed
    } else if item.status == WorkStatus::Cancelled || item.ended_reason == Some(EndedReason::Cancelled) {
        AgentRowState::Stopped
    } else {
        AgentRowState::Done
    }
}

/// `sessionAgentState`, with the six states the row needs.
pub fn session_state(lifecycle: &str, activity: &str) -> AgentRowState {
    match super::lineage::agent_state(lifecycle, activity) {
        super::lineage::AgentState::Waiting => AgentRowState::Waiting,
        super::lineage::AgentState::Running => AgentRowState::Running,
        super::lineage::AgentState::Failed => AgentRowState::Failed,
        super::lineage::AgentState::Idle => AgentRowState::Idle,
    }
}

fn work_row(item: &RunningWorkItem) -> AgentRow {
    let task = item.label.clone().filter(|l| !l.is_empty() && *l != item.title);
    let name = [Some(item.title.as_str()), item.label.as_deref()].into_iter().flatten().find(|s| !s.is_empty()).unwrap_or("subagent");
    AgentRow {
        key: format!("work:{}", item.id),
        kind: if item.kind == WorkKind::Task { AgentKind::Task } else { AgentKind::Subagent },
        name: name.to_string(),
        task,
        state: work_state(item),
        session_id: item.child_session_id.clone(),
        work: Some(item.clone()),
        session: None,
    }
}

fn session_row(session: &Session, kind: LineageKind) -> AgentRow {
    let title = session.title.trim();
    AgentRow {
        key: format!("session:{}", session.id),
        kind: kind_of(kind),
        name: if title.is_empty() { "Untitled session".into() } else { title.to_string() },
        task: None,
        state: session_state(&session.row.lifecycle, &session.row.activity),
        session_id: Some(session.id.clone()),
        work: None,
        session: Some(ListedSession::of(session)),
    }
}

/// `buildAgentsLineage`: what runs now (its sub-agents and tasks, and the sessions it started that are working or waiting on
/// you), the sessions it forked or started, and the sub-agents that ended, newest first. Shells and monitors stay in the
/// background strip. A sub-agent's session is listed through its work, once.
pub fn build_agents_lineage(session_id: &str, sessions: &[&Session], work: &[RunningWorkItem]) -> AgentsLineage {
    let agent_work: Vec<&RunningWorkItem> =
        work.iter().filter(|i| i.session_id == session_id && matches!(i.kind, WorkKind::Subagent | WorkKind::Task)).collect();
    let work_rows: Vec<AgentRow> = agent_work.iter().map(|i| work_row(i)).collect();
    let subagent_sessions: Vec<&str> = agent_work.iter().filter_map(|i| i.child_session_id.as_deref()).collect();

    let mut child_rows = Vec::new();
    for session in sessions {
        if session.id == session_id || subagent_sessions.contains(&session.id.as_str()) {
            continue;
        }
        let Some((parent, kind)) = lineage_of(&session.listing) else { continue };
        if parent != session_id || kind == LineageKind::Subagent {
            continue;
        }
        child_rows.push(session_row(session, kind));
    }

    let ended_at = |row: &AgentRow| {
        let work = row.work.as_ref();
        let started = work.and_then(|w| parse_iso_ms(&w.started_at)).unwrap_or(0);
        work.and_then(|w| w.ended_at.as_deref()).and_then(parse_iso_ms).unwrap_or(started)
    };
    let (active_work, past_work): (Vec<_>, Vec<_>) = work_rows.into_iter().partition(|r| r.state.is_active());
    let (active_child, past_child): (Vec<_>, Vec<_>) = child_rows.into_iter().partition(|r| r.state.is_active());
    let mut earlier = past_work;
    // A stable sort: ended together, the order they came.
    earlier.sort_by_key(|r| std::cmp::Reverse(ended_at(r)));
    AgentsLineage { running: active_work.into_iter().chain(active_child).collect(), started: past_child, earlier }
}

// ─── The words of a row ────────────────────────────────────────────────────────────────────────────────────────────

/// The state word at the row's right (`stateText`).
pub fn state_text(row: &AgentRow, now: i64) -> String {
    let ran = || row.work.as_ref().map_or(0, |w| work_elapsed_ms(w, now));
    match row.state {
        AgentRowState::Running => match &row.work {
            Some(w) => format_duration(work_elapsed_ms(w, now)),
            None => "working".into(),
        },
        AgentRowState::Waiting => "needs you".into(),
        AgentRowState::Failed => {
            if row.work.as_ref().is_some_and(|w| w.ended_reason == Some(EndedReason::Lost)) { "lost" } else { "failed" }.into()
        }
        AgentRowState::Stopped => "stopped".into(),
        // How long it ran; "done" for one that answered at once.
        AgentRowState::Done => {
            if ran() >= 1000 {
                format_duration(ran())
            } else {
                "done".into()
            }
        }
        AgentRowState::Idle => "idle".into(),
    }
}

/// The note after the sub line (`note`): a waiting row says so; running work with no session says it has none.
pub fn note(row: &AgentRow) -> Option<&'static str> {
    if row.state == AgentRowState::Waiting {
        Some("asked a question")
    } else if row.work.is_some() && row.session_id.is_none() && row.state == AgentRowState::Running {
        Some("no session of its own")
    } else {
        None
    }
}

/// `lineageParentNote`: what the parent did, on the parent's row.
pub fn parent_note(kind: LineageKind) -> &'static str {
    match kind {
        LineageKind::Fork => "forked into this session",
        LineageKind::Subagent => "delegated this session",
        LineageKind::Started => "started this session",
    }
}

fn harness_name(kind: Option<&str>, harnesses: &[HarnessInfo]) -> Option<String> {
    let kind = kind.filter(|k| !k.is_empty())?;
    Some(harnesses.iter().find(|h| h.kind == kind).and_then(|h| h.display_name.clone()).unwrap_or_else(|| kind.to_string()))
}

/// Joins what is known with " · "; none when nothing is.
fn dotted(parts: impl IntoIterator<Item = Option<String>>) -> Option<String> {
    let text = parts.into_iter().flatten().collect::<Vec<_>>().join(" · ");
    (!text.is_empty()).then_some(text)
}

/// What a row runs on (`runsOn`): "Claude Code · Sonnet 5.5". The harness is the row's session's, else this session's (a
/// sub-agent runs in its parent's harness). The model is the one the row's own session last answered with (`seen`, kept once
/// its detail has read it), else the listed session's.
pub fn runs_on(
    row: &AgentRow,
    own_harness: Option<&str>,
    seen: &HashMap<String, String>,
    harnesses: &[HarnessInfo],
    models: &[ModelOption],
) -> Option<String> {
    let listed = row.session.as_ref();
    let harness = harness_name(listed.and_then(|s| s.harness_type.as_deref()).or(own_harness), harnesses);
    let model_id = seen.get(&row.key).cloned().or_else(|| listed.and_then(|s| s.model_id.clone()));
    let model = model_id.filter(|m| !m.is_empty()).map(|m| super::turns::model_display_name(Some(&m), models));
    dotted([harness, model])
}

/// The parent row's `runsOn`: project · harness · model, of the parent's list item.
pub fn parent_runs_on(parent: Option<&ListedSession>, harnesses: &[HarnessInfo], models: &[ModelOption]) -> Option<String> {
    let p = parent?;
    let model = p.model_id.as_deref().filter(|m| !m.is_empty()).map(|m| super::turns::model_display_name(Some(m), models));
    dotted([p.project_name.clone().filter(|n| !n.is_empty()), harness_name(p.harness_type.as_deref(), harnesses), model])
}

/// The parent's row: its title (else "Parent session"), its state from its list item (else idle), and no work.
pub fn parent_row(parent_id: &str, kind: LineageKind, title: Option<String>, listed: Option<&Session>) -> AgentRow {
    AgentRow {
        key: format!("parent:{parent_id}"),
        kind: kind_of(kind),
        name: title.unwrap_or_else(|| "Parent session".into()),
        task: None,
        state: listed.map_or(AgentRowState::Idle, |s| session_state(&s.row.lifecycle, &s.row.activity)),
        session_id: Some(parent_id.to_string()),
        work: None,
        session: listed.map(ListedSession::of),
    }
}

/// The row's accessible label: name, task, kind (not on the parent), runsOn, note and state, joined with ", ".
pub fn label(row: &AgentRow, is_parent: bool, runs_on: Option<&str>, now: i64) -> String {
    let kind = (!is_parent).then(|| kind_label(row.kind));
    let state = state_text(row, now);
    [Some(row.name.as_str()), row.task.as_deref(), kind, runs_on, note(row), Some(state.as_str())]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ")
}

// ─── Selection ─────────────────────────────────────────────────────────────────────────────────────────────────────

/// Which row's detail shows: the one picked, else the first running row that has work, opened without asking. A pick of `""`
/// (a closed detail) is a choice too, so the automatic one doesn't come back (`shownKey`).
pub fn shown_key<'a>(selected: Option<&'a str>, lineage: &'a AgentsLineage) -> Option<&'a str> {
    match selected {
        Some("") => None,
        Some(key) => Some(key),
        None => lineage.running.iter().find(|r| r.work.is_some()).map(|r| r.key.as_str()),
    }
}

/// Pressing a row (`select`): the shown one closes, another opens.
pub fn select(selected: &mut Option<String>, lineage: &AgentsLineage, key: &str) {
    let shown = shown_key(selected.as_deref(), lineage).map(str::to_string);
    *selected = Some(if shown.as_deref() == Some(key) { String::new() } else { key.to_string() });
}

// ─── What a row's own session shows ────────────────────────────────────────────────────────────────────────────────

/// `AgentActivity`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgentActivity {
    /// The first thing it was asked, in its own session.
    pub asked: Option<String>,
    pub tool_calls: usize,
    /// The tools it used most, most used first.
    pub top_tools: Vec<String>,
    pub tokens_in: i64,
    pub tokens_out: i64,
    /// The last thing it said, on one line.
    pub latest: Option<String>,
    /// The model that answered last.
    pub model_id: Option<String>,
}

fn last_line(text: &str) -> Option<String> {
    text.lines().map(str::trim).rfind(|l| !l.is_empty()).map(str::to_string)
}

/// `latestOutputLine`: the last non-empty trimmed line of a work item's output.
pub fn latest_output_line(output: &str) -> Option<String> {
    last_line(output)
}

/// `summarizeAgentActivity`.
pub fn summarize_agent_activity(messages: &[Msg]) -> AgentActivity {
    let mut out = AgentActivity::default();
    let mut uses: Vec<(String, usize)> = Vec::new();
    for message in messages {
        match message.role {
            Role::User => {
                if out.asked.is_none() {
                    let text = message
                        .parts
                        .iter()
                        .filter_map(|p| if let PartData::Text { text, .. } = p { Some(text.as_str()) } else { None })
                        .collect::<Vec<_>>()
                        .join("\n");
                    out.asked = text.trim().lines().next().map(|l| l.trim().to_string()).filter(|l| !l.is_empty());
                }
            }
            Role::Assistant => {
                out.tokens_in += message.tokens.map_or(0, |t| t.input);
                out.tokens_out += message.tokens.map_or(0, |t| t.output);
                if let Some(model) = message.model_id.as_ref().filter(|m| !m.is_empty()) {
                    out.model_id = Some(model.clone());
                }
                for part in &message.parts {
                    match part {
                        PartData::Tool { tool, .. } => {
                            out.tool_calls += 1;
                            let name = get_tool(tool).display_name;
                            match uses.iter_mut().find(|(n, _)| *n == name) {
                                Some((_, count)) => *count += 1,
                                None => uses.push((name, 1)),
                            }
                        }
                        PartData::Text { text, .. } if !text.trim().is_empty() => {
                            out.latest = last_line(text).or(out.latest.take());
                        }
                        _ => {}
                    }
                }
            }
            Role::Shell => {}
        }
    }
    // A stable sort keeps the order tools were first used among equals, as a `Map` sort does.
    uses.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    out.top_tools = uses.into_iter().take(3).map(|(name, _)| name).collect();
    out
}

// ─── The detail ────────────────────────────────────────────────────────────────────────────────────────────────────

/// Where Open session goes (row 5.36).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenTarget {
    /// A work row's child, under this session: `parentSessionId` is this session.
    Child { child: String, instance_id: Option<String>, parent: String },
    /// A fork or started session, on its own.
    Own { session: String, instance_id: Option<String> },
}

/// What a selected row's detail shows (`AgentDetail.vue`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Detail {
    /// "Asked to": the work's label, else the first line of the first prompt in its own session.
    pub asked: Option<String>,
    /// "Tools": "2 calls · Read, Grep"; none at 0 calls.
    pub tools: Option<String>,
    /// "Tokens": "38.2k in · 1.9k out"; none when both are 0.
    pub tokens: Option<String>,
    /// "Ended": the work's result, ended work only.
    pub ended: Option<String>,
    /// The last line of what it said, or of its output.
    pub latest: Option<String>,
    /// "Reading its session…" while its session loads and there is no latest line.
    pub reading: bool,
    pub open: Option<OpenTarget>,
    /// Stop is offered (it runs and the server can stop it).
    pub can_stop: bool,
    /// "Stopping…", disabled.
    pub stopping: bool,
    /// Why the last Stop was refused.
    pub stop_error: Option<String>,
    /// "Runs inside the tool call. Interrupt the turn to stop it.": no Open and no Stop for work that runs.
    pub inside_turn: bool,
}

/// The words of the tools line: "1 call" or "n calls", then the most used.
pub fn tools_text(activity: &AgentActivity) -> Option<String> {
    if activity.tool_calls == 0 {
        return None;
    }
    let calls = if activity.tool_calls == 1 { "1 call".to_string() } else { format!("{} calls", activity.tool_calls) };
    Some(if activity.top_tools.is_empty() { calls } else { format!("{calls} · {}", activity.top_tools.join(", ")) })
}

/// The words of the tokens line, none when both are 0.
pub fn tokens_text(activity: &AgentActivity) -> Option<String> {
    if activity.tokens_in == 0 && activity.tokens_out == 0 {
        return None;
    }
    Some(format!("{} in · {} out", format_tokens(Some(activity.tokens_in as f64)), format_tokens(Some(activity.tokens_out as f64))))
}

/// What the detail's request for the row's work output said, kept for the row: read once, for work with no session.
pub struct DetailInput<'a> {
    pub row: &'a AgentRow,
    /// The session whose Agents tab this is.
    pub this_session: &'a str,
    /// What the row's own session shows, once its stream has said (none while it loads, and for a row with no session).
    pub activity: Option<&'a AgentActivity>,
    /// The last line of the work's output, once read.
    pub output_line: Option<&'a str>,
    pub stopping: bool,
    pub stop_error: Option<&'a str>,
}

/// `AgentDetail`'s model.
pub fn detail(input: &DetailInput) -> Detail {
    let row = input.row;
    let work = row.work.as_ref();
    let activity = input.activity;
    let asked = work.and_then(|w| w.label.clone().filter(|l| !l.is_empty())).or_else(|| activity.and_then(|a| a.asked.clone()));
    // A row with no session of its own reads the work's output instead, when the server can give it.
    let latest = activity.and_then(|a| a.latest.clone()).or_else(|| {
        (row.session_id.is_none() && work.is_some_and(|w| w.can_read_output)).then(|| input.output_line.map(str::to_string)).flatten()
    });
    let running = work.is_some_and(is_running);
    let can_stop = running && work.is_some_and(|w| w.can_stop);
    let open = row.session_id.as_ref().map(|id| match (&row.work, &row.session) {
        (Some(_), listed) => OpenTarget::Child {
            child: id.clone(),
            instance_id: listed.as_ref().and_then(|s| s.instance_id.clone()),
            parent: input.this_session.to_string(),
        },
        (None, listed) => OpenTarget::Own { session: id.clone(), instance_id: listed.as_ref().and_then(|s| s.instance_id.clone()) },
    });
    Detail {
        asked,
        tools: activity.and_then(tools_text),
        tokens: activity.and_then(tokens_text),
        ended: work.and_then(work_result),
        reading: row.session_id.is_some() && activity.is_none() && latest.is_none(),
        latest,
        inside_turn: running && row.session_id.is_none() && !can_stop,
        open,
        can_stop,
        stopping: input.stopping,
        stop_error: input.stop_error.map(str::to_string),
    }
}

// ─── The tab's work, kept per session ──────────────────────────────────────────────────────────────────────────────

impl State {
    /// A session's work as the tab sees it (`useRunningWork`): what was kept of the ended, with what the open stream says now
    /// over it, by id.
    pub fn agents_work(&self, id: &str) -> Vec<RunningWorkItem> {
        let mut items = self.agents_history.get(id).cloned().unwrap_or_default();
        if self.open_id().as_deref() == Some(id) {
            for item in self.running_work() {
                super::work::apply_item(&mut items, item.clone());
            }
        }
        items
    }

    /// The tab's three sections for a session, from the list and its work.
    pub fn agents_lineage(&self, id: &str) -> AgentsLineage {
        let sessions: Vec<&Session> = self.projects.iter().flat_map(|p| &p.sessions).collect();
        build_agents_lineage(id, &sessions, &self.agents_work(id))
    }

    /// Reads all of a session's work (`loadHistory`), off the UI thread: on showing the tab, when its session changes, and
    /// after a reconnect. An answer for work this session doesn't own is dropped by id.
    pub fn load_agents_history(&mut self, id: &str) {
        let Some(link) = self.link_for(id) else { return };
        let asked = id.to_string();
        let session = id.to_string();
        link.off_thread(move |base| super::http::work_all(base, &asked), move |s, _, result| s.on_agents_history(&session, result));
    }

    /// The answer of `load_agents_history`: ended items are kept; what is kept stays under what the stream says now.
    pub fn on_agents_history(&mut self, id: &str, result: Result<(u16, serde_json::Value), String>) {
        let Ok((status, body)) = result else { return };
        if !(200..300).contains(&status) {
            return;
        }
        let kept = self.agents_history.entry(id.to_string()).or_default();
        for item in super::work::to_items(Some(&body)).into_iter().filter(|i| i.session_id == id) {
            super::work::apply_item(kept, item);
        }
    }

    /// An item the server says ended (a Stop's answer) joins the history of its session at once.
    pub fn keep_in_agents_history(&mut self, item: &RunningWorkItem) {
        if let Some(kept) = self.agents_history.get_mut(&item.session_id) {
            super::work::apply_item(kept, item.clone());
        }
    }
}
