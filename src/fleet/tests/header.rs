//! The header's data (`fleet::header`): the cases of weave-fleet's `retry-status.test.ts`, `SessionDetailHeader.test.ts`
//! ("status" and "for a session on a machine that isn't answering"), `session-lineage.test.ts` ("lineageOf" and "moving a
//! session out of its parent") and the route's capability fallbacks, under Fleet's names, plus ours.
use crate::conv::working::{describe_retry, retry_countdown};
use crate::fleet::groups::{LineageKind, Listing, lineage_of, lineage_origin_of};
use crate::fleet::header::*;
use crate::fleet::rows::{Capabilities, Retrying, Row};
use serde_json::json;

// 2026-10-07T10:00:00Z
const NOW: i64 = 1_791_367_200_000;

fn retry(attempt: Option<i64>, max: Option<i64>, message: Option<&str>, next: Option<&str>) -> Retrying {
    Retrying { attempt, max_attempts: max, message: message.map(str::to_string), next: next.map(str::to_string) }
}

fn model_of(row: &Row, listing: &Listing, machine: Option<MachineChip>) -> HeaderModel {
    HeaderModel::build(&Inputs {
        title: "Consolidate status indicators",
        project_name: Some("Demo"),
        row,
        listing,
        branch: "",
        prompt_in_flight: false,
        machine,
        harnesses: &[],
        models: &[],
        now: NOW,
        offset_secs: 0,
        restoring_here: false,
        restored_note: false,
    })
}

fn row(lifecycle: &str, activity: &str) -> Row {
    Row { lifecycle: lifecycle.into(), activity: activity.into(), ..Row::default() }
}

// ─── retry-status.test.ts ───────────────────────────────────────────────────

#[test]
fn retry_status_says_the_attempt_out_of_how_many_when_and_why() {
    let r = retry(Some(3), Some(10), Some("API overloaded (529)"), Some("2026-10-07T10:00:12Z"));
    assert_eq!(describe_retry(&r, NOW), "Retrying · attempt 3 of 10 · in 12 s · API overloaded (529)");
}

#[test]
fn retry_status_leaves_out_what_the_harness_didnt_say() {
    assert_eq!(describe_retry(&retry(Some(2), None, Some("Too many requests"), None), NOW), "Retrying · attempt 2 · Too many requests");
    assert_eq!(describe_retry(&Retrying::default(), NOW), "Retrying");
}

#[test]
fn retry_status_counts_down_in_seconds_then_minutes_then_says_now() {
    let c = |next: &str| retry_countdown(&retry(None, None, None, Some(next)), NOW);
    assert_eq!(c("2026-10-07T10:00:00.400Z").as_deref(), Some("in 1 s"));
    assert_eq!(c("2026-10-07T10:02:30Z").as_deref(), Some("in 3 min"));
    assert_eq!(c("2026-10-07T09:59:59Z").as_deref(), Some("now"));
    assert_eq!(c("not a time"), None);
}

// (`retryShortLabel` is the list row's: `row_status`'s tests have it.)

// ─── SessionDetailHeader status ─────────────────────────────────────────────

#[test]
fn session_detail_header_status_shows_no_visible_status_while_a_sessions_list_is_on_screen() {
    // Whether the glyph is drawn depends on the list being shown, which is the view's (piece B); the model always has
    // the glyph ready and no retry note for a session that isn't retrying.
    let m = model_of(&row("active", "busy"), &Listing::default(), None);
    assert_eq!(m.retry_note, None);
}

#[test]
fn session_detail_header_status_keeps_a_screen_reader_status_with_the_activity() {
    let m = model_of(&row("active", "idle"), &Listing::default(), None);
    assert_eq!((m.indicator.word(), m.status_label.as_str()), ("idle", "Idle"));
}

#[test]
fn session_detail_header_status_shows_the_rows_glyph_when_no_sessions_list_is_on_screen() {
    let m = model_of(&row("active", "busy"), &Listing::default(), None);
    assert_eq!((m.glyph, m.status_label.as_str()), (Glyph::Active, "Working"));
}

#[test]
fn session_detail_header_status_names_a_retry_and_its_attempt_next_to_the_title() {
    let mut r = row("active", "retry");
    r.retrying = retry(Some(3), None, None, None);
    let m = model_of(&r, &Listing::default(), None);
    assert_eq!(m.retry_note.as_deref(), Some("Retrying · attempt 3"));
    assert_eq!(m.indicator.word(), "retry");
    assert_eq!(m.status_label, "Retrying (attempt 3)\u{2026}");
}

#[test]
fn session_detail_header_status_says_a_session_stopped_on_a_question_needs_input_with_the_rows_diamond() {
    let m = model_of(&row("active", "waiting_input"), &Listing::default(), None);
    assert_eq!((m.indicator.word(), m.status_label.as_str(), m.glyph), ("waiting", "Needs input", Glyph::WaitingInput));
}

/// Every status word, in one table: lifecycle and activity in, indicator, label and glyph out.
#[test]
fn the_status_words() {
    let table = [
        ("active", "busy", "working", "Working", Glyph::Active),
        ("active", "delegating", "working", "Working", Glyph::Active),
        ("active", "retry", "retry", "Retrying\u{2026}", Glyph::Active),
        ("active", "waiting_input", "waiting", "Needs input", Glyph::WaitingInput),
        ("idle", "idle", "idle", "Idle", Glyph::Idle),
        ("", "", "idle", "Idle", Glyph::Idle),
        ("disconnected", "busy", "disconnected", "Disconnected", Glyph::Disconnected),
        ("completed", "idle", "idle", "Idle", Glyph::Completed),
        ("stopped", "idle", "idle", "Idle", Glyph::Stopped),
        ("error", "idle", "idle", "Idle", Glyph::Error),
        ("error", "busy", "working", "Working", Glyph::Active),
    ];
    for (lifecycle, activity, word, label, glyph) in table {
        let m = model_of(&row(lifecycle, activity), &Listing::default(), None);
        assert_eq!((m.indicator.word(), m.status_label.as_str(), m.glyph), (word, label, glyph), "{lifecycle} / {activity}");
    }
}

#[test]
fn a_sent_prompt_not_yet_echoed_counts_as_busy_and_a_retrying_row_keeps_retry_for_the_header() {
    assert_eq!(effective_activity("idle", true), "busy");
    assert_eq!(effective_activity("retry", false), "busy");
    assert_eq!(header_activity("retry", "busy"), "retry");
    assert_eq!(header_activity("busy", "busy"), "busy");
    assert_eq!(effective_activity("nonsense", false), "idle");
    assert_eq!(effective_lifecycle(""), "running");
    assert_eq!(effective_lifecycle("complete"), "completed");
}

// ─── SessionDetailHeader for a session on a machine that isn't answering ────

#[test]
fn session_detail_header_for_a_session_on_a_machine_that_isnt_answering_says_so_instead_of_working() {
    let chip = MachineChip { name: "mini".into(), answering: false };
    let m = model_of(&row("active", "busy"), &Listing::default(), Some(chip));
    assert_eq!((m.indicator.word(), m.status_label.as_str()), ("not-answering", "Not answering"));
    let machine = m.machine.expect("a chip");
    assert_eq!((machine.label().as_str(), machine.tooltip().as_str()), ("mini \u{b7} not answering", "mini isn't answering"));
}

#[test]
fn an_answering_machine_leaves_the_status_alone_and_an_idle_session_on_a_silent_one_is_idle() {
    let on = MachineChip { name: "mini".into(), answering: true };
    assert_eq!(model_of(&row("active", "busy"), &Listing::default(), Some(on.clone())).indicator, Indicator::Working);
    assert_eq!((on.label().as_str(), on.tooltip().as_str()), ("mini", "Runs on mini"));
    let off = MachineChip { name: "mini".into(), answering: false };
    assert_eq!(model_of(&row("active", "idle"), &Listing::default(), Some(off)).indicator, Indicator::Idle);
}

// ─── session-lineage.test.ts: lineageOf and moving out (the code is 6A's; confirmed here through the header) ─

fn listing(forked: Option<&str>, spawned: Option<&str>, kind: Option<&str>, parent: Option<&str>, detached: bool) -> Listing {
    Listing {
        forked_from: forked.map(str::to_string),
        spawned_by: spawned.map(str::to_string),
        spawn_kind: kind.map(str::to_string),
        parent: parent.map(str::to_string),
        detached,
        ..Listing::default()
    }
}

#[test]
fn lineage_of_reads_a_fork_a_started_session_and_a_subagents_hidden_session() {
    let of = |l: &Listing| lineage_of(l);
    assert_eq!(of(&listing(Some("a"), None, Some("fork"), None, false)), Some(("a".into(), LineageKind::Fork)));
    assert_eq!(of(&listing(None, Some("b"), Some("api"), None, false)), Some(("b".into(), LineageKind::Started)));
    assert_eq!(of(&listing(None, None, None, Some("c"), false)), Some(("c".into(), LineageKind::Subagent)));
    assert_eq!(of(&Listing::default()), None);
}

#[test]
fn lineage_of_lets_spawn_kind_decide_when_a_session_is_both_a_fork_and_started_by_an_agent() {
    assert_eq!(lineage_of(&listing(Some("a"), Some("b"), Some("fork"), None, false)).map(|l| l.0).as_deref(), Some("a"));
    assert_eq!(lineage_of(&listing(Some("a"), Some("b"), Some("api"), None, false)).map(|l| l.0).as_deref(), Some("b"));
}

#[test]
fn moving_a_session_out_gives_it_no_parent_though_it_keeps_where_it_came_from() {
    let fork = listing(Some("a"), None, Some("fork"), None, true);
    assert_eq!(lineage_of(&fork), None);
    assert_eq!(lineage_origin_of(&fork), Some(("a".into(), LineageKind::Fork)));
    assert_eq!(model_of(&row("idle", "idle"), &fork, None).lineage, None);
}

#[test]
fn moving_a_session_out_never_moves_a_subagents_session() {
    // A subagent's session stays its parent's, whatever the mark says.
    let sub = listing(None, None, None, Some("c"), true);
    assert_eq!(lineage_of(&sub), Some(("c".into(), LineageKind::Subagent)));
    assert_eq!(model_of(&row("idle", "idle"), &sub, None).lineage, Some(("c".into(), LineageKind::Subagent)));
}

// ─── the route's capability fallbacks ───────────────────────────────────────

fn caps() -> Capabilities {
    Capabilities::default()
}

#[test]
fn capabilities_fall_back_when_the_row_has_none() {
    let a = Actions::resolve(&caps(), false, "running", "busy");
    assert_eq!((a.can_abort, a.can_archive, a.can_fork, a.can_delete, a.can_restore), (true, true, true, true, false));
    assert_eq!(a.fork_disabled_reason, None);
    // Abort needs a running, active session.
    assert!(!Actions::resolve(&caps(), false, "running", "idle").can_abort);
    assert!(!Actions::resolve(&caps(), false, "stopped", "busy").can_abort);
}

#[test]
fn capabilities_the_rows_own_answer_wins_over_the_fallback() {
    let c = Capabilities { can_abort: Some(false), can_archive: Some(false), can_delete: Some(false), ..Capabilities::default() };
    let a = Actions::resolve(&c, false, "running", "busy");
    assert_eq!((a.can_abort, a.can_archive, a.can_delete), (false, false, false));
    let c = Capabilities { can_abort: Some(true), ..Capabilities::default() };
    assert!(Actions::resolve(&c, false, "completed", "idle").can_abort);
}

#[test]
fn capabilities_fork_says_why_it_cant_and_falls_back_to_a_plain_sentence() {
    let c = Capabilities { can_fork: Some(false), fork_disabled_reason: Some("Copilot can't copy a conversation.".into()), ..caps() };
    let a = Actions::resolve(&c, false, "running", "idle");
    assert_eq!((a.can_fork, a.fork_disabled_reason.as_deref()), (false, Some("Copilot can't copy a conversation.")));
    assert!(a.show_fork(false));
    let a = Actions::resolve(&Capabilities { can_fork: Some(false), ..caps() }, false, "running", "idle");
    assert_eq!(a.fork_disabled_reason.as_deref(), Some("This session can't be forked."));
}

#[test]
fn capabilities_an_archived_session_has_no_fork_and_can_only_be_restored() {
    let c = Capabilities { can_fork: Some(false), can_archive: Some(true), ..caps() };
    let a = Actions::resolve(&c, true, "running", "idle");
    assert_eq!((a.can_archive, a.can_restore, a.fork_disabled_reason.clone()), (false, true, None));
    assert!(!a.show_fork(true));
}

// ─── the rest of the header's words ─────────────────────────────────────────

#[test]
fn the_directory_is_cut_at_its_last_separator() {
    assert_eq!(directory_parts("/home/demo/work/app"), ("/home/demo/work".into(), "/app".into()));
    assert_eq!(directory_parts("C:\\work\\app"), ("C:\\work".into(), "\\app".into()));
    assert_eq!(directory_parts("/app"), (String::new(), "/app".into()));
    assert_eq!(directory_parts("app"), (String::new(), "app".into()));
    assert_eq!(directory_parts(""), (String::new(), String::new()));
}

#[test]
fn the_removed_checkout_sentences() {
    let sentence = checkout_removed_sentence(Some("2026-10-05T08:00:00Z"), "fix/spinner", NOW, 0);
    assert_eq!(
        sentence,
        "Fleet removed this session's checkout on Oct 5 to save space. The branch fix/spinner and this conversation are kept; your next message puts it back."
    );
    assert_eq!(
        checkout_removed_sentence(None, "", NOW, 0),
        "Fleet removed this session's checkout to save space. The branch and this conversation are kept; your next message puts it back."
    );
    let removed = Row { checkout_removed_at: Some("2026-10-05T08:00:00Z".into()), ..Row::default() };
    assert_eq!(checkout_state(&removed, false, false), Some(Checkout::Removed));
    assert_eq!(checkout_state(&removed, true, false), Some(Checkout::Restoring));
    assert_eq!(checkout_state(&Row { checkout_restoring: true, ..Row::default() }, false, false), Some(Checkout::Restoring));
    assert_eq!(checkout_state(&Row::default(), false, true), Some(Checkout::Restored));
    assert_eq!(checkout_state(&Row::default(), false, false), None);
    assert_eq!(Checkout::Removed.text(), "Checkout removed");
    assert_eq!(Checkout::Restoring.text(), "Putting the checkout back\u{2026}");
}

#[test]
fn usage_strings_use_the_formats_and_dash_for_what_the_server_lacks() {
    assert_eq!(usage_strings(Some(12_500), Some(1.85)), ("12,500".into(), "$1.85".into()));
    assert_eq!(usage_strings(Some(0), Some(0.001)), ("0".into(), "<$0.01".into()));
    assert_eq!(usage_strings(None, None), ("\u{2014}".into(), "\u{2014}".into()));
}

#[test]
fn the_model_reads_the_detail_of_a_session_the_list_does_not_hold() {
    let body = json!({
        "id": "s1", "title": "  ", "projectName": "Demo", "branch": "feat/x", "workspaceDirectory": "/work/demo/app",
        "lifecycleStatus": "disconnected", "activityStatus": "busy", "retentionStatus": "archived", "harnessType": "opencode",
        "harnessProfileName": "Careful", "totalTokens": 1200, "totalCost": 0.42, "tags": ["bug", "ui"],
        "capabilities": {"canFork": false, "forkDisabledReason": "Not here."},
        "origin": {"sourceType": "pull_request", "providerId": "github", "title": "A change"},
        "selectedModel": {"providerID": "p", "modelID": "m-1"},
        "forkedFromSessionId": "parent", "spawnKind": "fork",
    });
    let d = Detail::from_wire(&body).expect("a detail");
    assert_eq!(
        (d.row.lifecycle.as_str(), d.branch.as_str(), d.row.tags.clone()),
        ("disconnected", "feat/x", vec!["bug".to_string(), "ui".into()])
    );
    assert_eq!(d.row.origin.as_ref().map(|o| o.provider_id.as_str()), Some("github"));
    let m = HeaderModel::build(&Inputs {
        title: &d.title,
        project_name: d.project_name.as_deref(),
        row: &d.row,
        listing: &d.listing,
        branch: &d.branch,
        prompt_in_flight: false,
        machine: None,
        harnesses: &[],
        models: &[],
        now: NOW,
        offset_secs: 0,
        restoring_here: false,
        restored_note: false,
    });
    assert_eq!(m.title, "Untitled session");
    assert_eq!((m.indicator, m.glyph), (Indicator::Disconnected, Glyph::Disconnected));
    assert_eq!(
        (m.branch.as_deref(), m.profile.as_deref(), m.harness_label.as_deref()),
        (Some("feat/x"), Some("Careful"), Some("opencode"))
    );
    assert_eq!((m.directory_head.as_str(), m.directory_leaf.as_str()), ("/work/demo", "/app"));
    assert_eq!((m.tokens_text.as_str(), m.cost_text.as_str()), ("1,200", "$0.42"));
    assert_eq!(m.model_label.as_deref(), Some("m-1"));
    assert!(m.archived && !m.actions.can_archive && m.actions.can_restore);
    assert_eq!(m.lineage, Some(("parent".into(), LineageKind::Fork)));
}

// (The request for the open session's record is checked against a scratch Fleet: the fake servers here have no
// `/api/sessions/{id}`.)

#[test]
fn rows_read_tags_directory_usage_origin_capabilities_and_profile_from_the_list_and_the_detail() {
    let item = json!({
        "workspaceDirectory": "/work/demo", "tags": ["bug", 3, "ui"], "totalTokens": 900, "totalCost": 0.02,
        "capabilities": {"canAbort": true, "canPrompt": false, "forkDisabledReason": ""},
        "origin": {"sourceType": "issue", "providerId": "github", "resourceUrl": "https://example.test/x/1"},
        "harnessProfileName": "Careful",
    });
    let r = Row::from_list_item(&item);
    assert_eq!((r.directory.as_str(), r.tags.clone()), ("/work/demo", vec!["bug".to_string(), "ui".into()]));
    assert_eq!((r.total_tokens, r.total_cost), (Some(900), Some(0.02)));
    assert_eq!((r.capabilities.can_abort, r.capabilities.can_fork, r.capabilities.fork_disabled_reason.clone()), (Some(true), None, None));
    assert_eq!(r.origin.as_ref().and_then(|o| o.resource_url.as_deref()), Some("https://example.test/x/1"));
    assert_eq!(r.harness_profile_name.as_deref(), Some("Careful"));
    let none = Row::from_list_item(&json!({}));
    assert_eq!((none.total_tokens, none.total_cost, none.origin, none.harness_profile_name), (None, None, None, None));
}
