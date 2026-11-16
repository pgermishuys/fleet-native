//! The sessions store and the open session's route (`stores/sessions.ts`, `composables/use-sessions.ts`, rows 6.1-6.5,
//! 6.9, 6.11-6.14). Pure: no Rune, no I/O.
//!
//! Fleet keeps the open session in the URL, so a refetch, a filter or a pin cannot move it. This app has no URL: the
//! open session is a `Route` that names it by id, and where it sits in the list is looked up each time it is needed
//! (`State::open_at`).
use super::groups::ProjectKind;
use crate::mock::{Project, Session};
use serde_json::Value;

/// What the window is showing. Fleet's other routes (`/sessions/new`, `/settings`) join it with their components.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Route {
    /// No session is open.
    #[default]
    None,
    /// `/sessions/<id>`.
    Session(String),
    /// `/sessions/new?projectId=`: the New session page (component 7).
    NewSession { project_id: Option<String> },
}

/// Ids of the invented sessions start with this; the server never hears of them.
const INVENTED: &str = "mock.";

/// The invented session at (project, session) in `mock::projects()`.
pub fn invented_id(project: usize, session: usize) -> String {
    format!("{INVENTED}{project}.{session}")
}

/// Whether `id` belongs to an invented session, which has no conversation on a server.
pub fn is_invented(id: &str) -> bool {
    id.starts_with(INVENTED)
}

/// Which sessions `GET /api/sessions` lists (`retentionStatus`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Retention {
    #[default]
    Active,
    Archived,
    All,
}

impl Retention {
    /// The query string of the list request: `retentionStatus` is left out for the active list (row 6.1).
    pub fn query(self) -> &'static str {
        match self {
            Retention::Active => "",
            Retention::Archived => "&retentionStatus=archived",
            Retention::All => "&retentionStatus=all",
        }
    }
}

/// `DEFAULT_PAGE_SIZE`: the list is one page and never pages (row 6.1).
pub const PAGE_SIZE: usize = 100;

/// The path of the list request.
pub fn list_path(retention: Retention) -> String {
    format!("/api/sessions?limit={PAGE_SIZE}&offset=0{}", retention.query())
}

/// `open_at`'s project number for a session kept from another machine (the live list never reaches it).
pub const ELSEWHERE: usize = usize::MAX - 1;

/// How many sessions on other machines `elsewhere` keeps (`ELSEWHERE_KEPT`).
pub const ELSEWHERE_KEPT: usize = 20;

/// The list's state beyond its rows: the load flags of `useSessions`, and the sessions opened from other machines.
#[derive(Default)]
pub struct SessionStore {
    pub retention: Retention,
    /// True after the first list arrived; a session added by `upsert` doesn't make a list.
    pub list_loaded: bool,
    /// The first fetch with nothing held.
    pub is_loading: bool,
    /// Any later fetch.
    pub is_refreshing: bool,
    /// The last fetch's failure, cleared by the next good one.
    pub error: Option<String>,
    /// The ids of the last list in the order it arrived.
    pub shown: Vec<String>,
    request: u64,
    loaded_once: bool,
    /// Sessions on other machines opened here, oldest first, with their machine's key. The live list never holds them.
    elsewhere: Vec<(String, Session)>,
}

impl SessionStore {
    /// A fetch starts: the id its answer must come back with. `has_rows` is whether the list holds anything.
    pub fn begin(&mut self, has_rows: bool) -> u64 {
        self.request += 1;
        if !self.loaded_once && !has_rows {
            self.is_loading = true;
        } else {
            self.is_refreshing = true;
        }
        self.request
    }

    /// Whether an answer to `request` is still wanted: a newer fetch has overtaken the ones before it.
    pub fn is_current(&self, request: u64) -> bool {
        request == self.request
    }

    /// An answer. `false` (and nothing changed) for a request a newer one has overtaken. A failure keeps the rows.
    pub fn finish(&mut self, request: u64, result: Result<(), String>) -> bool {
        if !self.is_current(request) {
            return false;
        }
        match result {
            Ok(()) => {
                self.error = None;
                self.list_loaded = true;
            }
            Err(e) => self.error = Some(e),
        }
        self.loaded_once = true;
        self.is_loading = false;
        self.is_refreshing = false;
        true
    }

    /// Keeps the row of a session on another machine that is open here, making it the newest (`upsertElsewhere`).
    pub fn upsert_elsewhere(&mut self, machine: &str, mut session: Session) {
        // A session opened before keeps the conversation it holds.
        if let Some((_, old)) = self.elsewhere.iter_mut().find(|(_, s)| s.id == session.id) {
            session.messages = std::mem::take(&mut old.messages);
        }
        self.elsewhere.retain(|(_, s)| s.id != session.id);
        self.elsewhere.push((machine.to_string(), session));
        while self.elsewhere.len() > ELSEWHERE_KEPT {
            self.elsewhere.remove(0);
        }
    }

    /// Forgets the sessions of a machine that is no longer listed.
    pub fn forget_elsewhere(&mut self, machine: &str) {
        self.elsewhere.retain(|(m, _)| m != machine);
    }

    /// The ids kept from other machines, oldest first.
    pub fn elsewhere_ids(&self) -> Vec<&str> {
        self.elsewhere.iter().map(|(_, s)| s.id.as_str()).collect()
    }

    /// Where a kept session is, for `open_at`'s place past the live list: its index, if it is kept.
    pub fn elsewhere_index(&self, id: &str) -> Option<usize> {
        self.elsewhere.iter().position(|(_, s)| s.id == id)
    }

    pub fn elsewhere_at(&self, index: usize) -> Option<&Session> {
        self.elsewhere.get(index).map(|(_, s)| s)
    }

    pub fn elsewhere_at_mut(&mut self, index: usize) -> Option<&mut Session> {
        self.elsewhere.get_mut(index).map(|(_, s)| s)
    }

    /// The machine a kept session is on.
    pub fn machine_of(&self, id: &str) -> Option<&str> {
        self.elsewhere.iter().find(|(_, s)| s.id == id).map(|(m, _)| m.as_str())
    }

    /// The session's row: from the live list, or from the sessions kept from other machines (`sessionById`).
    pub fn session_by_id<'a>(&'a self, projects: &'a [Project], id: &str) -> Option<&'a Session> {
        if id.is_empty() {
            return None;
        }
        projects
            .iter()
            .flat_map(|p| &p.sessions)
            .find(|s| s.id == id)
            .or_else(|| self.elsewhere.iter().map(|(_, s)| s).find(|s| s.id == id))
    }

    /// Changes a known session in place: a kept one first, then the live list; an unknown id does nothing (`patchSession`).
    pub fn patch(&mut self, projects: &mut [Project], id: &str, change: impl FnOnce(&mut Session)) {
        if let Some((_, s)) = self.elsewhere.iter_mut().find(|(_, s)| s.id == id) {
            return change(s);
        }
        if let Some(s) = projects.iter_mut().flat_map(|p| p.sessions.iter_mut()).find(|s| s.id == id) {
            change(s);
        }
    }

    /// Takes a session out of the store after a delete (`removeSession`). Clears the route when it named that session.
    /// Returns whether it was anywhere.
    pub fn remove(&mut self, projects: &mut [Project], route: &mut Route, id: &str) -> bool {
        let kept = self.elsewhere.len();
        self.elsewhere.retain(|(_, s)| s.id != id);
        let mut found = self.elsewhere.len() != kept;
        for p in projects.iter_mut() {
            let before = p.sessions.len();
            p.sessions.retain(|s| s.id != id);
            found |= p.sessions.len() != before;
        }
        if found && *route == Route::Session(id.to_string()) {
            *route = Route::None;
        }
        found
    }
}

/// Every row of the list, project by project.
pub fn rows(projects: &[Project]) -> impl Iterator<Item = &Session> {
    projects.iter().flat_map(|p| &p.sessions)
}

/// Moves a session to a project, with the rest of its workflow run: Fleet moves a run's steps together
/// (`patchSessionProject`). Only the rows' project is set; the caller groups the list again.
pub fn patch_project(projects: &mut [Project], id: &str, project_id: Option<&str>, project_name: Option<&str>) {
    let run = rows(projects).find(|s| s.id == id).and_then(|s| s.listing.workflow_run_id.clone());
    for s in projects.iter_mut().flat_map(|p| p.sessions.iter_mut()) {
        if s.id == id || (run.is_some() && s.listing.workflow_run_id == run) {
            s.listing.project_id = project_id.map(str::to_string);
            s.listing.project_name = project_name.map(str::to_string);
        }
    }
}

/// Adds a session the list has not seen, or updates the one it has, in place (`upsertSession`). The list is newest
/// first, so a new one goes to the top. Only the row's data changes; the conversation a known session carries stays.
/// The caller groups the list again.
pub fn upsert(projects: &mut Vec<Project>, session: Session) {
    if let Some(known) = projects.iter_mut().flat_map(|p| p.sessions.iter_mut()).find(|s| s.id == session.id) {
        let messages = std::mem::take(&mut known.messages);
        *known = Session { messages, ..session };
        return;
    }
    let name = session.listing.project_name.clone().unwrap_or_else(|| "Ungrouped".into());
    let project_id = session.listing.project_id.clone();
    let at = projects.iter().position(|p| project_id.as_ref().is_some_and(|id| &p.id == id));
    let at = at.unwrap_or_else(|| {
        projects.insert(
            0,
            Project {
                id: project_id.unwrap_or_default(),
                kind: ProjectKind::User,
                position: 0,
                name,
                path: session.listing.workspace_directory.clone(),
                open: true,
                sessions: Vec::new(),
            },
        );
        0
    });
    projects[at].sessions.insert(0, session);
}

/// Whether the list the server sent differs from the one shown, in anything the list draws (`sessionsChanged`). A poll
/// that says the same thing causes no redraw. `shown` is the ids of the last list in the order it arrived (the shown
/// groups reorder them). Compared: id and order, project, status words, title, lineage ids, pin order and the time
/// last used, the running count and the progress. Not yet compared because the row doesn't hold it: `archivedAt`
/// (piece I adds it).
pub fn changed(projects: &[Project], shown: &[String], next: &[Session]) -> bool {
    shown.len() != next.len()
        || shown.iter().zip(next).any(|(id, b)| id != &b.id || !rows(projects).find(|a| a.id == b.id).is_some_and(|a| same_row(a, b)))
}

fn same_row(a: &Session, b: &Session) -> bool {
    a.id == b.id
        && a.title == b.title
        && a.status == b.status
        && a.row.lifecycle == b.row.lifecycle
        && a.row.activity == b.row.activity
        && a.row.archived == b.row.archived
        && a.row.updated == b.row.updated
        && a.row.running_work == b.row.running_work
        && super::row_chips::same_progress(a.row.progress.as_ref(), b.row.progress.as_ref())
        && a.listing == b.listing
}

/// The words of a failed request (`readErrorMessage`): the body's `error`, then `detail`, then `title`, each when a
/// non-blank string; else the body's own text; else `HTTP <status>`.
pub fn error_text(status: u16, body: &str) -> String {
    if let Ok(Value::Object(fields)) = serde_json::from_str::<Value>(body) {
        for key in ["error", "detail", "title"] {
            if let Some(text) = fields.get(key).and_then(Value::as_str).filter(|t| !t.trim().is_empty()) {
                return text.to_string();
            }
        }
    }
    match body.trim() {
        "" => format!("HTTP {status}"),
        text => text.to_string(),
    }
}
