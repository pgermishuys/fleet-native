//! What the Progress tab and the progress strip read (`lib/session-progress.ts`, `lib/todo-utils.ts` and the choices
//! `ProgressCanvas.vue` makes about its data): the detail the server keeps per session, its plan, todos and subagents, and
//! the small rules over them. Pure: no Rune, no I/O. The summary (`done`, `total`, `current`) is `row_chips::Progress`.
use super::delegations::DelegationStatus;
use super::row_chips::{self, Progress};
use super::work::parse_iso_ms;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TodoPriority {
    High,
    Medium,
    Low,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TodoItem {
    pub content: String,
    pub status: TodoStatus,
    pub priority: TodoPriority,
}

/// `normalizeTodoItem`: `None` for an item without text; an unknown status is pending, an unknown priority medium.
pub fn parse_todo(v: &Value) -> Option<TodoItem> {
    let content = v.get("content")?.as_str().filter(|c| !c.trim().is_empty())?;
    let status = match v.get("status").and_then(Value::as_str) {
        Some("in_progress") => TodoStatus::InProgress,
        Some("completed") => TodoStatus::Completed,
        Some("cancelled") => TodoStatus::Cancelled,
        _ => TodoStatus::Pending,
    };
    let priority = match v.get("priority").and_then(Value::as_str) {
        Some("high") => TodoPriority::High,
        Some("low") => TodoPriority::Low,
        _ => TodoPriority::Medium,
    };
    Some(TodoItem { content: content.to_string(), status, priority })
}

/// One checkbox step of a plan.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanStep {
    pub key: String,
    pub number: Option<String>,
    pub title: String,
    pub checked: bool,
    pub sub_done: u32,
    pub sub_total: u32,
    /// When Fleet saw the box ticked; `None` if before it was watching, or not ticked.
    pub ticked_at: Option<String>,
    pub ticked_in_message_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanGroup {
    pub title: Option<String>,
    pub steps: Vec<PlanStep>,
}

/// A checklist plan the session is working through.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// Relative to the session's folder.
    pub path: String,
    pub title: Option<String>,
    /// When Fleet first read the file; empty when it didn't say.
    pub tracked_since: String,
    pub groups: Vec<PlanGroup>,
}

/// A subagent the session started, and how far along its own session is.
#[derive(Clone, Debug, PartialEq)]
pub struct Subagent {
    pub delegation_id: String,
    pub child_session_id: Option<String>,
    /// The kind of subagent, such as "shuttle".
    pub agent: String,
    /// The subagent session's title, usually its task.
    pub title: Option<String>,
    pub status: DelegationStatus,
    /// The plan step it was started for; `None` without a plan.
    pub step_key: Option<String>,
    pub done: u32,
    pub total: u32,
    pub current: Option<String>,
}

/// Everything the open session shows: `SessionProgressDetail`.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgressDetail {
    pub session_id: String,
    pub summary: Progress,
    pub todos: Vec<TodoItem>,
    /// ISO text; two of them compare as text, as Fleet's `apply` does.
    pub updated_at: String,
    pub plan: Option<Plan>,
    pub subagents: Vec<Subagent>,
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn count(v: &Value, key: &str) -> u32 {
    v.get(key).and_then(Value::as_u64).map_or(0, |n| n.min(u32::MAX as u64) as u32)
}

fn parse_subagent(v: &Value) -> Option<Subagent> {
    Some(Subagent {
        delegation_id: text(v, "delegationId")?,
        agent: text(v, "agent")?,
        child_session_id: text(v, "childSessionId"),
        title: text(v, "title"),
        status: DelegationStatus::from_wire(v.get("status").and_then(Value::as_str)),
        step_key: text(v, "stepKey"),
        done: count(v, "done"),
        total: count(v, "total"),
        current: text(v, "current"),
    })
}

fn parse_step(v: &Value) -> Option<PlanStep> {
    Some(PlanStep {
        key: text(v, "key")?,
        title: text(v, "title")?,
        number: text(v, "number"),
        checked: v.get("checked") == Some(&Value::Bool(true)),
        sub_done: count(v, "subDone"),
        sub_total: count(v, "subTotal"),
        ticked_at: text(v, "tickedAt"),
        ticked_in_message_id: text(v, "tickedInMessageId"),
    })
}

/// `parsePlan`: a group without a valid step is dropped, and no group left means no plan.
fn parse_plan(v: &Value) -> Option<Plan> {
    let path = text(v, "path")?;
    let groups: Vec<PlanGroup> = v
        .get("groups")?
        .as_array()?
        .iter()
        .filter_map(|g| {
            let steps: Vec<PlanStep> = g.get("steps")?.as_array()?.iter().filter_map(parse_step).collect();
            (!steps.is_empty()).then(|| PlanGroup { title: text(g, "title"), steps })
        })
        .collect();
    (!groups.is_empty()).then(|| Plan { path, title: text(v, "title"), tracked_since: text(v, "trackedSince").unwrap_or_default(), groups })
}

/// `parseProgressDetail`: a valid summary plus `updatedAt`, else `None`.
pub fn parse_detail(v: &Value) -> Option<ProgressDetail> {
    let (session_id, summary) = row_chips::parse_summary(v)?;
    let updated_at = text(v, "updatedAt")?;
    let list = |key: &str| v.get(key).and_then(Value::as_array);
    Some(ProgressDetail {
        session_id,
        summary,
        todos: list("todos").map(|a| a.iter().filter_map(parse_todo).collect()).unwrap_or_default(),
        updated_at,
        plan: v.get("plan").and_then(parse_plan),
        subagents: list("subagents").map(|a| a.iter().filter_map(parse_subagent).collect()).unwrap_or_default(),
    })
}

/// The step being worked on next, and where it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Current<'a> {
    pub group_index: usize,
    pub group: &'a PlanGroup,
    pub step: &'a PlanStep,
}

/// `currentPlanStep`: the first unticked step, groups in order; `None` when every step is ticked.
pub fn current_plan_step(plan: &Plan) -> Option<Current<'_>> {
    plan.groups
        .iter()
        .enumerate()
        .find_map(|(group_index, group)| group.steps.iter().find(|s| !s.checked).map(|step| Current { group_index, group, step }))
}

/// A run of text, and whether it was written between backticks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub code: bool,
}

/// The `` `x` `` runs of `s` (a backtick, one or more characters that aren't backticks, a backtick) as byte ranges.
fn backtick_runs(s: &str) -> Vec<(usize, usize)> {
    let b = s.as_bytes();
    let (mut runs, mut i) = (Vec::new(), 0);
    while i < b.len() {
        if b[i] == b'`'
            && let Some(len) = b[i + 1..].iter().position(|&c| c == b'`')
            && len > 0
        {
            runs.push((i, i + len + 2));
            i += len + 2;
        } else {
            i += 1;
        }
    }
    runs
}

/// `codeSpans`: the text split on backtick runs. A piece of more than two characters wrapped in backticks is code, without
/// them; empty pieces are dropped (so a lone `` `` `` stays text).
pub fn code_spans(s: &str) -> Vec<Span> {
    let mut pieces = Vec::new();
    let mut at = 0;
    for (start, end) in backtick_runs(s) {
        pieces.push(&s[at..start]);
        pieces.push(&s[start..end]);
        at = end;
    }
    pieces.push(&s[at..]);
    pieces
        .into_iter()
        .filter(|p| !p.is_empty())
        .map(|p| {
            if p.len() > 2 && p.starts_with('`') && p.ends_with('`') {
                Span { text: p[1..p.len() - 1].to_string(), code: true }
            } else {
                Span { text: p.to_string(), code: false }
            }
        })
        .collect()
}

/// `plainText`: the text without its backticks, for the one-line strip.
pub fn plain_text(s: &str) -> String {
    let (mut out, mut at) = (String::new(), 0);
    for (start, end) in backtick_runs(s) {
        out.push_str(&s[at..start]);
        out.push_str(&s[start + 1..end - 1]);
        at = end;
    }
    out.push_str(&s[at..]);
    out
}

/// `subagentTitle`: the session title without a trailing `(@… subagent)` (any case), trimmed; `None` when nothing is left.
pub fn subagent_title(subagent: &Subagent) -> Option<String> {
    let title = subagent.title.as_deref()?;
    let lower = title.to_ascii_lowercase();
    let end = lower.trim_end();
    let cut = lower
        .match_indices("(@")
        .map(|(i, _)| i)
        .find(|&i| end.ends_with("subagent)") && i + 2 <= end.len() - "subagent)".len() && !end[i + 2..end.len() - 1].contains(')'));
    let kept = cut.map_or(title, |i| &title[..i]).trim();
    (!kept.is_empty()).then(|| kept.to_string())
}

/// A plan with at most one group draws no group heads and one flow segment per step.
pub fn is_flat(plan: &Plan) -> bool {
    plan.groups.len() <= 1
}

/// How many of a group's steps are ticked.
pub fn group_done(group: &PlanGroup) -> usize {
    group.steps.iter().filter(|s| s.checked).count()
}

pub fn is_group_complete(group: &PlanGroup) -> bool {
    group_done(group) == group.steps.len()
}

/// The key a group's fold is held under: its place and title (`groupKey`).
pub fn group_key(index: usize, group: &PlanGroup) -> String {
    format!("{index}:{}", group.title.as_deref().unwrap_or_default())
}

/// Whether a group starts open: a flat plan always, otherwise a group that still has steps to do.
pub fn group_open_default(plan: &Plan, group: &PlanGroup) -> bool {
    is_flat(plan) || !is_group_complete(group)
}

/// The latest `tickedAt` among a group's steps. ISO text compares in time order.
pub fn last_tick(group: &PlanGroup) -> Option<&str> {
    group.steps.iter().filter_map(|s| s.ticked_at.as_deref()).max()
}

/// `formatTime`: the clock of an ISO time, en-US 12-hour (05's decision); empty for an empty or unreadable one.
pub fn format_time(iso: Option<&str>, offset_secs: i64) -> String {
    let Some(ms) = iso.and_then(parse_iso_ms) else { return String::new() };
    let (.., hour, minute) = crate::conv::time::civil(ms, offset_secs);
    crate::conv::time::clock(hour, minute)
}

/// The step's subagents: those started for it.
pub fn subagents_for<'a>(detail: &'a ProgressDetail, step: &PlanStep) -> Vec<&'a Subagent> {
    detail.subagents.iter().filter(|s| s.step_key.as_deref() == Some(step.key.as_str())).collect()
}

/// The subagents no step shows: no plan, no step key, or a key no step has. One in a folded group is shown nowhere.
pub fn unplaced(detail: &ProgressDetail) -> Vec<&Subagent> {
    let placed = |s: &Subagent| {
        detail
            .plan
            .as_ref()
            .is_some_and(|p| s.step_key.as_deref().is_some_and(|k| p.groups.iter().any(|g| g.steps.iter().any(|st| st.key == k))))
    };
    detail.subagents.iter().filter(|s| !placed(s)).collect()
}

/// The plan form's line under the flow bar (`progress-canvas__now`): `lead` is the bold group title or "Every step is
/// ticked"; `rest` follows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NowLine {
    pub lead: Option<String>,
    pub rest: String,
}

/// With a current step: **Group** · next up: step 4 (the group only in a phased plan that titles it), or "next up: the
/// next step" when the step has no number. With none left: **Every step is ticked**.
pub fn now_line(plan: &Plan) -> NowLine {
    let Some(current) = current_plan_step(plan) else {
        return NowLine { lead: Some("Every step is ticked".into()), rest: String::new() };
    };
    let next = match &current.step.number {
        Some(n) if !n.is_empty() => format!("next up: step {n}"),
        _ => "next up: the next step".into(),
    };
    match current.group.title.as_deref().filter(|t| !is_flat(plan) && !t.is_empty()) {
        Some(title) => NowLine { lead: Some(title.to_string()), rest: format!(" · {next}") },
        None => NowLine { lead: None, rest: next },
    }
}

/// `stepState`: what a screen reader hears after the step's title.
pub fn step_state(plan: &Plan, step: &PlanStep, offset_secs: i64) -> String {
    if step.checked {
        return match step.ticked_at.as_deref().map(|t| format_time(Some(t), offset_secs)).filter(|t| !t.is_empty()) {
            Some(at) => format!("ticked at {at}"),
            None => "ticked".into(),
        };
    }
    if current_plan_step(plan).is_some_and(|c| c.step.key == step.key) { "next up".into() } else { "not started".into() }
}

/// What the strip shows as its heading in the plan form: the current step's group title, else the plan's title or path,
/// markdown stripped. The todo form's is "Todos".
pub fn strip_heading(plan: &Plan) -> String {
    let current = current_plan_step(plan);
    plain_text(current.and_then(|c| c.group.title.as_deref()).or(plan.title.as_deref()).unwrap_or(&plan.path))
}

/// The strip's line in the plan form: "Next: 4. Title", or "Every step is ticked".
pub fn strip_line(plan: &Plan) -> String {
    match current_plan_step(plan) {
        None => "Every step is ticked".into(),
        Some(c) => {
            let number = c.step.number.as_deref().filter(|n| !n.is_empty()).map(|n| format!("{n}. ")).unwrap_or_default();
            format!("Next: {number}{}", plain_text(&c.step.title))
        }
    }
}
