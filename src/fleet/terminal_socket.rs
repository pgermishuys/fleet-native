//! A terminal's WebSocket (`client/src/lib/terminal-socket.ts`; server: `TerminalSocket.cs`). One thread per terminal reads
//! the socket and feeds the output to the `Terminal`, and writes what the UI sends: binary frames are input, text frames are
//! `resize` and `clear`. Binary frames from the server are the saved scrollback, then (after `{"type":"ready"}`) live output.
//! Nothing here runs on the UI thread. Reconnecting is the next piece's (component 13, piece B): a drop ends this socket.
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tungstenite::{Error as WsError, Message, WebSocket};

use super::link::{host_of, is_secure};
use super::tls::Stream;
use crate::term::Terminal;

/// `connecting` → `open` once the scrollback is in and the server said `ready`; `ended` after the shell's `exit` or a normal
/// close; `closed` on any other end (piece B reconnects from it).
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Connecting,
    Open,
    Ended(Option<i64>),
    Closed(String),
}

enum Out {
    Input(Vec<u8>),
    Resize(u16, u16),
    Clear,
    Dispose,
}

/// What the UI keeps of a connection. Dropping it closes the socket.
pub struct Socket {
    out: Sender<Out>,
    status: Arc<Mutex<Status>>,
}

impl Socket {
    /// Opens `{base}{path}/socket?cols=&rows=` (`ws` or `wss`, the machine's token in the query) on a thread of its own and
    /// feeds `term`. `changed` is called, at most about every 16 ms, when `term` has new content or the status moved.
    pub fn open(base: &str, path: &str, (cols, rows): (u16, u16), term: Arc<Terminal>, changed: impl Fn() + Send + 'static) -> Socket {
        let (out, rx) = channel();
        let status = Arc::new(Mutex::new(Status::Connecting));
        let (base, path, shared) = (base.to_string(), path.to_string(), status.clone());
        std::thread::spawn(move || {
            let end = match connect(&base, &path, cols, rows) {
                Ok(ws) => serve(ws, &rx, &term, &shared, &changed),
                Err(why) => Status::Closed(why),
            };
            let mut now = shared.lock().unwrap();
            if !matches!(*now, Status::Ended(_)) {
                *now = end;
            }
            drop(now);
            changed();
        });
        Socket { out, status }
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    /// Input for the shell: typed text, a paste, the terminal's own answers.
    pub fn write(&self, bytes: Vec<u8>) {
        if !bytes.is_empty() {
            let _ = self.out.send(Out::Input(bytes));
        }
    }

    /// `{"type":"resize",cols,rows}`: the shell's window is this size now.
    pub fn resize(&self, cols: u16, rows: u16) {
        let _ = self.out.send(Out::Resize(cols, rows));
    }

    pub fn clear(&self) {
        let _ = self.out.send(Out::Clear);
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        let _ = self.out.send(Out::Dispose);
    }
}

fn connect(base: &str, path: &str, cols: u16, rows: u16) -> Result<WebSocket<Stream>, String> {
    let host = host_of(base)?;
    let secure = is_secure(base);
    let name = host.rsplit_once(':').map_or(host.as_str(), |(h, _)| h).trim_start_matches('[').trim_end_matches(']').to_string();
    let stream = Stream::open(secure, &host, &name)?;
    let scheme = if secure { "wss" } else { "ws" };
    let url = format!("{scheme}://{host}{path}/socket?cols={cols}&rows={rows}{}", super::auth::hub_query_for(base));
    let (ws, _) = tungstenite::client(url, stream).map_err(|e| e.to_string())?;
    ws.get_ref().set_read_timeout(Some(Duration::from_millis(20))).map_err(|e| e.to_string())?;
    Ok(ws)
}

/// Reads and writes until the socket ends; says how it ended.
fn serve(mut ws: WebSocket<Stream>, rx: &Receiver<Out>, term: &Terminal, status: &Mutex<Status>, changed: &dyn Fn()) -> Status {
    // Each connect starts from a blank terminal; the scrollback replays into it.
    term.reset();
    let (mut dirty, mut last) = (false, Instant::now());
    let set = |s: Status| {
        *status.lock().unwrap() = s;
        changed();
    };
    loop {
        let mut idle = false;
        loop {
            match rx.try_recv() {
                Ok(Out::Input(b)) => {
                    if let Err(e) = ws.send(Message::binary(b)) {
                        return Status::Closed(e.to_string());
                    }
                }
                Ok(Out::Resize(c, r)) => {
                    if let Err(e) = ws.send(Message::text(json!({"type": "resize", "cols": c, "rows": r}).to_string())) {
                        return Status::Closed(e.to_string());
                    }
                }
                Ok(Out::Clear) => {
                    let _ = ws.send(Message::text(json!({"type": "clear"}).to_string()));
                }
                Ok(Out::Dispose) | Err(TryRecvError::Disconnected) => {
                    // 1000 "Closed by the page.", as Fleet's own client closes it.
                    let _ = ws.close(Some(tungstenite::protocol::CloseFrame {
                        code: tungstenite::protocol::frame::coding::CloseCode::Normal,
                        reason: "Closed by the page.".into(),
                    }));
                    return Status::Ended(None);
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        match ws.read() {
            Ok(Message::Binary(b)) => {
                term.feed(&b);
                let answer = term.take_answers();
                if !answer.is_empty() {
                    let _ = ws.send(Message::binary(answer));
                }
                dirty = true;
            }
            Ok(Message::Text(t)) => {
                match serde_json::from_str::<Value>(&t).ok().as_ref().and_then(|v| v.get("type")).and_then(Value::as_str) {
                    Some("ready") => set(Status::Open),
                    Some("cleared") => {
                        term.reset();
                        dirty = true;
                    }
                    Some("exit") => {
                        let code = serde_json::from_str::<Value>(&t).ok().and_then(|v| v.get("exitCode").and_then(Value::as_i64));
                        set(Status::Ended(code));
                    }
                    _ => {}
                }
            }
            Ok(Message::Close(frame)) => {
                let code = frame.as_ref().map(|f| u16::from(f.code));
                return if code == Some(1000) { Status::Ended(None) } else { Status::Closed(format!("closed {}", code.unwrap_or(1005))) };
            }
            Ok(_) => {}
            Err(WsError::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => idle = true,
            Err(WsError::ConnectionClosed | WsError::AlreadyClosed) => return Status::Closed("closed".into()),
            Err(e) => return Status::Closed(e.to_string()),
        }
        // The first quiet moment shows what came; a flood shows a frame's worth at a time.
        if dirty && (idle || last.elapsed() >= Duration::from_millis(16)) {
            dirty = false;
            last = Instant::now();
            changed();
        }
    }
}
