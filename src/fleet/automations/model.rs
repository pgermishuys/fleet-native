//! An automation and its runs, read from the server's JSON, and the request written to make or change one
//! (`stores/automations.ts`: `Automation`, `AutomationRun`, `AutomationDraft`, `CreateAutomationRequest`; row 15.1). Read with
//! `serde_json::Value` as the rest of `src/fleet/` does: an unknown field is ignored and a missing optional one is defaulted.
use serde_json::{Map, Value, json};

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn opt(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn number(v: &Value, key: &str, default: i64) -> i64 {
    v.get(key).and_then(Value::as_i64).unwrap_or(default)
}

fn strings(v: &Value, key: &str) -> Option<Vec<String>> {
    v.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
}

/// Where a run is (`AutomationRunState`). A word the server adds later reads as `Done`, as Fleet's `describeRunState` does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RunState {
    Starting,
    Running,
    Waiting,
    #[default]
    Done,
    Ended,
    Failed,
    Skipped,
    /// On another machine that didn't answer when asked: it may still be going, so not Done.
    Unanswered,
}

impl RunState {
    pub fn parse(word: &str) -> RunState {
        match word {
            "starting" => RunState::Starting,
            "running" => RunState::Running,
            "waiting" => RunState::Waiting,
            "ended" => RunState::Ended,
            "failed" => RunState::Failed,
            "skipped" => RunState::Skipped,
            "unanswered" => RunState::Unanswered,
            _ => RunState::Done,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            RunState::Starting => "starting",
            RunState::Running => "running",
            RunState::Waiting => "waiting",
            RunState::Done => "done",
            RunState::Ended => "ended",
            RunState::Failed => "failed",
            RunState::Skipped => "skipped",
            RunState::Unanswered => "unanswered",
        }
    }
}

/// One run of an automation (`AutomationRun`). Times stay as the server's ISO text; a view turns them into words.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AutomationRun {
    pub id: String,
    pub automation_id: String,
    /// `schedule`, `catch_up`, `once`, `manual`, or the event type.
    pub trigger: String,
    pub scheduled_for: Option<String>,
    pub started_at: String,
    pub state: RunState,
    pub session_id: Option<String>,
    pub instance_id: Option<String>,
    /// Why it failed or was skipped.
    pub error: Option<String>,
    /// The workflow run it started, for a workflow target.
    pub workflow_run_id: Option<String>,
    /// The other machine it went to; `None` for this one. Its session is there.
    pub machine_id: Option<String>,
    pub machine_name: Option<String>,
    /// What the run did, in words; `None` for runs that don't say.
    pub summary: Option<String>,
}

impl AutomationRun {
    pub fn from_json(v: &Value) -> AutomationRun {
        AutomationRun {
            id: text(v, "id"),
            automation_id: text(v, "automationId"),
            trigger: text(v, "trigger"),
            scheduled_for: opt(v, "scheduledFor"),
            started_at: text(v, "startedAt"),
            state: RunState::parse(v.get("state").and_then(Value::as_str).unwrap_or("done")),
            session_id: opt(v, "sessionId"),
            instance_id: opt(v, "instanceId"),
            error: opt(v, "error"),
            workflow_run_id: opt(v, "workflowRunId"),
            machine_id: opt(v, "machineId"),
            machine_name: opt(v, "machineName"),
            summary: opt(v, "summary"),
        }
    }

    /// The answer of `GET /api/automations/{id}/runs`: `{runs}`, newest first. Anything else is no runs.
    pub fn list_from_json(v: &Value) -> Vec<AutomationRun> {
        v.get("runs").and_then(Value::as_array).map(|a| a.iter().map(AutomationRun::from_json).collect()).unwrap_or_default()
    }
}

/// An automation (`Automation`). The optional fields are `None` for one made before the server kept them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Automation {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub trigger_type: String,
    pub trigger_config: String,
    pub max_concurrent_runs: i64,
    pub max_runs_per_hour: i64,
    pub timeout_minutes: i64,
    pub is_enabled: bool,
    pub workspace_id: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
    /// `new_session` or `workflow`; `None` for one made before it was kept.
    pub target_type: Option<String>,
    pub target_tags: Vec<String>,
    /// The IANA zone the schedule's cron is read in; `None` means UTC (made before zones were kept).
    pub time_zone: Option<String>,
    /// `worktree` or `existing`; `None` for one made before it was kept.
    pub isolation: Option<String>,
    /// Where a run's worktree starts; `None` for the repository's default.
    pub base_branch: Option<String>,
    pub harness_type: Option<String>,
    /// The workflow a workflow target runs (`builtin:…` or `repo:…`).
    pub workflow_id: Option<String>,
    pub workflow_steps: Option<Vec<String>>,
    /// When it runs next (ISO, UTC); `None` when it's off or waits for an event.
    pub next_run_at: Option<String>,
    pub last_run: Option<AutomationRun>,
    /// The machine in this Fleet's list its runs go to; `None` for this machine.
    pub target_machine_id: Option<String>,
    /// Which built-in automation Fleet keeps (`dreaming`); `None` for one you made.
    pub builtin: Option<String>,
}

impl Default for Automation {
    fn default() -> Automation {
        Automation {
            id: String::new(),
            name: String::new(),
            prompt: String::new(),
            trigger_type: String::new(),
            trigger_config: String::new(),
            max_concurrent_runs: 1,
            max_runs_per_hour: 0,
            timeout_minutes: 0,
            is_enabled: false,
            workspace_id: None,
            model: None,
            agent: None,
            created_at: String::new(),
            updated_at: None,
            target_type: None,
            target_tags: Vec::new(),
            time_zone: None,
            isolation: None,
            base_branch: None,
            harness_type: None,
            workflow_id: None,
            workflow_steps: None,
            next_run_at: None,
            last_run: None,
            target_machine_id: None,
            builtin: None,
        }
    }
}

impl Automation {
    pub fn from_json(v: &Value) -> Automation {
        Automation {
            id: text(v, "id"),
            name: text(v, "name"),
            prompt: text(v, "prompt"),
            trigger_type: text(v, "triggerType"),
            trigger_config: text(v, "triggerConfig"),
            max_concurrent_runs: number(v, "maxConcurrentRuns", 1),
            max_runs_per_hour: number(v, "maxRunsPerHour", 0),
            timeout_minutes: number(v, "timeoutMinutes", 0),
            is_enabled: v.get("isEnabled").and_then(Value::as_bool).unwrap_or(false),
            workspace_id: opt(v, "workspaceId"),
            model: opt(v, "model"),
            agent: opt(v, "agent"),
            created_at: text(v, "createdAt"),
            updated_at: opt(v, "updatedAt"),
            target_type: opt(v, "targetType"),
            target_tags: strings(v, "targetTags").unwrap_or_default(),
            time_zone: opt(v, "timeZone"),
            isolation: opt(v, "isolation"),
            base_branch: opt(v, "baseBranch"),
            harness_type: opt(v, "harnessType"),
            workflow_id: opt(v, "workflowId"),
            workflow_steps: strings(v, "workflowSteps"),
            next_run_at: opt(v, "nextRunAt"),
            last_run: v.get("lastRun").filter(|r| r.is_object()).map(AutomationRun::from_json),
            target_machine_id: opt(v, "targetMachineId"),
            builtin: opt(v, "builtin"),
        }
    }

    /// The answer of `GET /api/automations`: `{automations}`. Anything else is an empty list.
    pub fn list_from_json(v: &Value) -> Vec<Automation> {
        v.get("automations").and_then(Value::as_array).map(|a| a.iter().map(Automation::from_json).collect()).unwrap_or_default()
    }
}

/// A new automation's starting point from a session, "Repeat on a schedule…" (`AutomationDraft`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutomationDraft {
    pub prompt: String,
    /// The folder it ran in; `None` for a quick chat.
    pub folder: Option<String>,
    /// `worktree` or `existing`.
    pub isolation: String,
}

impl AutomationDraft {
    pub fn from_json(v: &Value) -> AutomationDraft {
        AutomationDraft {
            prompt: text(v, "prompt"),
            folder: opt(v, "folder"),
            isolation: opt(v, "isolation").filter(|i| i == "worktree" || i == "existing").unwrap_or_else(|| "existing".to_string()),
        }
    }
}

/// What a create or an update sends (`CreateAutomationRequest`). A `None` is left out of the body, as an `undefined` is in
/// Fleet's; a field that may be sent as `null` (to clear it) is `Some(None)`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AutomationRequest {
    pub name: String,
    pub prompt: String,
    pub trigger_type: String,
    pub trigger_config: String,
    pub max_concurrent_runs: Option<i64>,
    pub max_runs_per_hour: Option<i64>,
    pub timeout_minutes: Option<i64>,
    pub workspace_id: Option<Option<String>>,
    pub model: Option<Option<String>>,
    pub agent: Option<Option<String>>,
    pub target_type: Option<String>,
    pub target_tags: Option<Vec<String>>,
    pub time_zone: Option<Option<String>>,
    pub isolation: Option<Option<String>>,
    pub base_branch: Option<Option<String>>,
    pub harness_type: Option<Option<String>>,
    pub workflow_id: Option<Option<String>>,
    pub workflow_steps: Option<Vec<String>>,
    pub target_machine_id: Option<Option<String>>,
}

impl AutomationRequest {
    pub fn to_json(&self) -> Value {
        let mut o = Map::new();
        o.insert("name".into(), json!(self.name));
        o.insert("prompt".into(), json!(self.prompt));
        o.insert("triggerType".into(), json!(self.trigger_type));
        o.insert("triggerConfig".into(), json!(self.trigger_config));
        let mut put = |key: &str, value: Option<Value>| {
            if let Some(value) = value {
                o.insert(key.into(), value);
            }
        };
        put("maxConcurrentRuns", self.max_concurrent_runs.map(|n| json!(n)));
        put("maxRunsPerHour", self.max_runs_per_hour.map(|n| json!(n)));
        put("timeoutMinutes", self.timeout_minutes.map(|n| json!(n)));
        put("workspaceId", self.workspace_id.as_ref().map(|v| json!(v)));
        put("model", self.model.as_ref().map(|v| json!(v)));
        put("agent", self.agent.as_ref().map(|v| json!(v)));
        put("targetType", self.target_type.as_ref().map(|v| json!(v)));
        put("targetTags", self.target_tags.as_ref().map(|v| json!(v)));
        put("timeZone", self.time_zone.as_ref().map(|v| json!(v)));
        put("isolation", self.isolation.as_ref().map(|v| json!(v)));
        put("baseBranch", self.base_branch.as_ref().map(|v| json!(v)));
        put("harnessType", self.harness_type.as_ref().map(|v| json!(v)));
        put("workflowId", self.workflow_id.as_ref().map(|v| json!(v)));
        put("workflowSteps", self.workflow_steps.as_ref().map(|v| json!(v)));
        put("targetMachineId", self.target_machine_id.as_ref().map(|v| json!(v)));
        Value::Object(o)
    }
}
