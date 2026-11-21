//! Component 23, piece C, the feed's rules without a server: `machine-feed.test.ts`'s hub cases (the ones 17 left) and
//! `inbox-feed.test.ts`'s `findPendingQuestion`, under their names. The clock is the caller's, so every case here is exact.
use crate::fleet::mission::feed::{FOLD, Feed, POLL, SAFETY};
use crate::fleet::mission::read::{find_pending_question, parse_list, parse_now, refusal};
use crate::fleet::mission::{Ask, FeedStatus, NowKind};
use serde_json::json;

/// A feed that has read once and heard the machine (the hub not yet up).
fn started() -> Feed {
    let mut feed = Feed::new();
    assert!(feed.start(0.0), "the first read goes out at once");
    assert!(!feed.read_done(Ok(1_000)));
    feed
}

// ─── machine-feed.test.ts ───────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn reads_once_for_a_burst_of_events_and_again_for_an_event_that_came_in_during_a_read() {
    let mut feed = started();
    assert!(feed.hub_connected());
    for at in [10.0, 10.05, 10.1, 10.2] {
        feed.event(at);
    }
    assert!(!feed.due(10.0 + FOLD - 0.01), "the burst waits its fold");
    assert!(feed.due(10.0 + FOLD), "and is one read");
    assert!(!feed.due(10.0 + FOLD + 0.1), "no second read for the same burst");

    // An event comes while that read is under way: it is read again when the read ends, not before.
    feed.event(11.0);
    assert!(!feed.due(11.0 + FOLD), "a read is under way: the event waits for it");
    assert!(feed.read_done(Ok(2_000)), "and reads again at once when it ends");
    assert!(!feed.read_done(Ok(3_000)), "that was the only one owed");
}

#[test]
fn only_reads_on_the_safety_refresh_while_the_hub_is_live() {
    let mut feed = started();
    assert!(feed.hub_connected());
    assert!(!feed.due(POLL), "a live hub is not polled");
    assert!(!feed.due(SAFETY - 0.1));
    assert!(feed.due(SAFETY), "the safety read comes once a minute");
    feed.read_done(Ok(2_000));
    assert!(!feed.due(SAFETY + POLL));
    assert!(feed.due(2.0 * SAFETY));
    assert_eq!(feed.status, FeedStatus::Live);
}

#[test]
fn closes_a_hub_that_finished_connecting_after_the_feed_stopped_and_reads_no_more() {
    let mut feed = started();
    feed.event(5.0);
    feed.stop();
    assert!(!feed.hub_connected(), "the caller is told to let the connection go");
    assert!(!feed.due(1_000.0), "no timer is left");
    feed.event(6.0);
    feed.hub_lost(6.0);
    assert!(!feed.due(1_000.0), "nothing it is told afterwards makes it read");
    assert!(!feed.refresh());
    assert!(feed.next_deadline().is_none());
}

#[test]
fn says_connecting_until_the_first_answer_then_live_with_the_hub_and_polling_without() {
    let mut feed = Feed::new();
    assert_eq!(feed.status, FeedStatus::Connecting);
    feed.start(0.0);
    assert!(feed.hub_connected());
    assert_eq!(feed.status, FeedStatus::Live, "the hub being up is enough");
    feed.read_done(Ok(1_000));
    assert_eq!((feed.status, feed.last_heard_at), (FeedStatus::Live, Some(1_000)));

    feed.hub_lost(20.0);
    assert_eq!(feed.status, FeedStatus::Polling);
    assert!(!feed.due(20.0 + POLL - 0.1));
    assert!(feed.due(20.0 + POLL), "while the hub is down it reads every 15 s");
    feed.read_done(Ok(2_000));
    assert!(feed.due(20.0 + 2.0 * POLL));
}

#[test]
fn marks_the_machine_unreachable_when_a_read_fails_and_says_polling_when_it_answers_again() {
    let mut feed = started();
    feed.hub_lost(1.0);
    feed.due(1.0 + POLL);
    feed.read_done(Err(Some(refusal(500))));
    assert_eq!((feed.status, feed.error.as_deref(), feed.last_heard_at), (FeedStatus::Unreachable, Some("It answered 500."), Some(1_000)));
    feed.hub_lost(40.0);
    assert_eq!(feed.status, FeedStatus::Unreachable, "a hub that is down doesn't hide that the machine is");
    assert!(feed.due(1.0 + 2.0 * POLL));
    feed.read_done(Ok(5_000));
    assert_eq!((feed.status, feed.error.as_deref()), (FeedStatus::Polling, None));
}

#[test]
fn shares_one_read_between_callers_who_ask_while_it_is_under_way() {
    let mut feed = Feed::new();
    feed.start(0.0);
    assert!(!feed.refresh(), "the first read is still under way: this call shares it");
    feed.read_done(Ok(1));
    assert!(feed.refresh(), "after an answer or a nudge the machine is read at once");
    assert!(!feed.refresh());
}

// ─── inbox-feed.test.ts: the words and the question ─────────────────────────────────────────────────────────────────

#[test]
fn says_what_the_machine_said_for_a_refused_read() {
    assert_eq!(refusal(500), "It answered 500.");
    assert_eq!(refusal(401), "This phone's key for this machine stopped working.");
    assert_eq!(parse_list(Ok((503, "{}".into())), 0).err(), Some(Some("It answered 503.".to_string())));
    assert_eq!(parse_list(Err("no route".into()), 0).err(), Some(None), "no answer says no words");
    assert_eq!(parse_list(Ok((200, "not json".into())), 0).err(), Some(None));
}

/// A question call as the messages route writes it (Fleet's own shape): `state` is `ToolUseState`, 1 running, 2 completed, 3 error.
fn question_call(call: &str, status: &str, questions: usize) -> serde_json::Value {
    let list: Vec<_> = (0..questions)
        .map(|n| json!({"header": "Retries", "question": format!("Question {n}?"), "options": [{"label": "Three", "description": ""}]}))
        .collect();
    let state = match status {
        "running" => 1,
        "completed" => 2,
        _ => 3,
    };
    json!({"type": "tool", "toolCallId": call, "toolName": "question", "state": state, "arguments": {"questions": list}})
}

fn page(calls: Vec<serde_json::Value>) -> serde_json::Value {
    let messages: Vec<_> = calls
        .into_iter()
        .enumerate()
        .map(|(n, part)| json!({"id": format!("m{n}"), "role": "assistant", "timestamp": "2026-11-21T16:39:01.184+00:00", "textContent": "", "parts": [part]}))
        .collect();
    json!({ "messages": messages })
}

#[test]
fn ignores_answered_questions() {
    let waiting = page(vec![question_call("call-1", "completed", 1), question_call("call-2", "running", 2)]);
    let Some(Ask::Question { request_id, question, more }) = find_pending_question(&waiting) else { panic!("a question waits") };
    assert_eq!((request_id.as_str(), question.question.as_str(), more), ("call-2", "Question 0?", 1));

    // A call answered anywhere counts as answered, even when another part of it still says running.
    let both = page(vec![question_call("call-3", "running", 1), question_call("call-3", "completed", 1)]);
    assert!(find_pending_question(&both).is_none());
    assert!(find_pending_question(&page(vec![question_call("call-4", "error", 1)])).is_none());
    assert!(find_pending_question(&json!({})).is_none());
}

#[test]
fn reads_a_now_line_of_each_kind_and_skips_what_it_cannot_read() {
    let lines = parse_now(&json!({
        "a": {"text": "bash · cargo test", "kind": "tool", "at": "2026-11-21T10:00:00Z"},
        "b": {"text": "…", "kind": "thinking", "at": "x"},
        "c": {"text": "Done.", "kind": "reply"},
        "d": {"text": "x", "kind": "mystery"},
        "e": 4,
    }));
    let mut keys: Vec<_> = lines.keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["a", "b", "c"]);
    assert_eq!((lines["a"].kind, lines["a"].text.as_str()), (NowKind::Tool, "bash · cargo test"));
    assert_eq!(parse_now(&json!([])).len(), 0);
}
