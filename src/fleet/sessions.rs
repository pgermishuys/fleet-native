//! The session list from `GET /api/sessions`: rows grouped under their projects, in the shapes the
//! session list draws (`mock::Project`, `mock::Session`).
use super::groups::{Listing, ProjectInfo, ProjectKind, build_project_groups};
use super::model::StreamStatus;
use super::row_status;
use super::rows::{Row, glyph_of};
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

/// The sessions the list holds, newest first. Archived, hidden and sub-agent sessions aren't listed here (the session
/// list lane shows lineage and the Archived view). Pure, so it can run on a thread.
pub fn parse_sessions(rows: &Value, _now_ms: i64) -> Vec<Session> {
    let mut listed: Vec<(i64, Session)> = Vec::new();
    for row in rows.as_array().into_iter().flatten() {
        let text = |k: &str| row.get(k).and_then(Value::as_str);
        if row.get("isHidden") == Some(&Value::Bool(true))
            || text("retentionStatus").is_some_and(|r| r != "active")
            || text("parentSessionId").is_some()
        {
            continue;
        }
        let Some(id) = row.pointer("/session/id").and_then(Value::as_str) else { continue };
        let updated = row.pointer("/session/time/updated").and_then(Value::as_i64).unwrap_or(0);
        let mut info = Row::from_list_item(row);
        info.updated = updated;
        info.created = row.pointer("/session/time/created").and_then(Value::as_i64).unwrap_or(0);
        let status = glyph_of(&info.lifecycle, &info.activity);
        listed.push((
            updated,
            Session {
                id: id.to_string(),
                title: row_status::display_title(None, row.pointer("/session/title").and_then(Value::as_str).unwrap_or("")),
                status,
                ago_secs: None,
                branch: text("branch").unwrap_or("").to_string(),
                messages: Vec::new(),
                tokens: row.get("totalTokens").and_then(Value::as_u64).unwrap_or(0).min(u32::MAX as u64) as u32,
                row: info,
                listing: Listing::from_row(row),
            },
        ));
    }
    listed.sort_by_key(|(updated, ..)| std::cmp::Reverse(*updated));
    listed.into_iter().map(|(_, session)| session).collect()
}

/// The sessions under their projects (rows 1.39-1.41): every user project even when it has none, in the projects' own
/// order, Ungrouped last. Pinned sessions stay in their project until the Pinned group is drawn (component 6, piece J).
pub fn group_sessions(sessions: Vec<Session>, infos: &[ProjectInfo]) -> Vec<Project> {
    build_project_groups(sessions, infos, None)
        .into_iter()
        .map(|g| Project {
            kind: g.project_id.as_deref().and_then(|id| infos.iter().find(|p| p.id == id)).map_or(ProjectKind::User, |p| p.kind),
            position: g.project_id.as_deref().and_then(|id| infos.iter().find(|p| p.id == id)).map_or(0, |p| p.position),
            path: g.sessions.first().map(|s| s.listing.workspace_directory.clone()).unwrap_or_default(),
            id: g.id,
            name: g.name,
            open: true,
            sessions: g.sessions,
        })
        .collect()
}

/// `parse_sessions` and `group_sessions` in one, with no projects read: every group is named by its rows.
pub fn parse_list(rows: &Value, now_ms: i64) -> Vec<Project> {
    parse_list_in(rows, &[], now_ms)
}

/// The same under the projects read from `GET /api/projects`.
pub fn parse_list_in(rows: &Value, infos: &[ProjectInfo], now_ms: i64) -> Vec<Project> {
    group_sessions(parse_sessions(rows, now_ms), infos)
}
