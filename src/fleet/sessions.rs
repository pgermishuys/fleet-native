//! The session list from `GET /api/sessions`: rows grouped under their projects, in the shapes the
//! session list draws (`mock::Project`, `mock::Session`).
use super::model::StreamStatus;
use crate::mock::{Project, Session, Status};
use serde_json::Value;

/// How a session's activity reads in the list. Fleet's `waiting_input` is the amber diamond; every
/// kind of work is the working glyph.
pub fn status_of(activity: &str) -> Status {
    match activity {
        "busy" | "working" | "retry" | "delegating" => Status::Working,
        "waiting_input" => Status::NeedsInput,
        _ => Status::Idle,
    }
}

/// How the open conversation's derived status reads in the list: the same glyphs as `status_of`.
pub fn stream_status_of(status: StreamStatus) -> Status {
    match status {
        StreamStatus::Idle => Status::Idle,
        StreamStatus::WaitingInput => Status::NeedsInput,
        StreamStatus::Busy | StreamStatus::Retry | StreamStatus::Delegating => Status::Working,
    }
}

/// A compact age ("now", "2m", "3h", "4d") and how far Fleet dims a row for it: 55% after a day
/// idle, 40% after three.
pub fn age(now_ms: i64, then_ms: i64) -> (String, f32) {
    let secs = ((now_ms - then_ms) / 1000).max(0);
    let label = match secs {
        0..60 => "now".to_string(),
        60..3600 => format!("{}m", secs / 60),
        3600..86_400 => format!("{}h", secs / 3600),
        _ => format!("{}d", secs / 86_400),
    };
    (
        label,
        if secs >= 3 * 86_400 {
            0.4
        } else if secs >= 86_400 {
            0.55
        } else {
            1.0
        },
    )
}

/// Groups the rows by project name, most recently used first inside each group, groups in the
/// order they first appear. Archived, hidden and sub-agent sessions aren't listed here (the session
/// list lane shows lineage and the Archived view).
pub fn parse_list(rows: &Value, now_ms: i64) -> Vec<Project> {
    let mut projects: Vec<Project> = Vec::new();
    let mut listed: Vec<(i64, usize, Session)> = Vec::new();
    for row in rows.as_array().into_iter().flatten() {
        let text = |k: &str| row.get(k).and_then(Value::as_str);
        if row.get("isHidden") == Some(&Value::Bool(true))
            || text("retentionStatus").is_some_and(|r| r != "active")
            || text("parentSessionId").is_some()
        {
            continue;
        }
        let Some(id) = row.pointer("/session/id").and_then(Value::as_str) else { continue };
        let name = text("projectName").unwrap_or("Sessions");
        let at = match projects.iter().position(|p| p.name == name) {
            Some(at) => at,
            None => {
                projects.push(Project {
                    name: name.to_string(),
                    path: text("workspaceDirectory").unwrap_or("").to_string(),
                    open: true,
                    sessions: vec![],
                });
                projects.len() - 1
            }
        };
        let updated = row.pointer("/session/time/updated").and_then(Value::as_i64).unwrap_or(0);
        let (age, dim) = age(now_ms, updated);
        let status = status_of(text("activityStatus").or(text("sessionStatus")).unwrap_or("idle"));
        listed.push((
            updated,
            at,
            Session {
                id: id.to_string(),
                title: row.pointer("/session/title").and_then(Value::as_str).unwrap_or("Untitled").to_string(),
                status,
                age: if status == Status::Idle { age } else { String::new() },
                dim: if status == Status::Idle { dim } else { 1.0 },
                branch: text("branch").unwrap_or("").to_string(),
                messages: Vec::new(),
                tokens: row.get("totalTokens").and_then(Value::as_u64).unwrap_or(0).min(u32::MAX as u64) as u32,
            },
        ));
    }
    listed.sort_by_key(|(updated, ..)| std::cmp::Reverse(*updated));
    for (_, at, session) in listed {
        projects[at].sessions.push(session);
    }
    projects
}
