//! Delegations and the derived status (`delegations.rs`): the cases of weave-fleet's
//! `domain-event-reducer.test.ts` about sub-agents, and `reopened-session.test.ts` where they
//! need no tool card, under the same names.
use super::super::delegations::{Delegation, DelegationEvent, DelegationStatus, apply_created, apply_updated, is_waiting, upsert};
use super::super::events::Kind;
use super::super::model::{Explicit, StreamState, StreamStatus};
use super::super::reducer::{Event, apply, create_state, with_activity_status};
use super::super::sessions::stream_status_of;
use super::{new_state, snapshot};
use crate::mock::Status;
use serde_json::{Value, json};

// ─── builders, as the Vue tests have them ───────────────────────────────────

fn delegation() -> Delegation {
    Delegation {
        delegation_id: "delegation-1".into(),
        parent_tool_call_id: Some("tool-1".into()),
        child_session_id: Some("child-1".into()),
        title: "Delegate work".into(),
        status: DelegationStatus::Running,
        created_at: Some("2026-01-01T00:00:00Z".into()),
        child_activity_status: None,
        background: false,
    }
}

fn with(mut d: Delegation, f: impl FnOnce(&mut Delegation)) -> Delegation {
    f(&mut d);
    d
}

fn state_of(delegations: Vec<Delegation>, explicit: Explicit, status: StreamStatus) -> StreamState {
    StreamState { delegations, explicit_status: explicit, session_status: status, ..new_state() }
}

fn state(delegations: Vec<Delegation>) -> StreamState {
    state_of(delegations, Explicit::Idle, StreamStatus::Idle)
}

fn snapshot_delegation(extra: Value) -> Value {
    let mut d = json!({
        "delegationId": "delegation-1", "parentToolCallId": "tool-1", "childSessionId": "child-1",
        "title": "Delegate work", "status": "running", "createdAt": "2026-01-01T00:00:00Z",
    });
    d.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    d
}

fn snapshot_with(activity: &str, delegations: Vec<Value>) -> StreamState {
    let mut snap = snapshot(activity, vec![]);
    snap["delegations"] = json!(delegations);
    create_state(&snap)
}

fn event(kind: Kind, status: &str, extra: Value) -> Event {
    let mut props = json!({
        "delegationId": "delegation-1", "parentSessionId": "session-1", "parentToolCallId": "tool-1",
        "childSessionId": "child-1", "title": "Delegate work", "status": status, "createdAt": "2026-01-01T00:00:00Z",
    });
    props.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    Event::from_wire(&json!({"type": kind.wire(), "properties": props})).unwrap()
}

fn created() -> Event {
    event(Kind::DelegationCreated, "running", json!({}))
}

fn completed() -> Event {
    event(Kind::DelegationCompleted, "completed", json!({}))
}

fn completed_of(id: &str, tool: &str, child: &str) -> Event {
    event(Kind::DelegationCompleted, "completed", json!({"delegationId": id, "parentToolCallId": tool, "childSessionId": child}))
}

fn child_activity(session: &str, activity: &str) -> Event {
    Event::ActivityStatus { session_id: session.into(), activity_status: activity.into() }
}

fn run(mut s: StreamState, e: Event) -> StreamState {
    apply(&mut s, &e);
    s
}

fn pair(s: &StreamState) -> (Explicit, StreamStatus) {
    (s.explicit_status, s.session_status)
}

// ─── derived status ─────────────────────────────────────────────────────────

#[test]
fn derives_delegating_status_from_idle_snapshots_with_active_delegations() {
    let s = snapshot_with("idle", vec![snapshot_delegation(json!({}))]);
    assert_eq!(pair(&s), (Explicit::Idle, StreamStatus::Delegating));
}

#[test]
fn hydrates_as_delegating_from_busy_snapshots_with_active_delegations() {
    let s = snapshot_with("busy", vec![snapshot_delegation(json!({"status": "pending"}))]);
    assert_eq!(pair(&s), (Explicit::Busy, StreamStatus::Delegating));
}

#[test]
fn keeps_busy_status_while_turns_are_active_even_with_delegations() {
    let s = run(state_of(vec![delegation()], Explicit::Idle, StreamStatus::Delegating), Event::TurnStarted);
    assert_eq!(pair(&s), (Explicit::Busy, StreamStatus::Busy));
}

#[test]
fn enters_delegating_when_an_active_delegation_is_created_while_explicitly_idle() {
    let s = run(state(vec![]), created());
    assert_eq!(pair(&s), (Explicit::Idle, StreamStatus::Delegating));
}

#[test]
fn keeps_a_session_non_idle_when_a_turn_ends_with_an_active_delegation() {
    let pending = with(delegation(), |d| d.status = DelegationStatus::Pending);
    let s = run(state_of(vec![pending], Explicit::Busy, StreamStatus::Busy), Event::TurnEnded);
    assert_eq!(pair(&s), (Explicit::Idle, StreamStatus::Delegating));
}

#[test]
fn stays_delegating_after_explicit_idle_until_the_final_active_delegation_completes() {
    let idled = run(state_of(vec![delegation()], Explicit::Busy, StreamStatus::Busy), Event::SessionIdled);
    assert_eq!(pair(&idled), (Explicit::Idle, StreamStatus::Delegating));
    let done = run(idled, completed());
    assert_eq!(pair(&done), (Explicit::Idle, StreamStatus::Idle));
}

#[test]
fn returns_to_idle_when_the_final_active_delegation_completes() {
    let s = run(state_of(vec![delegation()], Explicit::Idle, StreamStatus::Delegating), completed());
    assert_eq!(pair(&s), (Explicit::Idle, StreamStatus::Idle));
}

#[test]
fn stays_busy_when_the_final_active_delegation_completes_before_explicit_idle() {
    let s = run(state_of(vec![delegation()], Explicit::Busy, StreamStatus::Busy), completed());
    assert_eq!(pair(&s), (Explicit::Busy, StreamStatus::Busy));
}

#[test]
fn does_not_return_to_idle_until_all_active_delegations_are_terminal() {
    let numbered = |n: u8, status| {
        with(delegation(), |d| {
            d.delegation_id = format!("delegation-{n}");
            d.parent_tool_call_id = Some(format!("tool-{n}"));
            d.child_session_id = Some(format!("child-{n}"));
            d.status = status;
        })
    };
    let s = state_of(
        vec![numbered(1, DelegationStatus::Running), numbered(2, DelegationStatus::Pending), numbered(3, DelegationStatus::Completed)],
        Explicit::Idle,
        StreamStatus::Delegating,
    );
    let after_first = run(s, completed_of("delegation-1", "tool-1", "child-1"));
    assert_eq!(after_first.session_status, StreamStatus::Delegating);
    let after_second = run(after_first, completed_of("delegation-2", "tool-2", "child-2"));
    assert_eq!(pair(&after_second), (Explicit::Idle, StreamStatus::Idle));
}

#[test]
fn treats_error_and_cancelled_delegations_as_terminal_states() {
    for status in ["error", "cancelled"] {
        let s = state_of(vec![delegation()], Explicit::Idle, StreamStatus::Delegating);
        let after = run(s, event(Kind::DelegationCompleted, status, json!({})));
        assert_eq!(after.session_status, StreamStatus::Idle, "{status}");
    }
}

#[test]
fn initializes_as_delegating_from_an_idle_snapshot_when_any_delegation_is_active() {
    let s = snapshot_with(
        "idle",
        vec![
            snapshot_delegation(json!({"status": "completed"})),
            snapshot_delegation(json!({"delegationId": "delegation-2", "parentToolCallId": "tool-2", "childSessionId": "child-2"})),
        ],
    );
    assert_eq!(pair(&s), (Explicit::Idle, StreamStatus::Delegating));
}

#[test]
fn does_not_oscillate_to_idle_when_the_child_idles_before_parent_delegation_completion() {
    let with_delegation = run(state_of(vec![], Explicit::Busy, StreamStatus::Busy), created());
    assert_eq!(with_delegation.session_status, StreamStatus::Busy);
    // The child's own `session.idled` is on its topic; the parent's stream hears the `sessions` one.
    let after_child_idle = run(with_delegation, Event::SessionIdled);
    assert_eq!(pair(&after_child_idle), (Explicit::Idle, StreamStatus::Delegating));
    assert_eq!(after_child_idle.delegations[0].status, DelegationStatus::Running);
    let after_turn = run(after_child_idle, Event::TurnEnded);
    assert_eq!(pair(&after_turn), (Explicit::Idle, StreamStatus::Delegating));
    let after_done = run(after_turn, completed());
    assert_eq!(pair(&after_done), (Explicit::Idle, StreamStatus::Idle));
}

#[test]
fn stays_delegating_when_turn_ended_arrives_with_active_delegations() {
    let s = run(
        state_of(vec![with(delegation(), |d| d.status = DelegationStatus::Running)], Explicit::Busy, StreamStatus::Busy),
        Event::TurnEnded,
    );
    assert_eq!(pair(&s), (Explicit::Idle, StreamStatus::Delegating));
}

// ─── a sub-agent waiting on a question ──────────────────────────────────────

#[test]
fn makes_the_parent_wait_on_input_over_its_own_busy_turn() {
    let busy = run(state(vec![delegation()]), Event::TurnStarted);
    let waiting = run(busy, child_activity("child-1", "waiting_input"));
    assert_eq!(waiting.session_status, StreamStatus::WaitingInput);
    assert_eq!(waiting.delegations[0].child_activity_status.as_deref(), Some("waiting_input"));
    assert!(is_waiting(&waiting.delegations[0]));
    assert!(waiting.session_status.working());
}

#[test]
fn goes_back_to_working_once_answered_then_idle_as_before() {
    let busy = run(state(vec![delegation()]), Event::TurnStarted);
    let waiting = run(busy, child_activity("child-1", "waiting_input"));
    let answered = run(waiting, child_activity("child-1", "busy"));
    assert_eq!(answered.session_status, StreamStatus::Busy);
    assert!(!is_waiting(&answered.delegations[0]));
    let child_done = run(answered, completed());
    let idled = run(child_done, Event::SessionIdled);
    assert_eq!(idled.session_status, StreamStatus::Idle);
}

#[test]
fn keeps_the_childs_status_when_its_delegation_is_updated() {
    let waiting = run(state(vec![delegation()]), child_activity("child-1", "waiting_input"));
    let updated = run(waiting, event(Kind::DelegationUpdated, "running", json!({})));
    assert_eq!(updated.session_status, StreamStatus::WaitingInput);
}

#[test]
fn leaves_the_state_alone_for_a_session_that_isnt_one_of_its_sub_agents() {
    let s = state_of(vec![delegation()], Explicit::Idle, StreamStatus::Delegating);
    for who in ["someone-else", "session-1"] {
        assert_eq!(run(s.clone(), child_activity(who, "waiting_input")), s);
    }
}

#[test]
fn stops_counting_once_the_delegation_has_finished() {
    let finished = with(delegation(), |d| {
        d.status = DelegationStatus::Completed;
        d.child_activity_status = Some("waiting_input".into());
    });
    assert!(!is_waiting(&finished));
    let s = state(vec![finished]);
    assert_eq!(run(s, child_activity("child-1", "waiting_input")).session_status, StreamStatus::Idle);
}

#[test]
fn hydrates_from_a_snapshot_taken_while_the_child_waits() {
    let s = snapshot_with("waiting_input", vec![snapshot_delegation(json!({"childActivityStatus": "waiting_input"}))]);
    assert_eq!(s.session_status, StreamStatus::WaitingInput);
    assert_eq!(s.delegations[0].child_activity_status.as_deref(), Some("waiting_input"));
}

#[test]
fn hydrates_as_delegating_when_the_child_is_only_working() {
    let s = snapshot_with("busy", vec![snapshot_delegation(json!({"childActivityStatus": "busy"}))]);
    assert_eq!(s.session_status, StreamStatus::Delegating);
}

// ─── a sub-agent in the background ──────────────────────────────────────────

fn background_update(background: Option<bool>) -> Event {
    let extra = background.map_or(json!({}), |b| json!({"background": b}));
    event(Kind::DelegationUpdated, "running", extra)
}

#[test]
fn reads_idle_once_its_turn_is_over_while_the_sub_agent_works() {
    let delegating = run(state(vec![delegation()]), Event::SessionIdled);
    assert_eq!(delegating.session_status, StreamStatus::Delegating);
    let background = run(delegating, background_update(Some(true)));
    assert!(background.delegations[0].background);
    assert_eq!(background.session_status, StreamStatus::Idle);
    assert!(!background.session_status.working());
}

#[test]
fn stays_in_the_background_when_a_later_update_doesnt_say_so() {
    let s = run(state(vec![with(delegation(), |d| d.background = true)]), background_update(None));
    assert!(s.delegations[0].background);
    assert_eq!(s.session_status, StreamStatus::Idle);
}

#[test]
fn still_works_through_its_own_turn() {
    let s = run(state(vec![with(delegation(), |d| d.background = true)]), Event::TurnStarted);
    assert_eq!(s.session_status, StreamStatus::Busy);
}

#[test]
fn waits_on_input_when_the_sub_agent_asks() {
    let s = state(vec![with(delegation(), |d| d.background = true)]);
    let waiting = run(s, child_activity("child-1", "waiting_input"));
    assert_eq!(waiting.session_status, StreamStatus::WaitingInput);
    assert!(is_waiting(&waiting.delegations[0]));
    assert_eq!(run(waiting, child_activity("child-1", "busy")).session_status, StreamStatus::Idle);
}

#[test]
fn counts_a_foreground_sub_agent_beside_it_as_work() {
    let second = with(delegation(), |d| {
        d.delegation_id = "delegation-2".into();
        d.parent_tool_call_id = Some("tool-2".into());
        d.child_session_id = Some("child-2".into());
    });
    let s = state(vec![with(delegation(), |d| d.background = true), second]);
    assert_eq!(run(s, Event::SessionIdled).session_status, StreamStatus::Delegating);
}

#[test]
fn hydrates_as_idle_from_a_snapshot_taken_while_the_sub_agent_works() {
    let s = snapshot_with("idle", vec![snapshot_delegation(json!({"childActivityStatus": "busy", "background": true}))]);
    assert_eq!(s.session_status, StreamStatus::Idle);
    assert!(s.delegations[0].background);
}

#[test]
fn hydrates_as_waiting_on_input_from_a_snapshot_taken_while_the_sub_agent_asks() {
    let s = snapshot_with("waiting_input", vec![snapshot_delegation(json!({"childActivityStatus": "waiting_input", "background": true}))]);
    assert_eq!(s.session_status, StreamStatus::WaitingInput);
}

// ─── reopened-session.test.ts, the cases that need no tool card ─────────────

#[test]
fn links_the_sub_agent_card_to_its_delegation() {
    let mut snap = snapshot("busy", vec![]);
    snap["messages"] = json!([{"info": {"id": "msg_1", "role": "assistant", "sessionID": "fleet-1", "time": {"created": 1000}},
        "parts": [{"type": "tool", "id": "prt_task", "tool": "task", "callID": "toolu_task", "state": {"status": "completed"}}]}]);
    snap["delegations"] = json!([{"delegationId": "del-1", "parentToolCallId": "toolu_task", "childSessionId": "fleet-child",
        "title": "thread", "status": "completed", "createdAt": "2026-09-13T00:00:00Z"}]);
    let s = create_state(&snap);
    let call = s.messages[0].parts.iter().find_map(|p| match p {
        super::super::model::PartData::Tool { tool, call_id, .. } if tool == "task" => Some(call_id.clone()),
        _ => None,
    });
    assert_eq!(s.delegations[0].parent_tool_call_id, call);
}

// ─── delegation-state.ts: the rules the reducer tests reach only in part ────

fn ev(status: DelegationStatus) -> DelegationEvent {
    DelegationEvent {
        delegation_id: "delegation-1".into(),
        parent_tool_call_id: Some("tool-1".into()),
        child_session_id: Some("child-1".into()),
        title: Some("Delegate work".into()),
        status,
        created_at: Some("2026-01-01T00:00:00Z".into()),
        background: false,
    }
}

#[test]
fn an_update_for_an_unknown_delegation_creates_it() {
    let mut list = vec![];
    upsert(&mut list, &ev(DelegationStatus::Completed));
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].status, DelegationStatus::Completed);
    // `apply_updated` alone does nothing for an id it doesn't know.
    assert!(!apply_updated(&mut [], &ev(DelegationStatus::Running)));
}

#[test]
fn an_update_keeps_what_it_doesnt_carry_and_says_whether_anything_changed() {
    let mut list = vec![with(delegation(), |d| d.child_activity_status = Some("busy".into()))];
    let sparse = DelegationEvent {
        title: None,
        parent_tool_call_id: None,
        child_session_id: None,
        created_at: None,
        ..ev(DelegationStatus::Running)
    };
    assert!(!apply_updated(&mut list, &sparse), "the same values: not a change");
    assert_eq!(list[0].title, "Delegate work");
    assert_eq!(list[0].child_activity_status.as_deref(), Some("busy"));
    assert!(apply_updated(&mut list, &ev(DelegationStatus::Completed)));
}

#[test]
fn a_created_event_merges_over_the_same_id_and_the_background_flag_is_sticky() {
    let mut list = vec![with(delegation(), |d| {
        d.background = true;
        d.child_activity_status = Some("waiting_input".into());
    })];
    apply_created(&mut list, &DelegationEvent { title: Some("Renamed".into()), ..ev(DelegationStatus::Running) });
    assert_eq!(list.len(), 1);
    assert_eq!((list[0].title.as_str(), list[0].background), ("Renamed", true));
    assert_eq!(list[0].child_activity_status.as_deref(), Some("waiting_input"));
}

#[test]
fn an_unknown_status_reads_as_pending() {
    assert_eq!(DelegationStatus::from_wire(Some("starting")), DelegationStatus::Pending);
    assert_eq!(DelegationStatus::from_wire(None), DelegationStatus::Pending);
}

#[test]
fn a_delegation_event_without_an_id_is_skipped() {
    let data = json!({"type": "delegation.created", "properties": {"title": "x"}});
    assert_eq!(Event::from_wire(&data), None);
}

#[test]
fn with_activity_status_takes_the_lists_status_into_a_kept_state() {
    let mut s = state(vec![delegation()]);
    with_activity_status(&mut s, "busy");
    assert_eq!(pair(&s), (Explicit::Busy, StreamStatus::Busy));
    with_activity_status(&mut s, "idle");
    assert_eq!(pair(&s), (Explicit::Idle, StreamStatus::Delegating));
}

// ─── status → the list's glyph ──────────────────────────────────────────────

#[test]
fn every_derived_status_reads_as_a_glyph_in_the_session_list() {
    let glyph = |s| stream_status_of(s);
    assert_eq!(glyph(StreamStatus::Idle), Status::Idle);
    for working in [StreamStatus::Busy, StreamStatus::Retry, StreamStatus::Delegating] {
        assert_eq!(glyph(working), Status::Working);
    }
    assert_eq!(glyph(StreamStatus::WaitingInput), Status::NeedsInput);
}
