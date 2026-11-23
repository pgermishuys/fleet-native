//! Component 15, piece C: the side conversation's live stream. "shows the side conversation's own messages, after what it
//! copied" (`SideConversationPanel.test.ts`) and the panel's own behaviour: its snapshot and events reach the panel and not the
//! main conversation, progress is reported only after the snapshot, the question shows before the server's echo, and a
//! reconnect listens again. The link's socket isn't faked here: the snapshot and events are handed to the state as the link does.
use super::side::{Fx, side_json};
use crate::fleet::events::Event;
use crate::fleet::reducer::create_state;
use crate::fleet::topics::Topics;
use serde_json::{Value, json};

fn msg(id: &str, role: &str, created: i64, text: &str) -> Value {
    json!({
        "info": {"id": id, "role": role, "sessionID": "side-1", "time": {"created": created, "completed": created + 1}},
        "parts": [{"type": "text", "id": format!("{id}-t"), "text": text}],
    })
}

/// What the side session holds: two messages copied from the session (up to `msg_0002`), then the question and its answer.
fn snapshot(activity: &str, answered: bool) -> Value {
    let mut messages = vec![
        msg("msg_0001", "user", 1000, "Copied question."),
        msg("msg_0002", "assistant", 2000, "Copied answer."),
        msg("msg_0003", "user", 3000, "What changed?"),
    ];
    if answered {
        messages.push(msg("msg_0004", "assistant", 4000, "Two files changed."));
    }
    json!({"session": {}, "messages": messages, "delegations": [], "activityStatus": activity, "lastEventId": 5})
}

fn deliver(fx: &mut Fx, snapshot: &Value) {
    let state = create_state(snapshot);
    fx.rt.update(move |s, tx| s.on_side_snapshot("side-1", state, tx));
    fx.rt.frame(1.0 / 60.0);
}

fn ids(fx: &Fx) -> Vec<String> {
    fx.rt.state.side_conv().0.messages.iter().map(|m| m.id.clone()).collect()
}

fn open_side() -> Fx {
    Fx::loaded(Some(side_json(false, None)))
}

// "shows the side conversation's own messages, after what it copied"
#[test]
fn shows_the_side_conversations_own_messages_after_what_it_copied() {
    let mut fx = open_side();
    deliver(&mut fx, &snapshot("idle", true));
    assert_eq!(ids(&fx), ["msg_0003", "msg_0004"], "the two it copied are its session's");
    assert!(fx.rt.presented("side-msg_0004").is_some(), "the answer is drawn");
    assert!(fx.rt.presented("side-msg_0002").is_none() && fx.rt.presented("side-msg_0001").is_none());
}

#[test]
fn a_snapshot_and_events_for_the_side_session_reach_the_panel_and_not_the_main_conversation() {
    let mut fx = open_side();
    let before: Vec<String> = fx.rt.state.session().messages.iter().map(|m| m.id.clone()).collect();
    deliver(&mut fx, &snapshot("busy", false));
    assert_eq!(ids(&fx), ["msg_0003"]);
    let answer = Event::from_wire(&json!({
        "type": "message.created",
        "properties": {"info": msg("msg_0004", "assistant", 4000, "")["info"].clone(), "parts": msg("msg_0004", "assistant", 4000, "Two files changed.")["parts"].clone()},
    }))
    .unwrap();
    fx.rt.update(move |s, tx| s.on_side_events("side-1", vec![answer], tx));
    fx.rt.frame(1.0 / 60.0);
    assert_eq!(ids(&fx), ["msg_0003", "msg_0004"]);
    let after: Vec<String> = fx.rt.state.session().messages.iter().map(|m| m.id.clone()).collect();
    assert_eq!(before, after, "the session's conversation is untouched");
    assert!(fx.rt.state.stream.as_ref().is_none_or(|st| st.messages.iter().all(|m| !m.id.starts_with("msg_000"))));
    // Another session's snapshot, or one for a side conversation that isn't the panel's, is dropped.
    let other = create_state(&snapshot("idle", true));
    fx.rt.update(move |s, tx| s.on_side_snapshot("side-9", other, tx));
    assert_eq!(ids(&fx), ["msg_0003", "msg_0004"]);
}

#[test]
fn events_before_the_snapshot_follow_it() {
    let mut fx = open_side();
    let tail = Event::from_wire(&json!({
        "type": "message.created",
        "properties": {"info": msg("msg_0004", "assistant", 4000, "")["info"].clone(), "parts": msg("msg_0004", "assistant", 4000, "Two files changed.")["parts"].clone()},
    }))
    .unwrap();
    fx.rt.update(move |s, tx| s.on_side_events("side-1", vec![tail], tx));
    assert!(!fx.st().reported, "nothing is reported before the snapshot");
    deliver(&mut fx, &snapshot("idle", false));
    assert_eq!(ids(&fx), ["msg_0003", "msg_0004"]);
}

#[test]
fn progress_is_reported_only_after_the_snapshot() {
    let mut fx = open_side();
    fx.rt.frame(1.0 / 60.0);
    assert!(!fx.st().reported && fx.st().latest_answer_id.is_none(), "an empty stream that hasn't loaded isn't \"no answer\"");
    deliver(&mut fx, &snapshot("busy", false));
    assert!(fx.st().reported && fx.st().working && fx.st().latest_answer_id.is_none());
    deliver(&mut fx, &snapshot("idle", true));
    assert!(fx.st().reported && !fx.st().working);
    assert_eq!(fx.st().latest_answer_id.as_deref(), Some("msg_0004"));
    assert_eq!(fx.st().latest_answer.as_deref(), Some("Two files changed."));
}

#[test]
fn the_question_shows_before_the_servers_echo() {
    let mut fx = Fx::start(None, None);
    let session = fx.id.clone();
    fx.ctx(move |s, tx| {
        s.ask_side(&session, "What changed?", Default::default(), tx, |_, _| {});
    });
    fx.wait("the side conversation to be made", |f| f.st().side.is_some());
    fx.wait("the question to show", |f| f.rt.state.side_conv().0.messages.iter().any(|m| m.id.starts_with("optimistic-")));
    assert_eq!(fx.rt.state.side_conv().0.messages.len(), 1, "shown as sent, before any snapshot");
    // The snapshot holds it under its own id: it is not drawn twice.
    let mut snap = snapshot("busy", false);
    snap["messages"] = json!([msg("msg_side_1", "user", 3000, "What changed?")]);
    deliver(&mut fx, &snap);
    assert_eq!(ids(&fx), ["msg_side_1"]);
}

#[test]
fn the_working_line_reads_the_side_conversation() {
    use crate::conv::working::Working;
    let mut fx = open_side();
    deliver(&mut fx, &snapshot("busy", false));
    assert!(matches!(fx.rt.state.side_working_text(), Working::Busy { .. }));
    assert_eq!(fx.rt.state.side_conv().0.status, crate::mock::Status::Working);
    assert!(fx.rt.presented("side-working-indicator").is_some(), "drawn in the panel");
    deliver(&mut fx, &snapshot("idle", true));
    assert!(fx.rt.presented("side-working-indicator").is_none());
}

#[test]
fn its_permission_asks_wait_in_the_panel() {
    let mut fx = open_side();
    deliver(&mut fx, &snapshot("busy", false));
    let ask = Event::Other(
        crate::fleet::events::Kind::PermissionAsked,
        json!({"id": "perm-1", "sessionId": "side-1", "kind": "shell", "tool": "bash", "title": "git push origin main",
               "always": ["git push *"], "askedAt": "2026-09-28T10:00:00Z"}),
    );
    fx.rt.update(move |s, tx| s.on_side_events("side-1", vec![ask], tx));
    fx.rt.frame(1.0 / 60.0);
    assert_eq!(fx.rt.state.asks.of("side-1").len(), 1, "kept for the side session");
    assert!(fx.rt.state.open_asks().is_empty(), "not the session's");
    assert!(fx.rt.presented("permission-allow-once-side").is_some(), "answerable in the panel");
    assert!(fx.rt.presented("permission-allow-once").is_none(), "its keys end in -side, so the session's own card keeps its own");
}

#[test]
fn the_link_listens_to_the_side_session_while_its_parent_is_open() {
    let mut fx = open_side();
    fx.rt.frame(1.0 / 60.0);
    assert_eq!(fx.rt.state.side_ui.live.watching().map(|(_, side)| side.as_str()), Some("side-1"));
    // The side conversation ends: the link lets go and the stream is forgotten.
    let owner = fx.id.clone();
    fx.ctx(move |s, _| s.discard_side(&owner));
    fx.rt.frame(1.0 / 60.0);
    assert!(fx.rt.state.side_ui.live.watching().is_none());
}

#[test]
fn a_reconnect_listens_again() {
    let mut topics = Topics::default();
    topics.set_open(Some("parent".into()));
    topics.mark_side("side-1", true);
    assert!(topics.watch("side-1"), "new to the connection: subscribed");
    assert!(topics.is_side("side-1"));
    // After a reconnect every topic is subscribed again, the side one too, and its snapshot goes to the panel.
    assert_eq!(topics.all(), ["parent", "side-1"]);
    topics.forget_snapshots();
    assert_eq!(topics.all(), ["parent", "side-1"]);
    topics.mark_side("side-1", false);
    assert!(topics.unwatch("side-1") && !topics.is_side("side-1"));
}

// Component 26, piece S (row 2.90): a side ask is answered with the keys, as the session's own is.
#[test]
fn a_side_ask_is_answered_by_the_keyboard() {
    use rune::Key;
    let mut fx = open_side();
    deliver(&mut fx, &snapshot("busy", false));
    let ask = |id: &str| {
        Event::Other(
            crate::fleet::events::Kind::PermissionAsked,
            json!({"id": id, "sessionId": "side-1", "kind": "shell", "tool": "bash", "title": "git push origin main",
                   "always": ["git push *"], "askedAt": "2026-09-28T10:00:00Z"}),
        )
    };
    let events = vec![ask("perm-1"), ask("perm-2")];
    fx.rt.update(move |s, tx| s.on_side_events("side-1", events, tx));
    fx.rt.frame(1.0 / 60.0);
    let keys: Vec<String> = fx.rt.elements().into_iter().filter_map(|e| e.key).filter(|k| k.starts_with("permission-allow-once")).collect();
    assert_eq!(keys, ["permission-allow-once-side", "permission-allow-once-side-perm-2"], "no two cards share a key");
    // The arrows move between the side card's choices, 2 answers the first ask, and only that one.
    crate::tests::click(&mut fx.rt, "permission-allow-once-side");
    crate::tests::press_key(&mut fx.rt, Key::Down, Default::default());
    assert_eq!(fx.rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key).as_deref(), Some("permission-allow-always-side"));
    crate::tests::press_key(&mut fx.rt, Key::Text("2".into()), Default::default());
    fx.rt.frame(1.0 / 60.0);
    let left: Vec<String> = fx.rt.state.asks.of("side-1").iter().map(|a| a.id.clone()).collect();
    assert_eq!(left, ["perm-2"], "the first side ask is answered, the second waits");
}
