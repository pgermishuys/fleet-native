//! `lib/__tests__/export-history.test.ts`, under the same names. The request is a stub that answers from a list.
use crate::conv::export_history::{HistoryError, encode_component, load_whole_history};
use crate::fleet::model::PartData;
use serde_json::{Value, json};

fn message(id: &str) -> Value {
    json!({ "id": id, "role": "user", "parts": [{ "type": "text", "kind": 0, "text": id }], "timestamp": "2026-10-20T10:00:00Z", "textContent": id })
}

fn page(ids: &[&str], has_more: bool) -> Result<(u16, String), String> {
    let body = json!({ "messages": ids.iter().map(|i| message(i)).collect::<Vec<_>>(), "pagination": { "hasMore": has_more, "oldestMessageId": ids.first() } });
    Ok((200, body.to_string()))
}

/// Runs the loader against `answers`, in order (the last repeats), and returns what it asked for and what it told.
fn run(
    session: &str,
    answers: Vec<Result<(u16, String), String>>,
) -> (Result<Vec<crate::fleet::model::Msg>, HistoryError>, Vec<String>, Vec<usize>) {
    let (mut asked, mut told) = (Vec::new(), Vec::new());
    let mut n = 0;
    let result = load_whole_history(
        session,
        &mut |path| {
            asked.push(path.to_string());
            n += 1;
            answers[(n - 1).min(answers.len() - 1)].clone()
        },
        &mut |count| told.push(count),
        &|| false,
    );
    (result, asked, told)
}

// loadWholeHistory: "pages back with the oldest id until nothing is left, oldest message first"
#[test]
fn pages_back_with_the_oldest_id_until_nothing_is_left_oldest_message_first() {
    let (result, asked, told) = run("s 1", vec![page(&["m3", "m4"], true), page(&["m1", "m2"], false)]);
    let all = result.unwrap();
    assert_eq!(all.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["m1", "m2", "m3", "m4"]);
    assert!(all.iter().all(|m| m.session_id == "s 1"));
    assert_eq!(told, [2, 4]);
    assert_eq!(asked[0], "/api/sessions/s%201/messages?limit=200");
    assert_eq!(asked[1], "/api/sessions/s%201/messages?limit=200&before=m3");
}

// "keeps what a tool call returned, which OpenCode 2 and Claude Code put on the part"
#[test]
fn keeps_what_a_tool_call_returned_which_opencode_2_and_claude_code_put_on_the_part() {
    let with_tool = json!({ "id": "m1", "role": "user", "timestamp": "2026-10-20T10:00:00Z", "textContent": "",
        "parts": [{ "type": "tool", "kind": 1, "toolCallId": "c1", "toolName": "shell", "arguments": { "command": "ls" }, "state": 2, "output": "a.ts" }] });
    let body = json!({ "messages": [with_tool], "pagination": { "hasMore": false, "oldestMessageId": "m1" } });
    let (result, ..) = run("s", vec![Ok((200, body.to_string()))]);
    let all = result.unwrap();
    let PartData::Tool { state, tool, call_id, .. } = &all[0].parts[0] else { panic!("a tool part") };
    assert_eq!((tool.as_str(), call_id.as_str()), ("shell", "c1"));
    assert_eq!(state["status"], "completed");
    assert_eq!(state["output"], "a.ts");
    assert_eq!(state["input"], json!({ "command": "ls" }));
}

// "fails with a message naming the page when one fails, instead of returning half"
#[test]
fn fails_with_a_message_naming_the_page_when_one_fails_instead_of_returning_half() {
    let (result, ..) = run("s", vec![page(&["m3"], true), Ok((502, String::new()))]);
    let Err(HistoryError::Failed(why)) = result else { panic!("a failure") };
    assert!(why.contains("error 502 after 1 messages"), "{why}");
}

// "fails when Fleet can't be reached"
#[test]
fn fails_when_fleet_cant_be_reached() {
    let (result, ..) = run("s", vec![Err("network".into())]);
    assert_eq!(result, Err(HistoryError::Failed("Could not reach Fleet to load the conversation (after 0 messages).".into())));
}

// "stops when the cursor doesn't move"
#[test]
fn stops_when_the_cursor_doesnt_move() {
    let (result, ..) = run("s", vec![page(&["m3"], true)]);
    let Err(HistoryError::Failed(why)) = result else { panic!("a failure") };
    assert!(why.contains("did not say where"), "{why}");
}

// Not in Fleet's file, the other two messages in `loadWholeHistory`
#[test]
fn says_so_when_a_page_cannot_be_read_and_stops_quietly_when_aborted() {
    let (result, ..) = run("s", vec![Ok((200, "not json".into()))]);
    let Err(HistoryError::Failed(why)) = result else { panic!("a failure") };
    assert!(why.contains("could not be read (after 0 messages)"), "{why}");
    let result = load_whole_history("s", &mut |_| Err("closed".into()), &mut |_| {}, &|| true);
    assert_eq!(result, Err(HistoryError::Aborted), "an aborted request shows nothing");
}

#[test]
fn a_tool_result_part_gives_its_call_the_output_and_a_message_seen_twice_counts_once() {
    let call = json!({ "id": "m1", "role": "assistant", "timestamp": "2026-10-20T10:00:00Z", "textContent": "", "agent": "build", "modelId": "m-1",
        "parts": [
            { "type": "tool", "kind": 1, "toolCallId": "c1", "toolName": "bash", "arguments": { "command": "ls" }, "state": 2 },
            { "type": "tool-result", "kind": 2, "toolCallId": "c1", "content": "{\"n\":1}" },
            { "type": "step-finish", "kind": 3, "cost": 0.5, "tokensInput": 10, "tokensOutput": 4, "tokensReasoning": 1 },
        ] });
    let body = json!({ "messages": [call.clone(), call], "pagination": { "hasMore": false } });
    let (result, ..) = run("s", vec![Ok((200, body.to_string()))]);
    let all = result.unwrap();
    assert_eq!(all.len(), 2, "the same page twice in one answer is not deduplicated within it, as in Fleet");
    let PartData::Tool { state, .. } = &all[0].parts[0] else { panic!("a tool part") };
    assert_eq!(state["output"], json!({ "n": 1 }), "JSON output is read");
    assert_eq!((all[0].cost, all[0].tokens.map(|t| t.output), all[0].model_id.as_deref()), (Some(0.5), Some(4), Some("m-1")));
    assert_eq!(all[0].parts.len(), 1, "the result part is not drawn");
}

#[test]
fn encodes_a_component_like_the_browser() {
    assert_eq!(encode_component("a b/c?d=é"), "a%20b%2Fc%3Fd%3D%C3%A9");
    assert_eq!(encode_component("ses_1-2.3~(x)!*'"), "ses_1-2.3~(x)!*'");
}
