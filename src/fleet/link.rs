//! The connection to a Fleet server: one thread owns the hub's WebSocket, short-lived threads make
//! the HTTP calls, and every result reaches the state through `AppHandle::update`, so the UI
//! thread does no network work. Events are batched to one hand-over per frame's worth.
use super::events::Event;
use super::reducer::create_state;
use super::signalr::{self, Frame};
use super::state::Topic;
use super::{http, sessions};
use crate::State;
use rune::{AppHandle, Tx};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tungstenite::{Error as WsError, Message, WebSocket};

/// The Vue client's reconnect delays (`use-signalr-socket.ts`).
const RECONNECT: [Duration; 4] = [Duration::from_secs(1), Duration::from_secs(2), Duration::from_secs(5), Duration::from_secs(10)];
/// How long events wait to be handed over together: about one frame.
const BATCH: Duration = Duration::from_millis(16);
const PING: Duration = Duration::from_secs(10);

/// A change to hand to the state, to run on the UI thread.
pub type Job = Box<dyn FnOnce(&mut State, &mut Tx) + Send>;

/// Where the connection's threads post their results. In the app that is `AppHandle::update`; a
/// test posts to a queue it drains itself, because a headless runtime doesn't drain the handle's.
#[derive(Clone)]
pub struct Post(Arc<dyn Fn(Job) + Send + Sync>);

impl Post {
    pub fn new(send: impl Fn(Job) + Send + Sync + 'static) -> Post {
        Post(Arc::new(send))
    }

    pub fn from_handle(handle: AppHandle<State>) -> Post {
        Post::new(move |job| handle.update(job))
    }

    pub fn update(&self, job: impl FnOnce(&mut State, &mut Tx) + Send + 'static) {
        (self.0)(Box::new(job));
    }
}

pub enum Cmd {
    /// Show this session's conversation.
    Open(String),
    /// Fetch the session list again.
    Refresh,
}

/// What the UI keeps of the connection.
pub struct Link {
    pub base: String,
    cmds: Sender<Cmd>,
    post: Post,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Connection {
    Connecting,
    Online,
    /// Lost, and trying again.
    Down(String),
}

impl Link {
    /// Starts the hub thread and the first list fetch. Returns at once.
    pub fn start(base: String, post: Post) -> Link {
        let (cmds, rx) = channel();
        let (b, p) = (base.clone(), post.clone());
        std::thread::spawn(move || run(b, rx, p));
        Link { base, cmds, post }
    }

    pub fn open(&self, session: &str) {
        let _ = self.cmds.send(Cmd::Open(session.to_string()));
    }

    pub fn send_prompt(&self, session: &str, text: &str) {
        let (base, session, text) = (self.base.clone(), session.to_string(), text.to_string());
        self.in_background(move || http::prompt(&base, &session, &text), "Couldn't send the message");
    }

    pub fn abort(&self, session: &str) {
        let (base, session) = (self.base.clone(), session.to_string());
        self.in_background(move || http::abort(&base, &session), "Couldn't interrupt the session");
    }

    fn in_background(&self, call: impl FnOnce() -> Result<(), String> + Send + 'static, what: &'static str) {
        let post = self.post.clone();
        std::thread::spawn(move || {
            if let Err(e) = call() {
                post.update(move |s, _| s.notice = Some(format!("{what}: {e}")));
            }
        });
    }
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn fetch_list(base: &str, post: &Post) {
    let (base, post) = (base.to_string(), post.clone());
    std::thread::spawn(move || match http::sessions(&base) {
        Ok(rows) => {
            let projects = sessions::parse_list(&rows, now_ms());
            post.update(move |s, tx| s.on_list(projects, tx));
        }
        Err(e) => post.update(move |s, _| s.notice = Some(format!("Couldn't load the session list: {e}"))),
    });
}

fn connection(post: &Post, c: Connection) {
    post.update(move |s, _| s.connection = c);
}

/// The hub thread: connect, serve, and on loss wait and connect again.
fn run(base: String, rx: Receiver<Cmd>, post: Post) {
    let mut open: Option<String> = None;
    let mut attempt = 0;
    loop {
        fetch_list(&base, &post);
        let why = match connect(&base) {
            Ok(ws) => {
                attempt = 0;
                connection(&post, Connection::Online);
                serve(ws, &base, &rx, &mut open, &post)
            }
            Err(e) => e,
        };
        connection(&post, Connection::Down(why));
        // Commands that come while waiting still count: the session to open on the next connection.
        let deadline = Instant::now() + RECONNECT[attempt.min(RECONNECT.len() - 1)];
        attempt += 1;
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match rx.recv_timeout(left) {
                Ok(Cmd::Open(id)) => open = Some(id),
                Ok(Cmd::Refresh) => {}
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }
}

fn host_of(base: &str) -> Result<String, String> {
    let rest = base.strip_prefix("http://").ok_or("only http:// servers on this machine are supported so far")?;
    Ok(rest.trim_end_matches('/').to_string())
}

fn connect(base: &str) -> Result<WebSocket<TcpStream>, String> {
    let host = host_of(base)?;
    let token = http::negotiate(base)?;
    let stream = TcpStream::connect(&host).map_err(|e| e.to_string())?;
    let _ = stream.set_nodelay(true);
    let (mut ws, _) = tungstenite::client(format!("ws://{host}/hubs/session-events?id={token}"), stream).map_err(|e| e.to_string())?;
    ws.send(Message::text(signalr::handshake())).map_err(|e| e.to_string())?;
    // The handshake's answer is the first frame; read it blocking, then switch to short reads.
    loop {
        match ws.read().map_err(|e| e.to_string())? {
            Message::Text(t) => match signalr::parse(&t).into_iter().next() {
                Some(Frame::Handshake(None)) => break,
                Some(Frame::Handshake(Some(e))) => return Err(format!("the hub refused the handshake: {e}")),
                _ => return Err("the hub's first frame wasn't a handshake answer".to_string()),
            },
            Message::Close(_) => return Err("the hub closed the connection".to_string()),
            _ => {}
        }
    }
    ws.get_ref().set_read_timeout(Some(Duration::from_millis(20))).map_err(|e| e.to_string())?;
    Ok(ws)
}

/// Serves one connection until it fails; the error says why.
fn serve(mut ws: WebSocket<TcpStream>, base: &str, rx: &Receiver<Cmd>, open: &mut Option<String>, post: &Post) -> String {
    let mut next_id = 0u32;
    let mut call = |ws: &mut WebSocket<TcpStream>, target: &str, args: Vec<Value>| -> Result<u32, String> {
        next_id += 1;
        ws.send(Message::text(signalr::invocation(next_id, target, &args))).map_err(|e| e.to_string())?;
        Ok(next_id)
    };
    // Which invocation asked for which session's snapshot.
    let mut asked: HashMap<String, String> = HashMap::new();
    // Events that came before the open session's snapshot are applied after it.
    let mut waiting_for_snapshot = false;
    let mut batch: Vec<Event> = Vec::new();
    let mut batch_since: Option<Instant> = None;
    let mut dirty_list: Option<Instant> = None;
    let mut last_ping = Instant::now();

    let started = (|| -> Result<(), String> {
        call(&mut ws, "SubscribeToSessionsTopicAsync", vec![])?;
        if let Some(id) = open.as_ref() {
            let n = call(&mut ws, "SubscribeToSessionAsync", vec![json!(id)])?;
            asked.insert(n.to_string(), id.clone());
            waiting_for_snapshot = true;
        }
        Ok(())
    })();
    if let Err(e) = started {
        return e;
    }

    loop {
        while let Ok(cmd) = rx.try_recv() {
            let result = match cmd {
                Cmd::Open(id) if open.as_ref() == Some(&id) => Ok(()),
                Cmd::Open(id) => (|| {
                    if let Some(old) = open.take() {
                        call(&mut ws, "UnsubscribeFromSessionAsync", vec![json!(old)])?;
                    }
                    batch.clear();
                    let n = call(&mut ws, "SubscribeToSessionAsync", vec![json!(id)])?;
                    asked.insert(n.to_string(), id.clone());
                    waiting_for_snapshot = true;
                    *open = Some(id);
                    Ok::<(), String>(())
                })(),
                Cmd::Refresh => {
                    fetch_list(base, post);
                    Ok(())
                }
            };
            if let Err(e) = result {
                return e;
            }
        }
        if last_ping.elapsed() >= PING {
            last_ping = Instant::now();
            if let Err(e) = ws.send(Message::text(signalr::pong())) {
                return e.to_string();
            }
        }
        match ws.read() {
            Ok(Message::Text(text)) => {
                for frame in signalr::parse(&text) {
                    match frame {
                        Frame::Event { topic, data, .. } if topic == "sessions" => {
                            // Whoever owns an event reads it from here (`State::subscribe`); the list's own
                            // reactions below don't wait for them.
                            if let Some(event) = Event::from_wire(&data) {
                                post.update(move |s, _| s.dispatch(&Topic::Sessions, std::slice::from_ref(&event)));
                            }
                            match data.get("type").and_then(Value::as_str) {
                                Some("activity_status") => {
                                    let id = data.pointer("/properties/sessionId").and_then(Value::as_str).unwrap_or("").to_string();
                                    let status = sessions::status_of(
                                        data.pointer("/properties/activityStatus").and_then(Value::as_str).unwrap_or("idle"),
                                    );
                                    post.update(move |s, _| s.on_activity(&id, status));
                                }
                                // A session was made, renamed, archived or removed: the list is the truth.
                                _ => dirty_list = dirty_list.or(Some(Instant::now())),
                            }
                        }
                        Frame::Event { topic, event_id, data } if open.as_ref().is_some_and(|id| topic == format!("session:{id}")) => {
                            // One frame that can't be read is skipped; the rest of the batch stands.
                            if let Some(event) = Event::from_wire(&data) {
                                batch_since.get_or_insert_with(Instant::now);
                                batch.push(event);
                                if let Some(id) = event_id {
                                    batch.push(Event::Cursor(id));
                                }
                            }
                        }
                        Frame::Completion { id, result, error } => {
                            let Some(session) = asked.remove(&id) else { continue };
                            if open.as_deref() != Some(session.as_str()) {
                                continue;
                            }
                            match error {
                                Some(e) => post.update(move |s, _| s.notice = Some(format!("Couldn't open the session: {e}"))),
                                None => {
                                    let state = create_state(&result);
                                    // Events that arrived before the snapshot follow it.
                                    let after = std::mem::take(&mut batch);
                                    batch_since = None;
                                    waiting_for_snapshot = false;
                                    post.update(move |s, tx| {
                                        s.on_snapshot(&session, state, tx);
                                        s.on_events(&session, after, tx);
                                    });
                                }
                            }
                        }
                        Frame::Close(e) => return e.unwrap_or_else(|| "the hub closed the connection".to_string()),
                        _ => {}
                    }
                }
            }
            Ok(Message::Close(_)) => return "the hub closed the connection".to_string(),
            Ok(_) => {}
            Err(WsError::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(e) => return e.to_string(),
        }
        if !waiting_for_snapshot
            && let (Some(since), Some(id)) = (batch_since, open.clone())
            && since.elapsed() >= BATCH
        {
            let events = std::mem::take(&mut batch);
            batch_since = None;
            post.update(move |s, tx| s.on_events(&id, events, tx));
        }
        if let Some(since) = dirty_list
            && since.elapsed() >= Duration::from_millis(400)
        {
            dirty_list = None;
            fetch_list(base, post);
        }
    }
}
