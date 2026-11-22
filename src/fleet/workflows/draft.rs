//! The workflow designer's model (component 18, piece K; `lib/workflow-draft.ts`): a workflow as its file says it, valid or
//! not yet, and the edits the inspector and the canvas make to it. Every edit returns a new draft. Checking and writing the
//! file are the server's: the draft goes to `/api/workflows/check` as JSON, which writes it in Fleet's layout. Plain Rust.
use super::{Choice, Step, StepKind, count, list, string, text};
use crate::composer::format::format_count;
use serde_json::{Map, Value, json};

/// Where an outcome or a choice leads when the run is over.
pub const END: &str = "end";

/// What the menu item and the dialog say it costs.
pub const DRAFT_COST_NOTE: &str = "Asks the model once. From a session it reads the conversation from the cache, so it's cheap.";

/// The variables Fleet fills in, for the chips under a prompt. Step variables are added per step.
pub const RUN_VARIABLES: [&str; 6] = ["request", "slug", "previous.summary", "previous.files", "run.branch", "run.base"];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkflowDraft {
    pub name: String,
    pub description: Option<String>,
    /// The Run box's hint.
    pub placeholder: Option<String>,
    pub steps: Vec<Step>,
}

/// An error the parser found: its line, and the index of the step whose lines hold it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowProblem {
    pub line: u32,
    pub message: String,
    pub step: Option<usize>,
}

/// A comment a save from the designer would remove.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowComment {
    pub line: u32,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkflowCheck {
    /// What Save writes: the File view's text, or the draft written in Fleet's layout.
    pub text: String,
    pub errors: Vec<WorkflowProblem>,
    /// What the designer shows; none when it can't show this text without losing some of it.
    pub draft: Option<WorkflowDraft>,
    pub comments: Vec<WorkflowComment>,
}

/// A repository's workflow file, open in the editor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkflowFile {
    pub workflow_id: String,
    /// The repo-relative path, e.g. `.weave/workflows/deps.yaml`.
    pub file: String,
    /// The file as it was read or last saved; a save with an older one is refused.
    pub hash: String,
    pub check: WorkflowCheck,
}

/// All the tokens asking the model cost, and how many were read from the provider's cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DraftTokens {
    pub total: u64,
    pub from_cache: u64,
}

/// A workflow the model drafted, from a session or a description. Nothing is written until it's saved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DraftedWorkflow {
    /// The repository Save creates the file in.
    pub repository: String,
    pub repository_name: String,
    /// The session it was drafted from; none when it was drafted from a description.
    pub session_title: Option<String>,
    pub check: WorkflowCheck,
    /// 2 when the first answer had errors and the model was asked again.
    pub asks: u32,
    /// None when the harness doesn't report tokens.
    pub tokens: Option<DraftTokens>,
}

// ─── Reading what the server sends ──────────────────────────────────────────────────────────────────────────────

impl WorkflowDraft {
    pub fn from_json(v: &Value) -> WorkflowDraft {
        WorkflowDraft {
            name: string(v, "name"),
            description: text(v, "description"),
            placeholder: text(v, "placeholder"),
            steps: list(v, "steps", Step::from_json),
        }
    }

    /// The body the server's check and create routes take (`draft`), in Fleet's field names.
    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "placeholder": self.placeholder,
            "steps": self.steps.iter().map(step_json).collect::<Vec<_>>(),
        })
    }
}

fn step_json(s: &Step) -> Value {
    let routes: Map<String, Value> = s.routes.iter().map(|(k, t)| (k.clone(), Value::String(t.clone()))).collect();
    json!({
        "id": s.id,
        "title": s.title,
        "kind": if s.kind == StepKind::You { "you" } else { "agent" },
        "agent": s.agent,
        "model": s.model,
        "effort": s.effort,
        "skill": s.skill,
        "optional": s.optional,
        "optionalHint": s.optional_hint,
        "outcomes": s.outcomes,
        "routes": routes,
        "maxLoops": s.max_loops,
        "ask": s.ask,
        "choices": s.choices.iter().map(|c| json!({"label": c.label, "to": c.to, "note": c.note})).collect::<Vec<_>>(),
        "finishYou": s.finish_you,
        "finishAgent": s.finish_agent,
        "writes": s.writes,
        "prompt": s.prompt,
    })
}

impl WorkflowCheck {
    pub fn from_json(v: &Value) -> WorkflowCheck {
        WorkflowCheck {
            text: string(v, "text"),
            errors: list(v, "errors", |e| WorkflowProblem {
                line: count(e, "line").unwrap_or(0),
                message: string(e, "message"),
                step: e.get("step").and_then(Value::as_u64).map(|n| n as usize),
            }),
            draft: v.get("draft").filter(|d| d.is_object()).map(WorkflowDraft::from_json),
            comments: list(v, "comments", |c| WorkflowComment { line: count(c, "line").unwrap_or(0), text: string(c, "text") }),
        }
    }
}

impl WorkflowFile {
    pub fn from_json(v: &Value) -> WorkflowFile {
        WorkflowFile {
            workflow_id: string(v, "workflowId"),
            file: string(v, "file"),
            hash: string(v, "hash"),
            check: WorkflowCheck::from_json(v.get("check").unwrap_or(&Value::Null)),
        }
    }
}

impl DraftedWorkflow {
    pub fn from_json(v: &Value) -> DraftedWorkflow {
        let tokens = v.get("tokens").filter(|t| t.is_object()).map(|t| DraftTokens {
            total: t.get("total").and_then(Value::as_u64).unwrap_or(0),
            from_cache: t.get("fromCache").and_then(Value::as_u64).unwrap_or(0),
        });
        DraftedWorkflow {
            repository: string(v, "repository"),
            repository_name: string(v, "repositoryName"),
            session_title: text(v, "sessionTitle"),
            check: WorkflowCheck::from_json(v.get("check").unwrap_or(&Value::Null)),
            asks: count(v, "asks").unwrap_or(1),
            tokens,
        }
    }
}

// ─── Words ──────────────────────────────────────────────────────────────────────────────────────────────────────

/// "Drafted from Fix the login bug. Review it before saving."
pub fn drafted_from(drafted: &DraftedWorkflow) -> String {
    match drafted.session_title.as_deref().filter(|t| !t.is_empty()) {
        Some(title) => format!("Drafted from {title}. Review it before saving."),
        None => "Drafted from your description. Review it before saving.".to_string(),
    }
}

/// "Asked the model once · 3,412 tokens, 3,100 from the cache."
pub fn draft_cost(drafted: &DraftedWorkflow) -> String {
    let asked = if drafted.asks > 1 { "Asked the model twice: its first answer had errors" } else { "Asked the model once" };
    let Some(tokens) = drafted.tokens else { return format!("{asked}.") };
    let total = format_count(Some(tokens.total as f64));
    let cached =
        if tokens.from_cache > 0 { format!(", {} from the cache", format_count(Some(tokens.from_cache as f64))) } else { String::new() };
    format!("{asked} · {total} tokens{cached}.")
}

/// "Tidy up a flaky test" → `tidy-up-a-flaky-test`, as the server names the file.
pub fn slug_of(name: &str) -> String {
    let mut slug = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    let slug: String = slug.chars().take(60).collect();
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() { "workflow".to_string() } else { slug.to_string() }
}

/// The chips under a step's prompt: the run's variables, then the steps before it that hand something on.
pub fn variables_for(draft: &WorkflowDraft, index: usize) -> Vec<String> {
    let mut variables: Vec<String> = RUN_VARIABLES.iter().map(|v| v.to_string()).collect();
    for step in draft.steps.iter().take(index).filter(|s| s.kind == StepKind::Agent) {
        variables.push(format!("steps.{}.summary", step.id));
        if !step.writes.is_empty() {
            variables.push(format!("steps.{}.files", step.id));
        }
    }
    variables
}

/// "Implement", "End the run", or "Next step" for where an outcome or choice leads.
pub fn target_title(draft: &WorkflowDraft, target: Option<&str>) -> String {
    match target.filter(|t| !t.is_empty()) {
        None => "Next step".to_string(),
        Some(END) => "End the run".to_string(),
        Some(t) => {
            draft.steps.iter().find(|s| s.id == t).map(|s| s.title.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| t.to_string())
        }
    }
}

// ─── Edits ──────────────────────────────────────────────────────────────────────────────────────────────────────

/// An id no step has yet: `step`, `step-2`, …
pub fn unique_id(draft: &WorkflowDraft, base: &str) -> String {
    let taken = |id: &str| draft.steps.iter().any(|s| s.id == id);
    if !taken(base) {
        return base.to_string();
    }
    (2..).map(|n| format!("{base}-{n}")).find(|id| !taken(id)).unwrap_or_default()
}

/// What + → Agent step adds: runs on Standard, sees the request and the step before it, and finishes with done.
pub fn new_agent_step(draft: &WorkflowDraft) -> Step {
    Step {
        id: unique_id(draft, "step"),
        title: "New step".into(),
        kind: StepKind::Agent,
        agent: Some("build".into()),
        model: Some("standard".into()),
        outcomes: vec!["done".into()],
        prompt: Some("The request: {{request}}\n\n{{previous.summary}}".into()),
        ..Step::default()
    }
}

/// What + → You decide adds before `next_id`: carry on to it, or stop there.
pub fn new_you_step(draft: &WorkflowDraft, next_id: Option<&str>) -> Step {
    Step {
        id: unique_id(draft, "ask"),
        title: "Check with you".into(),
        kind: StepKind::You,
        ask: Some("Carry on?".into()),
        choices: vec![
            Choice { label: "Carry on".into(), to: next_id.unwrap_or(END).into(), note: false },
            Choice { label: "Stop here".into(), to: END.into(), note: false },
        ],
        ..Step::default()
    }
}

pub fn insert_step(draft: &WorkflowDraft, index: usize, step: Step) -> WorkflowDraft {
    let mut next = draft.clone();
    next.steps.insert(index.min(next.steps.len()), step);
    next
}

pub fn remove_step(draft: &WorkflowDraft, index: usize) -> WorkflowDraft {
    let mut next = draft.clone();
    if index < next.steps.len() {
        next.steps.remove(index);
    }
    next
}

/// The fields of a step an edit changes (`Partial<WorkflowStep>`); `None` leaves a field alone, and the nested option sets
/// a nullable one to none.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StepPatch {
    pub title: Option<String>,
    pub agent: Option<Option<String>>,
    pub model: Option<Option<String>>,
    pub effort: Option<Option<String>>,
    pub skill: Option<Option<String>>,
    pub optional: Option<bool>,
    pub optional_hint: Option<Option<String>>,
    pub outcomes: Option<Vec<String>>,
    pub routes: Option<Vec<(String, String)>>,
    pub max_loops: Option<Option<u32>>,
    pub ask: Option<Option<String>>,
    pub choices: Option<Vec<Choice>>,
    pub finish_you: Option<bool>,
    pub finish_agent: Option<bool>,
    pub writes: Option<Vec<String>>,
    pub prompt: Option<Option<String>>,
}

impl StepPatch {
    pub fn apply(&self, step: &mut Step) {
        let p = self.clone();
        macro_rules! set {
            ($($f:ident),*) => { $(if let Some(v) = p.$f { step.$f = v; })* };
        }
        set!(
            title,
            agent,
            model,
            effort,
            skill,
            optional,
            optional_hint,
            outcomes,
            routes,
            max_loops,
            ask,
            choices,
            finish_you,
            finish_agent,
            writes,
            prompt
        );
    }
}

pub fn update_step(draft: &WorkflowDraft, index: usize, patch: &StepPatch) -> WorkflowDraft {
    let mut next = draft.clone();
    if let Some(step) = next.steps.get_mut(index) {
        patch.apply(step);
    }
    next
}

/// Does `rest` (just after `{{`) hold `\s*steps.<from>.(summary|files)\s*}}`? Returns the length it covers and the pieces.
fn step_variable<'a>(rest: &'a str, from: &str) -> Option<(usize, &'a str, &'a str, &'a str)> {
    let lead = rest.len() - rest.trim_start().len();
    let body = &rest[lead..];
    let after = body.strip_prefix("steps.")?.strip_prefix(from)?.strip_prefix('.')?;
    let field = ["summary", "files"].into_iter().find(|f| after.starts_with(f))?;
    let tail = &after[field.len()..];
    let trail = tail.len() - tail.trim_start().len();
    tail[trail..].starts_with("}}").then(|| {
        let used = lead + (body.len() - after.len()) + field.len() + trail + 2;
        (used, &rest[..lead], &tail[..trail], &after[..field.len()])
    })
}

fn rename_in_prompt(prompt: &str, from: &str, to: &str) -> String {
    let mut out = String::new();
    let mut rest = prompt;
    while let Some(at) = rest.find("{{") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 2..];
        match step_variable(after, from) {
            Some((used, lead, trail, field)) => {
                out.push_str(&format!("{{{{{lead}steps.{to}.{field}{trail}}}}}"));
                rest = &after[used..];
            }
            None => {
                out.push_str("{{");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Changes a step's id, and every reference to it follows: outcomes that lead there, choices that lead there, and
/// `{{steps.<id>.summary}}` and `{{steps.<id>.files}}` in every prompt.
pub fn rename_step_id(draft: &WorkflowDraft, from: &str, to: &str) -> WorkflowDraft {
    if from == to {
        return draft.clone();
    }
    let mut next = draft.clone();
    for step in &mut next.steps {
        if step.id == from {
            step.id = to.to_string();
        }
        for (_, target) in &mut step.routes {
            if target == from {
                *target = to.to_string();
            }
        }
        for choice in &mut step.choices {
            if choice.to == from {
                choice.to = to.to_string();
            }
        }
        step.prompt = step.prompt.take().map(|p| rename_in_prompt(&p, from, to));
    }
    next
}

/// Renames an outcome; where it led goes with it.
pub fn rename_outcome(step: &Step, from: &str, to: &str) -> StepPatch {
    let mut routes: Vec<(String, String)> = Vec::new();
    for (outcome, target) in &step.routes {
        let key = if outcome == from { to } else { outcome };
        // An object keeps one entry per key, the last one's value, in the first one's place.
        match routes.iter_mut().find(|(k, _)| k == key) {
            Some(entry) => entry.1 = target.clone(),
            None => routes.push((key.to_string(), target.clone())),
        }
    }
    StepPatch {
        outcomes: Some(step.outcomes.iter().map(|o| if o == from { to.to_string() } else { o.clone() }).collect()),
        routes: Some(routes),
        ..StepPatch::default()
    }
}

pub fn remove_outcome(step: &Step, outcome: &str) -> StepPatch {
    StepPatch {
        outcomes: Some(step.outcomes.iter().filter(|o| *o != outcome).cloned().collect()),
        routes: Some(step.routes.iter().filter(|(k, _)| k != outcome).cloned().collect()),
        ..StepPatch::default()
    }
}

/// An outcome that isn't used yet: `done`, `outcome-2`, …
pub fn new_outcome(step: &Step) -> String {
    let has = |name: &str| step.outcomes.iter().any(|o| o == name);
    if !has("done") {
        return "done".into();
    }
    (2..).map(|n| format!("outcome-{n}")).find(|name| !has(name)).unwrap_or_default()
}

/// Where an outcome leads: a step's id, `end`, or none for the next step (which leaves it out of `on:`).
pub fn set_route(step: &Step, outcome: &str, target: Option<&str>) -> StepPatch {
    let mut routes = step.routes.clone();
    match (target, routes.iter().position(|(k, _)| k == outcome)) {
        (None, Some(at)) => {
            routes.remove(at);
        }
        (None, None) => {}
        (Some(t), Some(at)) => routes[at].1 = t.to_string(),
        (Some(t), None) => routes.push((outcome.to_string(), t.to_string())),
    }
    StepPatch { routes: Some(routes), ..StepPatch::default() }
}

/// An outcome or choice that goes back to an earlier step, or the same one: a loop, which needs a max.
pub fn is_loop(draft: &WorkflowDraft, from: usize, target: Option<&str>) -> bool {
    let Some(target) = target.filter(|t| !t.is_empty() && *t != END) else { return false };
    draft.steps.iter().position(|s| s.id == target).is_some_and(|to| to <= from)
}

fn route_of<'a>(step: &'a Step, outcome: &str) -> Option<&'a str> {
    step.routes.iter().find(|(k, _)| k == outcome).map(|(_, t)| t.as_str())
}

/// The outcomes of a step that are loops.
pub fn loop_outcomes(draft: &WorkflowDraft, index: usize) -> Vec<String> {
    let Some(step) = draft.steps.get(index) else { return Vec::new() };
    step.outcomes.iter().filter(|o| is_loop(draft, index, route_of(step, o))).cloned().collect()
}

/// A step with no loop left has no max (the parser would say max doesn't apply). A new loop keeps its max empty, so the step
/// says it needs one.
pub fn drop_unused_max(draft: &WorkflowDraft, index: usize) -> WorkflowDraft {
    match draft.steps.get(index) {
        Some(step) if step.max_loops.is_some() && loop_outcomes(draft, index).is_empty() => {
            update_step(draft, index, &StepPatch { max_loops: Some(None), ..StepPatch::default() })
        }
        _ => draft.clone(),
    }
}

pub fn update_choice(step: &Step, index: usize, patch: impl Fn(&mut Choice)) -> StepPatch {
    let mut choices = step.choices.clone();
    if let Some(choice) = choices.get_mut(index) {
        patch(choice);
    }
    StepPatch { choices: Some(choices), ..StepPatch::default() }
}

/// Same workflow? What the editor compares to know there's something to save.
pub fn same_draft(a: Option<&WorkflowDraft>, b: Option<&WorkflowDraft>) -> bool {
    a == b
}
