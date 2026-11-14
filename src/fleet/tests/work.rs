//! Running work, context usage and the marks on messages: the cases of weave-fleet's
//! `running-work.test.ts`, `context-usage.test.ts` (`toContextUsage` only: the ring is component 5's),
//! `late-tool-update.test.ts`, and the matching cases of `domain-event-reducer.test.ts` and
//! `event-state.test.ts`, under the same names. A Fleet test that checks "the same state object" checks
//! here that nothing changed.
use super::super::context::to_context_usage;
use super::super::model::{ModKind, ModMark, PartData, SlashCommand, StreamState};
use super::super::reducer::{Event, Lifecycle, apply, create_state};
use super::super::work::*;
use super::{life, new_state, snapshot};
use crate::view::conversation::Role;
use serde_json::{Value, json};

// ─── running work ───────────────────────────────────────────────────────────

/// The payloads as the server sends them (nulls and false left out): a background shell, and a
/// sub-agent with a child session.
fn shell_started() -> Value {
    json!({
        "id": "5f0c2b7e9d", "sessionId": "s1", "workId": "sh_1076", "kind": "shell", "title": "shell",
        "label": "make test-all", "status": "running", "background": true, "toolCallId": "call_bg",
        "canStop": true, "canReadOutput": true, "startedAt": "2026-10-04T10:00:00Z",
    })
}

fn shell_ended() -> Value {
    let mut v = shell_started();
    v["status"] = json!("completed");
    v["endedAt"] = json!("2026-10-04T10:03:12Z");
    v["endedReason"] = json!("completed");
    v["detail"] = json!("exit 0");
    v
}

fn subagent_started() -> Value {
    json!({
        "id": "a91d", "sessionId": "s1", "workId": "call_sub", "kind": "subagent", "title": "code-reviewer",
        "label": "Review the diff", "status": "running", "background": true, "childSessionId": "child-1",
        "toolCallId": "call_sub", "canStop": true, "startedAt": "2026-10-04T10:01:00Z",
    })
}

fn item(payload: Value) -> RunningWorkItem {
    to_item(&payload).expect("not an item")
}

fn with(base: Value, extra: Value) -> Value {
    let mut v = base;
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

fn snapshot_with_work(work: Option<Vec<Value>>) -> StreamState {
    let mut s = snapshot("idle", vec![]);
    if let Some(work) = work {
        s["runningWork"] = json!(work);
    }
    create_state(&s)
}

fn work_event(kind: &str, payload: Value) -> Event {
    Event::from_wire(&json!({"type": kind, "eventId": 2, "properties": payload})).unwrap()
}

#[test]
fn reads_the_servers_item_filling_in_what_it_leaves_out() {
    let shell = item(shell_started());
    assert_eq!(shell.id, "5f0c2b7e9d");
    assert_eq!(shell.kind, WorkKind::Shell);
    assert_eq!(shell.label.as_deref(), Some("make test-all"));
    assert!(shell.background && shell.can_stop && shell.can_read_output);
    assert_eq!((shell.child_session_id, shell.ended_at, shell.ended_reason, shell.detail), (None, None, None, None));
    assert!(is_running(&item(shell_started())));

    // A sub-agent from OpenCode 2 has no output of its own: canReadOutput is left out, so it's false.
    let sub = item(subagent_started());
    assert!(!sub.can_read_output);
    assert_eq!(sub.child_session_id.as_deref(), Some("child-1"));
}

#[test]
fn drops_what_isnt_an_item_and_reads_an_unknown_kind_as_a_task() {
    assert_eq!(to_item(&Value::Null), None);
    assert_eq!(to_item(&json!({"sessionId": "s1"})), None);
    assert_eq!(to_item(&with(shell_started(), json!({"kind": "daemon"}))).map(|i| i.kind), Some(WorkKind::Task));
    let items = to_items(Some(&json!([shell_started(), 3, {"id": "x"}])));
    assert_eq!(items.iter().map(|w| w.id.as_str()).collect::<Vec<_>>(), ["5f0c2b7e9d"]);
}

#[test]
fn replaces_an_item_by_id_keeps_ended_work_ended_and_returns_the_same_list_when_nothing_changed() {
    let mut list = vec![item(shell_started())];
    assert!(apply_item(&mut list, item(shell_ended())));
    assert_eq!((list[0].status, list[0].detail.as_deref()), (WorkStatus::Completed, Some("exit 0")));
    let ended = list.clone();

    // A running event sent before the end but delivered after it doesn't bring the work back.
    assert!(!apply_item(&mut list, item(shell_started())));
    assert!(!apply_item(&mut list, item(shell_ended())));
    assert_eq!(list, ended);
}

#[test]
fn keeps_items_oldest_first() {
    let mut list = vec![item(subagent_started())];
    apply_item(&mut list, item(shell_started()));
    assert_eq!(list.iter().map(|w| w.kind.wire()).collect::<Vec<_>>(), ["shell", "subagent"]);
}

#[test]
fn says_how_long_and_how_it_ended_in_a_few_words() {
    assert_eq!(format_elapsed(58_000), "58s");
    assert_eq!(format_elapsed(192_000), "3m");
    assert_eq!(format_elapsed(3_840_000), "1h 4m");
    assert_eq!(format_elapsed(7_200_000), "2h");
    assert_eq!(format_ago(0), "just now");
    assert_eq!(format_ago(59_000), "just now");
    assert_eq!(format_ago(240_000), "4m ago");
    // Seconds tick in the first minute only; after that the minutes are all that changes.
    assert_eq!(elapsed_tick_ms(12_000), 1_000);
    assert_eq!(elapsed_tick_ms(61_000), 60_000);
    assert_eq!(work_result(&item(shell_started())), None);
    assert_eq!(work_result(&item(shell_ended())).as_deref(), Some("exit 0"));
    let stopped = with(shell_started(), json!({"status": "cancelled", "endedAt": "2026-10-04T10:01:00Z", "endedReason": "cancelled"}));
    assert_eq!(work_result(&item(stopped)).as_deref(), Some("stopped"));
    let lost = with(shell_started(), json!({"status": "error", "endedAt": "2026-10-04T10:01:00Z", "endedReason": "lost"}));
    assert_eq!(work_result(&item(lost.clone())).as_deref(), Some("lost"));
    assert!(work_failed(&item(lost)) && !work_failed(&item(shell_ended())));
}

#[test]
fn counts_how_long_it_has_run_from_its_start_to_its_end_or_to_now() {
    let started = parse_iso_ms("2026-10-04T10:00:00Z").unwrap();
    assert_eq!(parse_iso_ms("2026-10-04T10:03:12Z"), Some(started + 192_000));
    assert_eq!(parse_iso_ms("2026-10-04T10:00:00.250Z"), Some(started + 250));
    assert_eq!(parse_iso_ms("2026-10-04T12:00:00+02:00"), Some(started));
    assert_eq!(parse_iso_ms("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(parse_iso_ms("not a time"), None);
    assert_eq!(work_elapsed_ms(&item(shell_started()), started + 5_000), 5_000);
    assert_eq!(work_elapsed_ms(&item(shell_ended()), started + 999_000), 192_000);
    assert_eq!(work_elapsed_ms(&item(with(shell_started(), json!({"startedAt": "?"}))), 5), 0);
}

#[test]
fn starts_from_the_snapshots_running_work() {
    let state = snapshot_with_work(Some(vec![subagent_started(), shell_ended()]));
    let seen: Vec<_> = state.running_work.iter().map(|w| (w.kind.wire(), w.status)).collect();
    assert_eq!(seen, [("shell", WorkStatus::Completed), ("subagent", WorkStatus::Running)]);
}

#[test]
fn starts_with_no_work_when_the_snapshot_has_none() {
    assert!(snapshot_with_work(None).running_work.is_empty());
}

#[test]
fn adds_work_on_work_started_changes_it_on_work_updated_and_ends_it_on_work_ended() {
    let mut state = snapshot_with_work(None);
    apply(&mut state, &work_event("work.started", shell_started()));
    assert_eq!(state.running_work.len(), 1);

    apply(&mut state, &work_event("work.updated", with(shell_started(), json!({"label": "make test-all --shard 1"}))));
    assert_eq!(state.running_work[0].label.as_deref(), Some("make test-all --shard 1"));

    apply(&mut state, &work_event("work.ended", shell_ended()));
    let w = &state.running_work[0];
    assert_eq!((w.status, w.ended_reason, w.detail.as_deref()), (WorkStatus::Completed, Some(EndedReason::Completed), Some("exit 0")));
}

#[test]
fn doesnt_count_background_work_as_the_sessions_own() {
    let mut state = snapshot_with_work(None);
    apply(&mut state, &work_event("work.started", subagent_started()));
    assert_eq!(state.session_status, super::StreamStatus::Idle);
}

#[test]
fn keeps_the_same_state_for_an_event_it_already_has() {
    let mut state = snapshot_with_work(None);
    apply(&mut state, &work_event("work.started", shell_started()));
    let before = state.clone();
    apply(&mut state, &work_event("work.started", shell_started()));
    apply(&mut state, &work_event("work.ended", json!({"nope": true})));
    assert_eq!(state, before);
}

#[test]
fn is_the_prompt_that_began_the_turn_a_prompt_sent_mid_turn_doesnt_restart_the_clock() {
    let msg = |id: &str, role: Role, created: i64, steered: bool| {
        let mut m = super::super::model::Msg::new(id, "s1", role);
        (m.created_at, m.steered) = (Some(created), steered);
        m
    };
    let messages = [msg("a", Role::User, 1_000, false), msg("b", Role::Assistant, 2_000, false), msg("c", Role::User, 600_000, true)];
    assert_eq!(turn_started_at(&messages), Some(1_000));
    assert_eq!(turn_started_at(&[msg("a", Role::Assistant, 1, false)]), None);
}

// ─── context usage ──────────────────────────────────────────────────────────

fn usage() -> Value {
    json!({"sessionId": "session-1", "used": 76000, "limit": 200000, "compacting": false, "turns": [], "updatedAt": "2026-10-06T10:00:00Z"})
}

fn state_with_context(context: Option<Value>) -> StreamState {
    let mut s = snapshot("idle", vec![]);
    if let Some(c) = context {
        s["context"] = c;
    }
    create_state(&s)
}

#[test]
fn reads_fleets_payload_with_what_the_server_left_out_as_null() {
    let u = to_context_usage(&json!({
        "sessionId": "s1", "used": 76000, "limit": 200000,
        "lastCall": {"input": 1000, "cacheRead": 74500, "cacheWrite": 0, "output": 500, "reasoning": 0, "used": 76000},
        "compacting": false,
        "turns": [{"used": 76000, "limit": 200000, "at": "2026-10-06T10:00:00Z", "afterCompaction": false}, {"bad": true}],
        "updatedAt": "2026-10-06T10:00:00Z",
    }))
    .unwrap();
    assert_eq!((u.used, u.limit, u.compacts_at, u.model_id, u.compacted_at), (Some(76_000), Some(200_000), None, None, None));
    assert_eq!(u.last_call.unwrap().cache_read, 74_500);
    assert_eq!(u.turns.len(), 1);
}

#[test]
fn marks_a_call_whose_harness_gave_only_the_total_with_the_total_as_its_size() {
    let u = to_context_usage(&json!({
        "sessionId": "s1", "used": 12000, "turns": [],
        "lastCall": {"input": 12000, "cacheRead": 0, "cacheWrite": 0, "output": 0, "reasoning": 0, "used": 12000, "totalOnly": true},
    }))
    .unwrap();
    let call = u.last_call.unwrap();
    assert!(call.total_only);
    assert_eq!(call.used, 12_000);
}

#[test]
fn reads_a_call_without_the_field_as_one_with_a_known_split() {
    let u = to_context_usage(&json!({
        "sessionId": "s1", "turns": [],
        "lastCall": {"input": 1000, "cacheRead": 74500, "cacheWrite": 0, "output": 500, "reasoning": 0, "used": 76000},
    }))
    .unwrap();
    assert!(!u.last_call.unwrap().total_only);
}

#[test]
fn isnt_a_context_without_a_session() {
    assert_eq!(to_context_usage(&Value::Null), None);
    assert_eq!(to_context_usage(&json!({"used": 1})), None);
}

#[test]
fn reads_counts_the_api_sends_as_strings_and_a_missing_window_as_unknown() {
    let u = to_context_usage(&json!({"sessionId": "s1", "used": "17148", "limit": 0, "turns": []})).unwrap();
    assert_eq!(u.used, Some(17_148));
    assert_eq!(u.limit, None);
}

#[test]
fn starts_from_the_snapshots_context_and_takes_each_update_whole() {
    let mut state = state_with_context(Some(usage()));
    assert_eq!(state.context.as_ref().unwrap().used, Some(76_000));
    let messages = state.messages.clone();

    let mut compacting = usage();
    compacting["compacting"] = json!(true);
    apply(&mut state, &Event::from_wire(&json!({"type": "context.updated", "properties": compacting})).unwrap());
    assert!(state.context.as_ref().unwrap().compacting);
    assert_eq!(state.messages, messages);
}

#[test]
fn is_null_until_the_harness_reports_and_ignores_a_payload_that_isnt_one() {
    let mut state = state_with_context(None);
    assert_eq!(state.context, None);
    apply(&mut state, &Event::from_wire(&json!({"type": "context.updated", "properties": {"nope": true}})).unwrap());
    assert_eq!(state.context, None);
}

// ─── marks on messages ──────────────────────────────────────────────────────

fn prompt(id: &str, created: i64, extra: Value) -> Lifecycle {
    let mut l = life(id, "user", created, Some("Run the tests"), None);
    for (k, v) in extra.as_object().unwrap() {
        l.info[k] = v.clone();
    }
    l
}

fn mods(state: &StreamState) -> Vec<Option<ModMark>> {
    state.messages.iter().map(|m| m.mod_mark.clone()).collect()
}

#[test]
fn keeps_the_mod_mark_on_the_prompt_live_and_from_a_snapshot_and_leaves_the_others_alone() {
    let typed = prompt("msg_0001", 1000, json!({}));
    let sent = prompt("msg_0002", 2000, json!({"mod": {"kind": "sent", "name": "tests-after-edits"}}));
    let live = {
        let mut s = create_state(&snapshot("idle", vec![typed.clone()]));
        apply(&mut s, &Event::MessageUpdated(sent.clone()));
        s
    };
    let reloaded = create_state(&snapshot("idle", vec![typed, sent]));
    for state in [live, reloaded] {
        assert_eq!(mods(&state), [None, Some(ModMark { kind: ModKind::Sent, name: "tests-after-edits".into() })]);
    }
}

#[test]
fn keeps_the_mark_on_a_prompt_sent_into_a_running_turn_live_and_from_a_snapshot() {
    let mut step = life("msg_0001", "assistant", 1000, Some("Running the tests"), None);
    step.info["finish"] = json!("tool-calls");
    let steered = prompt("msg_0002", 2000, json!({"steered": true}));
    let next = life("msg_0003", "assistant", 3000, Some("Switching files"), None);

    let mut live = create_state(&snapshot("idle", vec![step.clone()]));
    for l in [steered.clone(), next.clone()] {
        apply(&mut live, &Event::MessageUpdated(l));
    }
    let reloaded = create_state(&snapshot("idle", vec![step, steered, next]));
    for state in [live, reloaded] {
        let seen: Vec<_> = state.messages.iter().map(|m| (m.id.as_str(), m.steered)).collect();
        assert_eq!(seen, [("msg_0001", false), ("msg_0002", true), ("msg_0003", false)]);
    }
}

#[test]
fn marks_a_prompt_the_server_says_was_steered_after_it_was_first_shown_without_the_mark() {
    let mut state = create_state(&snapshot("idle", vec![prompt("msg_0002", 2000, json!({}))]));
    apply(&mut state, &Event::MessageUpdated(prompt("msg_0002", 2000, json!({"steered": true}))));
    assert!(state.messages[0].steered);
}

#[test]
fn keeps_the_command_a_user_message_came_from() {
    let mut state = new_state();
    let info = json!({"id": "msg_0d1eda667001SQ0X1r7eLpxOMf", "sessionID": "session-1", "role": "user", "time": {"created": 1000},
        "command": {"name": "tidy", "arguments": "src/auth"}});
    apply(&mut state, &Event::MessageCreated(Lifecycle { info, parts: None }));
    assert_eq!(state.messages[0].command, Some(SlashCommand { name: "tidy".into(), arguments: Some("src/auth".into()) }));
}

#[test]
fn drops_empty_arguments_and_ignores_a_command_on_an_agent_message() {
    let mut state = new_state();
    let created = |id: &str, role: &str, command: Value| {
        Event::MessageCreated(Lifecycle {
            info: json!({"id": id, "sessionID": "session-1", "role": role, "command": command}),
            parts: None,
        })
    };
    apply(&mut state, &created("msg_0000010000000001_user", "user", json!({"name": "init", "arguments": null})));
    apply(&mut state, &created("msg_0000010000000002_agent", "assistant", json!({"name": "init"})));
    assert_eq!(state.messages[0].command, Some(SlashCommand { name: "init".into(), arguments: None }));
    assert_eq!(state.messages[1].command, None);
}

#[test]
fn takes_the_command_from_a_later_update() {
    let mut state = new_state();
    let info = |extra: Value| {
        let mut i = json!({"id": "msg_0000010000000001_user", "sessionID": "session-1", "role": "user"});
        if let Some(c) = extra.as_object() {
            i["command"] = Value::Object(c.clone());
        }
        Event::MessageUpdated(Lifecycle { info: i, parts: None })
    };
    apply(&mut state, &info(Value::Null));
    assert_eq!(state.messages[0].command, None);
    apply(&mut state, &info(json!({"name": "review", "arguments": "main"})));
    assert_eq!(state.messages[0].command, Some(SlashCommand { name: "review".into(), arguments: Some("main".into()) }));
    // A later update that doesn't say keeps what it had.
    apply(&mut state, &info(Value::Null));
    assert!(state.messages[0].command.is_some());
}

#[test]
fn ignores_a_mod_mark_that_names_no_mod_or_a_kind_it_doesnt_know() {
    let mut state = new_state();
    for mark in [json!({"kind": "sent", "name": ""}), json!({"kind": "deleted", "name": "x"}), json!("sent")] {
        apply(&mut state, &Event::MessageCreated(prompt("msg_1", 1, json!({"mod": mark}))));
    }
    assert_eq!(state.messages[0].mod_mark, None);
}

// ─── parts ──────────────────────────────────────────────────────────────────

fn parts_of(state: &StreamState, id: &str) -> Vec<PartData> {
    state.messages.iter().find(|m| m.id == id).unwrap().parts.clone()
}

fn user_with(id: &str, parts: Vec<Value>) -> Lifecycle {
    let mut l = life(id, "user", 1, None, None);
    l.parts = Some(parts);
    l.info["time"] = json!({"completed": 1234});
    l
}

#[test]
fn reconciles_text_delta_placeholder_with_lifecycle_without_duplicating_final_snapshot_text() {
    let id = "msg_0000010000000001_assistant";
    let mut state = new_state();
    apply(
        &mut state,
        &Event::TextDelta {
            session_id: "session-1".into(),
            message_id: id.into(),
            part_id: "assistant-text-1".into(),
            delta: "Streamed response text.".into(),
            offset: None,
        },
    );
    let lifecycle = |completed: Option<i64>| {
        let mut l = life(id, "assistant", 1000, Some("Streamed response text."), Some("assistant-text-1"));
        if let Some(c) = completed {
            l.info["time"]["completed"] = json!(c);
        }
        Event::MessageUpdated(l)
    };
    apply(&mut state, &lifecycle(None));
    apply(&mut state, &lifecycle(Some(2000)));

    assert_eq!(state.messages.iter().filter(|m| m.id == id).count(), 1);
    let m = &state.messages[0];
    assert_eq!((m.role, m.created_at, m.completed_at, m.session_id.as_str()), (Role::Assistant, Some(1000), Some(2000), "session-1"));
    assert_eq!(m.parts, [PartData::Text { id: "assistant-text-1".into(), text: "Streamed response text.".into() }]);
}

#[test]
fn includes_file_parts_from_committed_snapshot_even_when_no_prior_file_part_existed() {
    let id = "msg_0000010000000001_user";
    let mut state = create_state(&snapshot("idle", vec![user_with(id, vec![json!({"id": "text-1", "type": "text", "text": "hello"})])]));
    let file =
        json!({"id": "file-1", "type": "file", "mime": "image/png", "filename": "screenshot.png", "url": "data:image/png;base64,abc"});
    let both = vec![json!({"id": "text-1", "type": "text", "text": "hello"}), file];
    apply(&mut state, &Event::MessageUpdated(user_with(id, both)));
    assert!(parts_of(&state, id).contains(&PartData::File {
        id: "file-1".into(),
        mime: "image/png".into(),
        filename: Some("screenshot.png".into()),
        url: "data:image/png;base64,abc".into(),
    }));
}

#[test]
fn preserves_file_parts_already_accumulated_when_snapshot_arrives_without_them() {
    let id = "msg_0000010000000001_user";
    let mut state = new_state();
    let both = vec![
        json!({"id": "text-1", "type": "text", "text": "hello"}),
        json!({"id": "file-1", "type": "file", "mime": "image/png", "filename": "screenshot.png", "url": "data:image/png;base64,abc"}),
    ];
    apply(&mut state, &Event::MessageCreated(user_with(id, both)));
    apply(&mut state, &Event::MessageUpdated(user_with(id, vec![json!({"id": "text-1", "type": "text", "text": "hello"})])));
    assert!(parts_of(&state, id).iter().any(|p| matches!(p, PartData::File { id, mime, .. } if id == "file-1" && mime == "image/png")));
}

#[test]
fn adds_a_divider_live_and_its_summary_when_it_arrives() {
    let divider = json!({"messageID": "m1", "sessionID": "s1", "id": "m1-part-0", "type": "compaction", "trigger": "manual", "tokensBefore": 35438, "tokensAfter": 1450});
    let mut state = new_state();
    apply(&mut state, &Event::PartUpdated(divider.clone()));
    apply(&mut state, &Event::PartUpdated(with(divider, json!({"summary": "What we did."}))));
    assert_eq!(
        parts_of(&state, "m1"),
        [PartData::Compaction {
            id: "m1-part-0".into(),
            trigger: Some("manual".into()),
            tokens_before: Some(35_438),
            tokens_after: Some(1_450),
            summary: Some("What we did.".into())
        }]
    );
}

#[test]
fn keeps_a_snapshots_divider_and_marks_opencodes_summary_message() {
    let mut state = new_state();
    let info = |id: &str, extra: Value| with(json!({"id": id, "role": "assistant", "sessionID": "s1"}), extra);
    apply(&mut state, &Event::MessageCreated(Lifecycle { info: info("m1", json!({})), parts: None }));
    let compaction =
        json!({"id": "m1-c", "type": "compaction", "messageID": "m1", "sessionID": "s1", "tokensBefore": 181000, "tokensAfter": 34000});
    apply(&mut state, &Event::MessageUpdated(Lifecycle { info: info("m1", json!({})), parts: Some(vec![compaction]) }));
    apply(&mut state, &Event::MessageCreated(Lifecycle { info: info("m2", json!({})), parts: None }));
    apply(&mut state, &Event::MessageUpdated(Lifecycle { info: info("m2", json!({"compactionSummary": true})), parts: None }));

    assert_eq!(
        parts_of(&state, "m1"),
        [PartData::Compaction {
            id: "m1-c".into(),
            trigger: None,
            tokens_before: Some(181_000),
            tokens_after: Some(34_000),
            summary: None
        }]
    );
    assert!(state.messages[1].compaction_summary);
}

// ─── late-tool-update.test.ts ───────────────────────────────────────────────

fn tool(message: &str, id: &str, call: &str, state: Value) -> Event {
    Event::PartUpdated(
        json!({"id": id, "sessionID": "s1", "messageID": message, "type": "tool", "tool": "bash", "callID": call, "state": state}),
    )
}

fn late_base() -> StreamState {
    let mut state = new_state();
    for (id, role) in [("msg_001", "assistant"), ("msg_002", "user"), ("msg_003", "assistant")] {
        apply(&mut state, &Event::MessageCreated(Lifecycle { info: json!({"id": id, "sessionID": "s1", "role": role}), parts: None }));
    }
    state
}

fn tool_parts(state: &StreamState) -> Vec<&PartData> {
    state.messages.iter().flat_map(|m| m.parts.iter()).filter(|p| matches!(p, PartData::Tool { .. })).collect()
}

#[test]
fn updates_the_existing_part_of_the_older_message_not_appending() {
    let mut state = late_base();
    apply(&mut state, &tool("msg_001", "prt_1", "call_1", json!({"status": "running"})));
    apply(&mut state, &tool("msg_003", "prt_9", "call_9", json!({"status": "running"})));
    apply(&mut state, &tool("msg_001", "prt_1", "call_1", json!({"status": "completed", "output": "ok"})));
    assert_eq!(tool_parts(&state).len(), 2);
    assert_eq!(state.messages[0].parts.len(), 1);
    assert!(matches!(&state.messages[0].parts[0], PartData::Tool { state, .. } if state["status"] == "completed"));
}

#[test]
fn same_call_id_arriving_under_a_different_part_id_does_not_duplicate() {
    let mut state = late_base();
    apply(&mut state, &tool("msg_001", "prt_1", "call_1", json!({"status": "running"})));
    apply(&mut state, &tool("msg_001", "prt_1b", "call_1", json!({"status": "completed"})));
    assert_eq!(state.messages[0].parts.iter().filter(|p| matches!(p, PartData::Tool { .. })).count(), 1);
}
