//! The question card live (component 4, piece D): `use-question-answer.test.ts` and `QuestionCard.test.ts` under their
//! names, read from a fake server. The wording of the pieces and their drawing are `asks::tests_question` and
//! `tests_questions`.
use super::super::link::{Job, Link, Post};
use super::super::questions::{NOT_WAITING, Sent, failure};
use super::super::reducer;
use super::super::sessions;
use super::reconnect::app;
use super::*;
use crate::State;
use rune::{Key, KeyEvent, Mods, Runtime, TextEdit};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FRAME: f64 = 1.0 / 60.0;

#[derive(Default)]
struct Seen {
    /// Method, path and body of every call to a question route.
    calls: Vec<(String, String, Value)>,
    /// What a question route answers instead of 204.
    refuse: Option<u16>,
}

fn serve(mut stream: TcpStream, seen: Arc<Mutex<Seen>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let (mut length, mut request) = (0usize, String::new());
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        if request.is_empty() {
            request = line.clone();
        }
        if line == "\r\n" {
            break;
        }
        if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            length = n.trim().parse().unwrap_or(0);
        }
    }
    let mut raw = vec![0; length];
    let _ = reader.read_exact(&mut raw);
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
        reply(&mut stream, "200 OK", "[]");
    } else if method == "POST" && path.contains("/questions/") {
        seen.calls.push((method, path, body));
        match seen.refuse {
            Some(code) => reply(&mut stream, &format!("{code} Refused"), "{}"),
            None => reply(&mut stream, "204 No Content", ""),
        }
    } else {
        reply(&mut stream, "404 Not Found", "{}");
    }
}

fn questions() -> Value {
    json!([{"header": "Deploy target", "question": "Which environment should we deploy to?",
            "options": [{"label": "Staging", "description": "Safe to break."}, {"label": "Production", "description": "Real users."}]}])
}

struct Live {
    rt: Runtime<State>,
    jobs: std::sync::mpsc::Receiver<Job>,
    seen: Arc<Mutex<Seen>>,
}

impl Live {
    /// The fixture's first session open on a server, its conversation a question in `status`.
    fn start(status: &str, extra: Value) -> Live {
        let mut live = Live::bare();
        live.ask(status, extra);
        live
    }

    fn bare() -> Live {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Seen::default()));
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
        let mut live = Live { rt, jobs, seen };
        live.rt.frame(FRAME);
        live
    }

    /// The agent's message with a `question` call in `status` arrives.
    fn ask(&mut self, status: &str, extra: Value) {
        let mut state = json!({"status": status, "input": {"questions": questions()}});
        state.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        let part = json!({"type": "tool", "id": "p1", "tool": "question", "callID": "call-1", "state": state});
        let msg = |id: &str, role: &str, parts: Value| json!({"info": {"id": id, "role": role, "sessionID": "s", "time": {"created": 1}}, "parts": parts});
        let messages =
            vec![msg("u1", "user", json!([{"type": "text", "id": "u1-t", "text": "ask-me"}])), msg("a1", "assistant", json!([part]))];
        let snapshot = json!({"session": {}, "messages": messages, "delegations": [], "activityStatus": "waiting_input"});
        self.rt.update(move |s, _| {
            s.stream = Some(reducer::create_state(&snapshot));
            s.stream_of = None;
            s.sync_open();
        });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    fn wait(&mut self, until: &dyn Fn(&Live) -> bool, what: &str) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !until(self) {
            assert!(Instant::now() < deadline, "{what}");
            while let Ok(job) = self.jobs.try_recv() {
                self.rt.update(job);
            }
            self.rt.frame(FRAME);
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn calls(&self) -> Vec<(String, String, Value)> {
        self.seen.lock().unwrap().calls.clone()
    }

    fn click(&mut self, key: &str) {
        crate::tests::click(&mut self.rt, key);
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

    fn card(&self) -> &crate::fleet::questions::QuestionCard {
        self.rt.state.questions.card("call-1").expect("the card")
    }
}

// ─── use-question-answer.test.ts ────────────────────────────────────────────

// use-question-answer.test.ts: "says the agent isn't waiting…" (404 and 409)
#[test]
fn says_the_agent_isnt_waiting_when_fleet_answers_404_or_409() {
    for status in [404, 409] {
        for sent in [Sent::Answer, Sent::Skip] {
            assert_eq!(failure(sent, status), (NOT_WAITING.to_string(), true));
        }
    }
}

// use-question-answer.test.ts: "keeps the status for other failures"
#[test]
fn keeps_the_status_for_other_failures() {
    assert_eq!(failure(Sent::Answer, 500), ("Failed to answer question: Internal Server Error".to_string(), false));
    assert_eq!(failure(Sent::Skip, 502), ("Failed to reject question: Bad Gateway".to_string(), false));
}

// ─── QuestionCard.test.ts ───────────────────────────────────────────────────

// QuestionCard.test.ts: "takes keyboard focus on its first option, and the arrow keys move between options"
#[test]
fn takes_keyboard_focus_on_its_first_option_and_the_arrow_keys_move_between_options() {
    let mut live = Live::start("running", json!({}));
    assert_eq!(live.focused().as_deref(), Some("question-pill-Staging"));
    live.key(Key::Down);
    assert_eq!(live.focused().as_deref(), Some("question-pill-Production"));
    live.key(Key::Down);
    assert_eq!(live.focused().as_deref(), Some("question-pill-Staging"), "the arrows wrap round");
    live.key(Key::Up);
    assert_eq!(live.focused().as_deref(), Some("question-pill-Production"));
    live.click("question-pill-Production");
    assert_eq!(live.card().sel.picked[0], ["Production"]);
}

// QuestionCard.test.ts: "leaves focus alone while the user types in the composer"
#[test]
fn leaves_focus_alone_while_the_user_types_in_the_composer() {
    let mut live = Live::bare();
    live.rt.update(|s, tx| {
        s.draft = TextEdit::new("half a sentence");
        tx.focus("composer", false);
    });
    live.rt.frame(FRAME);
    live.ask("running", json!({}));
    assert_eq!(live.focused().as_deref(), Some("composer"));
}

// QuestionCard.test.ts: "still takes focus from an empty composer that autofocused"
#[test]
fn still_takes_focus_from_an_empty_composer_that_autofocused() {
    let mut live = Live::bare();
    live.rt.update(|_, tx| tx.focus("composer", false));
    live.rt.frame(FRAME);
    live.ask("running", json!({}));
    assert_eq!(live.focused().as_deref(), Some("question-pill-Staging"));
}

// QuestionCard.test.ts: "sends the picked choice"
#[test]
fn sends_the_picked_choice() {
    let mut live = Live::start("running", json!({}));
    live.click("question-pill-Staging");
    assert_eq!(live.card().sel.picked[0], ["Staging"]);
    live.click("question-submit-button");
    live.wait(&|l| !l.calls().is_empty(), "the answer");
    let calls = live.calls();
    assert_eq!(calls[0].1, "/api/sessions/".to_string() + &live.rt.state.open_id().unwrap() + "/questions/call-1/answer");
    assert_eq!(calls[0].2, json!({"answers": [["Staging"]]}));
    live.wait(&|l| !l.card().busy, "the card to be free again");
}

// QuestionCard.test.ts: "picks a choice by its number key and sends on Enter"
#[test]
fn picks_a_choice_by_its_number_key_and_sends_on_enter() {
    let mut live = Live::start("running", json!({}));
    // Fleet's test presses on the card itself: Enter on a focused choice is that choice's press, not a send.
    live.rt.update(|_, tx| tx.focus("question-card", false));
    live.rt.frame(FRAME);
    live.key(Key::Text("2".into()));
    assert_eq!(live.card().sel.picked[0], ["Production"]);
    live.key(Key::Enter);
    live.wait(&|l| !l.calls().is_empty(), "the answer");
    assert_eq!(live.calls()[0].2, json!({"answers": [["Production"]]}));
}

// QuestionCard.test.ts: "sends typed text instead of a choice"
#[test]
fn sends_typed_text_instead_of_a_choice() {
    let mut live = Live::start("running", json!({}));
    live.click("question-pill-Staging");
    live.click("question-other-input-0");
    live.key(Key::Text("Both, staging first".into()));
    live.rt.frame(FRAME);
    assert!(live.card().sel.picked[0].is_empty(), "typing drops the pick");
    live.click("question-submit-button");
    live.wait(&|l| !l.calls().is_empty(), "the answer");
    assert_eq!(live.calls()[0].2, json!({"answers": [["Both, staging first"]]}));
}

#[test]
fn enter_in_the_typed_field_sends_and_the_digit_after_the_options_goes_to_the_field() {
    let mut live = Live::start("running", json!({}));
    live.key(Key::Text("3".into()));
    assert_eq!(live.focused().as_deref(), Some("question-other-input-0"), "1 + options puts the caret in Something else…");
    live.key(Key::Enter);
    assert!(live.calls().is_empty(), "nothing is answered yet, so Enter sends nothing");
    live.key(Key::Text("3".into()));
    assert_eq!(live.card().typed[0].text, "3", "a digit typed in the field is the field's");
    live.key(Key::Enter);
    live.wait(&|l| !l.calls().is_empty(), "the answer");
    assert_eq!(live.calls()[0].2, json!({"answers": [["3"]]}));
}

// QuestionCard.test.ts: "skips"
#[test]
fn skips() {
    let mut live = Live::start("running", json!({}));
    live.click("question-dismiss-button");
    live.wait(&|l| !l.calls().is_empty(), "the skip");
    let calls = live.calls();
    assert!(calls[0].1.ends_with("/questions/call-1/reject"), "{}", calls[0].1);
}

// QuestionCard.test.ts: "shrinks to a row with the question and its answer once answered"
#[test]
fn shrinks_to_a_row_with_the_question_and_its_answer_once_answered() {
    let mut live = Live::start("running", json!({}));
    assert!(live.rt.find("question-card").is_some());
    live.ask("completed", json!({"metadata": {"answers": [["Staging"]]}}));
    assert!(live.rt.find("question-card").is_none(), "the card is gone");
    assert!(live.rt.find("question-row-call-1-0").is_some() && live.rt.find("question-answer-call-1-0").is_some());
}

// QuestionCard.test.ts: "says when a question was skipped"
#[test]
fn says_when_a_question_was_skipped() {
    let live = Live::start("error", json!({}));
    assert!(live.rt.find("question-skipped-call-1").is_some() && live.rt.find("question-card").is_none());
}

// QuestionCard.test.ts: "says the agent isn't waiting and lets the user dismiss the leftover card here, sending nothing"
#[test]
fn says_the_agent_isnt_waiting_and_lets_the_user_dismiss_the_leftover_card_here_sending_nothing() {
    let mut live = Live::start("running", json!({}));
    live.seen.lock().unwrap().refuse = Some(409);
    live.click("question-pill-Staging");
    live.click("question-submit-button");
    live.wait(&|l| l.card().gone, "the refusal");
    assert_eq!(live.card().error.as_deref(), Some(NOT_WAITING));
    assert!(live.rt.find("question-error").is_some(), "the card says so");
    assert_eq!(live.calls().len(), 1);
    live.click("question-dismiss-button");
    assert_eq!(live.calls().len(), 1, "Dismiss sends nothing");
    assert!(live.rt.find("question-card").is_none() && live.rt.find("question-skipped-call-1").is_some());
}

#[test]
fn another_failure_keeps_the_card_with_fleets_status_and_the_buttons_work_again() {
    let mut live = Live::start("running", json!({}));
    live.seen.lock().unwrap().refuse = Some(500);
    live.click("question-pill-Staging");
    live.click("question-submit-button");
    live.wait(&|l| l.card().error.is_some(), "the refusal");
    assert_eq!(live.card().error.as_deref(), Some("Failed to answer question: Internal Server Error"));
    assert!(!live.card().busy && !live.card().gone);
    assert!(live.rt.find("question-card").is_some());
}

#[test]
fn a_completed_call_with_no_recorded_answers_draws_neither_card_nor_row() {
    let live = Live::start("completed", json!({}));
    assert!(live.rt.find("question-card").is_none());
    assert!(live.rt.find("question-skipped-call-1").is_none() && live.rt.find("question-row-call-1-0").is_none());
}
