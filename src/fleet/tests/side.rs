//! Component 15, piece A: `lib/__tests__/side-conversation.test.ts` ("messagesAfter") and the state halves of
//! `SideConversationPanel.test.ts` and `session-calls-machine.test.ts`, under their names. The view halves are pieces B to D.
//! The server is a fake that keeps one side conversation the way Fleet's does and records what it was sent.
use super::queue::{mock_app, press, typed};
use super::{fixture, sessions};
use crate::State;
use crate::composer::drafts::{side_draft_key, side_main_draft_key};
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::side::{Draft, Progress, Side, SideState, messages_after};
use crate::test_http;
use rune::{Key, Mods, Runtime};
use serde_json::{Value, json};
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

const FRAME: f64 = 1.0 / 60.0;

fn side_json(minimized: bool, seen: Option<&str>) -> Value {
    json!({
        "sessionId": "side-1", "instanceId": "inst-1", "title": "what changed?", "boundaryMessageId": "msg_0002",
        "createdAt": "2026-11-19T10:00:00Z", "minimized": minimized, "seenAnswerId": seen,
    })
}

// ─── lib/side-conversation.ts: messagesAfter ────────────────────────────────

fn after(ids: &[&'static str], boundary: Option<&str>) -> Vec<&'static str> {
    messages_after(ids, boundary, |id| id).into_iter().copied().collect()
}

// "keeps what comes after the boundary"
#[test]
fn keeps_what_comes_after_the_boundary() {
    assert_eq!(after(&["a", "b", "c"], Some("a")), ["b", "c"]);
    assert!(after(&["a", "b", "c"], Some("c")).is_empty());
}

// "keeps everything without a boundary, or when it isn't loaded"
#[test]
fn keeps_everything_without_a_boundary_or_when_it_isnt_loaded() {
    assert_eq!(after(&["a", "b", "c"], None), ["a", "b", "c"]);
    assert_eq!(after(&["a", "b", "c"], Some("0-older")), ["a", "b", "c"]);
}

// "keeps what sorts after a boundary that isn't among them"
#[test]
fn keeps_what_sorts_after_a_boundary_that_isnt_among_them() {
    let copied = ["msg_0001a", "msg_0001b", "msg_0002a"];
    assert_eq!(after(&copied, Some("msg_0001zz")), ["msg_0002a"]);
    assert!(after(&copied, Some("msg_0002zz")).is_empty());
}

// ─── a fake Fleet that keeps one side conversation ──────────────────────────

#[derive(Default)]
struct Fake {
    side: Option<Value>,
    discarded: Option<(Value, u64)>,
    /// A route ("PUT /side/minimized") that is refused, with its status and body.
    refuse: Vec<(String, u16, Value)>,
    calls: Vec<(String, String, Value)>,
}

fn serve(mut stream: TcpStream, fake: Arc<Mutex<Fake>>) {
    let request = test_http::read(&mut stream);
    let (method, path, body) = (request.method.clone(), request.route().to_string(), request.json());
    let mut fake = fake.lock().unwrap();
    let reply = |stream: &mut TcpStream, status: u16, body: &Value| {
        let text = if status == 204 { String::new() } else { body.to_string() };
        let _ = stream.write_all(
            format!(
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
                text.len()
            )
            .as_bytes(),
        );
    };
    if path.starts_with("/hubs/") {
        let negotiate = json!({ "negotiateVersion": 1, "connectionToken": "tok" });
        return reply(&mut stream, if path.contains("negotiate") { 200 } else { 404 }, &negotiate);
    }
    if path == "/api/sessions" {
        return reply(&mut stream, 200, &serde_json::from_str(&fixture("api-sessions.json")).unwrap());
    }
    let Some((_, tail)) = path.split_once("/side") else { return reply(&mut stream, 404, &json!({})) };
    let route = format!("{method} /side{tail}");
    fake.calls.push((method.clone(), format!("/side{tail}"), body.clone()));
    if let Some((_, status, answer)) = fake.refuse.iter().find(|(r, _, _)| *r == route).cloned() {
        return reply(&mut stream, status, &answer);
    }
    match route.as_str() {
        "GET /side" => match fake.side.clone() {
            Some(side) => reply(&mut stream, 200, &side),
            None => reply(&mut stream, 204, &Value::Null),
        },
        "GET /side/discarded" => match fake.discarded.clone() {
            Some((side, left)) => reply(&mut stream, 200, &json!({ "sideConversation": side, "undoRemainingMs": left })),
            None => reply(&mut stream, 204, &Value::Null),
        },
        "POST /side" => {
            let mut side = side_json(false, None);
            if fake.discarded.take().is_some() {
                side["sessionId"] = json!("side-2");
            }
            fake.side = Some(side.clone());
            reply(
                &mut stream,
                200,
                &json!({ "sideConversation": side, "correlationId": body["correlationId"], "messageId": "msg_side_1" }),
            );
        }
        "PUT /side/minimized" => {
            let side = fake.side.as_mut().expect("a side conversation");
            side["minimized"] = body["minimized"].clone();
            reply(&mut stream, 200, &side.clone());
        }
        "PUT /side/seen" => {
            let side = fake.side.as_mut().expect("a side conversation");
            side["seenAnswerId"] = body["answerId"].clone();
            reply(&mut stream, 200, &side.clone());
        }
        "DELETE /side" => {
            let side = fake.side.take().expect("a side conversation");
            fake.discarded = Some((side, 8000));
            reply(&mut stream, 204, &Value::Null);
        }
        "POST /side/restore" => {
            let (side, _) = fake.discarded.take().expect("a discarded side conversation");
            fake.side = Some(side.clone());
            reply(&mut stream, 200, &side);
        }
        "POST /side/keep" => {
            let side = fake.side.take().expect("a side conversation");
            reply(&mut stream, 200, &side);
        }
        _ => reply(&mut stream, 404, &json!({})),
    }
}

struct Fx {
    rt: Runtime<State>,
    jobs: Receiver<Job>,
    fake: Arc<Mutex<Fake>>,
    id: String,
}

impl Fx {
    /// The fixture's first session open on a fake Fleet that already holds `side` and, maybe, a discard.
    fn start(side: Option<Value>, discarded: Option<(Value, u64)>) -> Fx {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let fake = Arc::new(Mutex::new(Fake { side, discarded, ..Default::default() }));
        let shared = fake.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let fake = shared.clone();
                std::thread::spawn(move || serve(stream, fake));
            }
        });
        let (tx, jobs) = channel::<Job>();
        let tx = Mutex::new(tx);
        let post = Post::new(move |job| {
            let _ = tx.lock().unwrap().send(job);
        });
        let mut rt = crate::tests::app();
        let link = Link::start(base, post);
        rt.update(move |s, tx| {
            s.link = Some(link);
            s.on_list(sessions::parse_list(&serde_json::from_str(&fixture("api-sessions.json")).unwrap(), 0), tx);
        });
        let id = rt.state.open_id().unwrap();
        rt.frame(FRAME);
        Fx { rt, jobs, fake, id }
    }

    fn wait(&mut self, what: &str, until: impl Fn(&Fx) -> bool) {
        crate::test_wait::until(
            self,
            what,
            |f| {
                if until(f) {
                    return true;
                }
                while let Ok(job) = f.jobs.try_recv() {
                    f.rt.update(job);
                }
                f.rt.frame(FRAME);
                until(f)
            },
            |f| format!("{}; calls {:?}", crate::test_wait::app_state(&f.rt.state), f.calls()),
        );
    }

    fn calls(&self) -> Vec<(String, Value)> {
        self.fake.lock().unwrap().calls.iter().map(|(m, p, b)| (format!("{m} {p}"), b.clone())).collect()
    }

    fn called(&self, route: &str) -> Vec<Value> {
        self.calls().into_iter().filter(|(r, _)| r == route).map(|(_, b)| b).collect()
    }

    fn st(&self) -> &SideState {
        &self.rt.state.sides[&self.id]
    }

    /// Opened and loaded: the first look found `side`.
    fn loaded(side: Option<Value>) -> Fx {
        let mut fx = Fx::start(side, None);
        fx.wait("the side conversation to load", |f| f.rt.state.sides.get(&f.id).is_some_and(|s| s.loaded));
        fx
    }

    fn ctx(&mut self, run: impl FnOnce(&mut State, &mut rune::Tx) + Send + 'static) {
        self.rt.update(run);
    }
}

fn report(fx: &mut Fx, working: bool, answer: &str, id: &str) {
    let (session, answer, id) = (fx.id.clone(), answer.to_string(), id.to_string());
    fx.ctx(move |s, _| s.report_side_progress(&session, Progress { working, latest_answer: Some(answer), latest_answer_id: Some(id) }));
}

fn minimize(fx: &mut Fx, to: bool) {
    let session = fx.id.clone();
    fx.ctx(move |s, tx| s.set_side_minimized(&session, to, tx));
}

fn discard(fx: &mut Fx) {
    let session = fx.id.clone();
    fx.ctx(move |s, _| s.discard_side(&session));
}

fn undo(fx: &mut Fx) {
    let session = fx.id.clone();
    fx.ctx(move |s, _| s.undo_side_discard(&session));
}

// ─── SideConversationPanel (state) ──────────────────────────────────────────

// "shows nothing when the session has no side conversation"
#[test]
fn shows_nothing_when_the_session_has_no_side_conversation() {
    let fx = Fx::loaded(None);
    assert!(fx.st().side.is_none() && !fx.st().is_open() && fx.st().discarded.is_none());
    assert!(!fx.rt.state.side_open && fx.rt.state.side_id.is_none());
    assert_eq!(fx.called("GET /side"), [Value::Null], "asked once");
}

// "minimizes into a tab, keeps the conversation mounted, and saves it on the server"
#[test]
fn minimizes_into_a_tab_keeps_the_conversation_mounted_and_saves_it_on_the_server() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    assert!(fx.st().is_open());
    minimize(&mut fx, true);
    assert!(fx.st().minimized() && !fx.st().is_open(), "shown before the answer");
    fx.wait("the PUT", |f| !f.called("PUT /side/minimized").is_empty());
    assert_eq!(fx.called("PUT /side/minimized"), [json!({ "minimized": true })]);
    assert_eq!(fx.st().side.as_ref().unwrap().title, "what changed?");
    assert!(fx.fake.lock().unwrap().side.as_ref().unwrap()["minimized"].as_bool().unwrap());
}

// "comes back minimized after a reload"
#[test]
fn comes_back_minimized_after_a_reload() {
    let fx = Fx::loaded(Some(side_json(true, None)));
    assert!(fx.st().minimized() && !fx.st().is_open());
    assert!(!fx.rt.state.side_open && fx.rt.state.side_id.as_deref() == Some("side-1"));
}

// "says Thinking while it works, then New answer when one lands while it's folded, until it's opened"
#[test]
fn says_thinking_while_it_works_then_new_answer_when_one_lands_while_its_folded_until_its_opened() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    report(&mut fx, false, "The first answer.", "a1");
    minimize(&mut fx, true);
    assert!(!fx.st().unread(), "Answered");
    report(&mut fx, true, "The first answer.", "a1");
    assert!(fx.st().working && !fx.st().unread(), "Thinking");
    report(&mut fx, false, "The second answer.", "a2");
    assert!(fx.st().unread() && fx.st().pulse(), "New answer, landed on this run: the dot pulses");
    assert_eq!(fx.st().latest_answer.as_deref(), Some("The second answer."));
    minimize(&mut fx, false);
    assert!(!fx.st().pulse());
    fx.wait("the PUTs", |f| !f.called("PUT /side/minimized").is_empty() && !f.called("PUT /side/seen").is_empty());
    assert!(fx.called("PUT /side/minimized").contains(&json!({ "minimized": false })));
    assert!(fx.called("PUT /side/seen").contains(&json!({ "answerId": "a2" })), "opening it is seeing it: the server keeps that");
    minimize(&mut fx, true);
    assert!(!fx.st().unread(), "Answered");
}

// "discards at once, offers Undo, and Undo brings it back as it was"
#[test]
fn discards_at_once_offers_undo_and_undo_brings_it_back_as_it_was() {
    let mut fx = Fx::loaded(Some(side_json(true, None)));
    discard(&mut fx);
    assert!(fx.st().side.is_none() && fx.st().discarded.is_some(), "gone at once, Undo offered");
    fx.wait("the DELETE", |f| !f.called("DELETE /side").is_empty());
    undo(&mut fx);
    fx.wait("the restore", |f| f.st().side.is_some());
    assert_eq!(fx.called("POST /side/restore").len(), 1);
    assert!(fx.st().minimized() && fx.st().discarded.is_none(), "as it was: minimized, and Undo is gone");
}

// "stops offering Undo after its window"
#[test]
fn stops_offering_undo_after_its_window() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    discard(&mut fx);
    assert!(fx.st().discarded.is_some());
    assert_eq!(fx.st().undo_ms, 8_000);
    fx.rt.frame(7.0);
    assert!(fx.st().discarded.is_some(), "still offered inside the window");
    fx.rt.frame(1.5);
    assert!(fx.st().discarded.is_none());
}

// "keeps it as a session and opens it"
#[test]
fn keeps_it_as_a_session_and_opens_it() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    let session = fx.id.clone();
    fx.ctx(move |s, tx| s.keep_side_as_session(&session, tx));
    // The app opens the kept session: it asks the server for its conversation. (Whether it stays open until the list holds
    // it is the list's business, so the route is not read: the list read that follows may move it, as on macOS.)
    fx.wait("the kept session to be opened", |f| f.calls().iter().any(|(r, _)| r.starts_with("GET /side-1")));
    assert_eq!(fx.called("POST /side/keep").len(), 1);
    assert!(fx.st().side.is_none(), "the panel goes");
}

// row 8.2: "The side conversation couldn't be kept (HTTP n)."
#[test]
fn a_refusal_to_keep_it_keeps_the_panel_and_says_so() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    fx.fake.lock().unwrap().refuse.push(("POST /side/keep".into(), 409, json!({})));
    let session = fx.id.clone();
    fx.ctx(move |s, tx| s.keep_side_as_session(&session, tx));
    fx.wait("the refusal", |f| f.st().error.is_some());
    assert_eq!(fx.st().error.as_deref(), Some("The side conversation couldn't be kept (HTTP 409)."));
    assert!(fx.st().side.is_some());
}

// ─── after a reload ─────────────────────────────────────────────────────────

// "says New answer for one newer than the one seen, without the pulse"
#[test]
fn says_new_answer_for_one_newer_than_the_one_seen_without_the_pulse() {
    let mut fx = Fx::loaded(Some(side_json(true, Some("a1"))));
    report(&mut fx, false, "Answered while the app was closed.", "a2");
    assert!(fx.st().unread() && !fx.st().pulse());
}

// "says Answered for the one already seen"
#[test]
fn says_answered_for_the_one_already_seen() {
    let mut fx = Fx::loaded(Some(side_json(true, Some("a2"))));
    report(&mut fx, false, "Seen before.", "a2");
    assert!(!fx.st().unread() && fx.st().latest_answer_id.as_deref() == Some("a2"));
}

// "offers Undo again for the time the server says is left"
#[test]
fn offers_undo_again_for_the_time_the_server_says_is_left() {
    let mut fx = Fx::start(None, Some((side_json(true, None), 5_000)));
    fx.wait("the discard to be offered", |f| f.rt.state.sides.get(&f.id).is_some_and(|s| s.discarded.is_some()));
    assert_eq!(fx.st().undo_ms, 5_000);
    assert_eq!(fx.st().undo_seconds_left(fx.rt.state.clock), Some(5));
    undo(&mut fx);
    fx.wait("the restore", |f| f.st().side.is_some());
    assert!(fx.st().minimized());
}

// "shows nothing once the window has closed"
#[test]
fn shows_nothing_once_the_window_has_closed() {
    let fx = Fx::loaded(None);
    assert!(fx.st().discarded.is_none());
    assert_eq!(fx.called("GET /side/discarded").len(), 1, "asked, and the server had none");
}

// "a new side question ends the Undo of a discarded one"
#[test]
fn a_new_side_question_ends_the_undo_of_a_discarded_one() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    discard(&mut fx);
    fx.wait("the DELETE", |f| !f.called("DELETE /side").is_empty());
    assert!(fx.st().discarded.is_some());
    let session = fx.id.clone();
    fx.ctx(move |s, tx| {
        s.ask_side(&session, "another one", Default::default(), tx, |_, _| {});
    });
    fx.wait("the new side conversation", |f| f.st().side.is_some());
    assert!(fx.st().discarded.is_none() && fx.st().undo_until.is_none());
    assert_eq!(fx.st().side.as_ref().unwrap().session_id, "side-2");
}

// ─── the ask (USC `ask`) and the composer's side ────────────────────────────

// "asks /btw <question> in the side conversation, not the session, even while the session works"
#[test]
fn asks_btw_in_the_side_conversation_with_the_composers_choice() {
    let mut fx = Fx::start(None, None);
    fx.rt.update(|s, _| {
        let owner = s.draft_owner().unwrap();
        s.drafts.set_choice(&owner, crate::composer::drafts::Choice { effort: "high".into(), ..Default::default() });
    });
    typed(&mut fx.rt, "/btw  what changed? ");
    press(&mut fx.rt, Key::Enter, Mods::default());
    assert_eq!(fx.rt.state.draft.text, "", "the draft clears at once");
    assert!(fx.rt.state.side_open, "the panel opens at once: the first question is forking");
    fx.wait("the question to be asked", |f| !f.called("POST /side").is_empty());
    let body = fx.called("POST /side").remove(0);
    assert_eq!((body["text"].as_str(), body["effort"].as_str()), (Some("what changed?"), Some("high")));
    assert!(body["agent"].is_null() || body["agent"].is_string());
    let correlation = body["correlationId"].as_str().unwrap();
    assert!(correlation.starts_with("side-") && correlation.len() == 5 + 32 && correlation[5..].chars().all(|c| c.is_ascii_hexdigit()));
    fx.wait("the side conversation to open", |f| f.st().side.is_some());
    assert!(fx.st().starting.is_none() && !fx.st().asking);
    assert!(fx.rt.state.side_open && fx.rt.state.side_id.as_deref() == Some("side-1"));
    // The question shows at once as a sent prompt of the side session (3.7).
    let prompts = fx.rt.state.prompts.of("side-1");
    assert_eq!(prompts.len(), 1);
    assert_eq!(
        (prompts[0].body.as_str(), prompts[0].id.as_str(), prompts[0].correlation_id.as_str()),
        ("what changed?", "msg_side_1", correlation)
    );
}

// "sends what's typed to the open side conversation"
#[test]
fn sends_whats_typed_to_the_open_side_conversation() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    fx.rt.frame(FRAME);
    assert!(fx.rt.find("composer-side-mode").is_some());
    typed(&mut fx.rt, "and the tests?");
    press(&mut fx.rt, Key::Enter, Mods::default());
    fx.wait("the question", |f| !f.called("POST /side").is_empty());
    assert_eq!(fx.called("POST /side")[0]["text"], "and the tests?");
}

// "asks for a question after a bare /btw"
#[test]
fn asks_for_a_question_after_a_bare_btw() {
    let mut fx = Fx::loaded(None);
    typed(&mut fx.rt, "/btw");
    press(&mut fx.rt, Key::Enter, Mods::default());
    assert!(fx.called("POST /side").is_empty());
    assert_eq!(fx.rt.state.draft.text, "/btw", "kept; the error text is checked in modes.rs against the invented app");
}

// "puts a refused question back and says why"
#[test]
fn puts_a_refused_question_back_and_says_why() {
    let mut fx = Fx::loaded(None);
    fx.fake.lock().unwrap().refuse.push(("POST /side".into(), 409, json!({ "error": "This harness can't fork." })));
    typed(&mut fx.rt, "/btw what changed?");
    press(&mut fx.rt, Key::Enter, Mods::default());
    assert_eq!(fx.rt.state.draft.text, "");
    fx.wait("the refusal", |f| f.st().error.is_some());
    assert_eq!(fx.rt.state.draft.text, "/btw what changed?");
    assert_eq!(fx.rt.state.side_error(), Some("This harness can't fork."));
    assert!(!fx.rt.state.side_open && fx.st().starting.is_none(), "the panel was never made");
    // Without a reason: Fleet's words with the status.
    fx.fake.lock().unwrap().refuse[0] = ("POST /side".into(), 500, json!({}));
    fx.rt.update(|s, _| s.draft.clear());
    typed(&mut fx.rt, "/btw again");
    press(&mut fx.rt, Key::Enter, Mods::default());
    fx.wait("the second refusal", |f| f.st().error.as_deref() == Some("The side question couldn't be asked (HTTP 500)."));
}

// "does nothing while a question is on its way" (USC `ask`)
#[test]
fn asks_nothing_while_a_question_is_on_its_way_or_empty() {
    let mut rt = mock_app(false, "opencode", false, false);
    let session = rt.state.draft_owner().unwrap();
    rt.update(move |s, tx| {
        s.side_state(&session).asking = true;
        assert!(!s.ask_side(&session, "x", Default::default(), tx, |_, _| {}), "one is on its way");
        s.side_state(&session).asking = false;
        assert!(!s.ask_side(&session, "   ", Default::default(), tx, |_, _| {}), "nothing to ask");
    });
}

// with no server the side conversation is made here, so scenes and tests need none
#[test]
fn with_no_server_the_side_conversation_is_made_here() {
    let mut rt = mock_app(false, "opencode", false, false);
    typed(&mut rt, "/btw what changed?");
    press(&mut rt, Key::Enter, Mods::default());
    let owner = rt.state.draft_owner().unwrap();
    let st = &rt.state.sides[&owner];
    assert_eq!(st.side.as_ref().map(|s| s.title.as_str()), Some("what changed?"));
    assert!(rt.state.side_open && rt.state.composer_error.is_none() && rt.state.draft.text.is_empty());
}

// ─── the drafts (9.3) and the error box (9.5) ───────────────────────────────

// "keeps the put-away draft through a reload" (USC `restoreDrafts`): a side opened at once takes its draft
#[test]
fn a_side_conversation_open_at_the_start_takes_its_draft_and_puts_the_sessions_away() {
    let mut fx = Fx::start(Some(side_json(false, None)), None);
    let session = fx.id.clone();
    fx.rt.update(move |s, _| {
        s.drafts.write(&side_draft_key(&session, "side-1"), "for the side", 0.0);
        s.drafts.write(&side_draft_key(&session, "side-gone"), "forgotten", 0.0);
    });
    typed(&mut fx.rt, "for the session");
    fx.wait("the load", |f| f.st().loaded && f.rt.state.draft.text == "for the side");
    assert_eq!(fx.rt.state.drafts.read(&side_main_draft_key(&fx.id)), Some("for the session"));
    assert_eq!(fx.rt.state.drafts.read(&side_draft_key(&fx.id, "side-gone")).unwrap_or(""), "", "a gone side's draft is forgotten");
}

#[test]
fn with_no_side_conversation_a_put_away_session_draft_goes_back_in_the_composer() {
    let mut fx = Fx::start(None, None);
    let session = fx.id.clone();
    fx.rt.update(move |s, _| {
        s.drafts.write(&side_main_draft_key(&session), "left behind", 0.0);
    });
    fx.wait("the load", |f| f.st().loaded);
    assert_eq!(fx.rt.state.draft.text, "left behind");
    assert_eq!(fx.rt.state.drafts.read(&side_main_draft_key(&fx.id)).unwrap_or(""), "");
}

#[test]
fn the_side_error_shows_in_the_composers_error_box_first() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    fx.fake.lock().unwrap().refuse.push(("PUT /side/minimized".into(), 500, json!({})));
    fx.rt.update(|s, _| s.composer_error = Some("another error".into()));
    minimize(&mut fx, true);
    fx.wait("the refusal", |f| f.st().error.is_some());
    assert_eq!(fx.st().error.as_deref(), Some("The side conversation couldn't be minimized (HTTP 500)."));
    fx.rt.frame(FRAME);
    assert_eq!(fx.rt.state.side_error(), fx.st().error.as_deref());
    assert!(fx.rt.find("send-prompt-error").is_some());
}

#[test]
fn a_refused_discard_puts_the_side_conversation_back() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    fx.fake.lock().unwrap().refuse.push(("DELETE /side".into(), 500, json!({ "title": "Nope" })));
    discard(&mut fx);
    fx.wait("the refusal", |f| f.st().side.is_some());
    assert_eq!(fx.st().error.as_deref(), Some("Nope"));
    assert!(fx.st().discarded.is_none() && fx.st().undo_until.is_none());
}

#[test]
fn a_refused_undo_drops_the_offer_and_says_so() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    discard(&mut fx);
    fx.wait("the DELETE", |f| !f.called("DELETE /side").is_empty());
    fx.fake.lock().unwrap().refuse.push(("POST /side/restore".into(), 410, json!({})));
    undo(&mut fx);
    fx.wait("the refusal", |f| f.st().error.is_some());
    assert_eq!(fx.st().error.as_deref(), Some("The side conversation couldn't be brought back (HTTP 410)."));
    assert!(fx.st().discarded.is_none() && fx.st().side.is_none());
}

#[test]
fn an_answer_that_finishes_while_it_is_open_is_marked_seen_at_once() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    report(&mut fx, false, "Done.", "a1");
    assert_eq!(fx.st().side.as_ref().unwrap().seen_answer_id.as_deref(), Some("a1"));
    fx.wait("the PUT", |f| !f.called("PUT /side/seen").is_empty());
    assert_eq!(fx.called("PUT /side/seen"), [json!({ "answerId": "a1" })]);
    let _ = (Side::from_json(&side_json(false, None)), Draft::Main);
}
