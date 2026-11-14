//! The event catalogue (`events.rs`): every type of Fleet's `DomainEvent` union is read, none is
//! dropped, and what the reducer doesn't apply reaches its subscribers. New tests: Fleet has none
//! for the dispatch itself.
use super::super::events::{Event, Kind};
use super::super::reducer::{apply, create_state};
use super::super::signalr::{self, Frame};
use super::super::state::Topic;
use super::{fixture, new_state};
use crate::State;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

fn catalogue() -> Vec<(String, Option<i64>, Value)> {
    fixture("hub-events-all.jsonl")
        .lines()
        .flat_map(signalr::parse)
        .filter_map(|f| match f {
            Frame::Event { topic, event_id, data } => Some((topic, event_id, data)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_union_has_64_event_types_and_the_fixture_has_one_frame_for_each() {
    // 64 `type` strings in domain-events.ts's `DomainEvent` (the file also has seven part types).
    assert_eq!(Kind::ALL.len(), 64);
    let wire: HashSet<&str> = Kind::ALL.iter().map(|k| k.wire()).collect();
    assert_eq!(wire.len(), 64, "no two kinds share a name");
    let in_fixture: Vec<String> = catalogue().iter().map(|(_, _, d)| d["type"].as_str().unwrap().to_string()).collect();
    assert_eq!(in_fixture.len(), 64);
    assert_eq!(in_fixture.iter().map(String::as_str).collect::<HashSet<_>>(), wire);
}

#[test]
fn every_type_in_the_union_parses_into_its_own_variant() {
    for (_, _, data) in catalogue() {
        let name = data["type"].as_str().unwrap();
        let event = Event::from_wire(&data).unwrap_or_else(|| panic!("{name} didn't parse"));
        // `user.prompt.committed` is still a plain message update until piece D (optimistic prompts)
        // gives it a variant of its own.
        let want = if name == "user.prompt.committed" { "message.updated" } else { name };
        assert_eq!(event.kind().map(Kind::wire), Some(want), "{name} came out as {event:?}");
        // The reducer's nine have fields of their own; the others carry the payload untouched.
        match &event {
            Event::Other(_, props) => assert_eq!(props, &data["properties"], "{name}"),
            Event::Unknown { .. } => panic!("{name} is in the union but read as unknown"),
            _ => assert!(event.applied(), "{name}"),
        }
    }
}

#[test]
fn a_type_this_build_does_not_know_is_kept_as_unknown() {
    let data = json!({"type": "future.thing", "eventId": null, "properties": {"a": 1}});
    assert_eq!(Event::from_wire(&data), Some(Event::Unknown { kind: "future.thing".into(), properties: json!({"a": 1}) }));
    assert!(!Event::from_wire(&data).unwrap().applied());
    // No payload at all is still an event.
    assert!(matches!(
        Event::from_wire(&json!({"type": "harness.catalog_changed"})),
        Some(Event::Other(Kind::HarnessCatalogChanged, Value::Null))
    ));
}

#[test]
fn a_reasoning_delta_is_kept_rather_than_dropped() {
    let data = json!({"type": "message.part.delta.streamed", "properties": {"sessionID": "s", "messageID": "m", "partID": "p", "field": "reasoning", "delta": "x"}});
    assert!(matches!(Event::from_wire(&data), Some(Event::Other(Kind::MessagePartDeltaStreamed, _))));
}

#[test]
fn a_malformed_frame_between_two_good_ones_skips_only_itself() {
    let message = |id: &str, text: &str| json!({"type": "message.created", "properties": {"info": {"id": id, "role": "user", "sessionID": "session-1", "time": {"created": 1}}, "parts": [{"type": "text", "id": "t", "text": text}]}});
    let frames = [
        message("msg_1", "first"),
        json!({"type": "message.created", "properties": {"nope": true}}), // an applied type without its fields
        json!({"properties": {}}),                                        // no type
        json!("not an event"),
        message("msg_2", "second"),
    ];
    let events: Vec<Event> = frames.iter().filter_map(Event::from_wire).collect();
    assert_eq!(events.len(), 2);
    let mut state = new_state();
    for e in &events {
        apply(&mut state, e);
    }
    assert_eq!(state.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["msg_1", "msg_2"]);
}

#[test]
fn the_hubs_event_id_moves_the_cursor_when_it_is_a_number() {
    let frames = catalogue();
    assert!(frames.iter().all(|(_, id, _)| id.is_some_and(|n| n < 0)), "the fixture carries negative ids");
    let mut state = create_state(&json!({"session": {}, "messages": [], "lastEventId": 7}));
    assert_eq!(state.last_event_id, Some(7));
    apply(&mut state, &Event::Cursor(-3));
    assert_eq!(state.last_event_id, Some(-3));
    // A frame whose id argument is null has none to give.
    let null = signalr::parse(r#"{"type":1,"target":"Event","arguments":["sessions",null,{"type":"session_created","properties":{}}]}"#);
    assert!(matches!(null[0], Frame::Event { event_id: None, .. }));
}

#[test]
fn subscribers_get_what_the_reducer_leaves_alone_and_only_what_they_asked_for() {
    let mut state = State::default();
    let (all, only) = (Arc::new(Mutex::new(Vec::new())), Arc::new(Mutex::new(Vec::new())));
    let a = all.clone();
    state.subscribe(None, move |t, e| a.lock().unwrap().push((t.clone(), e.kind())));
    let o = only.clone();
    state.subscribe(Some(Kind::PermissionAsked), move |_, e| o.lock().unwrap().push(e.kind()));

    let events: Vec<Event> = catalogue().iter().filter_map(|(_, _, d)| Event::from_wire(d)).collect();
    state.dispatch(&Topic::Session("s".into()), &events);
    state.dispatch(&Topic::Sessions, &[Event::Cursor(1), Event::from_wire(&json!({"type": "session_tokens", "properties": {}})).unwrap()]);

    let all = all.lock().unwrap();
    assert_eq!(all.len(), 65, "all 64, then the sessions event; the cursor mark isn't an event");
    assert_eq!(all[64], (Topic::Sessions, Some(Kind::SessionTokens)));
    assert_eq!(*only.lock().unwrap(), [Some(Kind::PermissionAsked)]);
}
