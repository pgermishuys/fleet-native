//! The session list's rows follow the `sessions` topic (checklist rows 7.1 and 7.4).
use super::*;
use crate::fleet::rows::{RowPatch, derive_session_status};
use crate::mock::{Project, Session};

/// An `activity_status` patch, as the hub would send it.
pub(super) fn activity(id: &str, status: &str) -> RowPatch {
    patch("activity_status", json!({"sessionId": id, "activityStatus": status}))
}

fn patch(kind: &str, properties: Value) -> RowPatch {
    RowPatch::from_wire(&json!({"type": kind, "properties": properties})).expect("a row event")
}

fn row(id: &str, workspace: &str, session_status: &str, activity: &str) -> Value {
    json!({
        "workspaceId": workspace, "workspaceDirectory": "/work/demo", "sessionStatus": session_status, "activityStatus": activity,
        "retentionStatus": "active", "projectName": "Demo", "branch": "main", "totalTokens": 10, "totalCost": 0.5,
        "session": {"id": id, "title": id, "time": {"updated": 1_000}},
    })
}

fn rows_of(items: Vec<Value>) -> Vec<Project> {
    sessions::parse_list(&Value::Array(items), 1_000)
}

fn find<'a>(projects: &'a [Project], id: &str) -> &'a Session {
    projects.iter().flat_map(|p| &p.sessions).find(|s| s.id == id).expect("no such row")
}

fn apply(projects: &mut [Project], p: RowPatch) {
    p.apply(projects);
}

#[test]
fn derives_session_status_from_activity_status() {
    assert_eq!(derive_session_status("idle", "active"), "idle");
    assert_eq!(derive_session_status("waiting_input", "idle"), "waiting_input");
    for busy in ["busy", "delegating", "retry"] {
        assert_eq!(derive_session_status(busy, "idle"), "active");
    }
    assert_eq!(derive_session_status("something-new", "idle"), "active");
}

#[test]
fn a_lifecycle_status_is_not_clobbered_by_an_activity_one() {
    for lifecycle in ["stopped", "completed", "error", "disconnected"] {
        assert_eq!(derive_session_status("busy", lifecycle), lifecycle);
        let mut list = rows_of(vec![row("a", "w", lifecycle, "idle")]);
        apply(&mut list, activity("a", "busy"));
        assert_eq!(find(&list, "a").row.lifecycle, lifecycle);
        assert_eq!(find(&list, "a").status, Status::Idle, "{lifecycle} has no working glyph");
    }
}

#[test]
fn an_activity_event_sets_the_glyph_and_the_status() {
    let mut list = rows_of(vec![row("a", "w", "idle", "idle")]);
    apply(&mut list, activity("a", "busy"));
    assert_eq!((find(&list, "a").status, find(&list, "a").row.lifecycle.as_str()), (Status::Working, "active"));
    apply(&mut list, activity("a", "waiting_input"));
    assert_eq!((find(&list, "a").status, find(&list, "a").row.lifecycle.as_str()), (Status::NeedsInput, "waiting_input"));
    apply(&mut list, activity("a", "idle"));
    assert_eq!((find(&list, "a").status, find(&list, "a").age.as_str()), (Status::Idle, "now"));
}

#[test]
fn every_activity_reads_as_a_glyph_on_the_row() {
    // `status_of`, as `session-row-status.ts` draws it.
    for (activity, glyph) in [
        ("busy", Status::Working),
        ("working", Status::Working),
        ("retry", Status::Working),
        ("delegating", Status::Working),
        ("waiting_input", Status::NeedsInput),
        ("idle", Status::Idle),
        ("something-new", Status::Idle),
    ] {
        assert_eq!(sessions::status_of(activity), glyph, "{activity}");
    }
}

#[test]
fn a_retrying_session_keeps_why_and_when_until_it_works_again() {
    let mut list = rows_of(vec![row("a", "w", "active", "busy")]);
    apply(
        &mut list,
        patch(
            "activity_status",
            json!({"sessionId": "a", "activityStatus": "retry", "attempt": 2, "maxAttempts": 5, "message": "API overloaded (529)", "next": "2026-11-15T10:00:00Z"}),
        ),
    );
    let r = &find(&list, "a").row.retrying;
    assert_eq!((r.attempt, r.max_attempts), (Some(2), Some(5)));
    assert_eq!((r.message.as_deref(), r.next.as_deref()), (Some("API overloaded (529)"), Some("2026-11-15T10:00:00Z")));
    assert_eq!(find(&list, "a").status, Status::Working);
    apply(&mut list, activity("a", "busy"));
    assert_eq!(find(&list, "a").row.retrying, Default::default());
}

#[test]
fn a_list_item_that_retries_carries_its_wording() {
    let mut item = row("a", "w", "active", "retry");
    item["retryAttempt"] = json!(3);
    item["retryMessage"] = json!("Rate limited");
    let list = rows_of(vec![item]);
    assert_eq!(find(&list, "a").row.retrying.attempt, Some(3));
    assert_eq!(find(&list, "a").row.retrying.message.as_deref(), Some("Rate limited"));
}

#[test]
fn a_scheduled_retry_is_set_and_cleared() {
    let mut list = rows_of(vec![row("a", "w", "idle", "idle")]);
    let retry =
        json!({"dueAt": "2026-11-15T10:00:00Z", "attempt": 2, "kind": "rate_limit", "reason": "Too many requests", "providerSaid": true});
    apply(&mut list, patch("session.retry", json!({"sessionId": "a", "retry": retry})));
    let got = find(&list, "a").row.scheduled_retry.clone().unwrap();
    assert_eq!((got.attempt, got.kind.as_str(), got.provider_said), (2, "rate_limit", true));
    apply(&mut list, patch("session.retry", json!({"sessionId": "a", "retry": null})));
    assert_eq!(find(&list, "a").row.scheduled_retry, None);
    // `toScheduledRetry`: an unknown kind, or no usable time, is none.
    apply(&mut list, patch("session.retry", json!({"sessionId": "a", "retry": {"dueAt": "2026-11-15T10:00:00Z", "kind": "weather"}})));
    assert_eq!(find(&list, "a").row.scheduled_retry, None);
}

#[test]
fn pages_interruptions_and_failures_patch_their_row() {
    let mut list = rows_of(vec![row("a", "w", "idle", "idle"), row("b", "w", "idle", "idle")]);
    apply(&mut list, patch("session.pages", json!({"sessionId": "a", "pageCount": 2, "lastPageShownAt": "2026-11-15T09:00:00Z"})));
    assert_eq!((find(&list, "a").row.page_count, find(&list, "b").row.page_count), (2, 0));
    assert_eq!(find(&list, "a").row.last_page_shown_at.as_deref(), Some("2026-11-15T09:00:00Z"));

    let cut = json!({"at": "2026-11-15T09:00:00Z", "waiting": true, "lastCall": " ", "changedFiles": 3, "auto": false});
    apply(&mut list, patch("session.interrupted", json!({"sessionId": "a", "interruption": cut})));
    let got = find(&list, "a").row.interrupted.clone().unwrap();
    assert_eq!((got.waiting, got.last_call, got.changed_files), (true, None, Some(3)));
    apply(&mut list, patch("session.interrupted", json!({"sessionId": "a", "interruption": null})));
    assert_eq!(find(&list, "a").row.interrupted, None);

    apply(
        &mut list,
        patch("session.failure", json!({"sessionId": "b", "failure": {"at": "2026-11-15T09:00:00Z", "message": "Provider returned 529"}})),
    );
    assert_eq!(find(&list, "b").row.failed.as_ref().map(|f| f.message.as_str()), Some("Provider returned 529"));
    assert_eq!(find(&list, "a").row.failed, None);
    apply(&mut list, patch("session.failure", json!({"sessionId": "b", "failure": null})));
    assert_eq!(find(&list, "b").row.failed, None);
}

#[test]
fn counted_tokens_and_cost_reach_the_row() {
    let mut list = rows_of(vec![row("a", "w", "idle", "idle")]);
    apply(&mut list, patch("session_tokens", json!({"sessionId": "a", "totalTokens": 4200, "totalCost": 1.25})));
    assert_eq!((find(&list, "a").tokens, find(&list, "a").row.cost), (4200, 1.25));
}

#[test]
fn a_branch_change_reaches_every_session_of_the_workspace() {
    let mut list = rows_of(vec![row("a", "w1", "idle", "idle"), row("b", "w1", "idle", "idle"), row("c", "w2", "idle", "idle")]);
    apply(&mut list, patch("session_branch", json!({"workspaceId": "w1", "branch": "feature"})));
    assert_eq!([&find(&list, "a").branch, &find(&list, "b").branch, &find(&list, "c").branch], ["feature", "feature", "main"]);
}

#[test]
fn a_checkout_event_reaches_every_session_of_the_workspace() {
    let mut list = rows_of(vec![row("a", "w1", "idle", "idle"), row("b", "w1", "idle", "idle"), row("c", "w2", "idle", "idle")]);
    apply(&mut list, patch("session_checkout", json!({"workspaceId": "w1", "state": "removed", "removedAt": "2026-11-15T09:00:00Z"})));
    assert_eq!(find(&list, "b").row.checkout_removed_at.as_deref(), Some("2026-11-15T09:00:00Z"));
    assert_eq!(find(&list, "c").row.checkout_removed_at, None);
    apply(&mut list, patch("session_checkout", json!({"workspaceId": "w1", "state": "restoring", "removedAt": null})));
    assert!(find(&list, "a").row.checkout_restoring);
    assert!(find(&list, "a").row.checkout_removed_at.is_some(), "restoring keeps the removal time until it is back");
    apply(&mut list, patch("session_checkout", json!({"workspaceId": "w1", "state": "restored", "removedAt": null})));
    assert_eq!((find(&list, "a").row.checkout_removed_at.clone(), find(&list, "a").row.checkout_restoring), (None, false));
}

#[test]
fn an_event_for_a_session_not_in_the_list_is_ignored() {
    let mut list = rows_of(vec![row("a", "w", "idle", "idle")]);
    for p in [
        activity("zzz", "busy"),
        patch("session_tokens", json!({"sessionId": "zzz", "totalTokens": 9, "totalCost": 9.0})),
        patch("session.pages", json!({"sessionId": "zzz", "pageCount": 4, "lastPageShownAt": null})),
    ] {
        apply(&mut list, p);
    }
    let a = find(&list, "a");
    assert_eq!((a.status, a.tokens, a.row.page_count), (Status::Idle, 10, 0));
}

#[test]
fn events_that_are_not_row_changes_leave_the_list_to_the_refetch() {
    for kind in ["session_created", "session_archived", "session.deleted", "smart_link.updated", "usage.limits"] {
        assert_eq!(RowPatch::from_wire(&json!({"type": kind, "properties": {"sessionId": "a"}})), None, "{kind}");
    }
    // An activity event without a session or a status is ignored, as in Vue.
    assert_eq!(RowPatch::from_wire(&json!({"type": "activity_status", "properties": {"activityStatus": "busy"}})), None);
    assert_eq!(RowPatch::from_wire(&json!({"type": "activity_status", "properties": {"sessionId": "a"}})), None);
}

#[test]
fn a_patch_reaches_the_open_row_and_starts_its_clock() {
    let mut rt = super::state::app();
    rt.update(|s, tx| s.on_list(super::state::list(), tx));
    let id = rt.state.open_id().unwrap();
    rt.update(move |s, _| s.on_row_patch(activity(&id, "busy")));
    assert_eq!(rt.state.session().status, Status::Working);
    assert!(rt.state.working_since >= 0.0);
}
