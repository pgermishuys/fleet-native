//! Optimistic prompts: the cases of weave-fleet's `domain-event-reducer.test.ts`
//! ("reconciles_optimistic_prompt…", "confirming_same_correlation_id_twice…"), `merge-messages.test.ts`
//! and `use-send-prompt.test.ts` (pending counts, reconciliation), under the same names, and the same
//! through the state.
use super::super::model::Msg;
use super::super::prompts::{Confirm, PromptStatus, Prompts, merge_by_timestamp};
use super::*;
use crate::{State, spec};
use rune::{App, Env, Runtime};

const SESSION: &str = "session-1";

fn app() -> Runtime<State> {
    let mut rt = App::new(spec()).headless_runtime((1280.0, 760.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(1.0 / 60.0);
    rt
}

fn user(id: &str, text: &str) -> Msg {
    state_with(vec![life(id, "user", 1, Some(text), Some(&format!("{id}-text")))]).messages.remove(0)
}

fn state_with(messages: Vec<Lifecycle>) -> StreamState {
    create_state(&snapshot("idle", messages))
}

fn confirm(correlation: &str, event: i64, server_id: &str) -> Confirm {
    Confirm { correlation_id: Some(correlation.into()), event_id: Some(event), server_message_id: Some(server_id.into()) }
}

/// A prompt sent as `send_prompt` does, and its correlation id.
fn send(prompts: &mut Prompts, text: &str) -> String {
    prompts.add(SESSION, text, 1_000, 0.0).unwrap()
}

fn committed(correlation: &str, id: &str, text: &str) -> Event {
    Event::PromptCommitted { life: life(id, "user", 1_000, Some(text), None), correlation_id: Some(correlation.into()), event_id: Some(31) }
}

// ─── domain-event-reducer.test.ts ───────────────────────────────────────────

#[test]
fn reconciles_optimistic_prompt_when_committed_event_has_matching_correlation_id() {
    let mut rt = app();
    rt.update(|s, tx| s.on_list(sessions::parse_list(&serde_json::from_str(&fixture("api-sessions.json")).unwrap(), 0), tx));
    let id = rt.state.open_id().unwrap();
    rt.update({
        let id = id.clone();
        move |s, tx| s.on_snapshot(&id, new_state(), tx)
    });
    rt.update({
        let id = id.clone();
        move |s, _| s.send_to_server(&id, "Ship the feature")
    });
    // No link in a test: the registry is driven as `send_to_server` drives it.
    let correlation = rt.state.prompts.add(&id, "Ship the feature", 1_000, 0.0).unwrap();
    assert_eq!(rt.state.prompts.of(&id).len(), 1);
    assert_eq!(rt.state.prompts.of(&id)[0].status, PromptStatus::Pending);
    assert!(rt.state.prompts.has_pending(&id));

    rt.update({
        let id = id.clone();
        move |s, tx| s.on_events(&id, vec![committed(&correlation, "user-server-id", "Ship the feature")], tx)
    });

    let stream = rt.state.stream.as_ref().unwrap();
    assert_eq!(stream.messages.len(), 1);
    assert_eq!(stream.messages[0].id, "user-server-id");
    // Confirmed, with the server's ids; the conversation holds the message, so reconciliation has let the prompt go.
    assert!(!rt.state.prompts.has_pending(&id));
    assert!(rt.state.prompts.of(&id).iter().all(|p| p.status == PromptStatus::Confirmed));
    let shown: Vec<_> = rt.state.session().messages.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(shown, ["user-server-id"], "the message shows once");
}

#[test]
fn confirming_same_correlation_id_twice_is_idempotent() {
    let mut prompts = Prompts::default();
    let correlation = send(&mut prompts, "Do it once");
    prompts.confirm(SESSION, &confirm(&correlation, 31, "user-server-id"));
    prompts.confirm(SESSION, &confirm(&correlation, 31, "user-server-id"));

    let sent = prompts.of(SESSION);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].id, "user-server-id");
    assert_eq!(sent[0].server_message_id.as_deref(), Some("user-server-id"));
    assert_eq!(sent[0].correlation_id, correlation);
    assert_eq!(sent[0].event_id, Some(31));
    assert_eq!(sent[0].status, PromptStatus::Confirmed);
    assert!(!prompts.has_pending(SESSION));
}

// ─── merge-messages.test.ts ─────────────────────────────────────────────────

fn ids_of(v: &[(&str, Option<i64>)]) -> Vec<String> {
    v.iter().map(|(id, _)| id.to_string()).collect()
}

#[test]
fn should_concatenate_delivered_then_optimistic() {
    let merged = merge_by_timestamp(vec![("d1", Some(100)), ("d2", Some(300))], vec![("o1", Some(200))]);
    assert_eq!(ids_of(&merged), ["d1", "d2", "o1"]);
}

#[test]
fn should_preserve_delivered_order() {
    let merged = merge_by_timestamp(vec![("d1", Some(100)), ("d2", Some(100))], vec![("o1", Some(100))]);
    assert_eq!(ids_of(&merged), ["d1", "d2", "o1"]);
}

#[test]
fn should_preserve_optimistic_order() {
    let merged = merge_by_timestamp(vec![], vec![("o1", Some(100)), ("o2", Some(100)), ("o3", Some(100))]);
    assert_eq!(ids_of(&merged), ["o1", "o2", "o3"]);
}

#[test]
fn should_place_optimistic_after_delivered_regardless_of_timestamp() {
    let merged = merge_by_timestamp(vec![("d1", Some(300)), ("d2", None)], vec![("o1", Some(100))]);
    assert_eq!(ids_of(&merged), ["d1", "d2", "o1"]);
}

#[test]
fn should_handle_empty_delivered_array() {
    let merged = merge_by_timestamp(vec![], vec![("o1", Some(100)), ("o2", Some(200))]);
    assert_eq!(ids_of(&merged), ["o1", "o2"]);
}

#[test]
fn should_handle_empty_optimistic_array() {
    let merged = merge_by_timestamp(vec![("d1", Some(100)), ("d2", Some(200))], vec![]);
    assert_eq!(ids_of(&merged), ["d1", "d2"]);
}

#[test]
fn should_handle_both_arrays_empty() {
    let merged: Vec<(&str, Option<i64>)> = merge_by_timestamp(vec![], vec![]);
    assert!(merged.is_empty());
}

// ─── use-send-prompt.test.ts ────────────────────────────────────────────────

#[test]
fn stops_counting_a_prompt_as_pending_when_reconciliation_removes_it_before_confirmation() {
    let mut prompts = Prompts::default();
    let correlation = send(&mut prompts, "Hello");
    // The delivered message carries the server's id, so it matches by text.
    prompts.reconcile(SESSION, &[user("user-server-id", "Hello")]);
    assert!(prompts.of(SESSION).is_empty());
    assert!(!prompts.has_pending(SESSION));
    // The late confirmation finds nothing to confirm.
    prompts.confirm(
        SESSION,
        &Confirm { correlation_id: Some(correlation), server_message_id: Some("user-server-id".into()), ..Default::default() },
    );
    assert!(!prompts.has_pending(SESSION));
}

#[test]
fn stops_counting_a_prompt_as_pending_when_sent_prompts_are_cleared_before_confirmation() {
    let mut prompts = Prompts::default();
    send(&mut prompts, "Hello");
    prompts.clear(SESSION);
    assert!(!prompts.has_pending(SESSION));
}

#[test]
fn does_not_count_a_confirmed_prompt_twice_when_reconciliation_removes_it() {
    let mut prompts = Prompts::default();
    let first = send(&mut prompts, "Hello");
    let second = send(&mut prompts, "Second");
    prompts.confirm(SESSION, &Confirm { correlation_id: Some(first), ..Default::default() });
    prompts.reconcile(SESSION, &[user("user-server-id", "Hello")]);
    // Only the second prompt is still waiting for its confirmation.
    assert!(prompts.has_pending(SESSION));
    prompts.confirm(SESSION, &Confirm { correlation_id: Some(second), ..Default::default() });
    assert!(!prompts.has_pending(SESSION));
}

#[test]
fn waits_while_the_historys_copy_has_no_text_yet() {
    let mut prompts = Prompts::default();
    send(&mut prompts, "Fix the login redirect");
    let textless = state_with(vec![life("msg_server", "user", 1, None, None)]).messages;
    prompts.reconcile(SESSION, &textless);
    assert_eq!(prompts.of(SESSION).len(), 1);
}

#[test]
fn shows_nothing_for_an_empty_message() {
    let mut prompts = Prompts::default();
    assert!(prompts.add(SESSION, "   ", 0, 0.0).is_none());
    assert!(prompts.of(SESSION).is_empty());
}

// ─── the 15 s, and what the conversation shows ──────────────────────────────

#[test]
fn a_prompt_not_confirmed_in_15_seconds_needs_a_retry_and_a_confirmed_one_does_not() {
    let mut prompts = Prompts::default();
    send(&mut prompts, "never answered");
    let answered = prompts.add(SESSION, "answered", 1_000, 5.0).unwrap();
    prompts.confirm(SESSION, &Confirm { correlation_id: Some(answered), ..Default::default() });
    assert!(!prompts.expire(14.9));
    assert!(prompts.expire(15.0));
    assert!(!prompts.expire(60.0), "nothing more to change");
    let status: Vec<_> = prompts.of(SESSION).iter().map(|p| p.status).collect();
    assert_eq!(status, [PromptStatus::NeedsRetry, PromptStatus::Confirmed]);
    assert!(!prompts.has_pending(SESSION));
}

#[test]
fn a_refused_prompt_leaves_the_conversation_and_its_text_returns_to_the_composer() {
    let mut rt = app();
    rt.update(|s, tx| s.on_list(sessions::parse_list(&serde_json::from_str(&fixture("api-sessions.json")).unwrap(), 0), tx));
    let id = rt.state.open_id().unwrap();
    rt.update({
        let id = id.clone();
        move |s, tx| s.on_snapshot(&id, new_state(), tx)
    });
    let correlation = rt.state.prompts.add(&id, "Ship the feature", 1_000, 0.0).unwrap();
    rt.update({
        let id = id.clone();
        move |s, tx| s.on_prompt_refused(&id, &correlation, "HTTP 500".into(), tx)
    });
    assert!(rt.state.prompts.of(&id).is_empty());
    assert_eq!(rt.state.draft.text, "Ship the feature");
    assert_eq!(rt.state.composer_error.as_deref(), Some("HTTP 500"));
    assert_eq!(rt.state.notice, None);
}

#[test]
fn a_prompt_never_shows_twice_after_a_reconnect_snapshot() {
    let mut rt = app();
    rt.update(|s, tx| s.on_list(sessions::parse_list(&serde_json::from_str(&fixture("api-sessions.json")).unwrap(), 0), tx));
    let id = rt.state.open_id().unwrap();
    let held = |rt: &Runtime<State>| rt.state.session().messages.iter().map(|m| m.id.clone()).collect::<Vec<_>>();
    rt.update({
        let id = id.clone();
        move |s, tx| s.on_snapshot(&id, new_state(), tx)
    });
    let correlation = rt.state.prompts.add(&id, "Ship the feature", 1_000, 0.0).unwrap();
    rt.update(|s, _| s.sync_open());
    assert_eq!(held(&rt), [format!("optimistic-{correlation}")]);

    // The connection drops before the confirmation; the snapshot after it already holds the message.
    let snap = snapshot("idle", vec![life("user-server-id", "user", 1, Some("Ship the feature"), Some("t"))]);
    rt.update({
        let id = id.clone();
        move |s, tx| s.on_snapshot(&id, create_state(&snap), tx)
    });
    assert_eq!(held(&rt), ["user-server-id"]);
    assert!(rt.state.prompts.of(&id).is_empty());
}

#[test]
fn a_prompt_follows_the_delivered_messages_and_says_so_when_it_needs_a_retry() {
    let mut rt = app();
    rt.update(|s, tx| s.on_list(sessions::parse_list(&serde_json::from_str(&fixture("api-sessions.json")).unwrap(), 0), tx));
    let id = rt.state.open_id().unwrap();
    // A delivered conversation whose messages differ from what is sent, so nothing matches by text.
    let snap = snapshot("idle", vec![life("msg_1", "assistant", 1, Some("Anything else?"), Some("t"))]);
    rt.update({
        let id = id.clone();
        move |s, tx| s.on_snapshot(&id, create_state(&snap), tx)
    });
    rt.state.prompts.add(&id, "No thanks", 1_000, 0.0);
    rt.update(|s, _| s.sync_open());
    assert_eq!(rt.state.session().messages.len(), 2);
    assert!(!rt.state.session().messages[1].needs_retry);
    rt.state.prompts.expire(16.0);
    rt.update(|s, _| s.sync_open());
    assert!(rt.state.session().messages[1].needs_retry);
    assert_eq!(rt.state.session().messages[0].id, "msg_1");
}

#[test]
fn a_new_reply_clears_the_prompts_that_were_waiting() {
    let mut prompts = Prompts::default();
    prompts.on_messages(SESSION, &[]);
    send(&mut prompts, "Hello");
    let reply = state_with(vec![life("msg_a", "assistant", 2, Some("Hi"), Some("t"))]).messages;
    prompts.on_messages(SESSION, &reply);
    assert!(prompts.of(SESSION).is_empty());
}
