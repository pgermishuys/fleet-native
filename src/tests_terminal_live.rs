//! Component 13, piece A, against a fake Fleet on loopback: `terminal-socket.test.ts` ("connects with the terminal's size, then
//! gets the scrollback, ready, and live output"; "sends input as binary and resize and clear as text") and the slice end to end:
//! the header button lists or starts the session's terminal, its output is drawn, typing reaches it, and a resize is seen.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::terminal_socket::{MAX_FAILED_ATTEMPTS, Socket, Status, reconnect_delay};
use crate::term::{Terminal, text_of};
use crate::test_wait::{drive, until_true};
use crate::tests::{FRAME, app, click};
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::{Value, json};
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tungstenite::Message;
use tungstenite::protocol::CloseFrame;
use tungstenite::protocol::frame::coding::CloseCode;

/// What the fake Fleet was asked, and what its socket heard.
#[derive(Default)]
struct Heard {
    /// (method, path) of each HTTP request, and the body of a POST.
    requests: Vec<(String, String, Value)>,
    /// The socket's request path, with its query.
    sockets: Vec<String>,
    input: Vec<u8>,
    texts: Vec<Value>,
    /// The close codes the client sent (with the reason).
    closes: Vec<(u16, String)>,
}

/// What a test tells the fake's socket to do next.
enum Cmd {
    /// A text frame (`exit`, `cleared`).
    Text(Value),
    /// A close frame with this code, then the socket ends.
    Close(u16),
    /// The connection is cut with no close frame (a dropped network).
    Cut,
}

/// What every connection of one fake shares: the live output, the commands, and the two switches.
struct Shared {
    live: Mutex<Receiver<Vec<u8>>>,
    cmds: Mutex<Receiver<Cmd>>,
    /// While set, a socket is accepted at the TCP level and dropped before the upgrade: it never gets the scrollback.
    refuse: AtomicBool,
    /// While set, a socket sends its scrollback but holds back `ready`.
    hold: AtomicBool,
}

struct Fake {
    base: String,
    heard: Arc<Mutex<Heard>>,
    /// Live output for the socket to send.
    live: Sender<Vec<u8>>,
    cmds: Sender<Cmd>,
    shared: Arc<Shared>,
}

impl Fake {
    /// `listed` are the session's terminals; `scrollback` is what a socket replays before `ready`.
    fn start(listed: &'static [&'static str], scrollback: &'static [u8]) -> Fake {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let heard = Arc::new(Mutex::new(Heard::default()));
        let (live, rx) = channel();
        let (cmds, cmd_rx) = channel();
        let shared = Arc::new(Shared {
            live: Mutex::new(rx),
            cmds: Mutex::new(cmd_rx),
            refuse: AtomicBool::new(false),
            hold: AtomicBool::new(false),
        });
        let (heard2, shared2) = (heard.clone(), shared.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (heard, shared) = (heard2.clone(), shared2.clone());
                std::thread::spawn(move || serve(stream, listed, scrollback, heard, shared));
            }
        });
        Fake { base, heard, live, cmds, shared }
    }

    fn input(&self) -> Vec<u8> {
        self.heard.lock().unwrap().input.clone()
    }

    fn sockets(&self) -> Vec<String> {
        self.heard.lock().unwrap().sockets.clone()
    }

    fn texts(&self) -> Vec<Value> {
        self.heard.lock().unwrap().texts.clone()
    }

    /// What was asked of the terminals' routes (the hub's and the list's own calls are not what is counted).
    fn requests(&self) -> Vec<(String, String, Value)> {
        self.heard.lock().unwrap().requests.iter().filter(|(_, p, _)| p.contains("/terminals")).cloned().collect()
    }
}

fn serve(stream: TcpStream, listed: &[&str], scrollback: &[u8], heard: Arc<Mutex<Heard>>, shared: Arc<Shared>) {
    // Look at the request without taking it: a socket's upgrade is read by `accept` itself.
    let mut buf = [0u8; 4096];
    let head = loop {
        let n = stream.peek(&mut buf).unwrap_or(0);
        if n == 0 {
            return;
        }
        if let Some(end) = String::from_utf8_lossy(&buf[..n]).find("\r\n\r\n") {
            break String::from_utf8_lossy(&buf[..end]).to_string();
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    };
    let line = head.lines().next().unwrap_or("").to_string();
    let path = line.split(' ').nth(1).unwrap_or("").to_string();
    if path.contains("/socket?") {
        heard.lock().unwrap().sockets.push(path);
        if shared.refuse.load(Ordering::SeqCst) {
            return;
        }
        return socket(stream, scrollback, heard, &shared);
    }
    let mut stream = stream;
    let body = crate::test_http::read(&mut stream).json();
    let method = line.split(' ').next().unwrap_or("").to_string();
    let entry = |id: &str| json!({"id": id, "title": "zsh", "status": "running", "createdAt": "2026-11-18T10:00:00Z"});
    let (status, text) = match (method.as_str(), path.ends_with("/terminals")) {
        ("GET", true) => (200, Value::Array(listed.iter().map(|id| entry(id)).collect()).to_string()),
        ("POST", true) => (201, entry("t-new").to_string()),
        _ => (404, "{}".to_string()),
    };
    heard.lock().unwrap().requests.push((method, path, body));
    let _ = stream.write_all(
        format!(
            "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
            text.len()
        )
        .as_bytes(),
    );
}

/// `TerminalSocket.cs`'s side: the saved scrollback in binary frames, `ready`, then live output; input and control are heard.
fn socket(stream: TcpStream, scrollback: &[u8], heard: Arc<Mutex<Heard>>, shared: &Shared) {
    let Ok(mut ws) = tungstenite::accept(stream) else { return };
    for chunk in scrollback.chunks(16 * 1024) {
        let _ = ws.send(Message::binary(chunk.to_vec()));
    }
    while shared.hold.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let _ = ws.send(Message::text(r#"{"type":"ready"}"#));
    let _ = ws.get_ref().set_read_timeout(Some(std::time::Duration::from_millis(5)));
    loop {
        while let Ok(out) = shared.live.lock().unwrap().try_recv() {
            if ws.send(Message::binary(out)).is_err() {
                return;
            }
        }
        let cmd = shared.cmds.lock().unwrap().try_recv();
        match cmd {
            Ok(Cmd::Text(v)) => {
                let _ = ws.send(Message::text(v.to_string()));
            }
            Ok(Cmd::Close(code)) => {
                let _ = ws.close(Some(CloseFrame { code: CloseCode::from(code), reason: "".into() }));
                let _ = ws.flush();
                return;
            }
            Ok(Cmd::Cut) => return,
            Err(_) => {}
        }
        match ws.read() {
            Ok(Message::Binary(b)) => heard.lock().unwrap().input.extend_from_slice(&b),
            Ok(Message::Text(t)) => heard.lock().unwrap().texts.push(serde_json::from_str(&t).unwrap_or(Value::Null)),
            Ok(Message::Close(frame)) => {
                let (code, reason) = frame.map_or((1005, String::new()), |f| (u16::from(f.code), f.reason.to_string()));
                heard.lock().unwrap().closes.push((code, reason));
                return;
            }
            Err(tungstenite::Error::ConnectionClosed) => return,
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(_) => return,
            Ok(_) => {}
        }
    }
}

const PATH: &str = "/api/sessions/s1/terminals/t1";

fn open(fake: &Fake, size: (u16, u16)) -> (Socket, Arc<Terminal>) {
    let term = Arc::new(Terminal::new(size.0, size.1));
    (Socket::open(&fake.base, PATH, size, term.clone(), || {}), term)
}

// terminal-socket.test.ts: "connects with the terminal's size, then gets the scrollback, ready, and live output"
#[test]
fn connects_with_the_terminals_size_then_gets_the_scrollback_ready_and_live_output() {
    let fake = Fake::start(&[], b"saved line\r\ndemo $ ");
    let (socket, term) = open(&fake, (100, 30));
    until_true("the socket to be ready", || socket.status() == Status::Open);
    assert_eq!(fake.heard.lock().unwrap().sockets, [format!("{PATH}/socket?cols=100&rows=30")]);
    assert_eq!(text_of(&term)[..2], ["saved line", "demo $"], "the scrollback came before ready");
    fake.live.send(b"ls\r\nsrc  docs\r\n".to_vec()).unwrap();
    until_true("live output", || text_of(&term).iter().any(|r| r == "src  docs"));
}

// terminal-socket.test.ts: "sends input as binary and resize and clear as text"
#[test]
fn sends_input_as_binary_and_resize_and_clear_as_text() {
    let fake = Fake::start(&[], b"");
    let (socket, _term) = open(&fake, (80, 24));
    until_true("the socket to be ready", || socket.status() == Status::Open);
    socket.write(b"ls\r".to_vec());
    socket.resize(120, 30);
    socket.clear();
    until_true("all three to arrive", || fake.texts().len() == 2 && fake.input() == b"ls\r");
    assert_eq!(fake.texts(), [json!({"type": "resize", "cols": 120, "rows": 30}), json!({"type": "clear"})]);
}

// 13.16: what the parser answers goes back as input, as xterm.js answers through `onData`.
#[test]
fn a_cursor_position_query_from_the_shell_is_answered_over_the_socket() {
    let fake = Fake::start(&[], b"one\r\ntwo");
    let (socket, _term) = open(&fake, (80, 24));
    until_true("the socket to be ready", || socket.status() == Status::Open);
    fake.live.send(b"\x1b[6n".to_vec()).unwrap();
    until_true("the answer", || fake.input() == b"\x1b[2;4R");
}

// ---- the connection, whole (piece B) -------------------------------------------------------------------------------------------

/// Waits of a few milliseconds, so a test needn't sit through 250 ms to 8 s.
fn quick(_attempt: u32) -> Duration {
    Duration::from_millis(2)
}

fn open_quick(fake: &Fake, path: &str, size: (u16, u16)) -> (Socket, Arc<Terminal>) {
    let term = Arc::new(Terminal::new(size.0, size.1));
    (Socket::open_with(&fake.base, path, size, term.clone(), quick, || {}), term)
}

/// After `what` happened, a connection that must not come stays away: the waits are 2 ms, so this is dozens of them.
fn nothing_more_comes(fake: &Fake, sockets: usize) {
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(fake.sockets().len(), sockets, "a connection was made that shouldn't have been");
}

#[test]
fn the_waits_between_attempts_are_fleets() {
    let waits: Vec<u128> = (1..=8).map(|n| reconnect_delay(n).as_millis()).collect();
    assert_eq!(waits, [250, 500, 1000, 2000, 4000, 8000, 8000, 8000]);
}

// terminal-socket.test.ts: "connects to a terminal that isn't a session's under its base path"
#[test]
fn connects_to_a_terminal_that_isnt_a_sessions_under_its_base_path() {
    let fake = Fake::start(&[], b"");
    let (socket, _term) = open_quick(&fake, "/api/setup/terminals/t1", (100, 30));
    until_true("the socket to be ready", || socket.status() == Status::Open);
    assert_eq!(fake.sockets(), ["/api/setup/terminals/t1/socket?cols=100&rows=30"]);
}

// session-calls-machine.test.ts: "opens a terminal's socket on the machine provided, with its token in the query"
#[test]
fn opens_a_terminals_socket_on_the_machine_provided_with_its_token_in_the_query() {
    let fake = Fake::start(&[], b"");
    crate::fleet::auth::set_for(&fake.base, Some("falcon+token/1"));
    let (socket, _term) = open_quick(&fake, PATH, (80, 24));
    until_true("the socket to be ready", || socket.status() == Status::Open);
    crate::fleet::auth::set_for(&fake.base, None);
    assert_eq!(fake.sockets(), [format!("{PATH}/socket?cols=80&rows=24&access_token=falcon%2Btoken%2F1")]);
    // Another machine's address carries its own token and never this one's.
    let url = crate::fleet::terminal_socket::socket_url("https://studio.example.test", PATH, 80, 24).unwrap();
    assert_eq!(url, format!("wss://studio.example.test:443{PATH}/socket?cols=80&rows=24"));
}

// terminal-socket.test.ts: "keeps what's typed before the socket is ready and sends it after"
#[test]
fn keeps_whats_typed_before_the_socket_is_ready_and_sends_it_after() {
    let fake = Fake::start(&[], b"");
    fake.shared.hold.store(true, Ordering::SeqCst);
    let (socket, _term) = open_quick(&fake, PATH, (80, 24));
    until_true("the socket to connect", || fake.sockets().len() == 1);
    socket.write(b"echo early\r".to_vec());
    assert_eq!(socket.status(), Status::Connecting);
    fake.shared.hold.store(false, Ordering::SeqCst);
    until_true("the early input to arrive", || fake.input() == b"echo early\r");
    assert_eq!(socket.status(), Status::Open);
}

// terminal-socket.test.ts: "reports the exit and doesn't reconnect after the shell ends"
#[test]
fn reports_the_exit_and_doesnt_reconnect_after_the_shell_ends() {
    let fake = Fake::start(&[], b"");
    let (socket, _term) = open_quick(&fake, PATH, (80, 24));
    until_true("the socket to be ready", || socket.status() == Status::Open);
    fake.cmds.send(Cmd::Text(json!({"type": "exit", "exitCode": 3}))).unwrap();
    fake.cmds.send(Cmd::Close(1000)).unwrap();
    until_true("the exit", || socket.status() == Status::Ended(Some(3)));
    nothing_more_comes(&fake, 1);
    assert_eq!(socket.status(), Status::Ended(Some(3)), "the close after the exit doesn't take the code away");
    // Typing at an ended shell goes nowhere.
    socket.write(b"ls\r".to_vec());
    assert!(fake.input().is_empty());
}

// terminal-socket.test.ts: "reports a null exit code when Fleet ended the shell"
#[test]
fn reports_a_null_exit_code_when_fleet_ended_the_shell() {
    let fake = Fake::start(&[], b"");
    let (socket, _term) = open_quick(&fake, PATH, (80, 24));
    until_true("the socket to be ready", || socket.status() == Status::Open);
    fake.cmds.send(Cmd::Text(json!({"type": "exit", "exitCode": null}))).unwrap();
    until_true("the exit", || socket.status() == Status::Ended(None));
}

// 13.24: a normal close with no `exit` is an end too.
#[test]
fn a_normal_close_ends_it_with_no_reconnect() {
    let fake = Fake::start(&[], b"");
    let (socket, _term) = open_quick(&fake, PATH, (80, 24));
    until_true("the socket to be ready", || socket.status() == Status::Open);
    fake.cmds.send(Cmd::Close(1000)).unwrap();
    until_true("the end", || socket.status() == Status::Ended(None));
    nothing_more_comes(&fake, 1);
}

// terminal-socket.test.ts: "reconnects after a drop, and the new connection starts from a reset"; ours: 4001, then 1001, then a cut
#[test]
fn reconnects_after_a_drop_and_the_new_connection_starts_from_a_reset() {
    for (n, drop) in [Some(4001), Some(1001), None].into_iter().enumerate() {
        let fake = Fake::start(&[], b"old output\r\n");
        // A long wait, so Reconnecting can be seen standing between the two.
        let term = Arc::new(Terminal::new(80, 24));
        let socket = Socket::open_with(&fake.base, PATH, (80, 24), term.clone(), |_| Duration::from_millis(400), || {});
        until_true("the socket to be ready", || socket.status() == Status::Open);
        fake.live.send(b"live line\r\n".to_vec()).unwrap();
        until_true("live output", || text_of(&term).iter().any(|r| r == "live line"));
        fake.cmds.send(drop.map_or(Cmd::Cut, Cmd::Close)).unwrap();
        until_true("the drop to be noticed", || socket.status() == Status::Reconnecting);
        until_true("the second connection", || fake.sockets().len() == 2 && socket.status() == Status::Open);
        // The scrollback replayed into a blank terminal: once, and the live line of the first connection is gone.
        let rows = text_of(&term);
        assert_eq!(rows.iter().filter(|r| *r == "old output").count(), 1, "case {n}: {rows:?}");
        assert!(!rows.iter().any(|r| r == "live line"), "case {n}: {rows:?}");
        // Input after the reconnect reaches the new socket.
        socket.write(b"x".to_vec());
        until_true("input on the new socket", || fake.input() == b"x");
    }
}

// terminal-socket.test.ts: "gives up after repeated attempts that never get the scrollback"
#[test]
fn gives_up_after_repeated_attempts_that_never_get_the_scrollback() {
    let fake = Fake::start(&[], b"");
    fake.shared.refuse.store(true, Ordering::SeqCst);
    let (socket, _term) = open_quick(&fake, PATH, (80, 24));
    until_true("the last attempt", || socket.status() == Status::Failed);
    assert_eq!(fake.sockets().len(), MAX_FAILED_ATTEMPTS as usize);
    nothing_more_comes(&fake, MAX_FAILED_ATTEMPTS as usize);
}

// 13.26: an attempt that reached `ready` starts the count again, so a flaky network never fails.
#[test]
fn an_attempt_that_reached_ready_starts_the_count_again() {
    let fake = Fake::start(&[], b"");
    let (socket, _term) = open_quick(&fake, PATH, (80, 24));
    until_true("the socket to be ready", || socket.status() == Status::Open);
    // Four drops that reach `ready`, then refusals: failing takes five of those, not one.
    for round in 1..=4 {
        fake.cmds.send(Cmd::Close(4001)).unwrap();
        until_true("the next connection", || fake.sockets().len() == round + 1 && socket.status() == Status::Open);
    }
    fake.shared.refuse.store(true, Ordering::SeqCst);
    fake.cmds.send(Cmd::Cut).unwrap();
    until_true("the end", || socket.status() == Status::Failed);
    assert_eq!(fake.sockets().len(), 5 + MAX_FAILED_ATTEMPTS as usize, "five refusals after the fifth good connection");
}

// terminal-socket.test.ts: "closes normally when disposed and doesn't reconnect"
#[test]
fn closes_normally_when_disposed_and_doesnt_reconnect() {
    let fake = Fake::start(&[], b"");
    let (socket, _term) = open_quick(&fake, PATH, (80, 24));
    until_true("the socket to be ready", || socket.status() == Status::Open);
    drop(socket);
    until_true("the close", || !fake.heard.lock().unwrap().closes.is_empty());
    assert_eq!(fake.heard.lock().unwrap().closes, [(1000, "Closed by the page.".to_string())]);
    nothing_more_comes(&fake, 1);
}

// Disposing while it waits to reconnect stops it for good.
#[test]
fn disposing_while_reconnecting_stops_the_reconnect() {
    let fake = Fake::start(&[], b"");
    let term = Arc::new(Terminal::new(80, 24));
    let socket = Socket::open_with(&fake.base, PATH, (80, 24), term, |_| Duration::from_millis(300), || {});
    until_true("the socket to be ready", || socket.status() == Status::Open);
    fake.cmds.send(Cmd::Cut).unwrap();
    until_true("the wait", || socket.status() == Status::Reconnecting);
    drop(socket);
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(fake.sockets().len(), 1, "no second connection after the page let go");
}

// terminal-socket.test.ts: "passes on a clear from another window"
#[test]
fn passes_on_a_clear_from_another_window() {
    let fake = Fake::start(&[], b"one\r\ntwo");
    let (socket, term) = open_quick(&fake, PATH, (80, 24));
    until_true("the socket to be ready", || socket.status() == Status::Open);
    assert_eq!(text_of(&term)[..2], ["one", "two"]);
    fake.cmds.send(Cmd::Text(json!({"type": "cleared"}))).unwrap();
    until_true("the screen to clear", || text_of(&term).iter().all(String::is_empty));
}

// ---- the slice, end to end --------------------------------------------------------------------------------------------------

struct Live {
    rt: Runtime<crate::State>,
    jobs: Receiver<Job>,
    fake: Fake,
}

impl Live {
    fn start(fake: Fake) -> Live {
        let (tx, jobs) = channel::<Job>();
        let tx = Mutex::new(tx);
        let post = Post::new(move |job| drop(tx.lock().unwrap().send(job)));
        let link = Link::start(fake.base.clone(), post);
        let mut rt = app();
        rt.update(move |s, _| {
            s.link = Some(link);
            s.terminals_allowed = Some(true);
            // A session the list holds, so the app has one open; the fake answers whatever it is asked.
            s.set_session_id((0, 0), "s1");
        });
        rt.frame(FRAME);
        Live { rt, jobs, fake }
    }

    fn wait(&mut self, what: &str, done: impl FnMut(&crate::State) -> bool) {
        let requests = self.fake.heard.clone();
        drive(&mut self.rt, &self.jobs, what, done, move |s| {
            format!("{}; {:?}", crate::test_wait::app_state(s), requests.lock().unwrap().requests)
        });
    }

    fn screen(&self) -> Vec<String> {
        self.rt.state.terminal.live.as_ref().map(|l| text_of(&l.term)).unwrap_or_default()
    }
}

fn type_text(rt: &mut Runtime<crate::State>, text: &str) {
    for ch in text.chars() {
        rt.key(KeyEvent { key: Key::Text(ch.to_string()), mods: Mods::default() });
    }
    rt.frame(FRAME);
}

#[test]
fn pressing_the_header_button_shows_the_prompt_and_what_is_typed_reaches_the_shell() {
    let mut live = Live::start(Fake::start(&["t1"], b"demo $ "));
    assert!(live.rt.find("terminal-drawer").is_none(), "nothing is drawn until it is opened");
    click(&mut live.rt, "terminal");
    assert!(live.rt.find("terminal-drawer").is_some());
    live.wait("the prompt", |s| s.terminal.live.is_some());
    live.wait("the scrollback to be drawn", |s| s.terminal.status() == Some(Status::Open));
    assert_eq!(live.screen()[0], "demo $");
    // The first of the session's terminals is used; none is created.
    assert!(live.fake.requests().iter().all(|(m, _, _)| m == "GET"), "{:?}", live.fake.requests());
    click(&mut live.rt, "terminal-view");
    type_text(&mut live.rt, "ls");
    live.rt.key(KeyEvent { key: Key::Enter, mods: Mods::default() });
    live.rt.frame(FRAME);
    crate::test_wait::until(
        &mut (),
        "the keys to reach the shell",
        |_| live.fake.input() == b"ls\r",
        |_| format!("input {:?}, focused {}", live.fake.input(), live.rt.state.terminal.focused),
    );
    // The shell's echo and output are drawn.
    live.fake.live.send(b"ls\r\nsrc  tests\r\ndemo $ ".to_vec()).unwrap();
    live.wait("the output", |s| s.terminal.live.as_ref().is_some_and(|l| text_of(&l.term).iter().any(|r| r == "src  tests")));
    // The cursor is a block while the box has the keyboard.
    live.rt.frame(FRAME);
    assert!(live.rt.state.terminal.focused);
    assert!(live.rt.find("terminal-cursor").is_some());
}

#[test]
fn with_no_terminals_listed_one_is_started_at_the_views_size_else_120_by_14() {
    let mut live = Live::start(Fake::start(&[], b"demo $ "));
    click(&mut live.rt, "terminal");
    live.wait("the socket to open", |s| s.terminal.status() == Some(Status::Open));
    let posts: Vec<_> = live.fake.requests().into_iter().filter(|(m, _, _)| m == "POST").collect();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].1, "/api/sessions/s1/terminals");
    // The view's size when it was laid out before the list came back, else 120 × 14 (`openNewTerminal`).
    let known = live.rt.state.terminal.fitted.map(|(c, r)| json!({"cols": c, "rows": r}));
    assert!(posts[0].2 == json!({"cols": 120, "rows": 14}) || Some(posts[0].2.clone()) == known, "{:?}", posts[0].2);
    assert!(live.fake.heard.lock().unwrap().sockets[0].contains("/terminals/t-new/socket?"));
}

#[test]
fn a_resize_of_the_box_is_seen_by_the_shell() {
    let mut live = Live::start(Fake::start(&["t1"], b"demo $ "));
    click(&mut live.rt, "terminal");
    live.wait("the socket to open", |s| s.terminal.status() == Some(Status::Open));
    let (cols, rows) = live.rt.state.terminal.fitted.expect("the box measured itself");
    // The shell knows the size the box fits: from the socket's address if the box was laid out first, else from a resize.
    let knows = |fake: &Fake, (c, r): (u16, u16)| {
        let url_size = fake.heard.lock().unwrap().sockets[0].ends_with(&format!("?cols={c}&rows={r}"));
        url_size || fake.texts().last() == Some(&json!({"type": "resize", "cols": c, "rows": r}))
    };
    until_true("the shell to know the size", || knows(&live.fake, (cols, rows)));
    // A taller drawer: more rows, same columns; the shell is told and the grid has them.
    live.rt.update(|s, _| s.terminal.height = 400.0);
    live.rt.frame(FRAME);
    live.rt.frame(FRAME);
    let (cols2, rows2) = live.rt.state.terminal.fitted.unwrap();
    assert_eq!(cols2, cols);
    assert!(rows2 > rows, "{rows} → {rows2}");
    until_true("the resize to arrive", || live.fake.texts().last() == Some(&json!({"type": "resize", "cols": cols2, "rows": rows2})));
    assert_eq!(live.rt.state.terminal.live.as_ref().unwrap().term.size(), (cols2, rows2));
}

#[test]
fn the_toggle_command_runs_and_hides_the_drawer_without_ending_the_shell() {
    let mut live = Live::start(Fake::start(&["t1"], b"demo $ "));
    live.rt.update(|s, tx| crate::ui::commands::run(s, tx, "toggle-terminal"));
    live.rt.frame(FRAME);
    assert!(live.rt.find("terminal-drawer").is_some());
    live.wait("the socket to open", |s| s.terminal.status() == Some(Status::Open));
    live.rt.update(|s, tx| crate::ui::commands::run(s, tx, "toggle-terminal"));
    live.rt.frame(FRAME);
    assert!(live.rt.find("terminal-drawer").is_none());
    assert_eq!(live.rt.state.terminal.status(), Some(Status::Open), "the connection stays");
}

#[test]
fn ctrl_j_toggles_and_other_keys_stay_the_shells() {
    let mut live = Live::start(Fake::start(&["t1"], b"demo $ "));
    live.rt.key(chord(Key::Command('j'), true, false));
    live.rt.frame(FRAME);
    assert!(live.rt.find("terminal-drawer").is_some(), "Ctrl J opened it");
    live.wait("the socket to open", |s| s.terminal.status() == Some(Status::Open));
    click(&mut live.rt, "terminal-view");
    // Esc and Ctrl K reach the shell, not Fleet's Interrupt or palette.
    live.rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    live.rt.key(KeyEvent { key: Key::Command('k'), mods: Mods { ctrl: true, cmd: true, ..Mods::default() } });
    live.rt.frame(FRAME);
    until_true("Esc and Ctrl K to reach the shell", || fake_input_is(&live.fake, &[0x1b, 0x0b]));
    assert!(live.rt.state.ui.open.is_none(), "the palette did not open");
    live.rt.key(chord(Key::Command('j'), true, false));
    live.rt.frame(FRAME);
    assert!(live.rt.find("terminal-drawer").is_none(), "Ctrl J hides it, from inside the terminal");
}

fn fake_input_is(fake: &Fake, want: &[u8]) -> bool {
    fake.input() == want
}

// ---- what the view shows of the connection (13.29, 13.30, 13.24) -------------------------------------------------------------

fn with_status(status: Status) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| {
        s.set_session_id((0, 0), "s1");
        s.terminal.session = s.open_id();
        s.terminals.add("s1", crate::fleet::http::Terminal { id: "replay".into(), title: "zsh".into(), running: true }, true);
        s.terminal.replay(60, 6, b"demo $ ls\r\nsrc  tests\r\ndemo $ ");
        s.terminal.live.as_mut().unwrap().socket = Some(Socket::fixed(status));
    });
    rt.frame(FRAME);
    rt
}

#[test]
fn the_reconnecting_notice_shows_only_while_reconnecting() {
    for (status, notice, failed) in [
        (Status::Reconnecting, true, false),
        (Status::Open, false, false),
        (Status::Connecting, false, false),
        (Status::Failed, false, true),
    ] {
        let rt = with_status(status.clone());
        assert_eq!((rt.find("terminal-notice").is_some(), rt.find("terminal-failed").is_some()), (notice, failed), "{status:?}");
    }
}

#[test]
fn the_failed_panel_says_why_and_close_tab_ends_the_tab() {
    let mut rt = with_status(Status::Failed);
    assert!(rt.find("terminal-close-tab").is_some());
    click(&mut rt, "terminal-close-tab");
    assert!(rt.find("terminal-drawer").is_none(), "the tab and its drawer went");
    assert!(rt.state.terminal.live.is_none());
}

#[test]
fn a_shell_that_exits_takes_its_tab_with_it() {
    let mut live = Live::start(Fake::start(&["t1"], b"demo $ "));
    click(&mut live.rt, "terminal");
    live.wait("the socket to open", |s| s.terminal.status() == Some(Status::Open));
    live.fake.cmds.send(Cmd::Text(json!({"type": "exit", "exitCode": 0}))).unwrap();
    live.fake.cmds.send(Cmd::Close(1000)).unwrap();
    live.wait("the tab to go", |s| s.terminal.live.is_none());
    assert!(live.rt.find("terminal-drawer").is_none());
}

// ---- piece E: the keyboard ----------------------------------------------------------------------------------------------------

fn resizes(fake: &Fake) -> Vec<Value> {
    fake.texts().into_iter().filter(|t| t["type"] == "resize").collect()
}

// TerminalView.test.ts
#[test]
fn tells_the_shell_its_own_size_when_it_gets_the_keyboard_so_another_client_that_attached_since_doesnt_leave_it_wrapping_at_their_width() {
    let mut live = Live::start(Fake::start(&["t1"], b"demo $ "));
    live.rt.update(|s, tx| crate::ui::commands::run(s, tx, "toggle-terminal"));
    live.rt.frame(FRAME);
    live.wait("the socket to open", |s| s.terminal.status() == Some(Status::Open));
    live.rt.update(|_, tx| tx.blur());
    live.rt.frame(FRAME);
    assert!(!live.rt.state.terminal.focused);
    let (cols, rows) = live.rt.state.terminal.live.as_ref().unwrap().term.size();
    let before = resizes(&live.fake).len();

    click(&mut live.rt, "terminal-view");

    let heard = live.fake.heard.clone();
    live.wait("the resize on focus", move |_| heard.lock().unwrap().texts.iter().filter(|t| t["type"] == "resize").count() > before);
    assert_eq!(resizes(&live.fake).last(), Some(&json!({"type": "resize", "cols": cols, "rows": rows})));
}

fn chord(key: Key, ctrl: bool, shift: bool) -> KeyEvent {
    KeyEvent { key, mods: Mods { ctrl, shift, cmd: ctrl, ..Mods::default() } }
}

#[test]
fn esc_ctrl_k_and_ctrl_b_reach_the_shell_and_not_fleet() {
    let mut live = Live::start(Fake::start(&["t1"], b"demo $ "));
    live.rt.key(chord(Key::Command('j'), true, false));
    live.rt.frame(FRAME);
    live.wait("the socket to open", |s| s.terminal.status() == Some(Status::Open));
    click(&mut live.rt, "terminal-view");
    let sidebar = live.rt.state.sidebar_open;
    for ev in
        [KeyEvent { key: Key::Escape, mods: Mods::default() }, chord(Key::Command('k'), true, false), chord(Key::Command('b'), true, false)]
    {
        live.rt.key(ev);
    }
    live.rt.frame(FRAME);
    until_true("the three keys to reach the shell", || fake_input_is(&live.fake, &[0x1b, 0x0b, 0x02]));
    assert!(live.rt.state.ui.open.is_none(), "the palette did not open");
    assert_eq!(live.rt.state.sidebar_open, sidebar, "Ctrl B did not toggle the sidebar");
    // Ctrl Shift B is Fleet's: it never reaches the shell.
    let panel = live.rt.state.content_open;
    live.rt.key(chord(Key::Command('b'), true, true));
    live.rt.frame(FRAME);
    assert_ne!(live.rt.state.content_open, panel);
    assert_eq!(live.fake.input(), [0x1b, 0x0b, 0x02]);
}

#[test]
fn shift_page_up_scrolls_the_view_and_is_not_sent() {
    let lines: String = (1..=60).map(|n| format!("line {n}\r\n")).collect();
    let fake = Fake::start(&["t1"], Box::leak(format!("{lines}demo $ ").into_boxed_str()).as_bytes());
    let mut live = Live::start(fake);
    live.rt.key(chord(Key::Command('j'), true, false));
    live.rt.frame(FRAME);
    live.wait("the scrollback", |s| s.terminal.live.as_ref().is_some_and(|l| text_of(&l.term).iter().any(|r| r == "demo $")));
    click(&mut live.rt, "terminal-view");
    let offset = |live: &Live| live.rt.state.terminal.live.as_ref().unwrap().term.display_offset();
    assert_eq!(offset(&live), 0);

    live.rt.key(KeyEvent { key: Key::PageUp, mods: Mods { shift: true, ..Mods::default() } });
    live.rt.frame(FRAME);
    assert!(offset(&live) > 0, "the view moved back through the scrollback");
    assert!(live.fake.input().is_empty(), "the shell heard nothing");

    // Typing brings the view back to the live screen (xterm's scrollOnUserInput).
    type_text(&mut live.rt, "q");
    assert_eq!(offset(&live), 0);
    until_true("the key", || fake_input_is(&live.fake, b"q"));
}

// On a Mac Ctrl Shift C is the shell's; ⌘ C copies (`terminal_keys` tests).
#[cfg(not(target_os = "macos"))]
#[test]
fn ctrl_shift_c_is_taken_for_copy_and_never_sends_a_ctrl_c() {
    let mut live = Live::start(Fake::start(&["t1"], b"demo $ "));
    live.rt.key(chord(Key::Command('j'), true, false));
    live.rt.frame(FRAME);
    live.wait("the socket to open", |s| s.terminal.status() == Some(Status::Open));
    click(&mut live.rt, "terminal-view");
    live.rt.key(chord(Key::Command('c'), true, true));
    // A plain Ctrl C still interrupts the running command.
    live.rt.key(chord(Key::Command('c'), true, false));
    live.rt.frame(FRAME);
    until_true("the one ^C", || fake_input_is(&live.fake, &[0x03]));
}
