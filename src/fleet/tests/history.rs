//! Older history (rows 6.1-6.4). The seven merge cases are weave-fleet's `history-merge.test.ts`, under its
//! names; the rest are new, because Fleet tests `loadOlder` through a browser socket.
use super::super::history::{Page, Pending, prepend_page};
use super::super::link::{Job, Link, Post};
use super::reconnect::{app, two_sessions};
use super::*;
use crate::fleet::model::Msg;
use crate::view::conversation::Older;
use crate::{State, spec};
use rune::{App, Env, Runtime, ScrollPos};
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tungstenite::Message;

// ─── history-merge.test.ts ──────────────────────────────────────────────────

fn accumulated(id: &str, role: Role, text: Option<&str>, created: Option<i64>) -> Msg {
    let mut m = Msg::new(id, "session-1", role);
    m.created_at = created;
    if let Some(t) = text {
        m.parts.push(PartData::Text { id: format!("{id}-text-1"), text: t.into() });
    }
    m
}

/// `createLifecyclePayload`.
fn payload(id: &str, role: &str, text: &str, created: i64) -> Value {
    json!({
        "info": {"id": id, "role": role, "sessionID": "session-1", "agent": null, "modelID": null, "parentID": null,
                 "time": {"created": created, "completed": null}, "cost": null, "tokens": null},
        "parts": [{"id": format!("{id}-text-1"), "sessionID": "session-1", "messageID": id, "type": "text", "text": text}],
    })
}

fn message_ids(messages: &[Msg]) -> Vec<&str> {
    messages.iter().map(|m| m.id.as_str()).collect()
}

fn first_text(m: &Msg) -> &str {
    match &m.parts[0] {
        PartData::Text { text, .. } => text,
        other => panic!("not text: {other:?}"),
    }
}

#[test]
fn prepends_older_page_before_current_messages() {
    let current = vec![
        accumulated("msg-3", Role::User, Some("third"), Some(3000)),
        accumulated("msg-4", Role::Assistant, Some("fourth"), Some(4000)),
    ];
    let page = [payload("msg-1", "user", "first", 1000), payload("msg-2", "assistant", "second", 2000)];
    let result = prepend_page(current, &page);
    assert_eq!(result.len(), 4);
    assert_eq!(message_ids(&result), ["msg-1", "msg-2", "msg-3", "msg-4"]);
}

#[test]
fn preserves_page_message_order_exactly() {
    // Page messages in a specific order, not sorted by timestamp.
    let page =
        [payload("msg-2", "assistant", "second", 2000), payload("msg-1", "user", "first", 1000), payload("msg-3", "user", "third", 3000)];
    let result = prepend_page(vec![], &page);
    assert_eq!(result.len(), 3);
    assert_eq!(message_ids(&result), ["msg-2", "msg-1", "msg-3"]);
}

#[test]
fn deduplicates_by_id_keeping_current_messages() {
    let current = vec![
        accumulated("msg-2", Role::Assistant, Some("live streamed text with more content"), Some(2000)),
        accumulated("msg-3", Role::User, Some("third"), Some(3000)),
    ];
    let page = [payload("msg-1", "user", "first", 1000), payload("msg-2", "assistant", "partial", 2000)];
    let result = prepend_page(current, &page);
    assert_eq!(result.len(), 3);
    assert_eq!(message_ids(&result), ["msg-1", "msg-2", "msg-3"]);
    let msg2 = result.iter().find(|m| m.id == "msg-2").unwrap();
    assert_eq!(first_text(msg2), "live streamed text with more content");
}

#[test]
fn does_not_reposition_existing_messages() {
    let current = vec![
        accumulated("user-1", Role::User, Some("first"), Some(1000)),
        accumulated("assistant-1", Role::Assistant, Some("reply"), None),
        accumulated("user-2", Role::User, Some("second"), Some(2000)),
    ];
    let result = prepend_page(current, &[payload("msg-0", "user", "oldest", 500)]);
    assert_eq!(result.len(), 4);
    assert_eq!(message_ids(&result), ["msg-0", "user-1", "assistant-1", "user-2"]);
}

#[test]
fn handles_empty_page() {
    let current = vec![accumulated("msg-1", Role::User, Some("first"), Some(1000))];
    let result = prepend_page(current.clone(), &[]);
    assert_eq!(result.len(), 1);
    assert_eq!(result, current);
}

#[test]
fn handles_empty_current_state() {
    let page = [payload("msg-1", "user", "first", 1000), payload("msg-2", "assistant", "second", 2000)];
    let result = prepend_page(vec![], &page);
    assert_eq!(result.len(), 2);
    assert_eq!(message_ids(&result), ["msg-1", "msg-2"]);
}

#[test]
fn handles_complete_overlap() {
    let current = vec![
        accumulated("msg-1", Role::User, Some("current version"), Some(1000)),
        accumulated("msg-2", Role::Assistant, Some("current version"), Some(2000)),
    ];
    let page = [payload("msg-1", "user", "page version", 1000), payload("msg-2", "assistant", "page version", 2000)];
    let result = prepend_page(current, &page);
    assert_eq!(result.len(), 2);
    assert_eq!(message_ids(&result), ["msg-1", "msg-2"]);
    assert_eq!(first_text(&result[0]), "current version");
    assert_eq!(first_text(&result[1]), "current version");
}

// ─── a page's payloads are read like a snapshot's ───────────────────────────

#[test]
fn a_page_converts_every_kind_of_part_and_the_messages_own_fields() {
    let page = [json!({
        "info": {"id": "msg-1", "role": "assistant", "sessionID": "session-1", "agent": "build", "modelID": "m-1", "parentID": "msg-0",
                 "time": {"created": 10, "completed": 20}, "cost": 0.5, "tokens": {"input": 3, "output": 4, "reasoning": 1},
                 "steered": true, "compactionSummary": true, "mod": {"kind": "sent", "name": "relay"}},
        "parts": [
            {"id": "p1", "type": "text", "text": "words"},
            {"id": "p2", "type": "reasoning", "text": "thinking", "summary": "short"},
            {"id": "p3", "type": "tool", "tool": "bash", "callID": "c1", "state": {"status": "completed"}},
            {"id": "p4", "type": "file", "mime": "image/png", "filename": "a.png", "url": "file:///a.png"},
            {"id": "p5", "type": "compaction", "trigger": "auto", "tokensBefore": 900, "tokensAfter": 100},
            {"id": "p6", "type": "step-start"},
            {"id": "p7", "type": "step-finish", "cost": 0.1},
        ],
    })];
    let result = prepend_page(vec![], &page);
    let m = &result[0];
    assert_eq!(m.parts.iter().map(PartData::id).collect::<Vec<_>>(), ["p1", "p2", "p3", "p4", "p5"], "step-* parts are dropped");
    assert!(matches!(&m.parts[1], PartData::Reasoning { summary: Some(s), .. } if s == "short"));
    assert!(matches!(&m.parts[2], PartData::Tool { tool, call_id, .. } if tool == "bash" && call_id == "c1"));
    assert!(matches!(&m.parts[4], PartData::Compaction { tokens_before: Some(900), tokens_after: Some(100), .. }));
    assert_eq!((m.created_at, m.completed_at), (Some(10), Some(20)));
    assert_eq!((m.agent.as_deref(), m.model_id.as_deref(), m.parent_id.as_deref()), (Some("build"), Some("m-1"), Some("msg-0")));
    assert_eq!(m.cost, Some(0.5));
    assert_eq!(m.tokens.map(|t| (t.input, t.output, t.reasoning)), Some((3, 4, 1)));
    assert!(m.steered && m.compaction_summary && m.mod_mark.is_some());
}

fn click(rt: &mut Runtime<State>, key: &str) {
    // The conversation sticks to its end; the button is at the top, so a person scrolls up to it.
    for _ in 0..6 {
        rt.wheel(640.0, 400.0, 600.0);
        rt.frame(1.0 / 60.0);
    }
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);
}

// ─── loadOlder, at the state ────────────────────────────────────────────────

fn page_value(ids: &[&str], cursor: Option<&str>) -> Page {
    let messages = ids.iter().enumerate().map(|(n, id)| payload(id, "user", id, n as i64)).collect();
    Page { messages, cursor: cursor.map(str::to_string), has_more: cursor.is_some() }
}

/// A runtime with a link that goes nowhere (its commands pile up unread) and a snapshot that says there is more.
fn app_with_history() -> (Runtime<State>, String) {
    let mut rt = app();
    let post = Post::new(|_| {});
    rt.update(move |s, tx| {
        s.link = Some(Link::start("http://127.0.0.1:9".into(), post));
        s.on_list(two_sessions(), tx);
    });
    let id = rt.state.open_id().unwrap();
    let mut snap =
        snapshot("idle", vec![life("msg-3", "user", 3, Some("third"), None), life("msg-4", "assistant", 4, Some("fourth"), None)]);
    snap["hasMore"] = json!(true);
    snap["cursor"] = json!("cursor-1");
    rt.update({
        let id = id.clone();
        move |s, tx| s.on_snapshot(&id, create_state(&snap), tx)
    });
    (rt, id)
}

/// What the conversation's scroll box reports: at `offset` of `max`, in a 532 px viewport.
fn reading(offset: f32, max: f32) -> ScrollPos {
    let mut at = ScrollPos::default();
    at.offset = [0.0, offset];
    at.max = [0.0, max];
    at.viewport = [836.0, 532.0];
    at.drawn = at.viewport;
    at
}

#[test]
fn the_box_before_the_conversation_was_laid_out_asks_for_nothing_where_the_conversation_lands_decides() {
    // The snapshot comes on another thread, so a frame may or may not be drawn while it is on its way. Either way the
    // reading `poll` sees first after it is the empty box's, at the top; only where the conversation lands may ask.
    for drawn_while_loading in [false, true] {
        for (end, asks) in [(300.0, false), (60.0, true)] {
            let mut rt = app();
            let post = Post::new(|_| {});
            rt.update(move |s, tx| {
                s.link = Some(Link::start("http://127.0.0.1:9".into(), post));
                s.on_list(two_sessions(), tx);
            });
            let id = rt.state.open_id().unwrap();
            rt.state.scroll_probe.set(Some(reading(0.0, 0.0)));
            if drawn_while_loading {
                rt.update(|s, tx| {
                    s.keep_scroll(tx);
                });
            }
            let mut snap = snapshot("idle", vec![life("msg-3", "user", 3, Some("third"), None)]);
            snap["hasMore"] = json!(true);
            snap["cursor"] = json!("cursor-1");
            rt.update(move |s, tx| s.on_snapshot(&id, create_state(&snap), tx));
            rt.update(|s, tx| {
                s.keep_scroll(tx);
            });
            assert_eq!(rt.state.older(), Some(Older::Ask), "drawn while loading {drawn_while_loading}: the empty box asked");
            rt.state.scroll_probe.set(Some(reading(end, end)));
            rt.update(|s, tx| {
                s.keep_scroll(tx);
            });
            let want = if asks { Older::Loading } else { Older::Ask };
            assert_eq!(rt.state.older(), Some(want), "drawn while loading {drawn_while_loading}, landed {end} px from the top");
        }
    }
}

fn stream(rt: &Runtime<State>) -> &StreamState {
    rt.state.stream.as_ref().unwrap()
}

#[test]
fn a_snapshot_says_whether_there_is_more_and_where_to_ask() {
    let (rt, _) = app_with_history();
    assert!(stream(&rt).has_more);
    assert_eq!(stream(&rt).cursor.as_deref(), Some("cursor-1"));
    assert!(!new_state().has_more && new_state().cursor.is_none());
    assert_eq!(rt.state.older(), Some(Older::Ask));
}

#[test]
fn a_page_joins_the_conversation_and_moves_the_cursor_on() {
    let (mut rt, id) = app_with_history();
    rt.update(|s, _| s.load_older());
    assert_eq!(rt.state.older(), Some(Older::Loading));
    rt.update(move |s, _| s.on_history_page(&id, "cursor-1", Ok(page_value(&["msg-1", "msg-2"], Some("cursor-2")))));
    assert_eq!(message_ids(&stream(&rt).messages), ["msg-1", "msg-2", "msg-3", "msg-4"]);
    assert_eq!(stream(&rt).cursor.as_deref(), Some("cursor-2"));
    assert_eq!(rt.state.older(), Some(Older::Ask));
    assert_eq!(rt.state.session().messages.len(), 4, "the conversation draws them");
    // The last page: no more, no button.
    let id = rt.state.open_id().unwrap();
    rt.update(move |s, _| s.on_history_page(&id, "cursor-2", Ok(page_value(&["msg-0"], None))));
    assert_eq!(rt.state.older(), None);
}

#[test]
fn load_older_is_ignored_while_loading_with_nothing_older_or_with_a_page_in_flight() {
    let (mut rt, _) = app_with_history();
    rt.update(|s, _| s.load_older());
    rt.update(|s, _| s.stream.as_mut().unwrap().cursor = Some("changed".into()));
    // A second press while a page is in flight does nothing: the flag is the guard.
    rt.update(|s, _| s.load_older());
    assert_eq!(stream(&rt).cursor.as_deref(), Some("changed"));
    rt.update(|s, _| {
        let st = s.stream.as_mut().unwrap();
        st.is_loading_older = false;
        st.has_more = false;
        s.load_older();
    });
    assert_eq!(rt.state.older(), None, "no older messages, nothing asked");
    rt.update(|s, _| {
        let st = s.stream.as_mut().unwrap();
        st.has_more = true;
        s.loading = true;
        s.load_older();
    });
    assert!(!stream(&rt).is_loading_older, "a kept copy's cursor may be stale: wait for the snapshot");
    rt.update(|s, _| {
        s.loading = false;
        s.stream.as_mut().unwrap().cursor = None;
        s.load_older();
    });
    assert!(!stream(&rt).is_loading_older, "no cursor, nothing to ask for");
}

#[test]
fn a_page_for_a_cursor_a_newer_snapshot_replaced_is_dropped() {
    let (mut rt, id) = app_with_history();
    rt.update(|s, _| s.load_older());
    // A reconnect's snapshot arrives first, with a cursor of its own.
    let mut snap = snapshot("idle", vec![life("msg-3", "user", 3, Some("third"), None)]);
    snap["hasMore"] = json!(true);
    snap["cursor"] = json!("cursor-new");
    rt.update({
        let id = id.clone();
        move |s, tx| s.on_snapshot(&id, create_state(&snap), tx)
    });
    rt.update(move |s, _| s.on_history_page(&id, "cursor-1", Ok(page_value(&["msg-1"], Some("cursor-2")))));
    assert_eq!(message_ids(&stream(&rt).messages), ["msg-3"]);
    assert_eq!(stream(&rt).cursor.as_deref(), Some("cursor-new"));
    assert_eq!(rt.state.older(), Some(Older::Ask), "not loading, and not failed either");
}

#[test]
fn a_failed_page_keeps_the_cursor_and_the_retry_asks_for_the_same_page() {
    let (mut rt, id) = app_with_history();
    rt.update(|s, _| s.load_older());
    rt.update({
        let id = id.clone();
        move |s, _| s.on_history_page(&id, "cursor-1", Err("timed out".into()))
    });
    assert_eq!(rt.state.older(), Some(Older::Failed));
    assert_eq!(stream(&rt).cursor.as_deref(), Some("cursor-1"));
    rt.update(|s, _| s.load_older());
    assert_eq!(rt.state.older(), Some(Older::Loading), "the failure mark clears while it tries again");
    rt.update(move |s, _| s.on_history_page(&id, "cursor-1", Ok(page_value(&["msg-1"], None))));
    assert_eq!(message_ids(&stream(&rt).messages), ["msg-1", "msg-3", "msg-4"]);
    assert_eq!(rt.state.older(), None);
}

#[test]
fn leaving_the_session_drops_the_answer() {
    let (mut rt, id) = app_with_history();
    rt.update(|s, _| s.load_older());
    rt.update(|s, tx| s.open_session(0, 1, tx));
    rt.update({
        let id = id.clone();
        move |s, _| s.on_history_page(&id, "cursor-1", Ok(page_value(&["msg-1"], Some("cursor-2"))))
    });
    assert!(rt.state.stream.as_ref().is_none_or(|s| s.messages.is_empty()), "the other session's conversation isn't touched");
    // Back again: what was kept starts from a plain button, and holds none of the dropped page.
    let at = rt.state.projects[0].sessions.iter().position(|s| s.id == id).unwrap();
    rt.update(move |s, tx| s.open_session(0, at, tx));
    assert_eq!(message_ids(&stream(&rt).messages), ["msg-3", "msg-4"]);
    assert_eq!(rt.state.older(), Some(Older::Ask));
}

#[test]
fn a_request_with_no_answer_fails_after_the_timeout() {
    let mut pending = Pending::with_timeout(Duration::from_millis(30));
    pending.add("7".into(), "session-1".into(), "cursor-1".into());
    assert!(pending.expired().is_empty());
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(pending.expired(), [("session-1".to_string(), "cursor-1".to_string())]);
    assert!(pending.expired().is_empty() && pending.complete("7").is_none(), "taken once");
    pending.add("8".into(), "session-1".into(), "cursor-2".into());
    assert_eq!(pending.complete("8"), Some(("session-1".into(), "cursor-2".into())));
    pending.add("9".into(), "session-1".into(), "cursor-3".into());
    assert_eq!(pending.drain().len(), 1, "a dropped connection fails what it carried");
}

#[test]
fn the_button_the_loading_line_and_the_retry_wording_are_drawn_with_their_keys() {
    let (mut rt, _) = app_with_history();
    rt.frame(1.0 / 60.0);
    assert!(rt.find("load-older").is_some());
    rt.update(|s, _| s.show_older(Older::Loading));
    rt.frame(1.0 / 60.0);
    assert!(rt.find("load-older").is_none() && rt.find("load-older-loading").is_some());
    rt.update(|s, _| s.show_older(Older::Failed));
    rt.frame(1.0 / 60.0);
    assert!(rt.find("load-older").is_some());
    assert_eq!(rt.state.older(), Some(Older::Failed));
}

// ─── on the wire ────────────────────────────────────────────────────────────

/// A stand-in for Fleet whose hub answers the subscription with a snapshot that has older history and
/// `LoadHistoryAsync` with pages: the first request is refused, the rest answered. Every request's cursor is recorded.
fn serve(mut stream: TcpStream, asked: Arc<Mutex<Vec<String>>>) {
    let mut peek = [0u8; 16];
    let n = stream.peek(&mut peek).unwrap_or(0);
    if peek[..n].starts_with(b"GET /hubs") {
        let Ok(mut ws) = tungstenite::accept(stream) else { return };
        while let Ok(Message::Text(text)) = ws.read() {
            for frame in text.split('\u{1e}').filter(|f| !f.is_empty()) {
                let call: Value = serde_json::from_str(frame).unwrap();
                let send = |ws: &mut tungstenite::WebSocket<TcpStream>, v: Value| {
                    let _ = ws.send(Message::text(format!("{v}\u{1e}")));
                };
                let id = &call["invocationId"];
                match call["target"].as_str() {
                    None if call["protocol"] == "json" => send(&mut ws, json!({})),
                    Some("SubscribeToSessionsTopicAsync") => send(&mut ws, json!({"type": 3, "invocationId": id, "result": null})),
                    Some("SubscribeToSessionAsync") => {
                        let mut snap = snapshot_of(&frames_of("hub-snapshot-after-turn.jsonl"));
                        snap["hasMore"] = json!(true);
                        snap["cursor"] = json!("cursor-1");
                        send(&mut ws, json!({"type": 3, "invocationId": id, "result": snap}));
                    }
                    Some("LoadHistoryAsync") => {
                        let cursor = call["arguments"][1].as_str().unwrap().to_string();
                        let mut seen = asked.lock().unwrap();
                        seen.push(cursor);
                        if seen.len() == 1 {
                            send(
                                &mut ws,
                                json!({"type": 3, "invocationId": id, "error": "An unexpected error occurred invoking 'LoadHistoryAsync'."}),
                            );
                        } else {
                            let page = json!({"messages": [payload("msg_0000old", "user", "An older question", 1)], "cursor": null, "hasMore": false});
                            send(&mut ws, json!({"type": 3, "invocationId": id, "result": page}));
                        }
                    }
                    _ => {}
                }
            }
        }
        return;
    }
    let heard = crate::test_http::read(&mut stream);
    if heard.method.is_empty() {
        return;
    }
    let (head, _body) = (heard.head, heard.body);
    let reply = |stream: &mut TcpStream, body: &str| {
        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(head.as_bytes());
    };
    if head.starts_with("POST /hubs/session-events/negotiate") {
        reply(&mut stream, "{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}");
    } else if head.starts_with("GET /api/sessions") {
        reply(&mut stream, &fixture("api-sessions.json"));
    }
}

#[test]
fn an_older_page_is_asked_for_over_the_hub_a_refusal_is_retried_and_the_page_joins_the_conversation() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let asked = Arc::new(Mutex::new(Vec::new()));
    let counted = asked.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let asked = counted.clone();
            std::thread::spawn(move || serve(stream, asked));
        }
    });
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    // A window the conversation fits in with room to spare, so nothing scrolls and nothing asks on its own: whether Fleet
    // asks by itself depends on where the box lands (within 100 px of the top), and at 760 px this one overflowed by 99 px
    // here, which another system's fonts can push past 100. The button asks, both times.
    let mut rt = App::new(spec()).headless_runtime((1280.0, 1400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(1.0 / 60.0);
    let link = Link::start(base, post);
    rt.update(move |s, _| s.link = Some(link));

    let step = |rt: &mut Runtime<State>, until: &dyn Fn(&State) -> bool, what: &str| {
        crate::test_wait::drive(rt, &queue, what, until, |s| {
            format!("{:?} {:?}, loading {}, {:?}", s.older(), s.notice, s.loading, s.connection)
        });
    };
    step(&mut rt, &|s| !s.loading && s.older().is_some(), "the snapshot with older history");
    click(&mut rt, "load-older");
    // The first ask is the one the server refuses.
    step(&mut rt, &|s| s.older() == Some(Older::Failed), "the refusal");
    rt.frame(1.0 / 60.0);
    click(&mut rt, "load-older");
    step(&mut rt, &|s| s.older().is_none(), "the page");
    assert_eq!(*asked.lock().unwrap(), ["cursor-1", "cursor-1"], "the retry asked for the same page");
    assert_eq!(rt.state.stream.as_ref().unwrap().messages[0].id, "msg_0000old");
    assert_eq!(rt.state.session().messages[0].id, "msg_0000old");
}
