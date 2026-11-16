//! The connection itself, against a fake server on loopback.
//!
//! A stand-in for Fleet on 127.0.0.1: serves the session list, the hub's negotiate and WebSocket
//! (replaying the captured frames) and records the prompts it is sent. Everything it says was
//! captured from a real server.
use super::*;
use crate::fleet::link::{Connection, Job, Link, Post};
use crate::{State, spec};
use rune::{App, Env};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Sender, channel};
use tungstenite::Message;

fn reply(stream: &mut TcpStream, body: &str) {
    let head =
        format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
    let _ = stream.write_all(head.as_bytes());
}

fn serve(mut stream: TcpStream, prompts: Sender<String>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut head = String::new();
    let mut length = 0;
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
        head.push_str(&line);
    }
    let mut body = vec![0; length];
    let _ = reader.read_exact(&mut body);
    let request = head.lines().next().unwrap_or("").to_string();
    if request.starts_with("POST /hubs/session-events/negotiate") {
        reply(&mut stream, "{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}");
    } else if request.starts_with("GET /api/sessions") {
        reply(&mut stream, &fixture("api-sessions.json"));
    } else if request.contains("/prompt ") {
        let _ = prompts.send(String::from_utf8_lossy(&body).to_string());
        reply(&mut stream, "{}");
    } else if request.starts_with("GET /hubs/session-events") {
        hub(stream, head);
    }
}

/// The hub: SignalR's handshake, then the captured snapshot and events for a subscription.
fn hub(stream: TcpStream, head: String) {
    // `accept` reads the request itself, so give it back the head already consumed.
    struct Replay(std::io::Cursor<Vec<u8>>, TcpStream);
    impl Read for Replay {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            match self.0.read(buf)? {
                0 => self.1.read(buf),
                n => Ok(n),
            }
        }
    }
    impl Write for Replay {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.1.write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.1.flush()
        }
    }
    let replay = Replay(std::io::Cursor::new(format!("{head}\r\n").into_bytes()), stream);
    let Ok(mut ws) = tungstenite::accept(replay) else { return };
    let live = frames_of("hub-turn-run-tests.jsonl");
    while let Ok(Message::Text(text)) = ws.read() {
        for frame in text.split('\u{1e}').filter(|f| !f.is_empty()) {
            let call: Value = serde_json::from_str(frame).unwrap();
            let reply_with = |ws: &mut tungstenite::WebSocket<Replay>, v: Value| ws.send(Message::text(format!("{v}\u{1e}"))).unwrap();
            match call["target"].as_str() {
                None if call["protocol"] == "json" => ws.send(Message::text("{}\u{1e}")).unwrap(),
                Some("SubscribeToSessionsTopicAsync") => {
                    reply_with(&mut ws, json!({"type": 3, "invocationId": call["invocationId"], "result": null}))
                }
                Some("SubscribeToSessionAsync") => {
                    reply_with(&mut ws, json!({"type": 3, "invocationId": call["invocationId"], "result": snapshot_of(&live)}));
                    for f in &live {
                        if let signalr::Frame::Event { topic, data, .. } = f {
                            reply_with(&mut ws, json!({"type": 1, "target": "Event", "arguments": [topic, 0, data]}));
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[test]
fn the_hub_thread_fills_the_state_from_a_server_and_sends_prompts() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (prompts, sent) = channel();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let prompts = prompts.clone();
            std::thread::spawn(move || serve(stream, prompts));
        }
    });

    let (jobs, queue) = channel::<Job>();
    let jobs = std::sync::Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = App::new(spec()).headless_runtime((1280.0, 760.0), Env::new(true, 1.0));
    let link = Link::start(base, post);
    rt.update(move |s, _| s.link = Some(link));
    // The server replays the turn's frames one by one; the reply text arrives before the closing
    // status event, so wait for the status too instead of reading it mid-replay.
    let done = |s: &State| {
        s.connection == Connection::Online
            && s.session().status == Status::Idle
            && s.session()
                .messages
                .iter()
                .any(|m| m.parts.iter().any(|p| matches!(p, Part::Text { text, .. } if text.contains("All 42 tests pass"))))
    };
    crate::test_wait::drive(&mut rt, &queue, "the list, snapshot and events", done, crate::test_wait::app_state);
    assert_eq!(rt.state.session().title, "Fix pagination");
    assert_eq!(rt.state.session().status, Status::Idle);

    let id = rt.state.open_id().unwrap();
    rt.update(move |s, _| s.send_to_server(&id, "run-tests"));
    let body: Value = serde_json::from_str(&crate::test_wait::recv("the prompt to reach the server", &sent)).unwrap();
    assert_eq!(body["text"], "run-tests");
    // The correlation id the optimistic prompt waits on goes with it.
    assert!(body["correlationId"].as_str().unwrap().starts_with("prompt-"), "{body}");
}
