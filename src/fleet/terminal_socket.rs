//! A terminal's WebSocket (`client/src/lib/terminal-socket.ts`; server: `TerminalSocket.cs`). One thread per terminal reads
//! the socket and feeds the output to the `Terminal`, and writes what the UI sends: binary frames are input, text frames are
//! `resize` and `clear`. Binary frames from the server are the saved scrollback, then (after `{"type":"ready"}`) live output.
//! The thread keeps the connection: after a drop it reconnects (the scrollback replays into a reset terminal), until the shell
//! ends, the tab closes, or five attempts in a row never reach `ready`. Nothing here runs on the UI thread.
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tungstenite::protocol::CloseFrame;
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::{Error as WsError, Message, WebSocket};

use super::link::{host_of, is_secure};
use super::tls::Stream;
use crate::term::Terminal;

/// `RECONNECT_DELAYS_MS`: the waits between attempts; the last one repeats.
pub const RECONNECT_DELAYS_MS: [u64; 6] = [250, 500, 1000, 2000, 4000, 8000];

/// `MAX_FAILED_ATTEMPTS`: attempts in a row that never reached `ready` before giving up.
pub const MAX_FAILED_ATTEMPTS: u32 = 5;

/// `MAX_PENDING_INPUT`: what is kept of input typed while the socket isn't ready.
pub const MAX_PENDING_INPUT: usize = 4096;

/// How long to wait before attempt number `attempt` (1 is the first connect; the wait is after it dropped).
pub fn reconnect_delay(attempt: u32) -> Duration {
    Duration::from_millis(RECONNECT_DELAYS_MS[(attempt.max(1) as usize - 1).min(RECONNECT_DELAYS_MS.len() - 1)])
}

/// `connecting` on the first connect; `open` once the scrollback is in and the server said `ready`; `reconnecting` between
/// and during later attempts; `ended` after the shell's `exit` or a normal close (the exit code, `None` when Fleet ended
/// it); `failed` after [`MAX_FAILED_ATTEMPTS`] in a row.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Connecting,
    Open,
    Reconnecting,
    Ended(Option<i64>),
    Failed,
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

/// How one connection ended.
enum End {
    /// The socket dropped; `ready` says whether the scrollback and `ready` had arrived.
    Dropped { ready: bool },
    /// The shell ended, or the server closed it normally.
    Ended(Option<i64>),
    /// The page closed it.
    Disposed,
}

/// What the thread keeps across connections.
struct Keep {
    cols: u16,
    rows: u16,
    /// Input typed while no socket is ready, up to [`MAX_PENDING_INPUT`] bytes.
    pending: Vec<Vec<u8>>,
    pending_bytes: usize,
}

impl Keep {
    fn hold(&mut self, bytes: Vec<u8>) {
        if self.pending_bytes + bytes.len() > MAX_PENDING_INPUT {
            return;
        }
        self.pending_bytes += bytes.len();
        self.pending.push(bytes);
    }
}

impl Socket {
    /// Opens `{base}{path}/socket?cols=&rows=` (`ws` or `wss`, the machine's token in the query) on a thread of its own and
    /// feeds `term`. `changed` is called, at most about every 16 ms, when `term` has new content or the status moved.
    pub fn open(base: &str, path: &str, size: (u16, u16), term: Arc<Terminal>, changed: impl Fn() + Send + 'static) -> Socket {
        Socket::open_with(base, path, size, term, reconnect_delay, changed)
    }

    /// [`Socket::open`] with its own waits between attempts, so a test needn't wait 250 ms to 8 s.
    pub fn open_with(
        base: &str,
        path: &str,
        (cols, rows): (u16, u16),
        term: Arc<Terminal>,
        delay: impl Fn(u32) -> Duration + Send + 'static,
        changed: impl Fn() + Send + 'static,
    ) -> Socket {
        let (out, rx) = channel();
        let status = Arc::new(Mutex::new(Status::Connecting));
        let (base, path, shared) = (base.to_string(), path.to_string(), status.clone());
        std::thread::spawn(move || {
            let mut keep = Keep { cols, rows, pending: Vec::new(), pending_bytes: 0 };
            let set = |s: Status| {
                *shared.lock().unwrap() = s;
                changed();
            };
            let mut attempt = 0u32;
            let mut failed = 0u32;
            loop {
                set(if attempt == 0 { Status::Connecting } else { Status::Reconnecting });
                attempt += 1;
                let end = match connect(&base, &path, keep.cols, keep.rows) {
                    Ok(ws) => serve(ws, &rx, &term, &mut keep, &shared, &changed),
                    Err(_) => End::Dropped { ready: false },
                };
                match end {
                    End::Disposed => return set(Status::Ended(None)),
                    End::Ended(code) => return set(Status::Ended(code)),
                    End::Dropped { ready } => {
                        if ready {
                            failed = 0;
                        } else {
                            failed += 1;
                        }
                        if failed >= MAX_FAILED_ATTEMPTS {
                            return set(Status::Failed);
                        }
                        set(Status::Reconnecting);
                        if !wait(&rx, &mut keep, delay(attempt)) {
                            return set(Status::Ended(None));
                        }
                    }
                }
            }
        });
        Socket { out, status }
    }

    /// A socket that is never connected and always says `status`: the invented data's scenes and the view's tests.
    pub fn fixed(status: Status) -> Socket {
        Socket { out: channel().0, status: Arc::new(Mutex::new(status)) }
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    /// Input for the shell: typed text, a paste, the terminal's own answers. Kept (up to 4,096 bytes) until the socket is
    /// ready; dropped once the shell ended.
    pub fn write(&self, bytes: Vec<u8>) {
        if !bytes.is_empty() {
            let _ = self.out.send(Out::Input(bytes));
        }
    }

    /// `{"type":"resize",cols,rows}`: the shell's window is this size now. A socket not ready yet connects at this size.
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
    let url = socket_url(base, path, cols, rows)?;
    let (ws, _) = tungstenite::client(url, stream).map_err(|e| e.to_string())?;
    ws.get_ref().set_read_timeout(Some(Duration::from_millis(20))).map_err(|e| e.to_string())?;
    Ok(ws)
}

/// `terminalSocketUrl`: the machine's own address, `ws` or `wss`, its token (never home's) in the query.
pub fn socket_url(base: &str, path: &str, cols: u16, rows: u16) -> Result<String, String> {
    let host = host_of(base)?;
    let scheme = if is_secure(base) { "wss" } else { "ws" };
    Ok(format!("{scheme}://{host}{path}/socket?cols={cols}&rows={rows}{}", super::auth::hub_query_for(base)))
}

/// Waits out the delay between attempts, still hearing the page: false when it closed the socket meanwhile.
fn wait(rx: &Receiver<Out>, keep: &mut Keep, delay: Duration) -> bool {
    let until = Instant::now() + delay;
    loop {
        let left = until.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(Out::Input(b)) => keep.hold(b),
            Ok(Out::Resize(c, r)) => (keep.cols, keep.rows) = (c, r),
            Ok(Out::Clear) => {}
            Ok(Out::Dispose) | Err(RecvTimeoutError::Disconnected) => return false,
            Err(RecvTimeoutError::Timeout) => return true,
        }
    }
}

/// Reads and writes until the socket ends; says how it ended.
fn serve(
    mut ws: WebSocket<Stream>,
    rx: &Receiver<Out>,
    term: &Terminal,
    keep: &mut Keep,
    status: &Mutex<Status>,
    changed: &dyn Fn(),
) -> End {
    // Each connect starts from a blank terminal; the scrollback replays into it.
    term.reset();
    let (mut dirty, mut last) = (false, Instant::now());
    let (mut ready, mut exit) = (false, None::<Option<i64>>);
    let set = |s: Status| {
        *status.lock().unwrap() = s;
        changed();
    };
    // The socket is gone: after the shell's `exit` that is the end, whatever the code.
    let gone = |ready: bool, exit: Option<Option<i64>>| match exit {
        Some(code) => End::Ended(code),
        None => End::Dropped { ready },
    };
    loop {
        let mut idle = false;
        loop {
            match rx.try_recv() {
                Ok(Out::Input(b)) => {
                    if exit.is_some() {
                        continue;
                    }
                    if !ready {
                        keep.hold(b);
                    } else if ws.send(Message::binary(b)).is_err() {
                        return gone(ready, exit);
                    }
                }
                Ok(Out::Resize(c, r)) => {
                    (keep.cols, keep.rows) = (c, r);
                    // Fleet sends these as soon as the socket is open, `ready` or not; only input waits for `ready`.
                    if ws.send(Message::text(json!({"type": "resize", "cols": c, "rows": r}).to_string())).is_err() {
                        return gone(ready, exit);
                    }
                }
                Ok(Out::Clear) => {
                    let _ = ws.send(Message::text(json!({"type": "clear"}).to_string()));
                }
                Ok(Out::Dispose) | Err(TryRecvError::Disconnected) => {
                    // 1000 "Closed by the page.", as Fleet's own client closes it.
                    let _ = ws.close(Some(CloseFrame { code: CloseCode::Normal, reason: "Closed by the page.".into() }));
                    let _ = ws.flush();
                    return End::Disposed;
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
                let value = serde_json::from_str::<Value>(&t).ok();
                match value.as_ref().and_then(|v| v.get("type")).and_then(Value::as_str) {
                    Some("ready") => {
                        ready = true;
                        set(Status::Open);
                        for chunk in std::mem::take(&mut keep.pending) {
                            let _ = ws.send(Message::binary(chunk));
                        }
                        keep.pending_bytes = 0;
                    }
                    Some("cleared") => {
                        term.reset();
                        dirty = true;
                    }
                    Some("exit") => {
                        let code = value.as_ref().and_then(|v| v.get("exitCode")).and_then(Value::as_i64);
                        exit = Some(code);
                        set(Status::Ended(code));
                    }
                    _ => {}
                }
            }
            Ok(Message::Close(frame)) => {
                let normal = frame.as_ref().is_some_and(|f| f.code == CloseCode::Normal);
                return match (exit, normal) {
                    (Some(code), _) => End::Ended(code),
                    (None, true) => End::Ended(None),
                    (None, false) => End::Dropped { ready },
                };
            }
            Ok(_) => {}
            Err(WsError::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => idle = true,
            Err(_) => return gone(ready, exit),
        }
        // The first quiet moment shows what came; a flood shows a frame's worth at a time.
        if dirty && (idle || last.elapsed() >= Duration::from_millis(16)) {
            dirty = false;
            last = Instant::now();
            changed();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 13.22: at most 4,096 bytes are kept; a chunk that would pass that is dropped whole, a smaller one after it still fits.
    // (Not a socket test: whether input reaches `serve` before or after `ready` is a race that changes nothing for the shell.)
    #[test]
    fn what_is_kept_before_ready_is_at_most_4096_bytes() {
        let mut keep = Keep { cols: 80, rows: 24, pending: Vec::new(), pending_bytes: 0 };
        keep.hold(vec![b'a'; MAX_PENDING_INPUT - 100]);
        keep.hold(vec![b'b'; 200]);
        keep.hold(vec![b'c'; 50]);
        let kept: Vec<u8> = keep.pending.concat();
        assert_eq!(kept.len(), MAX_PENDING_INPUT - 100 + 50);
        assert!(!kept.contains(&b'b'), "the 200-byte chunk was dropped whole");
        assert!(kept.ends_with(&[b'c'; 50]));
        keep.hold(vec![b'd'; 51]);
        assert_eq!(keep.pending_bytes, MAX_PENDING_INPUT - 100 + 50, "51 more would pass the cap");
    }
}
