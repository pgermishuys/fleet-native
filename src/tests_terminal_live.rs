//! Component 13, piece A, against a fake Fleet on loopback: `terminal-socket.test.ts` ("connects with the terminal's size, then
//! gets the scrollback, ready, and live output"; "sends input as binary and resize and clear as text") and the slice end to end:
//! the header button lists or starts the session's terminal, its output is drawn, typing reaches it, and a resize is seen.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::terminal_socket::{Socket, Status};
use crate::term::{Terminal, text_of};
use crate::test_wait::{drive, until_true};
use crate::tests::{FRAME, app, click};
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use tungstenite::Message;

/// What the fake Fleet was asked, and what its socket heard.
#[derive(Default)]
struct Heard {
    /// (method, path) of each HTTP request, and the body of a POST.
    requests: Vec<(String, String, Value)>,
    /// The socket's request path, with its query.
    sockets: Vec<String>,
    input: Vec<u8>,
    texts: Vec<Value>,
}

struct Fake {
    base: String,
    heard: Arc<Mutex<Heard>>,
    /// Live output for the socket to send.
    live: Sender<Vec<u8>>,
}

impl Fake {
    /// `listed` are the session's terminals; `scrollback` is what a socket replays before `ready`.
    fn start(listed: &'static [&'static str], scrollback: &'static [u8]) -> Fake {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let heard = Arc::new(Mutex::new(Heard::default()));
        let (live, rx) = channel();
        let rx = Arc::new(Mutex::new(Some(rx)));
        let shared = heard.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (heard, rx) = (shared.clone(), rx.clone());
                std::thread::spawn(move || serve(stream, listed, scrollback, heard, rx));
            }
        });
        Fake { base, heard, live }
    }

    fn input(&self) -> Vec<u8> {
        self.heard.lock().unwrap().input.clone()
    }

    fn texts(&self) -> Vec<Value> {
        self.heard.lock().unwrap().texts.clone()
    }

    /// What was asked of the terminals' routes (the hub's and the list's own calls are not what is counted).
    fn requests(&self) -> Vec<(String, String, Value)> {
        self.heard.lock().unwrap().requests.iter().filter(|(_, p, _)| p.contains("/terminals")).cloned().collect()
    }
}

fn serve(stream: TcpStream, listed: &[&str], scrollback: &[u8], heard: Arc<Mutex<Heard>>, live: Arc<Mutex<Option<Receiver<Vec<u8>>>>>) {
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
        let Some(rx) = live.lock().unwrap().take() else { return };
        return socket(stream, scrollback, heard, rx);
    }
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut length = 0usize;
    loop {
        let mut l = String::new();
        if reader.read_line(&mut l).unwrap_or(0) == 0 || l == "\r\n" {
            break;
        }
        if let Some(n) = l.to_ascii_lowercase().strip_prefix("content-length:") {
            length = n.trim().parse().unwrap_or(0);
        }
    }
    let mut raw = vec![0; length];
    let _ = reader.read_exact(&mut raw);
    let body: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
    let method = line.split(' ').next().unwrap_or("").to_string();
    let entry = |id: &str| json!({"id": id, "title": "zsh", "status": "running", "createdAt": "2026-11-18T10:00:00Z"});
    let (status, text) = match (method.as_str(), path.ends_with("/terminals")) {
        ("GET", true) => (200, Value::Array(listed.iter().map(|id| entry(id)).collect()).to_string()),
        ("POST", true) => (201, entry("t-new").to_string()),
        _ => (404, "{}".to_string()),
    };
    heard.lock().unwrap().requests.push((method, path, body));
    let mut stream = stream;
    let _ = stream.write_all(
        format!(
            "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
            text.len()
        )
        .as_bytes(),
    );
}

/// `TerminalSocket.cs`'s side: the saved scrollback in binary frames, `ready`, then live output; input and control are heard.
fn socket(stream: TcpStream, scrollback: &[u8], heard: Arc<Mutex<Heard>>, live: Receiver<Vec<u8>>) {
    let Ok(mut ws) = tungstenite::accept(stream) else { return };
    for chunk in scrollback.chunks(16 * 1024) {
        let _ = ws.send(Message::binary(chunk.to_vec()));
    }
    let _ = ws.send(Message::text(r#"{"type":"ready"}"#));
    let _ = ws.get_ref().set_read_timeout(Some(std::time::Duration::from_millis(5)));
    loop {
        while let Ok(out) = live.try_recv() {
            if ws.send(Message::binary(out)).is_err() {
                return;
            }
        }
        match ws.read() {
            Ok(Message::Binary(b)) => heard.lock().unwrap().input.extend_from_slice(&b),
            Ok(Message::Text(t)) => heard.lock().unwrap().texts.push(serde_json::from_str(&t).unwrap_or(Value::Null)),
            Ok(Message::Close(_)) | Err(tungstenite::Error::ConnectionClosed) => return,
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
fn with_no_terminals_listed_one_is_started_at_120_by_14() {
    let mut live = Live::start(Fake::start(&[], b"demo $ "));
    click(&mut live.rt, "terminal");
    live.wait("the socket to open", |s| s.terminal.status() == Some(Status::Open));
    let posts: Vec<_> = live.fake.requests().into_iter().filter(|(m, _, _)| m == "POST").collect();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].1, "/api/sessions/s1/terminals");
    assert_eq!(posts[0].2, json!({"cols": 120, "rows": 14}));
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
    live.rt.key(KeyEvent { key: Key::Command('j'), mods: Mods::default() });
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
    live.rt.key(KeyEvent { key: Key::Command('j'), mods: Mods::default() });
    live.rt.frame(FRAME);
    assert!(live.rt.find("terminal-drawer").is_none(), "Ctrl J hides it, from inside the terminal");
}

fn fake_input_is(fake: &Fake, want: &[u8]) -> bool {
    fake.input() == want
}
