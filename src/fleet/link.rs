//! The connection to a Fleet server: one thread owns the hub's WebSocket, short-lived threads make
//! the HTTP calls, and every result reaches the state through `AppHandle::update`, so the UI
//! thread does no network work. Events are batched to one hand-over per frame's worth.
use super::events::Event;
use super::history::{Page, Pending};
use super::presence::{Call, Presence};
use super::reducer::create_state;
use super::refresh::Refresh;
use super::rows::RowPatch;
use super::signalr::{self, Frame};
use super::state::Topic;
use super::topics::{Topics, reconnect_delay};
use super::{groups, http, sessions};
use crate::State;
use rune::{AppHandle, Tx};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tungstenite::{Error as WsError, Message, WebSocket};

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
    /// Listen to another session's topic as well (its events go to `State::subscribe`).
    Watch(String),
    Unwatch(String),
    /// Ask for the page of older messages before `cursor`.
    LoadOlder {
        session: String,
        cursor: String,
    },
    /// The window gained or lost the keyboard.
    WindowFocus(bool),
    /// The window went off screen or came back.
    WindowVisible(bool),
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

    /// "Try again" on the list: the sessions and the projects are read again (`SessionsPanel`'s `handleRetry`).
    pub fn refetch_list(&self) {
        fetch_list(&self.base, &self.post);
    }

    pub fn open(&self, session: &str) {
        let _ = self.cmds.send(Cmd::Open(session.to_string()));
    }

    /// Listens to a session that isn't open (the Files canvas keeps listening while you are on another).
    pub fn watch(&self, session: &str) {
        let _ = self.cmds.send(Cmd::Watch(session.to_string()));
    }

    /// Asks for the page before `cursor`; the answer, or why there is none, reaches `State::on_history_page`.
    pub fn load_older(&self, session: &str, cursor: &str) {
        let _ = self.cmds.send(Cmd::LoadOlder { session: session.to_string(), cursor: cursor.to_string() });
    }

    /// The window gained or lost focus (Rune's `on_focus`); presence and the open session's focus follow.
    pub fn window_focus(&self, focused: bool) {
        let _ = self.cmds.send(Cmd::WindowFocus(focused));
    }

    /// The window went off screen or came back (Rune's `on_visibility`).
    pub fn window_visible(&self, visible: bool) {
        let _ = self.cmds.send(Cmd::WindowVisible(visible));
    }

    pub fn unwatch(&self, session: &str) {
        let _ = self.cmds.send(Cmd::Unwatch(session.to_string()));
    }

    /// Sends a prompt already shown as pending under `correlation_id`; the receipt confirms it and a
    /// refusal takes it back (`State::on_prompt_receipt`, `on_prompt_refused`).
    pub fn send_prompt(&self, session: &str, correlation_id: &str, text: &str, pick: http::Pick) {
        let (base, session, correlation, text) = (self.base.clone(), session.to_string(), correlation_id.to_string(), text.to_string());
        let post = self.post.clone();
        std::thread::spawn(move || match http::prompt(&base, &session, &correlation, &text, &pick) {
            Ok(receipt) => post.update(move |s, tx| s.on_prompt_receipt(&session, &correlation, &receipt, tx)),
            Err(why) => post.update(move |s, tx| s.on_prompt_refused(&session, &correlation, why, tx)),
        });
    }

    /// Runs `call` (a blocking request, given the server's address) on a thread and hands its result to `done` on the
    /// UI thread: how the pickers' lists and the preferences load.
    pub fn off_thread<T: Send + 'static>(
        &self,
        call: impl FnOnce(&str) -> T + Send + 'static,
        done: impl FnOnce(&mut State, &mut Tx, T) + Send + 'static,
    ) {
        let (base, post) = (self.base.clone(), self.post.clone());
        std::thread::spawn(move || {
            let result = call(&base);
            post.update(move |s, tx| done(s, tx, result));
        });
    }

    /// The way back to the UI thread, for a thread that reports as it goes (an export's page count).
    pub fn post(&self) -> Post {
        self.post.clone()
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

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Asks for the session list. The request is numbered on the UI thread, so an answer that a newer fetch has overtaken is
/// dropped when it arrives (`useSessions`' request id).
fn fetch_list(base: &str, post: &Post) {
    let (base, post) = (base.to_string(), post.clone());
    let again = post.clone();
    again.update(move |s, _| {
        let retention = s.store.retention;
        let request = s.store.begin(s.has_rows());
        std::thread::spawn(move || match http::sessions(&base, retention) {
            Ok(rows) => {
                let listed = sessions::parse_sessions(&rows, now_ms());
                // Read with every list so a project's rename, move or delete shows; a failure keeps the last good answer
                // (`useProjects`).
                let projects = http::projects(&base).map(|body| groups::parse_projects(&body));
                post.update(move |s, tx| s.on_sessions(request, listed, projects, tx));
            }
            Err(e) => post.update(move |s, _| s.on_list_failed(request, e)),
        });
    });
}

fn connection(post: &Post, c: Connection) {
    post.update(move |s, _| s.connection = c);
}

/// The hub thread: connect, serve, and on loss wait and connect again.
fn run(base: String, rx: Receiver<Cmd>, post: Post) {
    let mut topics = Topics::default();
    let mut pending = Pending::default();
    let mut presence = Presence::default();
    let mut attempt = 0;
    loop {
        let why = match connect(&base) {
            Ok(ws) => {
                attempt = 0;
                // Only now that the server answers: a list asked for while it was down would fail, and the
                // list is what a restarted server has to hand back.
                fetch_list(&base, &post);
                connection(&post, Connection::Online);
                post.update(|s, _| s.on_reconnected());
                serve(ws, &base, &rx, &mut topics, &mut pending, &mut presence, &post)
            }
            Err(e) => e,
        };
        // A page asked for on the connection that just ended will never be answered.
        for (session, cursor) in pending.drain() {
            history_failed(&post, session, cursor, &why);
        }
        connection(&post, Connection::Down(why));
        topics.forget_snapshots();
        presence.disconnected();
        // Commands that come while waiting still count: the topics to subscribe on the next connection.
        let deadline = Instant::now() + reconnect_delay(attempt);
        attempt += 1;
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match rx.recv_timeout(left) {
                Ok(Cmd::Open(id)) => {
                    topics.set_open(Some(id));
                }
                Ok(Cmd::Watch(id)) => {
                    topics.watch(&id);
                }
                Ok(Cmd::Unwatch(id)) => {
                    topics.unwatch(&id);
                }
                Ok(Cmd::Refresh) => {}
                // Said again once connected (`connected`), so only what the window is like now is kept.
                Ok(Cmd::WindowFocus(focused)) => {
                    presence.focus_changed(focused, Instant::now());
                    // Back at the window while the hub is down: try now instead of waiting out the delay
                    // (`reconnectIfClosed`, on `online` and `visibilitychange`).
                    if focused {
                        attempt = 0;
                        break;
                    }
                }
                Ok(Cmd::WindowVisible(visible)) => {
                    presence.visibility_changed(visible, Instant::now());
                }
                // Not connected: no page can come (`loadSessionHistory` answers null).
                Ok(Cmd::LoadOlder { session, cursor }) => history_failed(&post, session, cursor, "not connected"),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }
}

/// Makes a presence or focus call. Its answer is never read: an older Fleet without the method answers with
/// an error, and `serve` ignores a completion it didn't ask for.
fn send_call(
    call: &mut impl FnMut(&mut WebSocket<TcpStream>, &str, Vec<Value>) -> Result<u32, String>,
    ws: &mut WebSocket<TcpStream>,
    c: Call,
) -> Result<(), String> {
    match c {
        Call::Presence(visible) => call(ws, "SetPresenceAsync", vec![json!(visible), json!("desktop")]),
        Call::Focus(session, focused) => call(ws, "SetSessionFocusAsync", vec![json!(session), json!(focused)]),
    }
    .map(|_| ())
}

fn history_failed(post: &Post, session: String, cursor: String, why: &str) {
    let why = why.to_string();
    post.update(move |s, _| s.on_history_page(&session, &cursor, Err(why)));
}

pub(crate) fn host_of(base: &str) -> Result<String, String> {
    let rest = base.strip_prefix("http://").ok_or("only http:// servers on this machine are supported so far")?;
    let host = rest.trim_end_matches('/');
    // `http://127.0.0.1` has no port to connect to: it is 80, as for any browser.
    let has_port = host.rsplit_once(':').is_some_and(|(_, p)| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    Ok(if has_port { host.to_string() } else { format!("{host}:80") })
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
fn serve(
    mut ws: WebSocket<TcpStream>,
    base: &str,
    rx: &Receiver<Cmd>,
    topics: &mut Topics,
    pending: &mut Pending,
    presence: &mut Presence,
    post: &Post,
) -> String {
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
    // The sessions topic's activity events carry no cursor of this session's, so they wait apart from its own.
    let mut pending_activity: Vec<Event> = Vec::new();
    // Snapshots to hand over: the ones that arrived, and a cached one reused for a topic already listened to.
    let mut ready: Vec<(String, Value)> = Vec::new();
    let mut batch_since: Option<Instant> = None;
    let mut refresh = Refresh::new(Instant::now());
    let mut last_ping = Instant::now();

    // Every topic again, so each gets a fresh snapshot (`topicListenersV2` after a reconnect).
    let started = (|| -> Result<(), String> {
        call(&mut ws, "SubscribeToSessionsTopicAsync", vec![])?;
        for id in topics.all() {
            let n = call(&mut ws, "SubscribeToSessionAsync", vec![json!(id)])?;
            asked.insert(n.to_string(), id);
        }
        waiting_for_snapshot = topics.open().is_some();
        for c in presence.connected(Instant::now()) {
            send_call(&mut call, &mut ws, c)?;
        }
        Ok(())
    })();
    if let Err(e) = started {
        return e;
    }

    loop {
        while let Ok(cmd) = rx.try_recv() {
            let result = match cmd {
                Cmd::Open(id) if topics.open() == Some(id.as_str()) => Ok(()),
                Cmd::Open(id) => (|| {
                    let held = topics.has_watched(&id);
                    for c in presence.left() {
                        send_call(&mut call, &mut ws, c)?;
                    }
                    if let Some(old) = topics.set_open(Some(id.clone())) {
                        call(&mut ws, "UnsubscribeFromSessionAsync", vec![json!(old)])?;
                    }
                    batch.clear();
                    pending_activity.clear();
                    waiting_for_snapshot = true;
                    // A second listener on a topic reuses its snapshot only if nothing arrived since.
                    match topics.fresh_snapshot(&id).filter(|_| held) {
                        Some(snapshot) => ready.push((id, snapshot.clone())),
                        None => {
                            let n = call(&mut ws, "SubscribeToSessionAsync", vec![json!(id)])?;
                            asked.insert(n.to_string(), id);
                        }
                    }
                    Ok::<(), String>(())
                })(),
                Cmd::Watch(id) => (|| {
                    if topics.watch(&id) {
                        let n = call(&mut ws, "SubscribeToSessionAsync", vec![json!(id)])?;
                        asked.insert(n.to_string(), id);
                    }
                    Ok::<(), String>(())
                })(),
                Cmd::Unwatch(id) => (|| {
                    if topics.unwatch(&id) {
                        call(&mut ws, "UnsubscribeFromSessionAsync", vec![json!(id)])?;
                    }
                    Ok::<(), String>(())
                })(),
                Cmd::Refresh => {
                    fetch_list(base, post);
                    Ok(())
                }
                Cmd::WindowFocus(focused) => {
                    presence.focus_changed(focused, Instant::now());
                    // Coming back to the window: whatever changed while it was away (`visibilitychange`).
                    if focused {
                        refresh.window_back();
                    }
                    Ok(())
                }
                Cmd::WindowVisible(visible) => {
                    refresh.set_visible(visible, Instant::now());
                    presence.visibility_changed(visible, Instant::now()).into_iter().try_for_each(|c| send_call(&mut call, &mut ws, c))
                }
                Cmd::LoadOlder { session, cursor } => (|| {
                    let n = call(&mut ws, "LoadHistoryAsync", vec![json!(session), json!(cursor)])?;
                    pending.add(n.to_string(), session, cursor);
                    Ok::<(), String>(())
                })(),
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
        for c in presence.tick(Instant::now()) {
            if let Err(e) = send_call(&mut call, &mut ws, c) {
                return e;
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
                                // A sub-agent's status reaches the open session's stream too (`use-session-stream.ts`).
                                if let (Event::ActivityStatus { .. }, Some(id)) = (&event, topics.open().map(str::to_string)) {
                                    if waiting_for_snapshot {
                                        pending_activity.push(event.clone());
                                    } else {
                                        let event = event.clone();
                                        post.update(move |s, tx| s.on_stream_events(&id, vec![event], tx));
                                    }
                                }
                                post.update(move |s, _| s.dispatch(&Topic::Sessions, std::slice::from_ref(&event)));
                            }
                            match RowPatch::from_wire(&data) {
                                Some(patch) => post.update(move |s, _| s.on_row_patch(patch)),
                                // A session was made, renamed, archived or removed: the list is the truth.
                                None => refresh.event(Instant::now()),
                            }
                        }
                        Frame::Event { topic, event_id, data } if topic.starts_with("session:") => {
                            let id = &topic["session:".len()..];
                            topics.note_event(id);
                            // One frame that can't be read is skipped; the rest of the batch stands.
                            let Some(event) = Event::from_wire(&data) else { continue };
                            if topics.open() == Some(id) {
                                batch_since.get_or_insert_with(Instant::now);
                                batch.push(event);
                                if let Some(cursor) = event_id {
                                    batch.push(Event::Cursor(cursor));
                                }
                            } else if topics.has_watched(id) {
                                let topic = Topic::Session(id.to_string());
                                post.update(move |s, _| s.dispatch(&topic, std::slice::from_ref(&event)));
                            }
                        }
                        Frame::Completion { id, result, error } => {
                            if let Some((session, cursor)) = pending.complete(&id) {
                                match error {
                                    Some(e) => history_failed(post, session, cursor, &e),
                                    None => {
                                        let page = Page::from_wire(&result);
                                        post.update(move |s, _| s.on_history_page(&session, &cursor, Ok(page)));
                                    }
                                }
                                continue;
                            }
                            let Some(session) = asked.remove(&id) else { continue };
                            match error {
                                Some(e) if topics.open() == Some(session.as_str()) => {
                                    post.update(move |s, _| s.notice = Some(format!("Couldn't open the session: {e}")))
                                }
                                Some(_) => {}
                                None => {
                                    topics.record_snapshot(&session, &result);
                                    if topics.open() == Some(session.as_str()) {
                                        ready.push((session, result));
                                    }
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
        for (session, result) in std::mem::take(&mut ready) {
            if topics.open() != Some(session.as_str()) {
                continue;
            }
            let state = create_state(&result);
            // Events that arrived before the snapshot follow it.
            let after = std::mem::take(&mut batch);
            let activity = std::mem::take(&mut pending_activity);
            batch_since = None;
            waiting_for_snapshot = false;
            for c in presence.snapshot(&session) {
                if let Err(e) = send_call(&mut call, &mut ws, c) {
                    return e;
                }
            }
            post.update(move |s, tx| {
                s.on_snapshot(&session, state, tx);
                s.on_events(&session, after, tx);
                s.on_stream_events(&session, activity, tx);
            });
        }
        if !waiting_for_snapshot
            && let (Some(since), Some(id)) = (batch_since, topics.open().map(str::to_string))
            && since.elapsed() >= BATCH
        {
            let events = std::mem::take(&mut batch);
            batch_since = None;
            post.update(move |s, tx| s.on_events(&id, events, tx));
        }
        for (session, cursor) in pending.expired() {
            history_failed(post, session, cursor, "the server didn't answer in time");
        }
        if refresh.due(Instant::now()) {
            fetch_list(base, post);
        }
    }
}
