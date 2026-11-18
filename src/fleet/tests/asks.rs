//! Permission asks live (component 4, piece B): `use-session-permissions.test.ts` under its names, and what a card's
//! keys and buttons send, read from a fake server. The wording and the drawing are `asks::tests` and `tests_asks`.
use super::super::events::{Event, Kind};
use super::super::link::{Job, Link, Post};
use super::super::sessions;
use super::super::state::Topic;
use super::reconnect::app;
use super::*;
use crate::State;
use crate::asks::permission::to_permission_ask;
use rune::{Key, KeyEvent, Mods, Runtime};
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

const FRAME: f64 = 1.0 / 60.0;

fn ask(id: &str, session: &str) -> Value {
    json!({"id": id, "sessionId": session, "kind": "shell", "tool": "bash", "title": "git push origin main",
           "always": ["git push *"], "askedAt": "2026-09-28T10:00:00Z"})
}

fn asked(payload: Value) -> Event {
    Event::Other(Kind::PermissionAsked, payload)
}

fn replied(id: &str) -> Event {
    Event::Other(Kind::PermissionReplied, json!({"id": id, "sessionId": "s1", "reply": "once"}))
}

fn titles(s: &State, session: &str) -> Vec<Option<String>> {
    s.asks.of(session).iter().map(|a| a.title.clone()).collect()
}

// ─── the store ──────────────────────────────────────────────────────────────

// use-session-permissions.test.ts: "adds an ask when one is asked and drops it when it's answered or gone"
#[test]
fn adds_an_ask_when_one_is_asked_and_drops_it_when_its_answered_or_gone() {
    let mut rt = app();
    rt.update(|s, _| {
        let topic = Topic::Session("s-events".into());
        s.dispatch(&topic, &[asked(ask("per_1", "s-events"))]);
        s.dispatch(&topic, &[asked(ask("per_1", "s-events"))]);
        assert_eq!(s.asks.of("s-events").len(), 1, "an id held already is not added twice");
        let held = &s.asks.of("s-events")[0];
        assert_eq!(
            (held.kind, held.always.clone(), held.subagent.clone()),
            (crate::conv::tools::PermissionKind::Shell, vec!["git push *".to_string()], None)
        );
        s.dispatch(&topic, &[replied("per_1")]);
        assert!(s.asks.of("s-events").is_empty());
    });
}

// use-session-permissions.test.ts: "ignores an ask it can't read"
#[test]
fn ignores_an_ask_it_cant_read() {
    let mut rt = app();
    rt.update(|s, _| {
        s.dispatch(&Topic::Session("s-bad".into()), &[asked(json!({"id": "per_2"}))]);
        assert!(s.asks.of("s-bad").is_empty());
        // Another topic's event is not this session's.
        s.dispatch(&Topic::Sessions, &[asked(ask("per_3", "s-bad"))]);
        assert!(s.asks.of("s-bad").is_empty());
    });
}

#[test]
fn a_load_that_an_event_overtook_is_dropped() {
    let mut asks = crate::fleet::asks::Asks::default();
    let first = asks.begin_load("s1");
    asks.asked("s1", to_permission_ask(&ask("per_1", "s1")).unwrap());
    asks.loaded("s1", first, Vec::new());
    assert_eq!(asks.of("s1").len(), 1, "the load was asked for before the event, so the event wins");
    let second = asks.begin_load("s1");
    asks.loaded("s1", second, Vec::new());
    assert!(asks.of("s1").is_empty());
}

#[test]
fn an_answered_ask_leaves_every_sessions_list() {
    let mut asks = crate::fleet::asks::Asks::default();
    asks.asked("s-parent", to_permission_ask(&ask("per_1", "s-child")).unwrap());
    asks.asked("s-child", to_permission_ask(&ask("per_1", "s-child")).unwrap());
    asks.forget("per_1");
    assert!(asks.of("s-parent").is_empty() && asks.of("s-child").is_empty());
}

#[test]
fn the_errors_text_is_fleets_own_or_a_plain_fallback() {
    use crate::fleet::asks::error_text;
    assert_eq!(error_text(&json!({"error": "The harness is restarting."}), 500), "The harness is restarting.");
    assert_eq!(error_text(&json!({"detail": "No."}), 409), "No.");
    assert_eq!(error_text(&json!({"title": "Conflict", "error": "  "}), 409), "Conflict");
    assert_eq!(error_text(&Value::Null, 502), "The answer didn't reach the agent (HTTP 502).");
}

// ─── the requests, read from a fake server ──────────────────────────────────

#[derive(Default)]
struct Seen {
    /// Method, path and body of every call to a permission route.
    calls: Vec<(String, String, Value)>,
    /// What `GET …/permissions` answers.
    asks: Value,
    gets: usize,
    /// What an answer gets instead of 204.
    refuse: Option<(u16, Value)>,
}

fn serve(mut stream: TcpStream, seen: Arc<Mutex<Seen>>) {
    let heard = crate::test_http::read(&mut stream);
    if heard.method.is_empty() {
        return;
    }
    let (head, raw) = (heard.head, heard.body);
    let request = head.lines().next().unwrap_or("").to_string();
    let body: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
    let mut parts = request.split(' ');
    let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
    let reply = |stream: &mut TcpStream, status: &str, body: &str| {
        let _ = stream.write_all(
            format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        );
    };
    let mut seen = seen.lock().unwrap();
    if path.starts_with("/hubs/session-events/negotiate") {
        reply(&mut stream, "200 OK", "{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}");
    } else if path.starts_with("/api/sessions?") || path == "/api/sessions" {
        reply(&mut stream, "200 OK", &fixture("api-sessions.json"));
    } else if method == "GET" && path.ends_with("/permissions") {
        seen.gets += 1;
        let answer = seen.asks.to_string();
        reply(&mut stream, "200 OK", &answer);
    } else if method == "POST" && path.contains("/permissions/") {
        seen.calls.push((method, path, body));
        match seen.refuse.clone() {
            Some((code, answer)) => reply(&mut stream, &format!("{code} Refused"), &answer.to_string()),
            None => reply(&mut stream, "204 No Content", ""),
        }
    } else {
        reply(&mut stream, "404 Not Found", "{}");
    }
}

struct Live {
    rt: Runtime<State>,
    id: String,
    jobs: std::sync::mpsc::Receiver<Job>,
    seen: Arc<Mutex<Seen>>,
}

impl Live {
    /// The fixture's first session open on a server that holds `asks`.
    fn start(asks: Value) -> Live {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Seen { asks, ..Default::default() }));
        let shared = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let seen = shared.clone();
                std::thread::spawn(move || serve(stream, seen));
            }
        });
        let (tx, jobs) = channel::<Job>();
        let tx = Mutex::new(tx);
        let post = Post::new(move |job| {
            let _ = tx.lock().unwrap().send(job);
        });
        let mut rt = app();
        let link = Link::start(base, post);
        rt.update(move |s, tx| {
            s.link = Some(link);
            s.on_list(sessions::parse_list(&serde_json::from_str(&fixture("api-sessions.json")).unwrap(), 0), tx);
        });
        let id = rt.state.open_id().unwrap();
        let mut live = Live { rt, id, jobs, seen };
        live.rt.frame(FRAME);
        live
    }

    fn wait(&mut self, until: &dyn Fn(&Live) -> bool, what: &str) {
        crate::test_wait::until(
            self,
            what,
            |l| {
                if until(l) {
                    return true;
                }
                while let Ok(job) = l.jobs.try_recv() {
                    l.rt.update(job);
                }
                l.rt.frame(FRAME);
                until(l)
            },
            |l| format!("{}; {} calls reached the fake server", crate::test_wait::app_state(&l.rt.state), l.calls().len()),
        );
    }

    fn calls(&self) -> Vec<(String, String, Value)> {
        self.seen.lock().unwrap().calls.clone()
    }

    fn event(&mut self, event: Event) {
        let id = self.id.clone();
        self.rt.update(move |s, tx| s.on_events(&id, vec![event], tx));
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    fn key(&mut self, key: Key) {
        self.rt.key(KeyEvent { key, mods: Mods::default() });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    fn focused(&self) -> Option<String> {
        self.rt.elements().iter().find(|e| e.focused).and_then(|e| e.key.clone())
    }
}

// use-session-permissions.test.ts: "loads what waits when the session opens, and again after a reconnect"
#[test]
fn loads_what_waits_when_the_session_opens_and_again_after_a_reconnect() {
    let mut live = Live::start(json!([ask("per_1", "s1")]));
    live.rt.update(|s, _| {
        let id = s.open_id().unwrap();
        s.load_asks(&id);
    });
    let id = live.id.clone();
    live.wait(&|l| titles(&l.rt.state, &id) == [Some("git push origin main".to_string())], "the asks to load");

    let before = live.seen.lock().unwrap().gets;
    live.seen.lock().unwrap().asks = json!([]);
    live.rt.update(|s, _| s.on_reconnected());
    live.wait(&|l| l.rt.state.asks.of(&l.id).is_empty() && l.seen.lock().unwrap().gets > before, "the asks to load again");
}

// use-session-permissions.test.ts: "answers on the session whose harness asked, and the card goes"
#[test]
fn answers_on_the_session_whose_harness_asked_and_the_card_goes() {
    let mut live = Live::start(json!([]));
    let mut sub = ask("per_1", "s-child");
    sub["subagent"] = json!("subagent");
    live.event(asked(sub));
    assert_eq!(live.rt.state.open_asks().len(), 1, "a sub-agent's ask shows on the session it works for");
    live.rt.update(|s, _| {
        s.deny = rune::TextEdit::new("  Not yet  ");
        let held = s.open_asks()[0].clone();
        s.answer_ask(&held, crate::fleet::asks::Reply::Reject);
    });
    live.wait(&|l| l.rt.state.open_asks().is_empty(), "the card to go");
    let calls = live.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].1, "/api/sessions/s-child/permissions/per_1");
    assert_eq!(calls[0].2, json!({"reply": "reject", "message": "Not yet"}));
}

// use-session-permissions.test.ts: "keeps the card and says why when the answer didn't reach the agent"
#[test]
fn keeps_the_card_and_says_why_when_the_answer_didnt_reach_the_agent() {
    let mut live = Live::start(json!([]));
    live.seen.lock().unwrap().refuse = Some((500, json!({"error": "The harness is restarting."})));
    live.event(asked(ask("per_1", "s-fail")));
    live.rt.update(|s, _| {
        let held = s.open_asks()[0].clone();
        s.answer_ask(&held, crate::fleet::asks::Reply::Once);
    });
    live.wait(&|l| l.rt.state.ask_act("per_1").error.is_some(), "the refusal");
    assert_eq!(live.rt.state.ask_act("per_1").error.as_deref(), Some("The harness is restarting."));
    assert_eq!(live.rt.state.open_asks().len(), 1);
    assert!(!live.rt.state.ask_act("per_1").busy, "the buttons work again");
    assert!(live.rt.find("permission-error").is_some(), "the card says why");
}

#[test]
fn an_ask_that_no_longer_waits_goes_on_a_404_and_a_refusal_without_words_says_its_status() {
    let mut live = Live::start(json!([]));
    live.seen.lock().unwrap().refuse = Some((404, Value::Null));
    live.event(asked(ask("per_1", "s1")));
    live.rt.update(|s, _| {
        let held = s.open_asks()[0].clone();
        s.answer_ask(&held, crate::fleet::asks::Reply::Once);
    });
    live.wait(&|l| l.rt.state.open_asks().is_empty(), "the card to go");

    live.seen.lock().unwrap().refuse = Some((502, Value::Null));
    live.event(asked(ask("per_2", "s1")));
    live.rt.update(|s, _| {
        let held = s.open_asks()[0].clone();
        s.answer_ask(&held, crate::fleet::asks::Reply::Once);
    });
    live.wait(&|l| l.rt.state.ask_act("per_2").error.is_some(), "the refusal");
    assert_eq!(live.rt.state.ask_act("per_2").error.as_deref(), Some("The answer didn't reach the agent (HTTP 502)."));
}

// ─── the card's keys and buttons ────────────────────────────────────────────

// PermissionCard.test.ts: "takes keyboard focus on its first choice and moves it with the arrow keys"
#[test]
fn takes_keyboard_focus_on_its_first_choice_and_moves_it_with_the_arrow_keys() {
    let mut live = Live::start(json!([]));
    live.event(asked(ask("per_1", "s1")));
    live.rt.frame(FRAME);
    assert_eq!(live.focused().as_deref(), Some("permission-allow-once"));
    live.key(Key::Down);
    assert_eq!(live.focused().as_deref(), Some("permission-allow-always"));
    live.key(Key::Down);
    assert_eq!(live.focused().as_deref(), Some("permission-allow-once"), "it wraps round");
    live.key(Key::Up);
    assert_eq!(live.focused().as_deref(), Some("permission-allow-always"));
}

// PermissionCard.test.ts: "leaves focus alone while the user types in the composer"
#[test]
fn leaves_focus_alone_while_the_user_types_in_the_composer() {
    let mut live = Live::start(json!([]));
    super::super::super::tests::click(&mut live.rt, "composer");
    live.rt.key(KeyEvent { key: Key::Text("half a sentence".into()), mods: Mods::default() });
    live.rt.frame(FRAME);
    assert_eq!(live.focused().as_deref(), Some("composer"));
    live.event(asked(ask("per_1", "s1")));
    live.rt.frame(FRAME);
    assert_eq!(live.focused().as_deref(), Some("composer"), "the sentence is not interrupted");
    assert_eq!(live.rt.state.draft.text, "half a sentence");
    // And a number key goes to the composer, not to the card.
    live.key(Key::Text("2".into()));
    assert!(live.calls().is_empty());
}

// PermissionCard.test.ts: "answers once, or for the rest of the session"
#[test]
fn answers_once_or_for_the_rest_of_the_session() {
    let mut live = Live::start(json!([]));
    live.event(asked(ask("per_1", "s1")));
    super::super::super::tests::click(&mut live.rt, "permission-allow-once");
    live.wait(&|l| l.calls().len() == 1 && l.rt.state.open_asks().is_empty(), "the first answer");
    live.event(asked(ask("per_2", "s1")));
    super::super::super::tests::click(&mut live.rt, "permission-allow-always");
    live.wait(&|l| l.calls().len() == 2, "the second answer");
    let calls = live.calls();
    assert_eq!(calls[0].2, json!({"reply": "once", "message": null}));
    assert_eq!(calls[1].2, json!({"reply": "always", "message": null}));
}

// PermissionCard.test.ts: "answers by number key, and denies with Escape"
#[test]
fn answers_by_number_key_and_denies_with_escape() {
    let mut live = Live::start(json!([]));
    live.event(asked(ask("per_1", "s1")));
    live.rt.frame(FRAME);
    live.key(Key::Text("2".into()));
    live.wait(&|l| l.calls().len() == 1 && l.rt.state.open_asks().is_empty(), "the first answer");
    live.event(asked(ask("per_2", "s1")));
    live.rt.frame(FRAME);
    live.key(Key::Escape);
    live.wait(&|l| l.calls().len() == 2, "the second answer");
    let calls = live.calls();
    assert_eq!(calls[0].2, json!({"reply": "always", "message": null}));
    assert_eq!(calls[1].2, json!({"reply": "reject", "message": null}));
}

// PermissionCard.test.ts: "denies with the words typed for the agent on Enter"
#[test]
fn denies_with_the_words_typed_for_the_agent_on_enter() {
    let mut live = Live::start(json!([]));
    live.event(asked(ask("per_1", "s1")));
    live.rt.frame(FRAME);
    // 3 puts the caret in the field instead of answering.
    live.key(Key::Text("3".into()));
    assert_eq!(live.focused().as_deref(), Some("permission-deny-input"));
    assert!(live.calls().is_empty());
    live.key(Key::Text("Push after the review".into()));
    live.key(Key::Enter);
    live.wait(&|l| l.calls().len() == 1, "the answer");
    assert_eq!(live.calls()[0].2, json!({"reply": "reject", "message": "Push after the review"}));
}

#[test]
fn clicking_the_deny_row_puts_the_caret_in_the_field_and_clicking_deny_denies() {
    let mut live = Live::start(json!([]));
    live.event(asked(ask("per_1", "s1")));
    super::super::super::tests::click(&mut live.rt, "permission-deny-row");
    live.rt.frame(FRAME);
    assert_eq!(live.focused().as_deref(), Some("permission-deny-input"));
    assert!(live.calls().is_empty());
    super::super::super::tests::click(&mut live.rt, "permission-deny");
    live.wait(&|l| l.calls().len() == 1, "the answer");
    assert_eq!(live.calls()[0].2, json!({"reply": "reject", "message": null}));
}

// PermissionCard.test.ts: "answers 1, 2 and 3 as once, always and reject, and Esc doesn't answer"
#[test]
fn answers_1_2_and_3_as_once_always_and_reject_and_esc_doesnt_answer() {
    let mut live = Live::start(json!([]));
    let handoff = json!({"id": "fleet-handoff-1", "sessionId": "s1", "kind": "other", "tool": "fleet_session_start", "title": "Start a session on taron",
        "always": [], "handoff": {"machine": "taron", "os": "windows", "level": "edits"}});
    live.event(asked(handoff));
    live.rt.frame(FRAME);
    live.key(Key::Escape);
    assert!(live.calls().is_empty(), "Esc doesn't answer a hand-off");
    assert_eq!(live.rt.state.open_asks().len(), 1);
    live.key(Key::Text("3".into()));
    live.wait(&|l| l.calls().len() == 1 && l.rt.state.open_asks().is_empty(), "reject");
    // The card went with the answer; ask again for the second.
    live.event(asked(json!({"id": "fleet-handoff-2", "sessionId": "s1", "kind": "other", "tool": "fleet_session_start", "always": [],
        "handoff": {"machine": "taron"}})));
    live.rt.frame(FRAME);
    live.key(Key::Text("2".into()));
    live.wait(&|l| l.calls().len() == 2, "always");
    let calls = live.calls();
    assert_eq!(calls[0].2, json!({"reply": "reject", "message": null}), "a hand-off sends no words");
    assert_eq!(calls[1].2, json!({"reply": "always", "message": null}));
}

#[test]
fn a_key_with_a_modifier_is_not_the_cards() {
    let mut live = Live::start(json!([]));
    live.event(asked(ask("per_1", "s1")));
    live.rt.frame(FRAME);
    live.rt.key(KeyEvent { key: Key::Command('1'), mods: Mods { cmd: true, ctrl: true, ..Mods::default() } });
    live.rt.frame(FRAME);
    assert!(live.calls().is_empty());
    assert_eq!(live.rt.state.open_asks().len(), 1);
}

#[test]
fn while_an_answer_is_on_its_way_the_choices_are_off_and_a_second_press_sends_nothing() {
    let mut live = Live::start(json!([]));
    live.event(asked(ask("per_1", "s1")));
    live.rt.update(|s, _| {
        let held = s.open_asks()[0].clone();
        s.answer_ask(&held, crate::fleet::asks::Reply::Once);
        s.answer_ask(&held, crate::fleet::asks::Reply::Always);
        assert!(s.ask_act("per_1").busy);
    });
    live.rt.frame(FRAME);
    live.wait(&|l| l.rt.state.open_asks().is_empty(), "the card to go");
    assert_eq!(live.calls().len(), 1, "one answer, not two");
}

// use-session-permissions.test.ts: "follows the session it's shown for"
// Fleet unsubscribes from the old session's topic and subscribes to the new one's; here the hub does that (`link.rs`,
// tested with the stream in `tests::wire`), so what is left to check is the state's side: a session's asks come in on its
// own topic, and the card list is the open session's, back at once when you return.
#[test]
fn follows_the_session_its_shown_for() {
    let mut rt = app();
    rt.update(|s, _| {
        s.dispatch(&Topic::Session("s-a".into()), &[asked(ask("per_a", "s-a"))]);
        s.dispatch(&Topic::Session("s-b".into()), &[asked(ask("per_b", "s-b"))]);
        s.route = crate::fleet::store::Route::Session("s-a".into());
        assert_eq!(s.open_asks().iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["per_a"]);
        s.route = crate::fleet::store::Route::Session("s-b".into());
        assert_eq!(s.open_asks().iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["per_b"]);
        s.route = crate::fleet::store::Route::Session("s-a".into());
        assert_eq!(s.open_asks().len(), 1, "back on the first session, its ask is there at once");
    });
}
