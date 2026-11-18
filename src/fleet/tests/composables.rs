//! `use-session-retry.test.ts` and `use-session-interruption.test.ts`, on the state: what a load answers, what the
//! `session.retry` and `session.interrupted` frames do, and what Try now, Don't retry, Continue and Dismiss leave
//! behind. The requests run on a thread in the app, so each case hands the answer to the same handler the thread
//! would (`on_*_loaded`, `on_*_acted`). "Loads again after a reconnect" is `on_reconnected`, which loads the retry and
//! the interruption beside the queue; `queue.rs` shows that call against a server.
use super::reconnect::app;
use super::*;
use crate::fleet::rows::RowPatch;

fn retry_json() -> Value {
    json!({"dueAt": "2026-10-09T12:00:00Z", "attempt": 2, "kind": "usage_limit", "reason": "Window used up", "providerSaid": true})
}

fn interruption_json() -> Value {
    json!({"at": "2026-10-17T14:02:00Z", "waiting": false, "lastCall": "bash: bun run build", "changedFiles": 3})
}

fn patch(kind: &str, key: &str, session: &str, value: Value) -> RowPatch {
    RowPatch::from_wire(&json!({"type": kind, "properties": {"sessionId": session, key: value}})).unwrap()
}

/// The fixture's first session, open, as another process would have listed it.
fn open_app() -> (rune::Runtime<crate::State>, String) {
    let mut rt = app();
    rt.update(|s, tx| s.on_list(sessions::parse_list(&serde_json::from_str(&fixture("api-sessions.json")).unwrap(), 0), tx));
    let id = rt.state.open_id().unwrap();
    (rt, id)
}

mod use_session_retry {
    use super::*;

    // use-session-retry.test.ts: "has no retry when the session has none waiting"
    #[test]
    fn has_no_retry_when_the_session_has_none_waiting() {
        let (mut rt, id) = open_app();
        rt.update(move |s, _| {
            s.on_retry_loaded(&id, Ok((204, Value::Null)));
            assert_eq!(s.retry(), None);
        });
    }

    // use-session-retry.test.ts: "loads the waiting retry when the session opens"
    #[test]
    fn loads_the_waiting_retry_when_the_session_opens() {
        let (mut rt, id) = open_app();
        rt.update(move |s, _| {
            s.on_retry_loaded(&id, Ok((200, retry_json())));
            let got = s.retry().expect("a retry");
            assert_eq!((got.attempt, got.kind.as_str(), got.provider_said), (2, "usage_limit", true));
        });
    }

    // use-session-retry.test.ts: "follows session.retry events, including the one that takes it away"
    #[test]
    fn follows_session_retry_events_including_the_one_that_takes_it_away() {
        let (mut rt, id) = open_app();
        rt.update(move |s, _| {
            s.on_row_patch(patch("session.retry", "retry", &id, retry_json()));
            assert_eq!(s.retry().map(|r| r.reason.as_str()), Some("Window used up"));
            s.on_row_patch(patch("session.retry", "retry", &id, Value::Null));
            assert_eq!(s.retry(), None);
        });
    }

    // use-session-retry.test.ts: "ignores another session's retry and other events"
    #[test]
    fn ignores_another_sessions_retry_and_other_events() {
        let (mut rt, _) = open_app();
        rt.update(|s, _| {
            s.on_row_patch(patch("session.retry", "retry", "some-other-session", retry_json()));
            assert_eq!(s.retry(), None);
            // A frame that isn't a row patch never reaches the row at all.
            assert!(RowPatch::from_wire(&json!({"type": "session.queue", "properties": {"sessionId": "x", "items": []}})).is_none());
        });
    }

    // use-session-retry.test.ts: "clears the retry once Fleet sends it now or calls it off"
    #[test]
    fn clears_the_retry_once_fleet_sends_it_now_or_calls_it_off() {
        let (mut rt, id) = open_app();
        rt.update(move |s, _| {
            for send in [true, false] {
                s.on_row_patch(patch("session.retry", "retry", &id, retry_json()));
                s.on_retry_acted(&id, send, Ok((200, Value::Null)));
                assert_eq!(s.retry(), None, "send: {send}");
                assert_eq!(s.retry_act(), crate::fleet::retry::Act::default());
            }
        });
    }

    // use-session-retry.test.ts: "says so when Fleet refuses, and loads again"
    #[test]
    fn says_so_when_fleet_refuses_and_loads_again() {
        let (mut rt, id) = open_app();
        rt.update(move |s, _| {
            s.on_row_patch(patch("session.retry", "retry", &id, retry_json()));
            s.on_retry_acted(&id, true, Ok((500, Value::Null)));
            assert_eq!(s.retry_act().error.as_deref(), Some("Fleet couldn't try again now (HTTP 500)."));
            assert!(s.retry().is_some(), "a refused request leaves the retry in place");
            s.on_retry_acted(&id, false, Ok((500, Value::Null)));
            assert_eq!(s.retry_act().error.as_deref(), Some("Fleet couldn't stop the retry (HTTP 500)."));
        });
    }
}

mod use_session_interruption {
    use super::*;

    // use-session-interruption.test.ts: "has no interruption on a 204"
    #[test]
    fn has_no_interruption_on_a_204() {
        let (mut rt, id) = open_app();
        rt.update(move |s, _| {
            s.on_interruption_loaded(&id, Ok((204, Value::Null)));
            assert_eq!(s.interruption(), None);
        });
    }

    // use-session-interruption.test.ts: "loads the interruption on a 200 when the session opens"
    #[test]
    fn loads_the_interruption_on_a_200_when_the_session_opens() {
        let (mut rt, id) = open_app();
        rt.update(move |s, _| {
            s.on_interruption_loaded(&id, Ok((200, interruption_json())));
            let got = s.interruption().expect("an interruption");
            assert_eq!((got.last_call.as_deref(), got.changed_files, got.waiting), (Some("bash: bun run build"), Some(3), false));
        });
    }

    // use-session-interruption.test.ts: "follows session.interrupted events, including the one that clears it"
    #[test]
    fn follows_session_interrupted_events_including_the_one_that_clears_it() {
        let (mut rt, id) = open_app();
        rt.update(move |s, _| {
            s.on_row_patch(patch("session.interrupted", "interruption", &id, interruption_json()));
            assert_eq!(s.interruption().map(|i| i.at.as_str()), Some("2026-10-17T14:02:00Z"));
            s.on_row_patch(patch("session.interrupted", "interruption", &id, Value::Null));
            assert_eq!(s.interruption(), None);
        });
    }

    // use-session-interruption.test.ts: "ignores another session's event"
    #[test]
    fn ignores_another_sessions_event() {
        let (mut rt, _) = open_app();
        rt.update(|s, _| {
            s.on_row_patch(patch("session.interrupted", "interruption", "some-other-session", interruption_json()));
            assert_eq!(s.interruption(), None);
        });
    }

    // use-session-interruption.test.ts: "continues through the endpoint and clears the interruption"
    // use-session-interruption.test.ts: "dismisses through the endpoint and clears the interruption"
    #[test]
    fn continues_or_dismisses_through_the_endpoint_and_clears_the_interruption() {
        let (mut rt, id) = open_app();
        rt.update(move |s, _| {
            for send in [true, false] {
                s.on_row_patch(patch("session.interrupted", "interruption", &id, interruption_json()));
                s.on_interruption_acted(&id, send, Ok((if send { 202 } else { 204 }, Value::Null)));
                assert_eq!(s.interruption(), None, "send: {send}");
                assert_eq!(s.interruption_act(), crate::fleet::retry::Act::default());
            }
        });
    }

    // use-session-interruption.test.ts: "says so when Fleet refuses"
    #[test]
    fn says_so_when_fleet_refuses() {
        let (mut rt, id) = open_app();
        rt.update(move |s, _| {
            s.on_row_patch(patch("session.interrupted", "interruption", &id, interruption_json()));
            s.on_interruption_acted(&id, true, Ok((500, Value::Null)));
            assert_eq!(s.interruption_act().error.as_deref(), Some("Fleet couldn't continue the session (HTTP 500)."));
            assert!(s.interruption().is_some());
        });
    }
}

mod use_running_work {
    use super::*;

    // use-running-work.test.ts: "says why Fleet refused to stop it"
    #[test]
    fn says_why_fleet_refused_to_stop_it() {
        let (mut rt, _) = open_app();
        rt.update(|s, _| {
            s.on_work_stopped("w-shell", Ok((409, json!({"error": "It has already ended."}))));
            assert_eq!(s.work.stop_errors["w-shell"], "It has already ended.");
            // Without a word from Fleet, the status says what happened.
            s.on_work_stopped("w-shell", Ok((500, Value::Null)));
            assert_eq!(s.work.stop_errors["w-shell"], "It couldn't be stopped (HTTP 500).");
        });
    }
}

mod use_agent_browser {
    use super::*;
    use crate::fleet::events::{Event, Kind};

    fn step(session: &str, seq: i64) -> Event {
        Event::Other(
            Kind::BrowserStep,
            json!({"sessionId": session, "seq": seq, "kind": "tabs.open", "ok": true, "callId": "call_1", "tabId": "tab-1",
                "url": "http://localhost:4000/", "title": "Demo app"}),
        )
    }

    // use-agent-browser.test.ts: "applies browser.step events from the session's topic"
    #[test]
    fn applies_browser_step_events_from_the_sessions_topic() {
        let (mut rt, id) = open_app();
        rt.update(move |s, _| {
            assert!(s.apply_browser_event(&id, &step(&id, 1)));
            assert!(s.apply_browser_event(&id, &step(&id, 1)));
            assert_eq!(s.browser.count(&id, "call_1"), 1);
            assert_eq!(s.browser.of(&id, "call_1")[0].title.as_deref(), Some("Demo app"));
            assert_eq!(s.browser.tabs(&id).unwrap().tabs[0].title, "Demo app");
        });
    }

    // use-agent-browser.test.ts: "loads the agent's tabs and steps when the session opens"
    #[test]
    fn loads_the_agents_tabs_and_steps_when_the_session_opens() {
        let (mut rt, id) = open_app();
        rt.update(move |s, _| {
            let tab = json!({"id": "tab-1", "url": "http://localhost:4000/", "title": "Demo app", "loading": false, "canGoBack": false,
                "canGoForward": false, "generation": 1});
            s.on_browser_loaded(&id, &json!({"tabs": [tab], "focusedTabId": "tab-1", "steps": []}));
            let tabs = s.browser.tabs(&id).unwrap();
            assert_eq!(tabs.tabs.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["tab-1"]);
            assert_eq!(tabs.focused.as_deref(), Some("tab-1"));
            // An answer without them reads as none.
            s.on_browser_loaded(&id, &json!({}));
            assert!(s.browser.tabs(&id).unwrap().tabs.is_empty());
        });
    }

    // use-agent-browser.test.ts: "ignores other events and steps for another session"
    #[test]
    fn ignores_other_events_and_steps_for_another_session() {
        let (mut rt, id) = open_app();
        rt.update(move |s, _| {
            s.apply_browser_event(&id, &step("other", 1));
            let queue = Event::Other(Kind::SessionQueue, json!({"sessionId": id, "items": []}));
            assert!(!s.apply_browser_event(&id, &queue), "not a browser event");
            assert_eq!(s.browser.count(&id, "call_1"), 0);
        });
    }
}
