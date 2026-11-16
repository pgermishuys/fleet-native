//! The last list of each other machine, kept in the app's data folder (`machine-sessions.json`, beside `settings.json`;
//! the format is ours) so a machine that is away still shows its rows after a restart (`SESSIONS_CACHE_KEY`,
//! `trimForCache`, `loadCachedSessions`). Only what a machine's tree draws is kept: no token, no path, no message.
//!
//! Pure except `load`, `write` and `spawn_save`, which the app calls on threads.
use super::groups::{ProjectInfo, ProjectKind};
use super::machines::{MachineSessions, Machines};
use super::sessions::parse_sessions;
use crate::mock::Session;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const FILE: &str = "machine-sessions.json";

/// A session reduced to what the tree shows (`trimForCache`), in the shape `GET /api/sessions` sends, so reading it back
/// is `parse_sessions`.
pub fn trim_session(s: &Session) -> Value {
    let l = &s.listing;
    json!({
        "session": { "id": s.id, "title": s.title, "time": { "created": s.row.created, "updated": s.row.updated } },
        "sessionStatus": s.row.lifecycle,
        "activityStatus": s.row.activity,
        "retentionStatus": if s.row.archived { "archived" } else { "active" },
        "parentSessionId": l.parent,
        "retryAttempt": s.row.retrying.attempt,
        "projectId": l.project_id,
        "projectName": l.project_name,
        "pinOrder": l.pin_order,
        "forkedFromSessionId": l.forked_from,
        "spawnedBySessionId": l.spawned_by,
        "spawnKind": l.spawn_kind,
        // Only whether the user moved it out of its parent is drawn; the time isn't.
        "lineageDetachedAt": l.detached.then_some("detached"),
        "runningWorkCount": s.row.running_work,
        "workflowRunId": l.workflow_run_id,
    })
}

fn trim_project(p: &ProjectInfo) -> Value {
    json!({ "id": p.id, "name": p.name, "type": if p.kind == ProjectKind::Scratch { "scratch" } else { "user" }, "position": p.position })
}

/// The file's text: each machine that has been read once, and where each session lives.
pub fn to_json(machines: &Machines) -> String {
    let others: serde_json::Map<String, Value> = machines
        .others
        .iter()
        .filter_map(|(key, m)| {
            let at = m.loaded_at?;
            Some((
                key.clone(),
                json!({
                    "sessions": m.sessions.iter().map(trim_session).collect::<Vec<_>>(),
                    "projects": m.projects.iter().map(trim_project).collect::<Vec<_>>(),
                    "loadedAt": at,
                }),
            ))
        })
        .collect();
    json!({ "machines": others, "sessionMachines": machines.session_machines }).to_string()
}

/// What the file held.
#[derive(Default)]
pub struct Cached {
    pub others: HashMap<String, MachineSessions>,
    pub session_machines: HashMap<String, String>,
}

/// Reads the file's text. A corrupt file, or any part that isn't the shape written, reads as empty (`loadCachedSessions`).
pub fn parse(text: &str) -> Cached {
    let Ok(root) = serde_json::from_str::<Value>(text) else { return Cached::default() };
    let others = root
        .get("machines")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(key, v)| {
            let rows = v.get("sessions").filter(|s| s.is_array()).cloned().unwrap_or(Value::Array(Vec::new()));
            let projects = v.get("projects").map(super::groups::parse_projects).unwrap_or_default();
            let state = MachineSessions {
                sessions: parse_sessions(&rows, 0),
                projects,
                error: None,
                loaded_at: v.get("loadedAt").and_then(Value::as_i64),
                loading: false,
            };
            (key.clone(), state)
        })
        .collect();
    let session_machines = root
        .get("sessionMachines")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(id, key)| Some((id.clone(), key.as_str()?.to_string())))
        .collect();
    Cached { others, session_machines }
}

/// Reads the file in `dir`; none, or one that can't be read, is empty. Blocking.
pub fn load(dir: &Path) -> Cached {
    std::fs::read_to_string(dir.join(FILE)).map(|t| parse(&t)).unwrap_or_default()
}

/// Writes to a temporary name, then renames over, so a crash leaves the old file whole. Blocking.
pub fn write(dir: &Path, text: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{FILE}.tmp"));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, dir.join(FILE))
}

static ASKED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static WRITTEN: std::sync::Mutex<u64> = std::sync::Mutex::new(0);

/// Writes on a thread; a write asked for before one already written is dropped, so a slow thread never puts an older
/// list over a newer one. A failure is dropped too: without the cache an away machine just shows no rows.
pub fn spawn_save(dir: PathBuf, text: String) {
    let mine = ASKED.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        let mut written = WRITTEN.lock().unwrap_or_else(|e| e.into_inner());
        if mine > *written {
            *written = mine;
            let _ = write(&dir, &text);
        }
    });
}
