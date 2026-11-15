//! Queue and steer (docs/parity/05-composer.md, piece I). Ports `use-session-queue.test.ts`, `QueuedMessages.test.ts` and
//! "Composer steering" of `Composer.test.ts` under their names. The store's rules are tested on the store; what a key or
//! a button sends is read from a fake server.
use super::super::events::{Event, Kind};
use super::super::link::{Job, Link, Post};
use super::super::queue::{Item, ItemKind, enqueue_body, read_queue, strip_session_references};
use super::super::state::Topic;
use super::reconnect::app;
use super::*;
use crate::State;
use crate::composer::status::{HarnessInfo, SteerDelivery};
use crate::mock::Status;
use rune::{Key, KeyEvent, Mods, Runtime};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FRAME: f64 = 1.0 / 60.0;

fn item(id: &str, text: &str) -> Item {
    Item { id: id.into(), text: text.into(), kind: ItemKind::Prompt, from_mod: None }
}

fn queue_event(session: &str, items: Value) -> Event {
    Event::Other(Kind::SessionQueue, json!({"sessionId": session, "items": items}))
}

// ─── the store ──────────────────────────────────────────────────────────────

// use-session-queue.test.ts: "replaces the queue with the one a session.queue event carries"
#[test]
fn replaces_the_queue_with_the_one_a_session_queue_event_carries() {
    let mut rt = app();
    rt.update(|s, _| {
        let owner = s.draft_owner().unwrap();
        s.dispatch(
            &Topic::Session(owner.clone()),
            &[queue_event(&owner, json!([{"id": "q1", "kind": "prompt", "text": "from the other tab"}]))],
        );
        assert_eq!(s.queued(), [item("q1", "from the other tab")]);
        s.dispatch(&Topic::Session(owner.clone()), &[queue_event(&owner, json!([]))]);
        assert!(s.queued().is_empty());
    });
}

// use-session-queue.test.ts: "ignores another session's queue and other events"
#[test]
fn ignores_another_sessions_queue_and_other_events() {
    let mut rt = app();
    rt.update(|s, _| {
        let owner = s.draft_owner().unwrap();
        s.dispatch(&Topic::Sessions, &[queue_event(&owner, json!([{"id": "q1", "text": "mine"}]))]);
        s.dispatch(&Topic::Sessions, &[queue_event("session-2", json!([]))]);
        s.dispatch(&Topic::Sessions, &[Event::Other(Kind::SessionRetry, json!({"sessionId": owner, "items": []}))]);
        assert_eq!(s.queued().len(), 1, "another session's queue and another kind of event leave this one alone");
        assert_eq!(s.queue.of("session-2"), []);
    });
}

#[test]
fn an_item_without_an_id_or_text_does_not_read_and_an_unknown_kind_is_a_prompt() {
    let items = read_queue(&json!([
        {"id": "a", "text": "one", "kind": "surprise"},
        {"text": "no id"},
        {"id": "b"},
        {"id": "c", "text": "run it", "kind": "shell", "mod": "nightly"},
        {"id": "d", "text": "two", "kind": "command", "mod": ""},
    ]));
    assert_eq!(
        items.iter().map(|i| (i.id.as_str(), i.kind)).collect::<Vec<_>>(),
        [("a", ItemKind::Prompt), ("c", ItemKind::Shell), ("d", ItemKind::Command)]
    );
    assert_eq!(items[1].from_mod.as_deref(), Some("nightly"));
    assert_eq!(items[2].from_mod, None, "an empty mod name is no mod");
    assert!(read_queue(&json!({"not": "a list"})).is_empty());
}

#[test]
fn the_queue_shows_what_was_typed_without_the_block_fleet_added_for_the_agents_sessions() {
    let sent = "look at this\n<fleet-session-references>\n<session ref=\"@a\" id=\"s1\" title=\"A\"/>\n</fleet-session-references>";
    assert_eq!(strip_session_references(sent), "look at this");
    assert_eq!(strip_session_references("no block"), "no block");
    assert_eq!(
        strip_session_references("<fleet-session-references>\nopen but never closed"),
        "<fleet-session-references>\nopen but never closed"
    );
}

#[test]
fn a_load_asked_for_before_an_event_is_older_than_it() {
    let mut q = super::super::queue::Queues::default();
    let g = q.begin_load("s");
    q.replace("s", vec![item("new", "from the event")]);
    q.loaded("s", g, vec![item("old", "from the load")]);
    assert_eq!(q.of("s"), [item("new", "from the event")]);
    let g = q.begin_load("s");
    q.loaded("s", g, vec![item("loaded", "a load nothing overtook")]);
    assert_eq!(q.of("s"), [item("loaded", "a load nothing overtook")]);
}

// use-session-queue.test.ts: "reports why a message couldn't be queued"
#[test]
fn reports_why_a_message_couldnt_be_queued_and_puts_the_text_back_unless_something_new_was_typed() {
    let mut rt = app();
    rt.update(|s, _| {
        let owner = s.draft_owner().unwrap();
        s.on_enqueued(&owner, "also check the tests".into(), Ok((409, json!({"error": "This session is archived."}))));
        assert_eq!(s.composer_error.as_deref(), Some("This session is archived."));
        assert_eq!(s.draft.text, "also check the tests", "the draft was empty, so the text comes back");
        s.draft = rune::TextEdit::new("something new");
        s.on_enqueued(&owner, "also check the tests".into(), Ok((500, Value::Null)));
        assert_eq!(s.composer_error.as_deref(), Some("The message couldn't be queued (HTTP 500)."));
        assert_eq!(s.draft.text, "something new", "newer text is not overwritten");
        s.on_enqueued(&owner, String::new(), Err("connection refused".into()));
        assert_eq!(s.composer_error.as_deref(), Some("connection refused"));
        assert!(s.queued().is_empty());
    });
}

// use-session-queue.test.ts: "queues a message and shows it at once" (the answer adds it; the event may have)
#[test]
fn queues_a_message_and_shows_it_once_fleet_has_taken_it_without_listing_it_twice() {
    let mut rt = app();
    rt.update(|s, _| {
        let owner = s.draft_owner().unwrap();
        let created = json!({"id": "queued-1", "kind": "prompt", "text": "hello"});
        s.on_enqueued(&owner, "hello".into(), Ok((201, created.clone())));
        s.on_enqueued(&owner, "hello".into(), Ok((201, created)));
        assert_eq!(s.queued(), [item("queued-1", "hello")]);
        assert!(s.composer_error.is_none());
    });
}

#[test]
fn the_queue_body_carries_the_kind_the_command_and_the_pick() {
    let options = super::super::queue::Options {
        kind: ItemKind::Command,
        command: Some(("review".into(), "src/auth".into())),
        pick: super::super::http::Pick {
            agent: Some("build".into()),
            model: Some(("p".into(), "m".into())),
            effort: Some("high".into()),
            ..Default::default()
        },
    };
    assert_eq!(
        enqueue_body("/review src/auth", &options),
        json!({"text": "/review src/auth", "kind": "command", "command": "review", "arguments": "src/auth", "agent": "build", "model": {"providerID": "p", "modelID": "m"}, "effort": "high"})
    );
    let plain = super::super::queue::Options { kind: ItemKind::Prompt, command: None, pick: Default::default() };
    assert_eq!(
        enqueue_body("hi", &plain),
        json!({"text": "hi", "kind": "prompt", "command": null, "arguments": null, "agent": null, "model": null, "effort": null})
    );
}

// ─── the list ───────────────────────────────────────────────────────────────

pub(super) fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(FRAME);
}

pub(super) fn press(rt: &mut Runtime<State>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

pub(super) fn ctrl() -> Mods {
    Mods { ctrl: true, ..Mods::default() }
}

pub(super) fn typed(rt: &mut Runtime<State>, text: &str) {
    click(rt, "composer");
    press(rt, Key::Text(text.into()), Mods::default());
}

/// The invented demo's open session, with harness `kind` able or not able to steer, working or idle.
pub(super) fn mock_app(working: bool, kind: &str, steering: bool, by_default: bool) -> Runtime<State> {
    let mut rt = app();
    let kind = kind.to_string();
    rt.update(move |s, _| {
        s.open = (0, 2);
        s.projects[0].sessions[2].status = if working { Status::Working } else { Status::Idle };
        s.projects[0].sessions[2].row.harness_type = Some(kind.clone());
        s.pickers.harnesses = vec![HarnessInfo {
            kind,
            supports_steering: steering,
            steers_by_default: by_default,
            steer_delivery: steering.then_some(SteerDelivery::NextStep),
            ..Default::default()
        }];
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn queued_on_screen(rt: &Runtime<State>) -> usize {
    (0..10).take_while(|i| rt.find(&format!("queued-{i}")).is_some()).count()
}

// QueuedMessages.test.ts: "lists what waits for the turn to end, next one first"
#[test]
fn lists_what_waits_for_the_turn_to_end_next_one_first() {
    let mut rt = mock_app(true, "opencode", false, false);
    assert!(rt.find("queued-messages").is_none(), "no list without items");
    rt.update(|s, _| {
        let owner = s.draft_owner().unwrap();
        s.queue.replace(&owner, vec![item("a", "first thing"), item("b", "second thing")]);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(queued_on_screen(&rt), 2);
    let (_, first) = rt.find("queued-0").unwrap();
    let (_, second) = rt.find("queued-1").unwrap();
    assert!(first < second);
    assert_eq!(crate::view::queued::label(0, &item("a", "")), "Next");
    assert_eq!(crate::view::queued::label(1, &item("b", "")), "Queued");
}

// QueuedMessages.test.ts: "names the mod that queued a message in its row, and only there"
#[test]
fn names_the_mod_that_queued_a_message_in_its_row_and_only_there() {
    let by_mod = Item { from_mod: Some("nightly".into()), ..item("a", "run the checks") };
    assert_eq!(crate::view::queued::label(0, &by_mod), "Next · from nightly");
    assert_eq!(crate::view::queued::label(2, &by_mod), "Queued · from nightly");
    assert_eq!(crate::view::queued::label(0, &item("b", "typed by hand")), "Next");
}

// QueuedMessages.test.ts: "offers Send now only for what can go into the turn"
#[test]
fn offers_send_now_only_for_what_can_go_into_the_turn() {
    let mut rt = mock_app(true, "opencode", true, false);
    rt.update(|s, _| {
        let owner = s.draft_owner().unwrap();
        let mut command = item("c", "/review");
        command.kind = ItemKind::Command;
        s.queue.replace(&owner, vec![item("a", "a message"), command]);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("queued-send-now-0").is_some());
    assert!(rt.find("queued-send-now-1").is_none(), "a command waits for the turn to end");
    // Idle: whatever is left over can go.
    rt.update(|s, _| s.projects[0].sessions[2].status = Status::Idle);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("queued-send-now-1").is_some());
}

// QueuedMessages.test.ts: "has no Send now when the session's harness can't take a message mid-turn"
#[test]
fn has_no_send_now_when_the_sessions_harness_cant_take_a_message_mid_turn() {
    let mut rt = mock_app(true, "claude-code", false, false);
    rt.update(|s, _| {
        let owner = s.draft_owner().unwrap();
        s.queue.replace(&owner, vec![item("a", "a message")]);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("queued-0").is_some());
    assert!(rt.find("queued-send-now-0").is_none());
}

// QueuedMessages.test.ts: "takes a message back out of the queue"
#[test]
fn takes_a_message_back_out_of_the_queue() {
    let mut rt = mock_app(true, "opencode", false, false);
    rt.update(|s, _| {
        let owner = s.draft_owner().unwrap();
        s.queue.replace(&owner, vec![item("a", "first"), item("b", "second")]);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    click(&mut rt, "queued-remove-0");
    rt.frame(FRAME);
    assert_eq!(rt.state.queued(), [item("b", "second")]);
    assert_eq!(queued_on_screen(&rt), 1);
}

// ─── Enter, Ctrl+Enter and the buttons, without a server ────────────────────

// Composer steering: "queues a message on Enter while the agent works, and sends it into the turn on Ctrl+Enter" (the
// queue half; the request is read below)
#[test]
fn enter_queues_while_the_agent_works_and_clears_the_draft() {
    let mut rt = mock_app(true, "opencode", true, false);
    let before = rt.state.session().messages.len();
    typed(&mut rt, "also check the tests");
    assert!(rt.find("prompt-send-now-button").is_some());
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.draft.text, "");
    assert_eq!(
        rt.state.queued().iter().map(|i| (i.text.as_str(), i.kind)).collect::<Vec<_>>(),
        [("also check the tests", ItemKind::Prompt)]
    );
    assert_eq!(rt.state.session().messages.len(), before, "nothing joined the conversation");
    assert_eq!(queued_on_screen(&rt), 1);
}

// Composer steering: "offers only the queue for a harness that can't steer"
#[test]
fn offers_only_the_queue_for_a_harness_that_cant_steer() {
    let mut rt = mock_app(true, "claude-code", false, false);
    let before = rt.state.session().messages.len();
    typed(&mut rt, "also check the tests");
    assert!(rt.find("prompt-send-now-button").is_none() && rt.find("prompt-queue-button").is_none());
    press(&mut rt, Key::Enter, ctrl());
    assert_eq!(rt.state.session().messages.len(), before);
    assert_eq!(rt.state.queued().len(), 1);
    assert!(rt.find("queued-send-now-0").is_none());
}

// Composer steering: "offers no Send now for a side question, which never waits for the turn"
#[test]
fn offers_no_send_now_for_a_side_question_which_never_waits_for_the_turn() {
    let mut rt = mock_app(true, "opencode", true, false);
    typed(&mut rt, "/btw what does this file do?");
    assert!(rt.find("prompt-send-now-button").is_none());
}

// Composer steering: "sends the normal way when the agent is idle, even on Ctrl+Enter"
#[test]
fn sends_the_normal_way_when_the_agent_is_idle_even_on_ctrl_enter() {
    let mut rt = mock_app(false, "opencode", true, false);
    let before = rt.state.session().messages.len();
    assert!(rt.find("prompt-send-now-button").is_none());
    typed(&mut rt, "hello");
    press(&mut rt, Key::Enter, ctrl());
    assert_eq!(rt.state.session().messages.len(), before + 1);
    assert!(rt.state.queued().is_empty());
}

// Composer steering: "on OpenCode 2, Enter sends into the turn and Ctrl+Enter or Queue waits for it to end" (the
// buttons; the requests are read below)
#[test]
fn on_opencode_2_the_other_button_reads_queue_and_queues() {
    let mut rt = mock_app(true, "opencode2", true, true);
    typed(&mut rt, "and the docs");
    assert!(rt.find("prompt-send-now-button").is_none());
    assert!(rt.find("prompt-queue-button").is_some());
    click(&mut rt, "prompt-queue-button");
    assert_eq!(rt.state.queued().iter().map(|i| i.text.as_str()).collect::<Vec<_>>(), ["and the docs"]);
    assert_eq!(rt.state.draft.text, "");
}

// Composer steering: "sends a queued message into the turn with Send now, leaving the draft alone"
#[test]
fn sends_a_queued_message_with_send_now_leaving_the_draft_alone() {
    let mut rt = mock_app(true, "opencode", true, false);
    typed(&mut rt, "use the other branch");
    press(&mut rt, Key::Enter, Mods::default());
    typed(&mut rt, "still typing");
    click(&mut rt, "queued-send-now-0");
    assert!(rt.state.queued().is_empty());
    assert_eq!(rt.state.draft.text, "still typing");
}

// Composer steering: "queues a slash command as a command, to run when the turn ends"
#[test]
fn queues_a_slash_command_as_a_command_to_run_when_the_turn_ends() {
    let mut rt = mock_app(true, "opencode", true, false);
    let before = rt.state.session().messages.len();
    typed(&mut rt, "/review src/auth");
    press(&mut rt, Key::Enter, ctrl());
    assert_eq!(rt.state.session().messages.len(), before);
    let queued = rt.state.queued().to_vec();
    assert_eq!(queued.iter().map(|i| (i.text.as_str(), i.kind)).collect::<Vec<_>>(), [("/review src/auth", ItemKind::Command)]);
    assert!(rt.find("queued-send-now-0").is_none());
}

#[test]
fn a_shell_command_typed_while_the_agent_works_waits_with_the_prompts() {
    let mut rt = mock_app(true, "opencode", true, false);
    rt.update(|s, _| s.pickers.harnesses[0].supports_shell_commands = true);
    typed(&mut rt, "!git status");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.queued().iter().map(|i| (i.text.as_str(), i.kind)).collect::<Vec<_>>(), [("!git status", ItemKind::Shell)]);
}

#[test]
fn what_is_queued_comes_back_to_the_history_for_arrow_up() {
    let mut rt = mock_app(true, "opencode", false, false);
    typed(&mut rt, "remember me");
    press(&mut rt, Key::Enter, Mods::default());
    let owner = rt.state.draft_owner().unwrap();
    assert_eq!(rt.state.drafts.history(&owner).map(|h| h.entries.clone()), Some(vec!["remember me".to_string()]));
}

// ─── the requests, read from a fake server ──────────────────────────────────

#[derive(Default)]
struct Seen {
    /// Method, path and body of every call that wasn't the list or the hub.
    calls: Vec<(String, String, Value)>,
    /// What `GET …/queue` answers.
    queue: Value,
    /// What `POST …/queue` answers instead of 201.
    refuse: Option<(u16, Value)>,
    gets: usize,
    /// Every `POST …/attachments`: the path, the type and the raw body (piece O).
    uploads: Vec<(String, String, Vec<u8>)>,
    /// What an upload answers instead of 200 (a status and the body).
    refuse_uploads: Option<(u16, Value)>,
}

fn serve(mut stream: TcpStream, seen: Arc<Mutex<Seen>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let (mut head, mut length, mut content_type) = (String::new(), 0usize, String::new());
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        if line == "\r\n" {
            break;
        }
        if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            length = n.trim().parse().unwrap_or(0);
        }
        if line.to_ascii_lowercase().starts_with("content-type:") {
            content_type = line["content-type:".len()..].trim().to_string();
        }
        head.push_str(&line);
    }
    let mut raw = vec![0; length];
    let _ = reader.read_exact(&mut raw);
    let body: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
    let request = head.lines().next().unwrap_or("").to_string();
    let (method, path) = {
        let mut parts = request.split(' ');
        (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string())
    };
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
    } else if method == "POST" && path.contains("/attachments") {
        seen.uploads.push((path.clone(), content_type, raw.clone()));
        match seen.refuse_uploads.clone() {
            Some((code, answer)) => reply(&mut stream, &format!("{code} Refused"), &answer.to_string()),
            None => {
                let name = path.split("name=").nth(1).unwrap_or("file").to_string();
                let text = String::from_utf8(raw.clone()).ok();
                let lines = text.map(|t| crate::composer::attached::count_lines(&t));
                let answer = json!({"path": format!(".weave/attachments/3f9a2c1d/{name}"), "name": name, "size": raw.len(), "lines": lines, "mime": "text/plain"});
                reply(&mut stream, "200 OK", &answer.to_string());
            }
        }
    } else if method == "GET" && path.ends_with("/queue") {
        seen.gets += 1;
        let answer = seen.queue.to_string();
        reply(&mut stream, "200 OK", &answer);
    } else if method == "POST" && path.ends_with("/shell") {
        let refused = seen.refuse.clone();
        seen.calls.push((method, path, body));
        match refused {
            Some((code, answer)) => reply(&mut stream, &format!("{code} Refused"), &answer.to_string()),
            None => reply(&mut stream, "202 Accepted", "{}"),
        }
    } else if method == "POST" && path.ends_with("/queue") {
        let refused = seen.refuse.clone();
        seen.calls.push((method, path, body.clone()));
        match refused {
            Some((code, answer)) => reply(&mut stream, &format!("{code} Refused"), &answer.to_string()),
            None => {
                let created = json!({"id": "queued-new", "kind": body["kind"], "text": body["text"]});
                reply(&mut stream, "201 Created", &created.to_string());
            }
        }
    } else if method == "POST" && path.ends_with("/prompt") && seen.refuse.is_some() {
        let (code, answer) = seen.refuse.clone().unwrap();
        seen.calls.push((method, path, body));
        reply(&mut stream, &format!("{code} Refused"), &answer.to_string());
    } else if path.contains("/queue/") || path.ends_with("/prompt") {
        let code = if method == "DELETE" {
            "204 No Content"
        } else if path.ends_with("/send") {
            "202 Accepted"
        } else {
            "200 OK"
        };
        seen.calls.push((method, path, body));
        reply(&mut stream, code, "{}");
    } else {
        reply(&mut stream, "404 Not Found", "{}");
    }
}

fn server(queue: Value, refuse: Option<(u16, Value)>) -> (String, Arc<Mutex<Seen>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Seen { queue, refuse, ..Default::default() }));
    let shared = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let seen = shared.clone();
            std::thread::spawn(move || serve(stream, seen));
        }
    });
    (base, seen)
}

pub(super) struct Live {
    pub(super) rt: Runtime<State>,
    pub(super) id: String,
    jobs: std::sync::mpsc::Receiver<Job>,
    seen: Arc<Mutex<Seen>>,
    kind: String,
}

impl Live {
    /// The fixture's first session open on a server, working, on a harness that can steer (`steers_by_default` or not).
    pub(super) fn start(queue: Value, refuse: Option<(u16, Value)>, kind: &str, by_default: bool) -> Live {
        let (base, seen) = server(queue, refuse);
        let (tx, jobs) = channel::<Job>();
        let tx = Mutex::new(tx);
        let post = Post::new(move |job| {
            let _ = tx.lock().unwrap().send(job);
        });
        let mut rt = app();
        let link = Link::start(base, post);
        let kind = kind.to_string();
        let name = kind.clone();
        rt.update(move |s, tx| {
            s.link = Some(link);
            s.pickers.harnesses = vec![HarnessInfo {
                kind: kind.clone(),
                supports_steering: true,
                steers_by_default: by_default,
                steer_delivery: Some(SteerDelivery::NextStep),
                ..Default::default()
            }];
            s.on_list(sessions::parse_list(&serde_json::from_str(&fixture("api-sessions.json")).unwrap(), 0), tx);
            s.projects[0].sessions[0].row.harness_type = Some(kind);
            s.projects[0].sessions[0].status = Status::Working;
        });
        let id = rt.state.open_id().unwrap();
        let mut live = Live { rt, id, jobs, seen, kind: name };
        live.rt.frame(FRAME);
        live
    }

    /// Runs the app until `until` holds.
    pub(super) fn wait(&mut self, until: &dyn Fn(&Live) -> bool, what: &str) {
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

    /// Every upload the server saw: its path, its type and its body.
    pub(super) fn uploads(&self) -> Vec<(String, String, Vec<u8>)> {
        self.seen.lock().unwrap().uploads.clone()
    }

    pub(super) fn refuse_uploads(&self, status: u16, answer: Value) {
        self.seen.lock().unwrap().refuse_uploads = Some((status, answer));
    }

    pub(super) fn calls(&self) -> Vec<(String, String, Value)> {
        self.seen.lock().unwrap().calls.clone()
    }

    /// The list is fetched again behind our back and puts back what the fixture says: say it again.
    pub(super) fn working(&mut self) {
        let kind = self.kind.clone();
        self.rt.update(move |s, _| {
            s.projects[0].sessions[0].status = Status::Working;
            s.projects[0].sessions[0].row.harness_type = Some(kind);
        });
    }
}

// use-session-queue.test.ts: "loads the queue when the session opens"
// Composer steering: "shows what Fleet has queued when the session opens again"
#[test]
fn loads_the_queue_when_the_session_opens_and_again_after_a_reconnect() {
    let mut live = Live::start(json!([{"id": "queued-kept", "kind": "prompt", "text": "queued before you left"}]), None, "opencode", false);
    live.rt.update(|s, _| {
        let id = s.open_id().unwrap();
        s.load_queue(&id);
    });
    live.wait(&|l| l.rt.state.queued().len() == 1, "the queue to load");
    assert_eq!(live.rt.state.queued()[0].text, "queued before you left");
    // use-session-queue.test.ts: "loads again after a reconnect and stops listening when unmounted"
    let before = live.seen.lock().unwrap().gets;
    live.seen.lock().unwrap().queue = json!([]);
    live.rt.update(|s, _| s.on_reconnected());
    live.wait(&|l| l.rt.state.queued().is_empty() && l.seen.lock().unwrap().gets > before, "the queue to load again");
}

// Composer steering: "queues a message on Enter while the agent works, and sends it into the turn on Ctrl+Enter"
#[test]
fn enter_queues_and_ctrl_enter_steers_on_a_harness_that_queues_by_default() {
    let mut live = Live::start(json!([]), None, "opencode", false);
    typed(&mut live.rt, "also check the tests");
    press(&mut live.rt, Key::Enter, Mods::default());
    live.wait(&|l| l.calls().len() == 1, "the queue request");
    let calls = live.calls();
    assert_eq!(calls[0].0, "POST");
    assert!(calls[0].1.ends_with("/queue"), "{}", calls[0].1);
    assert_eq!((calls[0].2["text"].as_str(), calls[0].2["kind"].as_str()), (Some("also check the tests"), Some("prompt")));
    live.wait(&|l| l.rt.state.queued().len() == 1, "the queued item to show");

    live.working();
    typed(&mut live.rt, "stop, that's the wrong file");
    press(&mut live.rt, Key::Enter, ctrl());
    live.wait(&|l| l.calls().len() == 2, "the prompt");
    let calls = live.calls();
    assert!(calls[1].1.ends_with("/prompt"), "{}", calls[1].1);
    assert_eq!(calls[1].2["text"], "stop, that's the wrong file");
    assert_eq!(calls[1].2["delivery"], "steer");
    assert_eq!(live.rt.state.queued().len(), 1, "the steered one never joined the queue");
}

// Composer steering: "on OpenCode 2, Enter sends into the turn and Ctrl+Enter or Queue waits for it to end"
#[test]
fn on_opencode_2_enter_steers_and_ctrl_enter_or_the_queue_button_waits() {
    let mut live = Live::start(json!([]), None, "opencode2", true);
    typed(&mut live.rt, "stop, that's the wrong file");
    assert!(live.rt.find("prompt-send-now-button").is_none());
    press(&mut live.rt, Key::Enter, Mods::default());
    live.wait(&|l| l.calls().len() == 1, "the steered prompt");
    assert_eq!(live.calls()[0].2["delivery"], "steer");

    live.working();
    typed(&mut live.rt, "also check the tests");
    press(&mut live.rt, Key::Enter, ctrl());
    live.working();
    typed(&mut live.rt, "and the docs");
    click(&mut live.rt, "prompt-queue-button");
    live.wait(&|l| l.calls().len() == 3, "the two queue requests");
    let calls = live.calls();
    assert_eq!(calls.iter().filter(|c| c.1.ends_with("/prompt")).count(), 1);
    let queued: Vec<&str> = calls.iter().filter(|c| c.1.ends_with("/queue")).filter_map(|c| c.2["text"].as_str()).collect();
    assert_eq!(queued, ["also check the tests", "and the docs"]);
}

// Composer steering: "queues a slash command as a command, to run when the turn ends"
#[test]
fn a_slash_command_is_queued_with_its_name_and_arguments() {
    let mut live = Live::start(json!([]), None, "opencode", false);
    typed(&mut live.rt, "/review src/auth");
    press(&mut live.rt, Key::Enter, ctrl());
    live.wait(&|l| l.calls().len() == 1, "the queue request");
    let body = live.calls()[0].2.clone();
    assert_eq!(
        (body["kind"].as_str(), body["command"].as_str(), body["arguments"].as_str()),
        (Some("command"), Some("review"), Some("src/auth"))
    );
    assert!(!live.calls().iter().any(|c| c.1.ends_with("/prompt")));
}

// use-session-queue.test.ts: "takes an item back, and sends one now"
// Composer steering: "sends a queued message into the turn with Send now, leaving the draft alone"
#[test]
fn takes_an_item_back_and_sends_one_now() {
    let mut live = Live::start(json!([{"id": "queued-a", "text": "first"}, {"id": "queued-b", "text": "second"}]), None, "opencode", false);
    live.rt.update(|s, _| {
        let id = s.open_id().unwrap();
        s.load_queue(&id);
    });
    live.wait(&|l| l.rt.state.queued().len() == 2, "the queue to load");
    live.rt.frame(FRAME);
    click(&mut live.rt, "queued-remove-1");
    assert_eq!(live.rt.state.queued().len(), 1, "it leaves at once");
    live.wait(&|l| l.calls().len() == 1, "the delete");
    typed(&mut live.rt, "still typing");
    click(&mut live.rt, "queued-send-now-0");
    live.wait(&|l| l.calls().len() == 2, "the send");
    let calls = live.calls();
    assert_eq!((calls[0].0.as_str(), calls[0].1.as_str()), ("DELETE", &*format!("/api/sessions/{}/queue/queued-b", live.id)));
    assert_eq!((calls[1].0.as_str(), calls[1].1.as_str()), ("POST", &*format!("/api/sessions/{}/queue/queued-a/send", live.id)));
    live.wait(&|l| l.rt.state.queued().is_empty(), "the item to leave");
    assert_eq!(live.rt.state.draft.text, "still typing");
}

#[test]
fn a_refused_queue_request_says_why_and_the_text_comes_back() {
    let mut live = Live::start(json!([]), Some((409, json!({"error": "This session is archived."}))), "opencode", false);
    typed(&mut live.rt, "also check the tests");
    press(&mut live.rt, Key::Enter, Mods::default());
    live.wait(&|l| l.rt.state.composer_error.is_some(), "the refusal");
    assert_eq!(live.rt.state.composer_error.as_deref(), Some("This session is archived."));
    assert_eq!(live.rt.state.draft.text, "also check the tests");
    assert!(live.rt.state.queued().is_empty());
    live.rt.frame(FRAME);
    assert!(live.rt.find("send-prompt-error").is_some());
}
