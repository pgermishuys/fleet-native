//! The "Remembered" notice (component 9, piece M): `use-memory-notices.test.ts` under its names. Fleet's test hands the
//! composable a fake socket and fake calls; here an event goes through `State::dispatch` and Undo's calls go to a function
//! that records them.
use crate::fleet::events::{Event, Kind};
use crate::fleet::memory_notice::{Call, parse, run_undo, undo_calls};
use crate::fleet::state::Topic;
use crate::tests::app;
use serde_json::{Value, json};

fn note(over: Value) -> Value {
    let mut n = json!({
        "id": "1a2b3c4d", "list": "machine", "text": "WebFetch can't read github.com pages; use gh.", "kind": "learned",
        "repository": null, "updated": "2026-09-27T08:00:00Z",
    });
    for (k, v) in over.as_object().unwrap() {
        n[k] = v.clone();
    }
    n
}

fn save(rt: &mut rune::Runtime<crate::State>, payload: Value) {
    rt.update(move |s, _| s.dispatch(&Topic::Sessions, &[Event::Other(Kind::MemorySaved, payload)]));
}

fn first(rt: &rune::Runtime<crate::State>) -> crate::fleet::notices::Notice {
    rt.state.notices.notices()[0].clone()
}

// use-memory-notices.test.ts: "shows a saved note with Undo for six seconds, with the time left as a bar"
#[test]
fn shows_a_saved_note_with_undo_for_six_seconds_with_the_time_left_as_a_bar() {
    let mut rt = app();
    save(&mut rt, json!({"note": note(json!({})), "repositoryName": null, "previous": null}));
    let shown = first(&rt);
    assert_eq!(shown.title, "Remembered for this machine");
    assert_eq!(shown.body.as_deref(), Some("WebFetch can't read github.com pages; use gh."));
    assert_eq!(shown.hold_secs, Some(6.0));
    assert!(shown.countdown);
    assert_eq!(shown.chip, None);
    assert_eq!(shown.actions.iter().map(|a| a.label.as_str()).collect::<Vec<_>>(), ["Undo"]);
    assert_eq!(shown.expires_secs, Some(120.0));
}

// use-memory-notices.test.ts: "names the repository a repository note is for"
#[test]
fn names_the_repository_a_repository_note_is_for() {
    let mut rt = app();
    let n = note(json!({"list": "repository", "repository": "/src/example"}));
    save(&mut rt, json!({"note": n, "repositoryName": "example", "previous": null}));
    assert_eq!(first(&rt).title, "Remembered for example");
}

fn saved(note: Value, previous: Option<Value>) -> crate::fleet::memory_notice::Saved {
    parse(&json!({"note": note, "repositoryName": null, "previous": previous})).unwrap()
}

type Seen = (String, String, Option<Value>);

fn record(saved: &crate::fleet::memory_notice::Saved, answer: Result<(u16, String), String>) -> (Vec<Seen>, Result<(), String>) {
    let mut seen = Vec::new();
    let r = run_undo(saved, |c: &Call| {
        seen.push((c.method.to_string(), c.path.clone(), c.body.clone()));
        answer.clone()
    });
    (seen, r)
}

// use-memory-notices.test.ts: "Undo forgets a new note and closes the notice"
#[test]
fn undo_forgets_a_new_note_and_closes_the_notice() {
    let mut rt = app();
    let payload = json!({"note": note(json!({})), "repositoryName": null, "previous": null});
    save(&mut rt, payload.clone());
    let (calls, result) = record(&parse(&payload).unwrap(), Ok((204, String::new())));
    assert_eq!(calls, [("DELETE".to_string(), "/api/memory/notes/1a2b3c4d".to_string(), None)]);
    assert_eq!(result, Ok(()));
    let id = first(&rt).id;
    rt.update(move |s, _| s.memory_undone(&id, Ok(())));
    assert!(rt.state.notices.notices().is_empty());
}

// use-memory-notices.test.ts: "Undo puts back the note a correction replaced"
#[test]
fn undo_puts_back_the_note_a_correction_replaced() {
    let previous = note(json!({"text": "Client tests run with bun run test:unit."}));
    let s = saved(note(json!({"text": "Client tests run with bun run test."})), Some(previous));
    let calls = undo_calls(&s);
    assert_eq!(calls.len(), 1, "no forget");
    assert_eq!((calls[0].method, calls[0].path.as_str()), ("PUT", "/api/memory/notes/1a2b3c4d"));
    assert_eq!(calls[0].body, Some(json!({"text": "Client tests run with bun run test:unit."})));
}

#[test]
fn undo_of_a_note_that_replaced_another_forgets_it_and_adds_the_old_one_again() {
    let previous = note(json!({"id": "9f9f9f9f", "list": "repository", "repository": "/src/example", "text": "Old."}));
    let calls = undo_calls(&saved(note(json!({})), Some(previous)));
    assert_eq!(
        calls.iter().map(|c| (c.method, c.path.as_str())).collect::<Vec<_>>(),
        [("DELETE", "/api/memory/notes/1a2b3c4d"), ("POST", "/api/memory/notes")]
    );
    assert_eq!(calls[1].body, Some(json!({"list": "repository", "repository": "/src/example", "text": "Old."})));
}

// use-memory-notices.test.ts: "says so when Undo fails, and keeps the notice"
#[test]
fn says_so_when_undo_fails_and_keeps_the_notice() {
    let mut rt = app();
    let payload = json!({"note": note(json!({})), "repositoryName": null, "previous": null});
    save(&mut rt, payload.clone());
    let (_, result) = record(&parse(&payload).unwrap(), Err("Fleet isn't reachable.".into()));
    let why = result.unwrap_err();
    assert_eq!(why, "Fleet isn't reachable.");
    let id = first(&rt).id;
    rt.update(move |s, _| s.memory_undone(&id, Err(why)));
    let n = first(&rt);
    assert_eq!((n.title.as_str(), n.body.as_deref()), ("Couldn't undo it", Some("Fleet isn't reachable.")));
    assert!(n.actions.is_empty());
}

#[test]
fn a_refusal_says_the_servers_own_words_else_points_to_settings() {
    let s = saved(note(json!({})), None);
    let (_, r) = record(&s, Ok((404, r#"{"error":"No such note."}"#.into())));
    assert_eq!(r, Err("No such note.".into()));
    let (_, r) = record(&s, Ok((500, String::new())));
    assert_eq!(r, Err("Forget the note in Settings → Memory instead.".into()));
}

// use-memory-notices.test.ts: "ignores other events on the sessions topic"
#[test]
fn ignores_other_events_on_the_sessions_topic() {
    let mut rt = app();
    rt.update(|s, _| s.dispatch(&Topic::Sessions, &[Event::Other(Kind::SmartLinkUpdated, json!({"id": "x"}))]));
    assert!(rt.state.notices.notices().is_empty());
    save(&mut rt, json!({"note": {"text": "no id"}}));
    assert!(rt.state.notices.notices().is_empty(), "a payload without a note id is ignored");
    rt.update(|s, _| s.dispatch(&Topic::Session("s1".into()), &[Event::Other(Kind::MemorySaved, json!({"note": note(json!({}))}))]));
    assert!(rt.state.notices.notices().is_empty(), "only the sessions topic");
}

#[test]
fn the_undo_button_runs_undo_through_the_link_and_a_press_without_one_says_so() {
    let mut rt = app();
    save(&mut rt, json!({"note": note(json!({})), "repositoryName": null, "previous": null}));
    rt.update(|s, _| s.quiet.started = -10.0);
    for _ in 0..30 {
        rt.frame(1.0 / 20.0);
    }
    crate::tests::click(&mut rt, "notice-action-Undo");
    rt.frame(1.0 / 60.0);
    // No server is connected in a headless app: the card says why instead of closing.
    assert_eq!(first(&rt).title, "Couldn't undo it");
}
