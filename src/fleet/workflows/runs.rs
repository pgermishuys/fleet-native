//! The workflow runs of one machine (`stores/workflows.ts`): every run the app has heard of, kept current by the
//! `workflow_run` events on the sessions topic. Sessions groups a run's step sessions by it, a step session's header shows
//! its stepper, and the dashboard's Needs you counts the runs waiting on the user. One store per machine key; every
//! request goes to the machine the store is for, never to the live one by mistake. Every request runs on a thread.
use super::{Library, ModelChoice, Role, Run, RunStatus};
use crate::State;
use crate::conv::image_sniff::encode_component;
use crate::fleet::events::{Event, Kind};
use crate::fleet::http;
use crate::fleet::settings::HOME_MACHINE_KEY;
use rune::Tx;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

/// The recent runs the first load asks for.
pub const RECENT_RUNS: &str = "/api/workflows/runs?limit=100";

/// One machine's runs, in the order they were first heard of (a newer copy takes its place), and what the load is doing.
#[derive(Debug, Default)]
pub struct WorkflowRuns {
    runs: Vec<Run>,
    /// The recent runs have been read (`isLoaded`).
    pub loaded: bool,
    /// They are being asked for; "No runs yet" waits for the answer (`isLoading`).
    pub loading: bool,
    /// `ensureLoaded` was called: `workflow_run` events are followed from then on.
    listening: bool,
}

impl WorkflowRuns {
    /// `upsert`: events can arrive out of order with a load; an older copy never replaces a newer one. True when the
    /// run was kept.
    pub fn upsert(&mut self, run: Run) -> bool {
        match self.runs.iter_mut().find(|r| r.id == run.id) {
            Some(held) if held.updated_at > run.updated_at => false,
            Some(held) => {
                *held = run;
                true
            }
            None => {
                self.runs.push(run);
                true
            }
        }
    }

    pub fn get(&self, id: &str) -> Option<&Run> {
        self.runs.iter().find(|r| r.id == id)
    }

    /// `orderedRuns`: newest first (`createdAt`); runs created at the same moment stay in the order they came.
    pub fn ordered(&self) -> Vec<&Run> {
        let mut runs: Vec<&Run> = self.runs.iter().collect();
        runs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        runs
    }

    /// `runForSession`: the run a session is a step of. A session two runs claim belongs to the later one heard of.
    pub fn run_for_session(&self, session_id: &str) -> Option<&Run> {
        if session_id.is_empty() {
            return None;
        }
        self.runs.iter().rev().find(|r| r.sessions.iter().any(|s| s.session_id == session_id))
    }

    /// `waitingSessionIds`: the sessions whose conversation holds a card that waits on the user.
    pub fn waiting_session_ids(&self) -> HashSet<&str> {
        self.runs
            .iter()
            .filter(|r| r.status == RunStatus::Waiting)
            .filter_map(|r| r.waiting.as_ref()?.session_id.as_deref().filter(|s| !s.is_empty()))
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }
}

/// Every machine's runs, by the machine's key (`defineMachineStore`).
#[derive(Debug, Default)]
pub struct Workflows {
    by_machine: HashMap<String, WorkflowRuns>,
}

impl Workflows {
    /// A machine's runs; none until something asked.
    pub fn of(&self, machine: &str) -> Option<&WorkflowRuns> {
        self.by_machine.get(machine)
    }

    pub fn of_mut(&mut self, machine: &str) -> &mut WorkflowRuns {
        self.by_machine.entry(machine.to_string()).or_default()
    }
}

/// `extractApiError`: the words of a refusal, from `{detail}`, `{error}`, `{title}` or `{message}`, else `fallback`.
pub fn extract_api_error(error: &Value, fallback: &str) -> String {
    let words =
        |o: &Value| ["detail", "error", "title", "message"].iter().find_map(|k| o.get(k).and_then(Value::as_str).map(str::to_string));
    match error {
        Value::String(s) if !s.is_empty() => s.clone(),
        Value::Object(_) => words(error).unwrap_or_else(|| fallback.to_string()),
        _ => fallback.to_string(),
    }
}

/// `errorFrom`: a refusal's body read as Fleet reads it, `body.error ?? body`.
pub fn error_from(body: &str, fallback: &str) -> String {
    match serde_json::from_str::<Value>(body) {
        Ok(v) => extract_api_error(v.get("error").filter(|e| !e.is_null()).unwrap_or(&v), fallback),
        Err(_) => fallback.to_string(),
    }
}

/// A good answer's JSON, or the words of what went wrong.
fn answer(fallback: &str, got: Result<(u16, String), String>) -> Result<Value, String> {
    let (status, body) = got.map_err(|_| fallback.to_string())?;
    if !(200..300).contains(&status) {
        return Err(error_from(&body, fallback));
    }
    serde_json::from_str(&body).map_err(|_| fallback.to_string())
}

/// `StartWorkflowRunRequest`. A field left `None` is left out of the body, as `JSON.stringify` leaves out `undefined`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StartRequest {
    pub workflow_id: String,
    pub directory: String,
    pub request: String,
    pub base_branch: Option<String>,
    pub harness_type: Option<String>,
    pub harness_profile_id: Option<String>,
    pub optional_steps: Option<Vec<String>>,
    pub role_overrides: Option<Vec<(Role, ModelChoice)>>,
    /// "Check with me after each step": every agent step is one the user finishes.
    pub check_with_me: Option<bool>,
    /// The project the run's first step session goes in; none is Scratch.
    pub project_id: Option<String>,
}

impl StartRequest {
    pub fn body(&self) -> Value {
        let mut body = Map::new();
        body.insert("workflowId".into(), json!(self.workflow_id));
        body.insert("directory".into(), json!(self.directory));
        body.insert("request".into(), json!(self.request));
        let mut put = |key: &str, value: Option<Value>| {
            if let Some(v) = value {
                body.insert(key.into(), v);
            }
        };
        put("baseBranch", self.base_branch.as_ref().map(|v| json!(v)));
        put("harnessType", self.harness_type.as_ref().map(|v| json!(v)));
        put("harnessProfileId", self.harness_profile_id.as_ref().map(|v| json!(v)));
        put("optionalSteps", self.optional_steps.as_ref().map(|v| json!(v)));
        put(
            "roleOverrides",
            self.role_overrides.as_ref().map(|o| {
                Value::Object(o.iter().map(|(role, c)| (role.key().to_string(), json!({"model": c.model, "effort": c.effort}))).collect())
            }),
        );
        put("checkWithMe", self.check_with_me.map(|v| json!(v)));
        put("projectId", self.project_id.as_ref().map(|v| json!(v)));
        Value::Object(body)
    }
}

/// `/api/workflows/runs/{id}` followed by `tail`.
pub fn run_path(id: &str, tail: &str) -> String {
    format!("/api/workflows/runs/{}{tail}", encode_component(id))
}

/// `loadLibrary`'s path: `/api/workflows`, with the repository's directory when there is one.
pub fn library_path(directory: Option<&str>) -> String {
    match directory.filter(|d| !d.is_empty()) {
        Some(d) => format!("/api/workflows?directory={}", encode_component(d)),
        None => "/api/workflows".to_string(),
    }
}

/// A choice of a card that waits on the user. `check_with_me` goes with a You step's choice: how involved the user wants
/// to be from here.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Choice {
    pub id: String,
    pub note: Option<String>,
    pub check_with_me: Option<bool>,
}

/// One change to a run: the request and the words said when it fails without the server's own.
struct Change {
    method: &'static str,
    path: String,
    body: Value,
    fallback: &'static str,
}

impl Change {
    fn new(method: &'static str, path: String, body: Value, fallback: &'static str) -> Change {
        Change { method, path, body, fallback }
    }
}

/// What a request for a run hands back on the UI thread: the run (already kept in the store) or the words of why not.
pub type RunDone = Box<dyn FnOnce(&mut State, &mut Tx, Result<Run, String>) + Send>;

impl State {
    /// A machine's runs; empty until `workflow_runs_ensure_loaded` or an event made its store.
    pub fn workflow_runs(&self, machine: &str) -> &WorkflowRuns {
        static NONE: std::sync::OnceLock<WorkflowRuns> = std::sync::OnceLock::new();
        self.workflows.of(machine).unwrap_or_else(|| NONE.get_or_init(WorkflowRuns::default))
    }

    /// The machine the home dashboard shows: another machine's page reads its own runs.
    pub fn dashboard_machine_key(&self) -> String {
        self.dashboard_on_other().unwrap_or_else(|| HOME_MACHINE_KEY.to_string())
    }

    /// A run waiting on the user in this session: the step it waits at and the workflow's name (`DashboardSessionRow`).
    pub fn waiting_run_step(&self, session_id: &str) -> Option<(String, String)> {
        let run = self.workflow_runs(&self.dashboard_machine_key()).run_for_session(session_id)?;
        let waiting = run.waiting.as_ref().filter(|w| run.status == RunStatus::Waiting && w.session_id.as_deref() == Some(session_id))?;
        Some((waiting.step_title.clone(), run.workflow_name.clone()))
    }

    /// `ensureLoaded`: follows `workflow_run` events and reads the recent runs once. A read that failed leaves the list to
    /// fill from events, and is tried again by the next call. With no server (scenes, tests) there is nothing to read.
    pub fn workflow_runs_ensure_loaded(&mut self, machine: &str) {
        let link = self.link_for_machine(machine);
        // Another machine's hub listens to `sessions` for its runs only from here on.
        if machine != HOME_MACHINE_KEY
            && let Some(link) = &link
        {
            link.runs_feed(true);
        }
        let store = self.workflows.of_mut(machine);
        store.listening = true;
        if store.loaded || store.loading {
            return;
        }
        let Some(link) = link else { return };
        store.loading = true;
        let key = machine.to_string();
        link.off_thread(
            |base| http::get(base, RECENT_RUNS),
            move |s, _, result| {
                let store = s.workflows.of_mut(&key);
                store.loading = false;
                let Ok(body) = result else { return };
                for run in body.as_array().into_iter().flatten().filter_map(Run::from_json) {
                    store.upsert(run);
                }
                store.loaded = true;
            },
        );
    }

    /// Workflows follows its preference: while it is on, the runs are read whichever page is open, so Sessions, the dashboard
    /// and step sessions know them (row 1.2). While it is off nothing is asked and no event is kept.
    pub fn workflows_settle(&mut self) {
        if crate::fleet::rails::Features::from_prefs(&self.pickers.prefs).workflows {
            self.workflow_runs_ensure_loaded(HOME_MACHINE_KEY);
            // Another machine's runs are read without opening Workflows there first, once it answers.
            let others: Vec<String> = self
                .machines
                .list
                .iter()
                .map(|m| m.id.clone())
                .filter(|k| self.machines.others.get(k).is_some_and(|o| o.error.is_none() && o.loaded_at.is_some()))
                .collect();
            for key in others {
                self.workflow_runs_ensure_loaded(&key);
            }
        }
    }

    /// A `workflow_run` event on the sessions topic of the live machine. Kept once the store is listening.
    pub fn on_workflow_event(&mut self, event: &Event) {
        self.on_workflow_event_on(HOME_MACHINE_KEY, event);
    }

    /// The same for an event heard on `machine`'s own hub.
    pub fn on_workflow_event_on(&mut self, machine: &str, event: &Event) {
        let Event::Other(Kind::WorkflowRun, props) = event else { return };
        let Some(store) = self.workflows.by_machine.get_mut(machine).filter(|s| s.listening) else { return };
        if let Some(run) = Run::from_json(props) {
            store.upsert(run);
        }
    }

    /// `loadLibrary`: the Library for `directory` on `machine`.
    pub fn workflow_library(
        &mut self,
        machine: &str,
        directory: Option<&str>,
        tx: &mut Tx,
        done: impl FnOnce(&mut State, &mut Tx, Result<Library, String>) + Send + 'static,
    ) {
        let fallback = "Couldn't load the workflows.";
        let Some(link) = self.link_for_machine(machine) else { return done(self, tx, Err(fallback.into())) };
        let path = library_path(directory);
        link.off_thread(
            move |base| answer(fallback, http::text_with("GET", base, &path, None)),
            move |s, tx, result| done(s, tx, result.map(|v| Library::from_json(&v))),
        );
    }

    /// Sends a change to a run on `machine`; the run that comes back is kept at once, then `done` hears it.
    fn workflow_change(&mut self, machine: &str, change: Change, tx: &mut Tx, done: RunDone) {
        let Change { method, path, body, fallback } = change;
        let Some(link) = self.link_for_machine(machine) else { return done(self, tx, Err(fallback.into())) };
        let key = machine.to_string();
        link.off_thread(
            move |base| answer(fallback, http::text_with(method, base, &path, Some(&body))),
            move |s, tx, result| {
                let result = result.and_then(|v| Run::from_json(&v).ok_or_else(|| fallback.to_string()));
                if let Ok(run) = &result {
                    s.workflows.of_mut(&key).upsert(run.clone());
                }
                done(s, tx, result);
            },
        );
    }

    /// `start`: a run, in a new worktree.
    pub fn workflow_start(&mut self, machine: &str, request: &StartRequest, tx: &mut Tx, done: RunDone) {
        self.workflow_change(
            machine,
            Change::new("POST", "/api/workflows/runs".into(), request.body(), "Couldn't start the run."),
            tx,
            done,
        );
    }

    /// `answer`: a choice of a card that waits on the user.
    pub fn workflow_answer(&mut self, machine: &str, run_id: &str, choice: &Choice, tx: &mut Tx, done: RunDone) {
        let mut body = json!({"choice": choice.id, "note": choice.note});
        if let Some(on) = choice.check_with_me {
            body["checkWithMe"] = json!(on);
        }
        let path = run_path(run_id, "/answer");
        self.workflow_change(machine, Change::new("POST", path, body, "Couldn't answer the run."), tx, done);
    }

    /// `moveOn`: from a step the user finishes. Fleet asks its agent to wrap up, then starts the next step.
    pub fn workflow_move_on(&mut self, machine: &str, run_id: &str, outcome: Option<&str>, note: Option<&str>, tx: &mut Tx, done: RunDone) {
        let body = json!({"outcome": outcome, "note": note});
        self.workflow_change(machine, Change::new("POST", run_path(run_id, "/move-on"), body, "Couldn't move the run on."), tx, done);
    }

    /// `setCheckWithMe`: "Check with me after each step", from the run's header. It applies from the next step.
    pub fn workflow_set_check_with_me(&mut self, machine: &str, run_id: &str, on: bool, tx: &mut Tx, done: RunDone) {
        let path = run_path(run_id, "/check-with-me");
        self.workflow_change(machine, Change::new("PUT", path, json!({"on": on}), "Couldn't change Check with me."), tx, done);
    }

    /// `end`: stops the run where it is.
    pub fn workflow_end(&mut self, machine: &str, run_id: &str, tx: &mut Tx, done: RunDone) {
        self.workflow_change(machine, Change::new("POST", run_path(run_id, "/end"), json!({}), "Couldn't end the run."), tx, done);
    }
}
