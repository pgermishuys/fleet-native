//! The reducer's contract: the cases of weave-fleet's `domain-event-reducer.test.ts`,
//! `message-ordering.test.ts` and `reopened-session.test.ts` that this path touches, under the same
//! names, and the captured hub frames of a real server (`tests/fixtures/`, invented content).
//! Delegation, running-work, context and optimistic-prompt cases belong to the reducer lane.
use super::model::{Explicit, PartData, StreamState, StreamStatus};
use super::reducer::{Event, Lifecycle, apply, create_state};
use super::{sessions, signalr, view};
use crate::mock::Status;
use crate::view::conversation::{Part, Role, ToolState};
use serde_json::{Value, json};

// ─── builders, as the Vue tests have them ───────────────────────────────────

fn life(id: &str, role: &str, created: i64, text: Option<&str>, part_id: Option<&str>) -> Lifecycle {
    let parts = text.map(|t| vec![json!({"type": "text", "id": part_id.unwrap_or("text-1"), "text": t})]);
    Lifecycle { info: json!({"id": id, "role": role, "sessionID": "session-1", "time": {"created": created}}), parts }
}

fn with_completed(mut l: Lifecycle, completed: i64) -> Lifecycle {
    l.info["time"]["completed"] = json!(completed);
    l
}

pub(super) fn snapshot(activity: &str, messages: Vec<Lifecycle>) -> Value {
    let messages: Vec<Value> = messages.into_iter().map(|l| json!({"info": l.info, "parts": l.parts})).collect();
    json!({"session": {}, "messages": messages, "delegations": [], "activityStatus": activity, "lastEventId": null})
}

pub(crate) fn new_state() -> StreamState {
    create_state(&snapshot("idle", vec![]))
}

fn delta(message: &str, part: &str, text: &str, offset: Option<i64>) -> Event {
    Event::TextDelta { session_id: "session-1".into(), message_id: message.into(), part_id: part.into(), delta: text.into(), offset }
}

fn part_updated(message: &str, part: &str, text: &str) -> Event {
    Event::PartUpdated(json!({"id": part, "sessionID": "session-1", "messageID": message, "type": "text", "text": text}))
}

fn apply_all(mut state: StreamState, events: Vec<Event>) -> StreamState {
    for e in &events {
        apply(&mut state, e);
    }
    state
}

fn text_of(state: &StreamState, message: &str) -> Vec<String> {
    let msg = state.messages.iter().find(|m| m.id == message).expect("no such message");
    msg.parts.iter().filter_map(|p| if let PartData::Text { text, .. } = p { Some(text.clone()) } else { None }).collect()
}

fn ids(state: &StreamState) -> Vec<&str> {
    state.messages.iter().map(|m| m.id.as_str()).collect()
}

// ─── message ordering and lifecycle ─────────────────────────────────────────

#[test]
fn keeps_user_prompt_before_assistant_response_when_lifecycle_events_have_equal_timestamps() {
    let user = life("user-message-1", "user", 1000, Some("Explain why stream ordering matters."), None);
    let mut state = create_state(&snapshot("idle", vec![user]));
    apply(&mut state, &Event::MessageCreated(life("assistant-message-1", "assistant", 1000, None, None)));
    apply(&mut state, &delta("assistant-message-1", "assistant-text-1", "The assistant response is now partial.", None));
    let done = with_completed(
        life("assistant-message-1", "assistant", 1000, Some("The assistant response is now complete."), Some("assistant-text-1")),
        2000,
    );
    apply(&mut state, &Event::MessageUpdated(done));

    let roles: Vec<Role> = state.messages.iter().map(|m| m.role).collect();
    assert_eq!(roles, [Role::User, Role::Assistant]);
    assert_eq!(text_of(&state, "assistant-message-1"), ["The assistant response is now complete."]);
}

#[test]
fn does_not_duplicate_assistant_message_when_part_update_arrives_before_lifecycle_events() {
    let user = life("user-message-1", "user", 1000, Some("Explain why out-of-order events must merge."), None);
    let mut state = create_state(&snapshot("idle", vec![user]));
    let longer = "Partial response from the part update with extra streamed text.";
    apply(&mut state, &part_updated("assistant-message-1", "assistant-text-1", longer));
    apply(&mut state, &Event::MessageCreated(life("assistant-message-1", "assistant", 1000, Some(longer), Some("assistant-text-1"))));
    let shorter = life("assistant-message-1", "assistant", 1000, Some("Partial response from the part update."), Some("assistant-text-1"));
    apply(&mut state, &Event::MessageUpdated(with_completed(shorter, 2000)));

    assert_eq!(ids(&state), ["user-message-1", "assistant-message-1"]);
    let assistant = &state.messages[1];
    assert_eq!((assistant.role, assistant.created_at, assistant.completed_at), (Role::Assistant, Some(1000), Some(2000)));
    assert_eq!(text_of(&state, "assistant-message-1"), [longer]);
}

#[test]
fn preserves_longer_streamed_text_when_shorter_final_lifecycle_snapshot_arrives() {
    let streamed = "Streamed response text with additional delta words.";
    let mut state = new_state();
    apply(&mut state, &delta("assistant-message-1", "assistant-text-1", streamed, None));
    apply(&mut state, &Event::MessageCreated(life("assistant-message-1", "assistant", 1000, Some(streamed), Some("assistant-text-1"))));
    let done =
        with_completed(life("assistant-message-1", "assistant", 1000, Some("Streamed response text."), Some("assistant-text-1")), 2000);
    apply(&mut state, &Event::MessageUpdated(done));

    assert_eq!(ids(&state), ["assistant-message-1"]);
    assert_eq!(state.messages[0].created_at, Some(1000));
    assert_eq!(state.messages[0].completed_at, Some(2000));
    assert_eq!(text_of(&state, "assistant-message-1"), [streamed]);
}

#[test]
fn keeps_the_snapshot_when_a_pending_message_update_has_no_parts() {
    let user = life("user-message-1", "user", 1000, Some("Carry on in the worktree"), Some("user-text-1"));
    let assistant = life("assistant-message-1", "assistant", 1100, Some("On it."), Some("assistant-text-1"));
    let before = create_state(&snapshot("idle", vec![user.clone(), assistant.clone()]));
    // The harness's own message.updated: info only, no parts.
    let mut info = assistant.info.clone();
    info["time"] = json!({"created": 1100, "completed": 1500});
    let state = apply_all(before.clone(), vec![Event::MessageUpdated(Lifecycle { info, parts: None })]);

    let roles: Vec<Role> = state.messages.iter().map(|m| m.role).collect();
    assert_eq!(roles, [Role::User, Role::Assistant]);
    assert_eq!(state.messages[1].parts, before.messages[1].parts);
    assert_eq!(state.messages[1].completed_at, Some(1500));
}

#[test]
fn orders_messages_by_id_wherever_they_arrive() {
    let mut state = new_state();
    for id in ["msg_c", "msg_a", "msg_b"] {
        apply(&mut state, &Event::MessageCreated(life(id, "user", 1, None, None)));
    }
    assert_eq!(ids(&state), ["msg_a", "msg_b", "msg_c"]);
}

#[test]
fn orders_a_forks_counted_copies_by_their_counter() {
    let mut state = new_state();
    for id in ["msg_x_17", "msg_x_6", "msg_x_9"] {
        apply(&mut state, &Event::MessageCreated(life(id, "user", 1, None, None)));
    }
    assert_eq!(ids(&state), ["msg_x_6", "msg_x_9", "msg_x_17"]);
}

// ─── status ─────────────────────────────────────────────────────────────────

#[test]
fn hydrates_a_retrying_snapshot_as_working() {
    let state = create_state(&snapshot("retry", vec![]));
    assert_eq!(state.session_status, StreamStatus::Retry);
    assert!(state.session_status.working());
}

#[test]
fn shows_work_for_every_status_but_idle() {
    for status in [StreamStatus::Busy, StreamStatus::Delegating, StreamStatus::Retry, StreamStatus::WaitingInput] {
        assert!(status.working());
    }
    assert!(!StreamStatus::Idle.working());
}

#[test]
fn becomes_idle_when_a_session_idles_with_no_delegations() {
    let mut state = create_state(&snapshot("busy", vec![]));
    apply(&mut state, &Event::SessionIdled);
    assert_eq!((state.explicit_status, state.session_status), (Explicit::Idle, StreamStatus::Idle));
}

#[test]
fn transitions_to_idle_when_turn_ended_arrives_without_a_preceding_session_idled_and_no_active_delegations() {
    let mut state = new_state();
    apply(&mut state, &Event::TurnStarted);
    assert_eq!((state.explicit_status, state.session_status), (Explicit::Busy, StreamStatus::Busy));
    apply(&mut state, &Event::TurnEnded);
    assert_eq!((state.explicit_status, state.session_status), (Explicit::Idle, StreamStatus::Idle));
}

// ─── deltas that overlap a snapshot ─────────────────────────────────────────

fn joined_with(text: &str) -> StreamState {
    create_state(&snapshot("busy", vec![life("msg_reply", "assistant", 1000, Some(text), Some("prt_reply"))]))
}

fn reply_text(state: &StreamState) -> String {
    text_of(state, "msg_reply").remove(0)
}

#[test]
fn skips_a_delta_the_snapshot_already_has() {
    let state = apply_all(joined_with("Checking the rate limit "), vec![delta("msg_reply", "prt_reply", "rate limit ", Some(13))]);
    assert_eq!(reply_text(&state), "Checking the rate limit ");
}

#[test]
fn adds_only_the_part_of_a_delta_the_snapshot_doesnt_have() {
    let state = apply_all(joined_with("Checking the rate"), vec![delta("msg_reply", "prt_reply", "rate limit ", Some(13))]);
    assert_eq!(reply_text(&state), "Checking the rate limit ");
}

#[test]
fn adds_a_delta_that_starts_where_the_snapshot_ends() {
    let state = apply_all(joined_with("Checking the "), vec![delta("msg_reply", "prt_reply", "rate limit ", Some(13))]);
    assert_eq!(reply_text(&state), "Checking the rate limit ");
}

#[test]
fn adds_a_delta_without_an_offset_whole_as_from_a_fleet_that_doesnt_send_one() {
    let state = apply_all(joined_with("Checking the "), vec![delta("msg_reply", "prt_reply", "rate limit ", None)]);
    assert_eq!(reply_text(&state), "Checking the rate limit ");
}

#[test]
fn streams_on_from_the_snapshot_with_no_text_twice() {
    let deltas = vec![
        delta("msg_reply", "prt_reply", "Checking the ", Some(0)),
        delta("msg_reply", "prt_reply", "rate limit ", Some(13)),
        delta("msg_reply", "prt_reply", "headers", Some(24)),
        delta("msg_reply", "prt_reply", ".", Some(31)),
    ];
    assert_eq!(reply_text(&apply_all(joined_with("Checking the rate"), deltas)), "Checking the rate limit headers.");
}

#[test]
fn counts_a_delta_offset_in_utf16_units_as_the_server_does() {
    // "é" is one unit and two bytes; "😀" is two units and four bytes.
    let state = apply_all(joined_with("é😀 and"), vec![delta("msg_reply", "prt_reply", " and more", Some(3))]);
    assert_eq!(reply_text(&state), "é😀 and more");
}

// ─── turn.failed ────────────────────────────────────────────────────────────

fn error() -> Value {
    json!({"name": "APIError", "message": "Overloaded", "isRetryable": false})
}

fn failed(message: Option<&str>) -> Event {
    Event::TurnFailed { session_id: "session-1".into(), message_id: message.map(str::to_string), error: error() }
}

#[test]
fn pins_the_failure_to_the_assistant_message_the_turn_produced() {
    let mut state = new_state();
    apply(&mut state, &Event::MessageCreated(life("message-1", "assistant", 1, Some("Half an ans"), None)));
    apply(&mut state, &failed(Some("message-1")));
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0].turn_error, Some(error()));
    assert_eq!(state.messages[0].parts.len(), 1);
}

#[test]
fn falls_back_to_the_last_assistant_message_when_the_failure_names_none() {
    let mut state = new_state();
    apply(&mut state, &Event::MessageCreated(life("message-1", "user", 1, Some("Go"), None)));
    apply(&mut state, &Event::MessageCreated(life("message-2", "assistant", 2, Some("On it"), None)));
    apply(&mut state, &failed(None));
    assert_eq!(state.messages[0].turn_error, None);
    assert_eq!(state.messages[1].turn_error, Some(error()));
}

#[test]
fn gives_the_failure_a_message_of_its_own_when_the_turn_produced_none() {
    let mut state = new_state();
    apply(&mut state, &Event::MessageCreated(life("message-1", "user", 1, Some("Go"), None)));
    apply(&mut state, &failed(None));
    assert_eq!(state.messages.len(), 2);
    assert_eq!((state.messages[1].role, state.messages[1].session_id.as_str()), (Role::Assistant, "session-1"));
    assert_eq!(state.messages[1].turn_error, Some(error()));
}

#[test]
fn gives_a_failure_before_the_reply_its_own_message_after_the_prompt_even_when_it_names_an_earlier_reply() {
    let mut state = new_state();
    for (id, role, at) in [("message-1", "user", 1), ("message-2", "assistant", 2), ("message-3", "user", 3)] {
        apply(&mut state, &Event::MessageCreated(life(id, role, at, Some("x"), None)));
    }
    apply(&mut state, &failed(Some("message-2")));
    assert_eq!(state.messages[1].turn_error, None);
    assert_eq!(state.messages.len(), 4);
    assert_eq!((state.messages[3].role, state.messages[3].turn_error.clone()), (Role::Assistant, Some(error())));
}

#[test]
fn stops_the_session_looking_busy() {
    let mut state = create_state(&snapshot("busy", vec![]));
    apply(&mut state, &failed(None));
    assert_eq!((state.explicit_status, state.session_status), (Explicit::Idle, StreamStatus::Idle));
}

#[test]
fn keeps_a_failure_that_arrived_on_the_message_when_it_is_rebuilt_from_a_snapshot() {
    let mut l = life("message-1", "assistant", 1, Some("Half an ans"), None);
    l.info["turnError"] = error();
    l.info["finish"] = json!("error");
    let state = create_state(&snapshot("idle", vec![l]));
    assert_eq!(state.messages[0].turn_error, Some(error()));
    assert_eq!(state.messages[0].finish.as_deref(), Some("error"));
}

#[test]
fn keeps_the_failure_when_a_later_message_updated_carries_nothing_new() {
    let mut state = new_state();
    apply(&mut state, &Event::MessageCreated(life("message-1", "assistant", 1, Some("Half an ans"), None)));
    let mut with_error = life("message-1", "assistant", 1, None, None);
    with_error.info["turnError"] = error();
    apply(&mut state, &Event::MessageUpdated(with_error));
    assert_eq!(state.messages[0].turn_error, Some(error()));
    apply(&mut state, &Event::MessageUpdated(life("message-1", "assistant", 1, None, None)));
    assert_eq!(state.messages[0].turn_error, Some(error()));
}

// ─── what this port adds: tool parts and the wire ───────────────────────────

#[test]
fn merges_a_tool_calls_state_key_by_key_and_keeps_its_row() {
    let tool = |state: Value| {
        Event::PartUpdated(
            json!({"id": "prt_1", "messageID": "msg_1", "sessionID": "s", "type": "tool", "tool": "bash", "callID": "call_1", "state": state}),
        )
    };
    let mut state = new_state();
    apply(&mut state, &tool(json!({"status": "running", "input": {"command": "ls"}})));
    apply(&mut state, &tool(json!({"status": "completed", "output": "ok"})));
    let PartData::Tool { state: merged, .. } = &state.messages[0].parts[0] else { panic!("not a tool part") };
    assert_eq!(state.messages[0].parts.len(), 1);
    assert_eq!(merged, &json!({"status": "completed", "input": {"command": "ls"}, "output": "ok"}));
}

#[test]
fn a_step_finish_adds_its_tokens_and_cost_to_the_message() {
    let finish = |n: i64| {
        Event::PartUpdated(
            json!({"id": format!("f{n}"), "messageID": "msg_1", "sessionID": "s", "type": "step-finish", "cost": 0.5, "tokens": {"input": n, "output": 1, "reasoning": 0}}),
        )
    };
    let state = apply_all(new_state(), vec![finish(10), finish(20)]);
    assert_eq!(state.messages[0].cost, Some(1.0));
    assert_eq!(state.messages[0].tokens.map(|t| (t.input, t.output)), Some((30, 2)));
}

#[test]
fn applies_nine_events_and_keeps_the_rest() {
    let wire = |t: &str, p: Value| json!({"type": t, "eventId": null, "properties": p});
    assert_eq!(Event::from_wire(&wire("turn.started", json!({}))), Some(Event::TurnStarted));
    assert_eq!(Event::from_wire(&wire("session.idled", json!({}))), Some(Event::SessionIdled));
    // The rest are kept for their owners (`events.rs`), not dropped.
    assert!(matches!(Event::from_wire(&wire("session.status", json!({}))), Some(Event::Other(..))));
    // Only text deltas stream into a part.
    let reasoning =
        wire("message.part.delta.streamed", json!({"sessionID": "s", "messageID": "m", "partID": "p", "field": "reasoning", "delta": "x"}));
    assert!(!Event::from_wire(&reasoning).unwrap().applied());
}

#[test]
fn signalr_frames_end_in_the_record_separator_and_parse_back() {
    assert_eq!(signalr::handshake(), "{\"protocol\":\"json\",\"version\":1}\u{1e}");
    let call = signalr::invocation(2, "SubscribeToSessionAsync", &[json!("abc")]);
    assert!(call.ends_with('\u{1e}'));
    let body: Value = serde_json::from_str(call.trim_end_matches('\u{1e}')).unwrap();
    assert_eq!(
        (body["type"].clone(), body["invocationId"].clone(), body["target"].clone(), body["arguments"].clone()),
        (json!(1), json!("2"), json!("SubscribeToSessionAsync"), json!(["abc"]))
    );
    // One WebSocket message can hold several frames.
    let frames = signalr::parse("{}\u{1e}{\"type\":6}\u{1e}{\"type\":3,\"invocationId\":\"1\",\"result\":null}\u{1e}");
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[0], signalr::Frame::Handshake(None));
    assert_eq!(frames[1], signalr::Frame::Ping);
}

// ─── frames captured from a real server ─────────────────────────────────────

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn frames_of(name: &str) -> Vec<signalr::Frame> {
    fixture(name).lines().flat_map(signalr::parse).collect()
}

fn snapshot_of(frames: &[signalr::Frame]) -> Value {
    frames
        .iter()
        .find_map(|f| match f {
            signalr::Frame::Completion { id, result, .. } if id == "2" => Some(result.clone()),
            _ => None,
        })
        .expect("no snapshot in the capture")
}

#[test]
fn the_snapshot_of_a_finished_turn_reads_as_text_and_a_tool_row() {
    let state = create_state(&snapshot_of(&frames_of("hub-snapshot-after-turn.jsonl")));
    assert_eq!(state.session_status, StreamStatus::Idle);
    let shown = view::messages(&state.messages, &Default::default());
    let tools: Vec<(&str, &str, ToolState)> = shown
        .iter()
        .flat_map(|m| &m.parts)
        .filter_map(|p| match p {
            Part::Tool { tool, detail, state, .. } => Some((tool.as_str(), detail.as_str(), *state)),
            _ => None,
        })
        .collect();
    assert_eq!(tools.len(), 2, "two bash calls in the capture: it ran the tests twice");
    for (tool, detail, state) in tools {
        assert_eq!((tool, detail), ("bash", "Run the tests"));
        // The server's snapshot leaves out the call's timestamps, so a snapshot has no duration to show.
        assert!(state == ToolState::Plain);
    }
    assert_eq!(shown.iter().filter(|m| m.role == Role::User).count(), 3);
}

/// The whole captured turn, fed event by event as the hub sent it, ends where the snapshot taken
/// afterwards says it ends: the live path and the snapshot path agree.
#[test]
fn live_events_and_a_later_snapshot_agree() {
    let live = frames_of("hub-turn-run-tests.jsonl");
    let mut state = create_state(&snapshot_of(&live));
    let mut seen_busy = false;
    for frame in &live {
        if let signalr::Frame::Event { topic, data, .. } = frame
            && topic.starts_with("session:")
            && let Some(event) = Event::from_wire(data)
        {
            apply(&mut state, &event);
            seen_busy |= state.session_status == StreamStatus::Busy;
        }
    }
    assert!(seen_busy, "turn.started should have made the session busy");
    assert_eq!(state.session_status, StreamStatus::Idle);

    let later = create_state(&snapshot_of(&frames_of("hub-snapshot-after-turn.jsonl")));
    let flatten = |s: &StreamState| {
        view::messages(&s.messages, &Default::default()).into_iter().map(|m| (m.id, m.role, m.parts.len())).collect::<Vec<_>>()
    };
    assert_eq!(flatten(&state), flatten(&later));
    let last_text = |s: &StreamState| {
        s.messages
            .iter()
            .rev()
            .flat_map(|m| m.parts.iter())
            .find_map(|p| if let PartData::Text { text, .. } = p { Some(text.clone()) } else { None })
    };
    assert_eq!(last_text(&state), last_text(&later));
    assert!(last_text(&state).unwrap().contains("All 42 tests pass"));
    // Live, the call carries its start and end, so its row shows how long it took.
    let took = view::messages(&state.messages, &Default::default())
        .iter()
        .flat_map(|m| m.parts.iter())
        .any(|p| matches!(p, Part::Tool { state: ToolState::Took(secs), .. } if *secs > 0.0));
    assert!(took, "the live bash call should show its duration");
}

#[test]
fn the_session_list_groups_rows_under_their_project() {
    let rows: Value = serde_json::from_str(&fixture("api-sessions.json")).unwrap();
    let projects = sessions::parse_list(&rows, 1_794_674_800_000);
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name, "Scratch");
    let session = &projects[0].sessions[0];
    assert_eq!((session.title.as_str(), session.status), ("Fix pagination", Status::Idle));
    assert!(!session.id.is_empty());
}

#[test]
fn the_list_leaves_out_sub_agents_and_dims_old_sessions() {
    let row = |id: &str, extra: Value| {
        let mut r = json!({"session": {"id": id, "title": id, "time": {"updated": 0}}, "projectName": "P", "activityStatus": "idle", "retentionStatus": "active"});
        r.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        r
    };
    let day = 86_400_000;
    let rows = json!([
        row("recent", json!({"session": {"id": "recent", "title": "recent", "time": {"updated": 3 * day - 5000}}})),
        row("child", json!({"parentSessionId": "recent"})),
        row("archived", json!({"retentionStatus": "archived"})),
        row("busy", json!({"activityStatus": "busy"})),
    ]);
    let projects = sessions::parse_list(&rows, 3 * day);
    let sessions: Vec<&str> = projects[0].sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(sessions, ["recent", "busy"]);
    assert_eq!(projects[0].sessions[0].row.updated, 3 * day - 5000);
}

mod agents;
mod app_runs;
mod asks;
mod attach;
mod auth;
mod autocomplete;
mod canvases;
mod cards;
mod catalog;
mod composables;
mod dashboard;
mod delegations;
mod diff_contents;
mod diffs;
mod discover;
mod drafts;
mod events;
mod export;
mod files;
mod find;
mod finder;
mod groups;
mod header;
mod history;
mod instance;
mod lineage;
mod list_work;
mod live;
mod local_token;
mod machine;
mod machine_diagnosis;
mod machine_routes;
mod machines;
mod modes;
mod ordering;
mod pages;
mod presence;
mod progress;
mod prompts;
mod questions;
mod queue;
mod reconnect;
mod references;
mod refresh;
mod routing;
mod row_status;
mod rows;
mod rules;
mod scroll;
mod selection;
mod selector;
mod server_canvases;
mod settings;
mod smart_links;
mod state;
mod store;
mod tls;
mod turns;
mod usage;
pub(crate) mod usage_rules;
mod web_page;
mod wire;
mod work;

#[test]
fn a_recap_comes_with_the_snapshot_and_is_replaced_or_cleared_by_session_recap() {
    let mut state = create_state(&json!({"messages": [], "recap": {"text": "Paging the list."}}));
    assert_eq!(state.recap.as_deref(), Some("Paging the list."));
    apply(
        &mut state,
        &Event::from_wire(&json!({"type": "session.recap", "properties": {"sessionId": "s", "text": "Next, ship it."}})).unwrap(),
    );
    assert_eq!(state.recap.as_deref(), Some("Next, ship it."));
    apply(&mut state, &Event::from_wire(&json!({"type": "session.recap", "properties": {"sessionId": "s", "text": null}})).unwrap());
    assert_eq!(state.recap, None);
    assert_eq!(create_state(&json!({"messages": []})).recap, None);
}
