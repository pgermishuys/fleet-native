//! One invented project that shows every word and glyph a session row can have (the `row-statuses` scene, and the
//! tests of the row). Nothing here comes from a real Fleet.
use super::{Project, Session, Status, long};
use crate::fleet::groups::ProjectKind;
use crate::fleet::rows::{Failure, Interruption, Retrying, ScheduledRetry};

/// The wall clock the scene fixes: noon UTC on 4 October 2024 (day 20 000 of the epoch).
pub const NOW: i64 = 20_000 * 86_400_000 + 12 * 3_600_000;

fn ago(secs: i64) -> i64 {
    NOW - secs * 1000
}

fn iso(ms: i64) -> String {
    let (y, mo, d, h, mi) = crate::conv::time::civil(ms, 0);
    format!("{y:04}-{:02}-{d:02}T{h:02}:{mi:02}:00Z", mo + 1)
}

fn row(title: &str, status: Status, lifecycle: &str, activity: &str, updated_ago: i64, edit: impl FnOnce(&mut Session)) -> Session {
    let mut s = Session {
        id: String::new(),
        title: title.into(),
        status,
        ago_secs: None,
        branch: "main".into(),
        messages: Vec::new(),
        tokens: 0,
        row: Default::default(),
        listing: Default::default(),
    };
    s.row.lifecycle = lifecycle.into();
    s.row.activity = activity.into();
    s.row.updated = ago(updated_ago);
    edit(&mut s);
    s
}

/// A project with a row for each status: live ones, the words, every age and each fade.
pub fn statuses() -> Project {
    let min = 60;
    let hour = 3600;
    let day = 86_400;
    let rows = vec![
        row("Move the image cache to disk", Status::Working, "active", "busy", 5, |_| {}),
        row("Fan out the migration checks", Status::Working, "active", "delegating", 5, |_| {}),
        row("Rebuild the search index", Status::Working, "active", "retry", 5, |s| {
            s.row.retrying =
                Retrying { attempt: Some(3), max_attempts: Some(10), message: Some("API overloaded (529)".into()), next: None };
        }),
        row("Which plan does the checkout page show?", Status::NeedsInput, "waiting_input", "waiting_input", 5, |_| {}),
        row("Rotate the signing keys", Status::Idle, "error", "idle", 5, |_| {}),
        row("Fix the flaky checkout test", Status::Idle, "idle", "idle", 12 * min, |_| {}),
        row("Upgrade the date picker", Status::Idle, "idle", "idle", 3 * hour, |_| {}),
        row("Tidy the release notes", Status::Idle, "idle", "idle", 2 * day, |_| {}),
        row("Add a dark map style", Status::Idle, "idle", "idle", 5 * day, |_| {}),
        row("Summarise last week's notes", Status::Idle, "idle", "idle", 3 * 7 * day, |_| {}),
        row("Draft the onboarding copy", Status::Idle, "idle", "idle", 120 * day, |_| {}),
        row("Ship the invoice export", Status::Idle, "completed", "idle", 20 * min, |_| {}),
        row("Check the staging box", Status::Idle, "disconnected", "idle", 40 * min, |_| {}),
        row("Port the importer", Status::Idle, "idle", "idle", 30 * min, |s| {
            s.row.failed = Some(Failure { at: iso(ago(30 * min)), message: "The model returned no answer".into() });
        }),
        row("Backfill the audit log", Status::Idle, "idle", "idle", 50 * min, |s| {
            s.row.interrupted =
                Some(Interruption { at: iso(ago(50 * min)), waiting: false, last_call: None, changed_files: None, auto: false });
        }),
        row("Reindex the catalogue", Status::Idle, "idle", "idle", 45 * min, |s| {
            s.row.interrupted =
                Some(Interruption { at: iso(ago(45 * min)), waiting: true, last_call: None, changed_files: None, auto: false });
        }),
        row("Summarise the quarter", Status::Idle, "idle", "idle", 10 * min, |s| {
            s.row.scheduled_retry = Some(ScheduledRetry {
                due_at: iso(NOW + 2 * hour * 1000 + 5 * min * 1000),
                attempt: 1,
                kind: "usage_limit".into(),
                reason: String::new(),
                provider_said: true,
            });
        }),
        row(long::TITLE, Status::Idle, "idle", "idle", hour + 5, |_| {}),
        row("   ", Status::Idle, "idle", "idle", 2 * hour, |_| {}),
    ];
    let mut project = Project {
        id: "mock-statuses".into(),
        kind: ProjectKind::User,
        position: 1,
        name: "Statuses".into(),
        path: "~/code/statuses".into(),
        open: true,
        sessions: rows,
    };
    for (i, s) in project.sessions.iter_mut().enumerate() {
        s.id = crate::fleet::store::invented_id(0, i);
        s.listing.project_id = Some(project.id.clone());
        s.listing.project_name = Some(project.name.clone());
    }
    project
}
