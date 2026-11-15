//! What the autocomplete popup lists from the server (docs/parity/05-composer.md, piece K): the session's commands and
//! skills, its agents (the list the agent picker loads, kept here too), the files and folders an `@` searches, and the
//! sessions an `@` can name. Ports the loading half of `use-autocomplete.ts`, `use-find-files.ts` and
//! `use-referable-sessions.ts`. Every request runs on a thread (`Link::off_thread`) and answers into this store; the
//! store itself is plain data, so the rules (the 80 ms wait, a newer search replacing an older, the 30 s cache, the
//! last answer narrowed while a new one is awaited) are tested with no server.
use crate::composer::files::narrow_file_matches;
use crate::composer::refs::{Command, SessionItem};
use crate::composer::triggers::Agent;
use serde_json::Value;
use std::collections::HashMap;

/// Short: the list already follows each key from the last answer, so this only spares the server a burst of typing.
pub const SEARCH_DEBOUNCE: f64 = 0.08;
/// How long the sessions fetched for the `@` list are reused before they're asked for again.
pub const SESSIONS_REFRESH: f64 = 30.0;

/// What one list holds, and why it may have failed. A list that is asked for again keeps what it had meanwhile.
#[derive(Debug, Clone, Default)]
pub struct Loaded<T> {
    pub items: Vec<T>,
    pub error: Option<String>,
    /// Set only by a scene: a list that has arrived is never loading, and one that hasn't has no entry.
    pub loading: bool,
}

/// The reason in an error answer: the body's `error`, else "HTTP n" (`payload?.error ?? HTTP ${status}`).
pub fn failure(status: u16, body: &Value) -> String {
    body.get("error").and_then(Value::as_str).filter(|e| !e.trim().is_empty()).map_or_else(|| format!("HTTP {status}"), str::to_string)
}

/// `{commands: [{name, description, kind, userOnly}]}`.
pub fn parse_commands(body: &Value) -> Vec<Command> {
    body.get("commands")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let text = |k: &str| c.get(k).and_then(Value::as_str).map(str::to_string);
            Some(Command {
                name: text("name")?,
                description: text("description").filter(|d| !d.is_empty()),
                kind: text("kind"),
                user_only: c.get("userOnly").and_then(Value::as_bool) == Some(true),
            })
        })
        .collect()
}

/// `{agents: [...]}` or a bare list of `{name, description, mode, color}`, as the harness lists them (hidden ones too).
pub fn parse_agents(body: &Value) -> Vec<Agent> {
    body.get("agents")
        .unwrap_or(body)
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| {
            let text = |k: &str| a.get(k).and_then(Value::as_str).filter(|v| !v.is_empty()).map(str::to_string);
            Some(Agent { name: text("name")?, description: text("description"), mode: text("mode"), color: text("color") })
        })
        .collect()
}

/// The sessions of `GET /api/sessions` as the `@` list reads them.
pub fn parse_sessions(body: &Value) -> Vec<SessionItem> {
    body.as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| {
            let text = |k: &str| row.get(k).and_then(Value::as_str).filter(|v| !v.is_empty()).map(str::to_string);
            let id = row.pointer("/session/id").and_then(Value::as_str)?.to_string();
            let time = |k: &str| row.pointer(&format!("/session/time/{k}")).and_then(Value::as_f64);
            Some(SessionItem {
                id,
                title: row.pointer("/session/title").and_then(Value::as_str).unwrap_or("").to_string(),
                status: text("sessionStatus").unwrap_or_default(),
                activity: text("activityStatus"),
                harness_type: text("harnessType").unwrap_or_default(),
                project_name: text("projectName"),
                workspace_display_name: text("workspaceDisplayName"),
                workspace_directory: text("workspaceDirectory").unwrap_or_default(),
                parent_session_id: text("parentSessionId"),
                archived: text("retentionStatus").as_deref() == Some("archived"),
                updated: time("updated").or_else(|| time("created")).unwrap_or(0.0),
            })
        })
        .collect()
}

/// `files` of `{files: [...]}`; anything else is none.
pub fn parse_files(body: &Value) -> Vec<String> {
    body.get("files")
        .and_then(Value::as_array)
        .map(|f| f.iter().filter_map(|p| p.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// Where an `@` looks for files: the session's directory, or, with no session yet, a folder (the New Session page).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Session(String),
    Directory(String),
}

/// What the file search asks of its caller after a look at the typed query.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Idle,
    /// Look again in this many seconds: the request is waiting out the debounce.
    Wait(f64),
    /// Send the request now; hand its answer back with this number.
    Send {
        generation: u64,
        query: String,
    },
}

/// The `@` file search (`useFindFiles`): the server's last answer and the query it answers, what is wanted now, and
/// whether a request is out. A newer query replaces the one in flight (its answer is dropped by number).
#[derive(Debug, Clone, Default)]
pub struct FileSearch {
    scope: Option<Scope>,
    answer_query: String,
    answer: Vec<String>,
    wanted: Option<String>,
    generation: u64,
    /// The clock at which the wanted query may be sent; none once it has been.
    due: Option<f64>,
    pub loading: bool,
    pub error: Option<String>,
}

impl FileSearch {
    /// The files to list for what is typed: the server's answer when it is for this query, else the last answer
    /// narrowed to what still matches (`narrowFileMatches`).
    pub fn files(&self, typed: &str) -> Vec<String> {
        let typed = typed.trim();
        if typed == self.answer_query { self.answer.clone() } else { narrow_file_matches(&self.answer, typed) }
    }

    fn reset(&mut self) {
        self.generation += 1;
        self.answer_query.clear();
        self.answer.clear();
        self.wanted = None;
        self.due = None;
        self.loading = false;
        self.error = None;
    }

    /// Looks at what is typed after the `@` (none: no such list is open) and says what to do about it. An empty query,
    /// or one ending in "/", lists a folder at once; any other waits the debounce.
    pub fn want(&mut self, scope: Option<&Scope>, query: Option<&str>, now: f64) -> Step {
        // Another session's (or folder's) files say nothing about this one's.
        if self.scope.as_ref() != scope {
            self.reset();
            self.scope = scope.cloned();
        }
        let (Some(_), Some(query)) = (scope, query) else {
            if self.wanted.is_some() {
                self.reset();
            }
            return Step::Idle;
        };
        let typed = query.trim();
        if self.wanted.as_deref() != Some(typed) {
            self.generation += 1;
            self.wanted = Some(typed.to_string());
            self.loading = true;
            let listing = typed.is_empty() || typed.ends_with('/');
            self.due = Some(if listing { now } else { now + SEARCH_DEBOUNCE });
        }
        match self.due {
            Some(due) if now >= due => {
                self.due = None;
                self.error = None;
                Step::Send { generation: self.generation, query: typed.to_string() }
            }
            Some(due) => Step::Wait(due - now),
            None => Step::Idle,
        }
    }

    /// A request's answer. One that a newer query has replaced is dropped.
    pub fn answered(&mut self, generation: u64, query: &str, result: Result<Vec<String>, String>) {
        if generation != self.generation {
            return;
        }
        self.loading = false;
        match result {
            Ok(files) => {
                self.answer_query = query.to_string();
                self.answer = files;
                self.error = None;
            }
            Err(why) => self.error = Some(why),
        }
    }
}

/// The sessions fetched for the `@` list, reused for 30 seconds (`fetchedOn`), and whether the list was open at the last
/// look: it is fetched when it opens, not while it stays open.
#[derive(Debug, Clone, Default)]
pub struct Fetched {
    pub sessions: Vec<SessionItem>,
    at: Option<f64>,
    pending: bool,
    was_open: bool,
}

impl Fetched {
    /// Whether to ask now: the list just opened and what is held is older than the refresh (or none) and no request is out.
    pub fn should_fetch(&mut self, open: bool, now: f64) -> bool {
        let opened = open && !self.was_open;
        self.was_open = open;
        let stale = self.at.is_none_or(|at| now - at >= SESSIONS_REFRESH);
        if opened && stale && !self.pending {
            self.pending = true;
            return true;
        }
        false
    }

    /// The answer; errors are swallowed (the session list's own sessions are still offered) and leave the cache stale.
    pub fn arrived(&mut self, result: Result<Vec<SessionItem>, String>, now: f64) {
        self.pending = false;
        if let Ok(sessions) = result {
            self.sessions = sessions;
            self.at = Some(now);
        }
    }
}

/// Everything the popup lists from, per open session.
#[derive(Debug, Default)]
pub struct Finder {
    pub commands: HashMap<String, Loaded<Command>>,
    pub agents: HashMap<String, Loaded<Agent>>,
    pub files: FileSearch,
    pub sessions: Fetched,
}

impl Finder {
    /// The session's commands arrived (or failed: the old list stays up, with the reason).
    pub fn on_commands(&mut self, session: &str, result: Result<(u16, Value), String>) {
        let entry = self.commands.entry(session.to_string()).or_default();
        match result {
            Ok((status, body)) if (200..300).contains(&status) => {
                entry.items = parse_commands(&body);
                entry.error = None;
            }
            Ok((status, body)) => entry.error = Some(failure(status, &body)),
            Err(why) => entry.error = Some(why),
        }
    }

    /// The agent list the picker asked for (shared: one request serves both).
    pub fn on_agents(&mut self, session: &str, result: Result<&Value, &str>) {
        let entry = self.agents.entry(session.to_string()).or_default();
        match result {
            Ok(body) => {
                entry.items = parse_agents(body);
                entry.error = None;
            }
            Err(why) => entry.error = Some(if why.trim().is_empty() { "Failed to load agents".to_string() } else { why.to_string() }),
        }
    }
}
