//! `message-ordering.test.ts` (7) and the cases of `event-state.test.ts` no earlier piece took, under Fleet's
//! names: tool parts, `mergeMessageUpdate`, `compareMessageIds`, `ensureMessage` and where a message lands.
use super::super::model::{Msg, PartData};
use super::super::reducer::{
    Event, apply, apply_part_update, apply_text_delta, compare_message_ids, create_state, ensure_message, merge_update,
};
use super::*;
use std::cmp::Ordering;

fn payload(id: &str, role: &str, created: i64, text: Option<&str>) -> Lifecycle {
    life(id, role, created, text, Some(&format!("{id}-text-1")))
}

fn created(events: impl IntoIterator<Item = Lifecycle>) -> StreamState {
    apply_all(new_state(), events.into_iter().map(Event::MessageCreated).collect())
}

fn order(state: &StreamState) -> Vec<&str> {
    ids(state)
}

fn ensure(messages: &mut Vec<Msg>, info: Value) {
    ensure_message(messages, &info);
}

/// `mergeMessageUpdate(messages, info)`: the info carries its `parts` as Fleet's tests write it.
fn merge(messages: &mut [Msg], info: Value) {
    let parts = info.get("parts").and_then(Value::as_array).cloned();
    merge_update(messages, &info, parts.as_deref());
}

fn ids_of(messages: &[Msg]) -> Vec<&str> {
    messages.iter().map(|m| m.id.as_str()).collect()
}

fn msg(id: &str, role: &str, created: Option<i64>) -> Msg {
    let mut m = Msg::new(id, "session-1", super::super::reducer::to_role(&json!(role)));
    m.created_at = created;
    m
}

fn tool_part(state: Value) -> Value {
    json!({"messageID": "message-1", "sessionID": "session-1", "id": "tool-1", "type": "tool", "tool": "bash", "callID": "call-1", "state": state})
}

fn first_tool_state(messages: &[Msg]) -> Value {
    let PartData::Tool { state, .. } = &messages[0].parts[0] else { panic!("not a tool part") };
    state.clone()
}

// ─── message-ordering.test.ts ───────────────────────────────────────────────

#[test]
fn sorts_messages_by_message_id() {
    let state = created([
        payload("msg_0000010000000001_user1", "user", 1000, Some("hey")),
        payload("msg_0000010000000002_assistant1", "assistant", 1500, Some("assistant reply")),
        payload("msg_0000010000000003_user2", "user", 2000, Some("yo")),
    ]);
    assert_eq!(order(&state), ["msg_0000010000000001_user1", "msg_0000010000000002_assistant1", "msg_0000010000000003_user2"]);
}

#[test]
fn keeps_an_assistant_message_created_by_a_delta_where_it_arrived() {
    let mut state = created([payload("msg_0000010000000001_user1", "user", 1000, Some("first"))]);
    apply(&mut state, &delta("assistant-1", "assistant-text-1", "assistant reply", None));
    apply(&mut state, &Event::MessageCreated(payload("msg_0000010000000002_user2", "user", 2000, Some("second"))));
    assert_eq!(order(&state), ["msg_0000010000000001_user1", "assistant-1", "msg_0000010000000002_user2"]);
}

#[test]
fn sorts_snapshot_messages_by_message_id() {
    let snap = snapshot(
        "idle",
        vec![
            payload("msg_0000010000000002_msg1", "user", 2000, Some("second by timestamp")),
            payload("msg_0000010000000001_msg2", "assistant", 1000, Some("first by timestamp")),
            payload("msg_0000010000000003_msg3", "user", 1500, Some("middle by timestamp")),
        ],
    );
    assert_eq!(order(&create_state(&snap)), ["msg_0000010000000001_msg2", "msg_0000010000000002_msg1", "msg_0000010000000003_msg3"]);
}

#[test]
fn reorders_when_lifecycle_backfills_createdat() {
    let mut state = created([payload("msg_0000010000000001_user1", "user", 1000, Some("prompt"))]);
    apply(&mut state, &delta("msg_0000010000000002_assistant1", "assistant-text-1", "reply", None));
    apply(&mut state, &Event::MessageCreated(payload("msg_0000010000000003_user2", "user", 2000, Some("second prompt"))));
    apply(&mut state, &Event::MessageCreated(payload("msg_0000010000000002_assistant1", "assistant", 1500, Some("reply"))));
    // The delta had no time and no neighbour to go by, so it arrived where it arrived and the id put it before the second prompt.
    assert_eq!(order(&state), ["msg_0000010000000001_user1", "msg_0000010000000002_assistant1", "msg_0000010000000003_user2"]);
    assert_eq!(state.messages[1].created_at, Some(1500));
}

/// A Pi session's snapshot (14 November 2026): Pi's ids for its own messages, Fleet's `msg_` id for a prompt sent later.
fn pi_conversation() -> Vec<Lifecycle> {
    let pi = |role: &str, at: i64| format!("pi-5f3c-{role}-{at}");
    vec![
        payload(&pi("user", 1794681358517), "user", 1794681358517, Some("Write the export plan.")),
        payload(&pi("assistant", 1794681358669), "assistant", 1794681358669, None),
        payload(&pi("assistant", 1794681358754), "assistant", 1794681358754, Some("Wrote the plan.")),
        payload("msg_1db5887aa00040s2PcEAhH8c7P", "user", 1794681374642, Some("Do it once more.")),
        payload(&pi("assistant", 1794681374643), "assistant", 1794681374643, None),
        payload(&pi("assistant", 1794681374683), "assistant", 1794681374683, Some("Wrote it again.")),
    ]
}

fn pi_order() -> Vec<String> {
    pi_conversation().iter().map(|l| l.info["id"].as_str().unwrap().to_string()).collect()
}

#[test]
fn keeps_the_snapshots_order_after_a_reload() {
    let state = create_state(&snapshot("idle", pi_conversation()));
    assert_eq!(order(&state), pi_order());
}

#[test]
fn keeps_the_order_the_messages_arrive_in_live() {
    assert_eq!(order(&created(pi_conversation())), pi_order());
}

#[test]
fn keeps_the_order_when_the_replies_have_no_time() {
    let untimed: Vec<Lifecycle> = pi_conversation()
        .into_iter()
        .map(|mut l| {
            if l.info["role"] == "assistant" {
                l.info["time"] = json!({"created": 0, "completed": null});
            }
            l
        })
        .collect();
    assert_eq!(order(&create_state(&snapshot("idle", untimed))), pi_order());
}

// ─── event-state.test.ts: applyPartUpdate ───────────────────────────────────

#[test]
fn creates_a_message_for_the_first_tool_part() {
    let mut messages = vec![];
    apply_part_update(&mut messages, &tool_part(json!({"status": "running"})));
    assert_eq!(messages.len(), 1);
    assert_eq!((messages[0].id.as_str(), messages[0].session_id.as_str(), messages[0].role), ("message-1", "session-1", Role::Assistant));
    assert_eq!(
        messages[0].parts,
        [PartData::Tool { id: "tool-1".into(), tool: "bash".into(), call_id: "call-1".into(), state: json!({"status": "running"}) }]
    );
}

#[test]
fn merges_sequential_tool_state_updates() {
    let mut messages = vec![];
    apply_part_update(&mut messages, &tool_part(json!({"status": "running", "stdout": "first chunk"})));
    apply_part_update(&mut messages, &tool_part(json!({"stderr": "warning", "exitCode": 0})));
    assert_eq!(messages.len(), 1);
    assert_eq!(first_tool_state(&messages), json!({"status": "running", "stdout": "first chunk", "stderr": "warning", "exitCode": 0}));
}

#[test]
fn overwrites_existing_tool_state_keys_with_latest_values() {
    let mut messages = vec![];
    apply_part_update(&mut messages, &tool_part(json!({"status": "running", "stdout": "first chunk", "exitCode": 1})));
    apply_part_update(&mut messages, &tool_part(json!({"status": "completed", "exitCode": 0})));
    assert_eq!(first_tool_state(&messages), json!({"status": "completed", "stdout": "first chunk", "exitCode": 0}));
}

// ─── event-state.test.ts: mergeMessageUpdate ────────────────────────────────

fn user_then(created_at: i64, text: &str) -> Vec<Msg> {
    let mut user = msg("msg_0000010000000001_user", "user", Some(created_at));
    user.parts.push(PartData::Text { id: "user-text-1".into(), text: text.into() });
    vec![user]
}

fn assistant_text(m: &Msg) -> Vec<String> {
    m.parts.iter().filter_map(|p| if let PartData::Text { text, .. } = p { Some(text.clone()) } else { None }).collect()
}

#[test]
fn keeps_user_prompt_before_assistant_response_when_assistant_timestamp_arrives_late() {
    let mut messages = user_then(1000, "Explain stream ordering.");
    let mut assistant = msg("msg_0000010000000002_assistant", "assistant", None);
    assistant.parts.push(PartData::Text { id: "assistant-text-1".into(), text: "Partial response".into() });
    messages.push(assistant);
    merge(
        &mut messages,
        json!({"id": "msg_0000010000000002_assistant", "time": {"created": 1000, "completed": 2000},
            "parts": [{"id": "assistant-text-1", "type": "text", "text": "Complete response"}]}),
    );
    let roles: Vec<_> = messages.iter().map(|m| m.role).collect();
    assert_eq!(roles, [Role::User, Role::Assistant]);
    assert_eq!(assistant_text(&messages[1]), ["Complete response"]);
}

fn part_then_lifecycle(streamed: &str, lifecycle: &str) -> Vec<Msg> {
    let mut messages = user_then(1000, "Explain out-of-order message events.");
    apply_part_update(
        &mut messages,
        &json!({"messageID": "msg_0000010000000002_assistant", "sessionID": "session-1", "id": "assistant-text-1", "type": "text", "text": streamed}),
    );
    merge(
        &mut messages,
        json!({"id": "msg_0000010000000002_assistant", "role": "assistant", "sessionID": "session-1", "time": {"created": 1000, "completed": 2000},
            "parts": [{"id": "assistant-text-1", "type": "text", "text": lifecycle}]}),
    );
    messages
}

#[test]
fn does_not_duplicate_assistant_message_and_preserves_longer_part_when_lifecycle_arrives_after_part_update() {
    let messages = part_then_lifecycle("Part update text that is longer than the lifecycle snapshot.", "Lifecycle text.");
    assert_eq!(ids_of(&messages), ["msg_0000010000000001_user", "msg_0000010000000002_assistant"]);
    assert_eq!((messages[1].role, messages[1].created_at, messages[1].completed_at), (Role::Assistant, Some(1000), Some(2000)));
    assert_eq!(assistant_text(&messages[1]), ["Part update text that is longer than the lifecycle snapshot."]);
}

#[test]
fn does_not_duplicate_assistant_message_and_replaces_shorter_part_when_lifecycle_arrives_after_part_update() {
    let messages = part_then_lifecycle("Partial.", "Lifecycle text that replaces the shorter part update.");
    assert_eq!(ids_of(&messages), ["msg_0000010000000001_user", "msg_0000010000000002_assistant"]);
    assert_eq!((messages[1].created_at, messages[1].completed_at), (Some(1000), Some(2000)));
    assert_eq!(assistant_text(&messages[1]), ["Lifecycle text that replaces the shorter part update."]);
}

#[test]
fn does_not_re_sort_when_createdat_is_backfilled() {
    let mut messages = vec![
        msg("msg_0000010000000001_user1", "user", Some(1000)),
        msg("msg_0000010000000003_user2", "user", Some(3000)),
        msg("msg_0000010000000002_agent1", "assistant", None),
    ];
    merge(&mut messages, json!({"id": "msg_0000010000000002_agent1", "time": {"created": 2000}}));
    // Merging doesn't move a message, though its id would sort it into the middle.
    assert_eq!(ids_of(&messages), ["msg_0000010000000001_user1", "msg_0000010000000003_user2", "msg_0000010000000002_agent1"]);
    assert_eq!(messages[2].created_at, Some(2000));
}

#[test]
fn updates_message_metadata_without_changing_position() {
    let mut messages = vec![msg("msg_0000010000000001_aaa", "user", Some(1000)), msg("msg_0000010000000002_bbb", "assistant", Some(2000))];
    merge(&mut messages, json!({"id": "msg_0000010000000002_bbb", "time": {"created": 2000, "completed": 3000}}));
    assert_eq!(ids_of(&messages), ["msg_0000010000000001_aaa", "msg_0000010000000002_bbb"]);
    assert_eq!(messages[1].completed_at, Some(3000));
}

#[test]
fn maintains_id_based_order_regardless_of_timestamp() {
    let mut messages = vec![
        msg("msg_0000010000000001_aaa", "user", Some(1000)),
        msg("msg_0000010000000002_bbb", "assistant", Some(1000)),
        msg("msg_0000010000000003_ccc", "assistant", None),
    ];
    merge(&mut messages, json!({"id": "msg_0000010000000003_ccc", "time": {"created": 500}}));
    assert_eq!(ids_of(&messages), ["msg_0000010000000001_aaa", "msg_0000010000000002_bbb", "msg_0000010000000003_ccc"]);
    assert_eq!(messages[2].created_at, Some(500));
}

// ─── event-state.test.ts: message ordering ──────────────────────────────────

#[test]
fn orders_a_forks_copies_by_their_counter_not_as_text() {
    let mut list = ["msg_0ec1ad2f2_18", "msg_0ec1ad2f2_6", "msg_0ec1ad2f2_17", "msg_0ec1ad2f2_7"];
    list.sort_by(|a, b| compare_message_ids(a, b));
    assert_eq!(list, ["msg_0ec1ad2f2_6", "msg_0ec1ad2f2_7", "msg_0ec1ad2f2_17", "msg_0ec1ad2f2_18"]);
}

#[test]
fn orders_other_ids_as_text() {
    assert_eq!(compare_message_ids("msg_0ec1ad2f2001aaa", "msg_0ec1ae749000bbb"), Ordering::Less);
    assert_eq!(compare_message_ids("msg_b", "msg_a"), Ordering::Greater);
    assert_eq!(compare_message_ids("msg_a", "msg_a"), Ordering::Equal);
    assert_eq!(compare_message_ids("msg_0ec1ad2f2_18", "msg_0ec1ae749000FlcM"), Ordering::Less);
}

#[test]
fn puts_a_scheduled_prompt_where_it_happened_not_after_every_message() {
    let at = |id: &str, created: i64, role: &str| json!({"id": id, "role": role, "sessionID": "s", "time": {"created": created}});
    let mut messages = vec![];
    ensure(&mut messages, at("msg_1867b23b5000Xzf2bren2Fjn2Q", 1000, "user"));
    ensure(&mut messages, at("msg_1867b24360010HFWaH3z4iPiGE", 1010, "assistant"));
    ensure(&mut messages, at("msg_sch_18662af63001G2JCjtpTAOtXq4_1", 2000, "user"));
    ensure(&mut messages, at("msg_1867b6bdc001pz7oLomlez6UFG", 2010, "assistant"));
    ensure(&mut messages, at("msg_1867c0000001abc", 3000, "user"));
    assert_eq!(
        ids_of(&messages),
        [
            "msg_1867b23b5000Xzf2bren2Fjn2Q",
            "msg_1867b24360010HFWaH3z4iPiGE",
            "msg_sch_18662af63001G2JCjtpTAOtXq4_1",
            "msg_1867b6bdc001pz7oLomlez6UFG",
            "msg_1867c0000001abc"
        ]
    );
}

#[test]
fn keeps_a_forks_conversation_in_order_as_it_arrives() {
    let mut messages = vec![];
    for (id, role) in
        [("msg_0ec1ad2f2_17", "user"), ("msg_0ec1ad2f2_6", "user"), ("msg_0ec1ae749000Flc", "user"), ("msg_0ec1ad2f2_7", "assistant")]
    {
        ensure(&mut messages, json!({"id": id, "role": role, "sessionID": "s"}));
    }
    assert_eq!(ids_of(&messages), ["msg_0ec1ad2f2_6", "msg_0ec1ad2f2_7", "msg_0ec1ad2f2_17", "msg_0ec1ae749000Flc"]);
}

fn ensure_two(first: (&str, &str, i64), second: (&str, &str, i64)) -> Vec<Msg> {
    let mut messages = vec![];
    for (id, role, at) in [first, second] {
        ensure(&mut messages, json!({"id": id, "sessionID": "session-1", "role": role, "time": {"created": at}}));
    }
    messages
}

#[test]
fn inserts_messages_in_chronological_order_by_message_id() {
    let m = ensure_two(("msg_000001a2b3c4d5e6_abc123", "user", 1000), ("msg_0000010203040506_def456", "assistant", 500));
    assert_eq!(ids_of(&m), ["msg_0000010203040506_def456", "msg_000001a2b3c4d5e6_abc123"]);
}

#[test]
fn preserves_lexicographic_order_for_sequential_ids() {
    let m = ensure_two(("msg_0000010000000001_aaa", "user", 1000), ("msg_0000010000000002_bbb", "assistant", 1000));
    assert_eq!(ids_of(&m), ["msg_0000010000000001_aaa", "msg_0000010000000002_bbb"]);
}

#[test]
fn orders_messages_with_same_millisecond_by_counter() {
    let m = ensure_two(
        ("msg_00000194000000000002_second", "assistant", 1735689600000),
        ("msg_00000194000000000001_first", "user", 1735689600000),
    );
    assert_eq!(ids_of(&m), ["msg_00000194000000000001_first", "msg_00000194000000000002_second"]);
}

#[test]
fn places_a_message_with_another_kind_of_id_by_its_time() {
    let m = ensure_two(("msg_0000010000000001_aaa", "user", 1000), ("legacy-message-id", "assistant", 500));
    assert_eq!(ids_of(&m), ["legacy-message-id", "msg_0000010000000001_aaa"]);
}

#[test]
fn handles_out_of_order_arrival_user_assistant_user_assistant() {
    let mut messages = vec![];
    for (id, role, at) in [
        ("msg_0000010000000001_user1", "user", 1000),
        ("msg_0000010000000003_user2", "user", 3000),
        ("msg_0000010000000002_agent1", "assistant", 2000),
        ("msg_0000010000000004_agent2", "assistant", 4000),
    ] {
        ensure(&mut messages, json!({"id": id, "sessionID": "session-1", "role": role, "time": {"created": at}}));
    }
    assert_eq!(
        ids_of(&messages),
        ["msg_0000010000000001_user1", "msg_0000010000000002_agent1", "msg_0000010000000003_user2", "msg_0000010000000004_agent2"]
    );
}

fn two_with_gap() -> Vec<Msg> {
    vec![msg("msg_0000010000000001_aaa", "user", Some(1000)), msg("msg_0000010000000003_ccc", "assistant", Some(3000))]
}

#[test]
fn inserts_new_message_in_sorted_position_when_creating_from_part() {
    let mut messages = two_with_gap();
    apply_part_update(
        &mut messages,
        &json!({"messageID": "msg_0000010000000002_bbb", "sessionID": "session-1", "id": "part-1", "type": "text", "text": "hello"}),
    );
    assert_eq!(ids_of(&messages), ["msg_0000010000000001_aaa", "msg_0000010000000002_bbb", "msg_0000010000000003_ccc"]);
}

#[test]
fn inserts_new_message_in_sorted_position_when_creating_from_delta() {
    let mut messages = two_with_gap();
    apply_text_delta(&mut messages, "msg_0000010000000002_bbb", "part-1", "session-1", "hello", None);
    assert_eq!(ids_of(&messages), ["msg_0000010000000001_aaa", "msg_0000010000000002_bbb", "msg_0000010000000003_ccc"]);
}
