//! Workflows' data (component 18, piece A; `lib/workflows.ts`): the server's shapes for the library and for a run, read
//! from JSON the way the rest of `src/fleet/` does, and the model roles preference. Plain Rust: no Rune, no I/O. The
//! helpers every view shares are in `helpers`, the per-machine runs store in `runs`.
use crate::composer::choice;
use serde_json::Value;
use std::collections::BTreeMap;

pub mod helpers;
pub mod runs;

/// The user preference behind the switch in Settings → Labs.
pub const WORKFLOWS_PREFERENCE_KEY: &str = "Workflows";

/// The user preference that maps each role to a model, per harness. Kept in Fleet, not in the repo.
pub const MODEL_ROLES_PREFERENCE_KEY: &str = "WorkflowModelRoles";

/// The tool a step's agent calls to finish the step.
pub const STEP_TOOL: &str = "fleet_step_done";

/// The choice that goes on past a missing file or a wrap-up that didn't finish.
pub const MOVE_ON_ANYWAY: &str = "move-on-anyway";

/// The choice that starts a step whose skill is off without it, for the rest of the run.
pub const WITHOUT_SKILL: &str = "without-skill";

/// How the footer Fleet adds to a step's prompt starts; it marks the prompt a step started with.
pub const STEP_FOOTER_START: &str = "This is one step of a Fleet workflow.";

// ─── Model roles ────────────────────────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Strong,
    Standard,
    Fast,
}

pub const ROLES: [Role; 3] = [Role::Strong, Role::Standard, Role::Fast];

impl Role {
    pub fn key(self) -> &'static str {
        match self {
            Role::Strong => "strong",
            Role::Standard => "standard",
            Role::Fast => "fast",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Role::Strong => "Strong",
            Role::Standard => "Standard",
            Role::Fast => "Fast",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Role::Strong => "Judgement: deciding what to build and whether it's right",
            Role::Standard => "Doing the work: writing and running code",
            Role::Fast => "Reading and sorting: short tasks with a clear answer",
        }
    }

    /// `isRole`: a step's model is a role name or an exact `provider/model`.
    pub fn parse(model: Option<&str>) -> Option<Role> {
        ROLES.into_iter().find(|r| Some(r.key()) == model)
    }
}

/// A model and effort for a role or a step. A `None` model is the composer's default model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelChoice {
    pub model: Option<String>,
    pub effort: Option<String>,
}

/// Harness type → role → model.
pub type ModelRoles = BTreeMap<String, BTreeMap<Role, ModelChoice>>;

/// `parseModelRoles`: the preference's JSON; nothing, unreadable text and anything but an object read as no roles.
pub fn parse_model_roles(value: Option<&str>) -> ModelRoles {
    let Some(Value::Object(harnesses)) = value.filter(|v| !v.is_empty()).and_then(|v| serde_json::from_str::<Value>(v).ok()) else {
        return ModelRoles::new();
    };
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    harnesses
        .iter()
        .map(|(harness, roles)| {
            let roles = ROLES
                .into_iter()
                .filter_map(|role| {
                    let choice = roles.get(role.key()).filter(|c| c.is_object())?;
                    Some((role, ModelChoice { model: text(choice, "model"), effort: text(choice, "effort") }))
                })
                .collect();
            (harness.clone(), roles)
        })
        .collect()
}

/// `keyFromPath`: a `provider/model` path as the model picker's selection key; empty for the default model.
pub fn key_from_path(path: Option<&str>) -> String {
    choice::model_from_path(path).map(|m| choice::key_for_model(&m)).unwrap_or_default()
}

/// `pathFromKey`: the picker's selection key as a model path; `None` for the default model.
pub fn path_from_key(key: &str) -> Option<String> {
    choice::model_to_path(choice::model_from_key(key).as_ref())
}

// ─── The library ────────────────────────────────────────────────────────────────────────────────────────────────

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn string(v: &Value, key: &str) -> String {
    text(v, key).unwrap_or_default()
}

fn flag(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn count(v: &Value, key: &str) -> Option<u32> {
    v.get(key).and_then(Value::as_u64).map(|n| n as u32)
}

fn list<T>(v: &Value, key: &str, read: impl Fn(&Value) -> T) -> Vec<T> {
    v.get(key).and_then(Value::as_array).map(|a| a.iter().map(read).collect()).unwrap_or_default()
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Choice {
    pub label: String,
    pub to: String,
    pub note: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StepKind {
    #[default]
    Agent,
    You,
}

impl StepKind {
    fn read(v: &Value) -> StepKind {
        if v.get("kind").and_then(Value::as_str) == Some("you") { StepKind::You } else { StepKind::Agent }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Step {
    pub id: String,
    pub title: String,
    pub kind: StepKind,
    pub agent: Option<String>,
    /// A role, or an exact `provider/model`.
    pub model: Option<String>,
    pub effort: Option<String>,
    pub skill: Option<String>,
    pub optional: bool,
    pub optional_hint: Option<String>,
    pub outcomes: Vec<String>,
    /// Outcome → the step it leads to, in the file's order.
    pub routes: Vec<(String, String)>,
    pub max_loops: Option<u32>,
    pub ask: Option<String>,
    pub choices: Vec<Choice>,
    /// `finish: you`: the user moves the step on, not the agent.
    pub finish_you: bool,
    /// `finish: agent`: the agent moves the step on, even with Check with me on.
    pub finish_agent: bool,
    /// The files the step declares, with their variables, e.g. `docs/design/{{slug}}.md`.
    pub writes: Vec<String>,
    /// An agent step's instructions; none for a You step.
    pub prompt: Option<String>,
}

impl Step {
    pub fn from_json(v: &Value) -> Step {
        Step {
            id: string(v, "id"),
            title: string(v, "title"),
            kind: StepKind::read(v),
            agent: text(v, "agent"),
            model: text(v, "model"),
            effort: text(v, "effort"),
            skill: text(v, "skill"),
            optional: flag(v, "optional"),
            optional_hint: text(v, "optionalHint"),
            outcomes: strings(v, "outcomes"),
            routes: v
                .get("routes")
                .and_then(Value::as_object)
                .map_or_else(Vec::new, |o| o.iter().filter_map(|(k, t)| Some((k.clone(), t.as_str()?.to_string()))).collect()),
            max_loops: count(v, "maxLoops"),
            ask: text(v, "ask"),
            choices: list(v, "choices", |c| Choice { label: string(c, "label"), to: string(c, "to"), note: flag(c, "note") }),
            finish_you: flag(v, "finishYou"),
            finish_agent: flag(v, "finishAgent"),
            writes: strings(v, "writes"),
            prompt: text(v, "prompt"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Workflow {
    pub id: String,
    pub built_in: bool,
    /// The repo-relative file; none for a built-in.
    pub file: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub placeholder: Option<String>,
    pub starts_from: Option<String>,
    pub runs_in: Option<String>,
    pub steps: Vec<Step>,
    /// Each names the file and line. A workflow with errors can't run.
    pub errors: Vec<String>,
}

impl Workflow {
    pub fn from_json(v: &Value) -> Workflow {
        Workflow {
            id: string(v, "id"),
            built_in: flag(v, "builtIn"),
            file: text(v, "file"),
            name: string(v, "name"),
            description: text(v, "description"),
            placeholder: text(v, "placeholder"),
            starts_from: text(v, "startsFrom"),
            runs_in: text(v, "runsIn"),
            steps: list(v, "steps", Step::from_json),
            errors: strings(v, "errors"),
        }
    }
}

/// `GET /api/workflows[?directory=]`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Library {
    pub repository: Option<String>,
    pub repository_name: Option<String>,
    pub workflows: Vec<Workflow>,
}

impl Library {
    pub fn from_json(v: &Value) -> Library {
        Library {
            repository: text(v, "repository"),
            repository_name: text(v, "repositoryName"),
            workflows: list(v, "workflows", Workflow::from_json),
        }
    }
}

// ─── A run ──────────────────────────────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RunStatus {
    Running,
    Waiting,
    #[default]
    Done,
    Ended,
    Failed,
}

impl RunStatus {
    /// A word the client doesn't know reads as done, as `runStatusLabel`'s default branch does.
    fn read(word: &str) -> RunStatus {
        match word {
            "running" => RunStatus::Running,
            "waiting" => RunStatus::Waiting,
            "ended" => RunStatus::Ended,
            "failed" => RunStatus::Failed,
            _ => RunStatus::Done,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StepState {
    #[default]
    Pending,
    Running,
    Waiting,
    Done,
    Skipped,
}

impl StepState {
    fn read(word: &str) -> StepState {
        match word {
            "running" => StepState::Running,
            "waiting" => StepState::Waiting,
            "done" => StepState::Done,
            "skipped" => StepState::Skipped,
            _ => StepState::Pending,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunStep {
    pub id: String,
    pub title: String,
    pub kind: StepKind,
    pub state: StepState,
    pub visits: u32,
    pub optional: bool,
    pub enabled: bool,
    pub outcome: Option<String>,
    pub session_id: Option<String>,
    pub model: Option<String>,
    pub role: Option<String>,
    pub skill: Option<String>,
    pub max_loops: Option<u32>,
    /// The workflow file says `finish: you`.
    pub finish_you: bool,
    /// The workflow file says `finish: agent`: Check with me doesn't make it the user's.
    pub finish_agent: bool,
    /// The user finishes it: how its last visit started, or, for a step still to come, what it will be now.
    pub with_you: bool,
    pub outcomes: Vec<String>,
}

impl RunStep {
    fn from_json(v: &Value) -> RunStep {
        RunStep {
            id: string(v, "id"),
            title: string(v, "title"),
            kind: StepKind::read(v),
            state: StepState::read(&string(v, "state")),
            visits: count(v, "visits").unwrap_or(0),
            optional: flag(v, "optional"),
            enabled: flag(v, "enabled"),
            outcome: text(v, "outcome"),
            session_id: text(v, "sessionId"),
            model: text(v, "model"),
            role: text(v, "role"),
            skill: text(v, "skill"),
            max_loops: count(v, "maxLoops"),
            finish_you: flag(v, "finishYou"),
            finish_agent: flag(v, "finishAgent"),
            with_you: flag(v, "withYou"),
            outcomes: strings(v, "outcomes"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunSession {
    pub session_id: String,
    pub step_id: String,
    pub visit: u32,
    pub outcome: Option<String>,
    pub summary: Option<String>,
    pub note: Option<String>,
    /// The user finishes this visit.
    pub with_you: bool,
    /// The files the step declares, relative to the run's worktree.
    pub files: Vec<String>,
    /// They were all there before the next step started.
    pub files_checked: bool,
    /// The short SHA of the commit Fleet made of them then; none when there was nothing to commit.
    pub files_commit: Option<String>,
    /// Why Fleet couldn't commit them. The run went on.
    pub files_commit_error: Option<String>,
    /// The prompt the session started with.
    pub prompt_message_id: Option<String>,
    /// The one prompt Fleet sent when the user pressed Move on.
    pub wrap_up_message_id: Option<String>,
    /// The user's note for the next step.
    pub hand_off_note: Option<String>,
}

impl RunSession {
    fn from_json(v: &Value) -> RunSession {
        RunSession {
            session_id: string(v, "sessionId"),
            step_id: string(v, "stepId"),
            visit: count(v, "visit").unwrap_or(1),
            outcome: text(v, "outcome"),
            summary: text(v, "summary"),
            note: text(v, "note"),
            with_you: flag(v, "withYou"),
            files: strings(v, "files"),
            files_checked: flag(v, "filesChecked"),
            files_commit: text(v, "filesCommit"),
            files_commit_error: text(v, "filesCommitError"),
            prompt_message_id: text(v, "promptMessageId"),
            wrap_up_message_id: text(v, "wrapUpMessageId"),
            hand_off_note: text(v, "handOffNote"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunChoice {
    pub id: String,
    pub label: String,
    pub note: bool,
    pub to: Option<String>,
    /// It goes on to two or more agent steps, so the card offers it two ways: let it run, or check with me.
    pub involvement: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitingKind {
    You,
    NoOutcome,
    LoopLimit,
    StartFailed,
    MissingFiles,
    WrapUpFailed,
    SkillOff,
}

impl WaitingKind {
    fn read(word: &str) -> Option<WaitingKind> {
        Some(match word {
            "you" => WaitingKind::You,
            "no-outcome" => WaitingKind::NoOutcome,
            "loop-limit" => WaitingKind::LoopLimit,
            "start-failed" => WaitingKind::StartFailed,
            "missing-files" => WaitingKind::MissingFiles,
            "wrap-up-failed" => WaitingKind::WrapUpFailed,
            "skill-off" => WaitingKind::SkillOff,
            _ => return None,
        })
    }
}

/// A way to move on from a step the user finishes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Move {
    pub outcome: String,
    /// Where it leads, past optional steps that are off; none for the end of the run.
    pub to: Option<String>,
    pub to_title: Option<String>,
    /// It sends the work back to an earlier step.
    pub back: bool,
    pub loops_used: Option<u32>,
    pub max_loops: Option<u32>,
    /// False when the loop's maximum is used up.
    pub allowed: bool,
}

/// The step the user finishes that's open now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WithYou {
    pub step_id: String,
    pub step_title: String,
    pub session_id: String,
    pub files: Vec<String>,
    /// The user moved on; the agent is bringing the files up to date and writing the summary.
    pub wrapping_up: bool,
    pub outcome: Option<String>,
    pub moves: Vec<Move>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Waiting {
    pub kind: WaitingKind,
    pub step_id: String,
    pub step_title: String,
    pub message: String,
    pub question: Option<String>,
    /// The session the card shows in.
    pub session_id: Option<String>,
    pub choices: Vec<RunChoice>,
    /// For a You step, the files the step before it declares (they open next to the card); for missing files, the step's.
    pub files: Vec<String>,
    /// What "let it run" runs: the agent steps before the next You step.
    pub then_alone: Vec<String>,
    pub next_you_title: Option<String>,
}

/// "Started by …": the automation that started a run, by its name when it did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartedBy {
    pub automation_id: String,
    pub automation_name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Run {
    pub id: String,
    pub workflow_id: String,
    pub workflow_name: String,
    pub title: String,
    pub request: String,
    pub status: RunStatus,
    pub current_step_id: Option<String>,
    pub result: Option<String>,
    pub repository_path: String,
    pub branch: Option<String>,
    pub base_branch: Option<String>,
    pub worktree_path: Option<String>,
    pub harness_type: String,
    pub created_at: String,
    pub updated_at: String,
    pub ended_at: Option<String>,
    pub steps: Vec<RunStep>,
    pub sessions: Vec<RunSession>,
    pub waiting: Option<Waiting>,
    /// "Check with me after each step": agent steps that start from now on are ones the user finishes.
    pub check_with_me: bool,
    pub with_you: Option<WithYou>,
    /// The automation that started the run; none when it was started from the Run box.
    pub started_by: Option<StartedBy>,
}

impl Run {
    /// `isWorkflowRun` guards what an event or an answer carries: an id, a status, and both lists. Anything else is not a run.
    pub fn from_json(v: &Value) -> Option<Run> {
        let (Some(id), Some(status)) = (v.get("id").and_then(Value::as_str), v.get("status").and_then(Value::as_str)) else { return None };
        if !v.get("steps").is_some_and(Value::is_array) || !v.get("sessions").is_some_and(Value::is_array) {
            return None;
        }
        let object = |k: &str| v.get(k).filter(|o| o.is_object());
        Some(Run {
            id: id.to_string(),
            workflow_id: string(v, "workflowId"),
            workflow_name: string(v, "workflowName"),
            title: string(v, "title"),
            request: string(v, "request"),
            status: RunStatus::read(status),
            current_step_id: text(v, "currentStepId"),
            result: text(v, "result"),
            repository_path: string(v, "repositoryPath"),
            branch: text(v, "branch"),
            base_branch: text(v, "baseBranch"),
            worktree_path: text(v, "worktreePath"),
            harness_type: string(v, "harnessType"),
            created_at: string(v, "createdAt"),
            updated_at: string(v, "updatedAt"),
            ended_at: text(v, "endedAt"),
            steps: list(v, "steps", RunStep::from_json),
            sessions: list(v, "sessions", RunSession::from_json),
            waiting: object("waiting").and_then(read_waiting),
            check_with_me: flag(v, "checkWithMe"),
            with_you: object("withYou").map(read_with_you),
            started_by: object("startedBy")
                .map(|s| StartedBy { automation_id: string(s, "automationId"), automation_name: string(s, "automationName") }),
        })
    }
}

fn read_waiting(w: &Value) -> Option<Waiting> {
    Some(Waiting {
        kind: WaitingKind::read(w.get("kind")?.as_str()?)?,
        step_id: string(w, "stepId"),
        step_title: string(w, "stepTitle"),
        message: string(w, "message"),
        question: text(w, "question"),
        session_id: text(w, "sessionId"),
        choices: list(w, "choices", |c| RunChoice {
            id: string(c, "id"),
            label: string(c, "label"),
            note: flag(c, "note"),
            to: text(c, "to"),
            involvement: flag(c, "involvement"),
        }),
        files: strings(w, "files"),
        then_alone: strings(w, "thenAlone"),
        next_you_title: text(w, "nextYouTitle"),
    })
}

fn read_with_you(w: &Value) -> WithYou {
    WithYou {
        step_id: string(w, "stepId"),
        step_title: string(w, "stepTitle"),
        session_id: string(w, "sessionId"),
        files: strings(w, "files"),
        wrapping_up: flag(w, "wrappingUp"),
        outcome: text(w, "outcome"),
        moves: list(w, "moves", |m| Move {
            outcome: string(m, "outcome"),
            to: text(m, "to"),
            to_title: text(m, "toTitle"),
            back: flag(m, "back"),
            loops_used: count(m, "loopsUsed"),
            max_loops: count(m, "maxLoops"),
            allowed: flag(m, "allowed"),
        }),
    }
}
