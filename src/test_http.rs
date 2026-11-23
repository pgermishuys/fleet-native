//! The one way a fake server in a test reads a request. A single `read()` returns whatever has arrived, which on a slow
//! runner is often half a request (the head without its body, or a head cut mid-line), and a fake that answers on that
//! answers wrongly or resets the client's connection (#203, #214, #250, #265 were each this). `read` takes the head up
//! to the blank line, then exactly the body the head announces (Content-Length, or chunked), however many reads and
//! however long that takes, up to [`crate::test_wait::BOUND`]. Every fake that binds a `TcpListener` reads through it;
//! `fakes_read_requests_through_test_http` below fails the build of a new one that doesn't.
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

/// One whole request. Empty (no method) when the client connected and closed without sending any, as a probe does.
#[derive(Debug, Default, Clone)]
pub struct Request {
    pub method: String,
    /// The request target as sent, query included.
    pub path: String,
    /// The request line and the headers, without the blank line.
    pub head: String,
    pub body: Vec<u8>,
}

impl Request {
    /// A header's value, by name in any case.
    pub fn header(&self, name: &str) -> Option<String> {
        self.head.lines().skip(1).find_map(|l| {
            let (key, value) = l.split_once(':')?;
            key.trim().eq_ignore_ascii_case(name).then(|| value.trim().to_string())
        })
    }

    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// The body as JSON, `Null` when it is empty or not JSON.
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or(serde_json::Value::Null)
    }

    /// The path without its query.
    pub fn route(&self) -> &str {
        self.path.split('?').next().unwrap_or("")
    }

    /// The request as text, head then blank line then body: what a fake that matches on substrings reads.
    pub fn raw(&self) -> String {
        if self.method.is_empty() { String::new() } else { format!("{}\r\n\r\n{}", self.head, self.body_text()) }
    }
}

/// Reads one request whole. A connection that closes (or runs past the bound) part-way gives what had arrived.
pub fn read(stream: &mut TcpStream) -> Request {
    read_within(stream, crate::test_wait::BOUND)
}

/// [`read`] with a bound of the caller's choosing (the helper's own test uses a short one).
pub fn read_within(stream: &mut TcpStream, bound: Duration) -> Request {
    let request = read_bounded(stream, bound);
    // The bound was only for this read: a WebSocket the fake goes on to serve on the same stream must block as usual.
    let _ = stream.set_read_timeout(None);
    request
}

fn read_bounded(stream: &mut TcpStream, bound: Duration) -> Request {
    let deadline = Instant::now() + bound;
    let mut got: Vec<u8> = Vec::new();
    let Some(end) = fill_until(stream, &mut got, deadline, |g| g.windows(4).position(|w| w == b"\r\n\r\n").map(|e| e + 4)) else {
        return Request::default();
    };
    let head = String::from_utf8_lossy(&got[..end - 4]).into_owned();
    let mut first = head.lines().next().unwrap_or("").split(' ');
    let (method, path) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
    let header = |name: &str| {
        head.lines().skip(1).find_map(|l| {
            let (key, value) = l.split_once(':')?;
            key.trim().eq_ignore_ascii_case(name).then(|| value.trim().to_ascii_lowercase())
        })
    };
    let mut rest = got.split_off(end);
    let body = if header("transfer-encoding").is_some_and(|v| v.contains("chunked")) {
        read_chunked(stream, &mut rest, deadline)
    } else {
        let wanted = header("content-length").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
        fill_until(stream, &mut rest, deadline, |g| (g.len() >= wanted).then_some(wanted));
        rest.truncate(wanted);
        rest
    };
    Request { method, path, head, body }
}

/// Reads into `got` until `done` finds what it wants in it (and says where it ends); None if the stream ended first.
fn fill_until(stream: &mut TcpStream, got: &mut Vec<u8>, deadline: Instant, done: impl Fn(&[u8]) -> Option<usize>) -> Option<usize> {
    let mut buf = [0u8; 4096];
    loop {
        if let Some(at) = done(got) {
            return Some(at);
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return None;
        }
        let _ = stream.set_read_timeout(Some(left));
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => return None,
            Ok(n) => got.extend_from_slice(&buf[..n]),
        }
    }
}

/// A chunked body: `size CRLF data CRLF` until a zero size (trailers are read and dropped).
fn read_chunked(stream: &mut TcpStream, got: &mut Vec<u8>, deadline: Instant) -> Vec<u8> {
    let mut body = Vec::new();
    let mut at = 0;
    loop {
        let Some(line_end) = fill_until(stream, got, deadline, |g| g[at..].windows(2).position(|w| w == b"\r\n").map(|e| at + e)) else {
            return body;
        };
        let size =
            usize::from_str_radix(String::from_utf8_lossy(&got[at..line_end]).split(';').next().unwrap_or("").trim(), 16).unwrap_or(0);
        at = line_end + 2;
        if size == 0 {
            // Trailers, then the blank line that ends them.
            fill_until(stream, got, deadline, |g| g[at - 2..].windows(4).position(|w| w == b"\r\n\r\n").map(|e| at - 2 + e));
            return body;
        }
        if fill_until(stream, got, deadline, |g| (g.len() >= at + size + 2).then_some(at + size)).is_none() {
            return body;
        }
        body.extend_from_slice(&got[at..at + size]);
        at += size + 2;
    }
}

/// A whole reply: status line, a body length, `connection: close`, then the body, written in one call.
pub fn respond(stream: &mut TcpStream, status: impl std::fmt::Display, content_type: &str, body: &str) {
    let reply =
        format!("HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
    let _ = stream.write_all(reply.as_bytes());
}

/// An address that answers nobody: the port stays bound for the whole run (so no other test's fake can be handed it, as
/// it could after a bind-then-drop) and each call is closed unanswered. One listener and one thread serve every test.
pub fn dead_address() -> String {
    static DEAD: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DEAD.get_or_init(|| {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || for _ in listener.incoming() {});
        base
    })
    .clone()
}

/// A fake server's listener on a free loopback port. Its accept loop ends when the test that bound it is over: a fake whose
/// `for stream in listener.incoming()` never ended kept a thread per test, and by the end of the suite the process held
/// about 2,000 of them, which is macOS's cap on threads per process (spawns then failed with EAGAIN, code 35).
///
/// "The test is over" is the end of the thread that called `bind`: libtest runs each test on its own thread, and that
/// thread's locals are dropped when it exits. Binding from some other thread ties the listener to that thread instead.
pub fn bind() -> std::io::Result<Listener> {
    bind_at("127.0.0.1:0")
}

/// [`bind`] at a given address.
pub fn bind_at(address: &str) -> std::io::Result<Listener> {
    let inner = std::sync::Arc::new(std::net::TcpListener::bind(address)?);
    let over = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let end = End { over: over.clone(), listener: std::sync::Arc::downgrade(&inner) };
    ENDS.with(|ends| ends.borrow_mut().push(end));
    Ok(Listener { inner, over })
}

pub struct Listener {
    inner: std::sync::Arc<std::net::TcpListener>,
    over: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Listener {
    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.inner.local_addr()
    }

    /// The next connection, or an error once the test that bound this is over.
    pub fn accept(&self) -> std::io::Result<(TcpStream, std::net::SocketAddr)> {
        let (stream, addr) = self.inner.accept()?;
        if self.over.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(std::io::Error::new(std::io::ErrorKind::ConnectionAborted, "the test that bound this listener is over"));
        }
        Ok((stream, addr))
    }

    /// Connections until the test that bound this is over. Like `TcpListener::incoming`, a failed accept is an `Err` item.
    pub fn incoming(&self) -> impl Iterator<Item = std::io::Result<TcpStream>> + '_ {
        std::iter::from_fn(move || match self.accept() {
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionAborted && self.over.load(std::sync::atomic::Ordering::SeqCst) => None,
            other => Some(other.map(|(s, _)| s)),
        })
    }
}

/// Set on a listener when the thread that bound it ends.
struct End {
    over: std::sync::Arc<std::sync::atomic::AtomicBool>,
    listener: std::sync::Weak<std::net::TcpListener>,
}

impl Drop for End {
    fn drop(&mut self) {
        self.over.store(true, std::sync::atomic::Ordering::SeqCst);
        // One connection wakes an accept that is waiting. Only while the listener is still open: on Windows a refused
        // loopback connection takes about two seconds to fail, which every test would wait out at its end.
        if let Some(addr) = self.listener.upgrade().and_then(|l| l.local_addr().ok()) {
            drop(TcpStream::connect_timeout(&addr, Duration::from_secs(2)));
        }
    }
}

thread_local! {
    static ENDS: std::cell::RefCell<Vec<End>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// [`respond`] with a JSON body.
pub fn respond_json(stream: &mut TcpStream, status: impl std::fmt::Display, body: &str) {
    respond(stream, status, "application/json", body);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::mpsc::channel;

    /// Serves one connection with `read`, and writes `pieces` to it from a client, a pause after each.
    fn through(pieces: &[&[u8]], pause: Duration) -> Request {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            tx.send(read(&mut stream)).unwrap();
        });
        let mut client = TcpStream::connect(addr).unwrap();
        client.set_nodelay(true).unwrap();
        for piece in pieces {
            client.write_all(piece).unwrap();
            client.flush().unwrap();
            std::thread::sleep(pause);
        }
        rx.recv_timeout(Duration::from_secs(20)).expect("the request is read once the last piece is in")
    }

    #[test]
    fn a_body_split_across_writes_with_delays_is_read_whole() {
        let pieces: [&[u8]; 6] =
            [b"POST /api/x?y=1 HTTP/1.1\r\nHo", b"st: h\r\nContent-Le", b"ngth: 13\r\n\r", b"\n{\"a\":", b"1,\"b\"", b":2}"];
        let r = through(&pieces, Duration::from_millis(40));
        assert_eq!((r.method.as_str(), r.path.as_str(), r.route()), ("POST", "/api/x?y=1", "/api/x"));
        assert_eq!(r.body_text(), r#"{"a":1,"b":2}"#);
        assert_eq!(r.json()["a"], 1);
        assert_eq!(r.header("HOST").as_deref(), Some("h"));
    }

    #[test]
    fn a_body_is_cut_at_its_announced_length_and_a_get_has_none() {
        let r = through(&[b"POST /p HTTP/1.1\r\ncontent-length: 3\r\n\r\nabcEXTRA"], Duration::ZERO);
        assert_eq!(r.body_text(), "abc");
        let r = through(&[b"GET /g HTTP/1.1\r\nhost: h\r\n\r\n"], Duration::ZERO);
        assert_eq!((r.method.as_str(), r.body.len()), ("GET", 0));
    }

    #[test]
    fn a_chunked_body_arriving_in_pieces_is_joined() {
        let pieces: [&[u8]; 5] =
            [b"POST /c HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nWi", b"ki\r\n5\r\npedia\r", b"\n0\r\n", b"\r", b"\n"];
        let r = through(&pieces, Duration::from_millis(30));
        assert_eq!(r.body_text(), "Wikipedia");
    }

    #[test]
    fn a_connection_that_closes_without_a_request_gives_an_empty_one() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(TcpStream::connect(addr).unwrap());
        let (mut stream, _) = listener.accept().unwrap();
        let r = read_within(&mut stream, Duration::from_secs(5));
        assert!(r.method.is_empty() && r.raw().is_empty());
    }

    #[test]
    fn respond_writes_a_reply_a_client_can_read() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            read(&mut stream);
            respond_json(&mut stream, "200 OK", "{}");
        });
        let mut client = TcpStream::connect(addr).unwrap();
        client.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();
        let mut text = String::new();
        client.read_to_string(&mut text).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n") && text.ends_with("\r\n\r\n{}"), "{text}");
    }

    /// `bind`'s promise: a fake's thread does not outlive its test (each one that did was a thread for the rest of the run).
    #[test]
    fn a_fakes_accept_loop_ends_when_the_test_that_bound_it_is_over() {
        let (tx, rx) = channel();
        // The thread stands for a test: it binds, starts a fake's accept loop the usual way, and ends.
        std::thread::spawn(move || {
            let listener = bind().unwrap();
            let addr = listener.local_addr().unwrap();
            let serving = std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    drop(stream);
                }
            });
            // While the test runs, the fake answers.
            TcpStream::connect(addr).unwrap();
            tx.send(serving).unwrap();
        })
        .join()
        .unwrap();
        let serving = rx.recv().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while !serving.is_finished() {
            assert!(std::time::Instant::now() < deadline, "the accept loop outlived the test that bound its listener");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Every fake server binds and reads its requests through this module: no file but the app's own port probe binds a
    /// `TcpListener` itself, and nothing parses Content-Length by hand (`OTHER_STREAMS` read over something else).
    #[test]
    fn fakes_read_requests_through_test_http() {
        // A fake that reads over something other than a bare `TcpStream`. tls.rs reads through rustls, byte by byte to the
        // blank line and then exactly Content-Length bytes, so it cannot half-read a request.
        // desktop/server.rs binds only to find a free port and reads no request.
        const OTHER_STREAMS: &[&str] = &["src/fleet/tests/tls.rs", "src/desktop/server.rs"];
        let mut bad = Vec::new();
        let mut stack = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("test_http.rs") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    let name = path.strip_prefix(env!("CARGO_MANIFEST_DIR")).unwrap().to_string_lossy().replace('\\', "/");
                    // A fake binds through `bind`, whose accept loop ends with its test; one that binds its own listener keeps
                    // a thread for the rest of the run (`bind`'s comment says what that cost on macOS).
                    if text.contains("TcpListener::bind") && name != "src/desktop/server.rs" {
                        bad.push(format!("{name}: binds its own TcpListener; use test_http::bind so its thread ends with the test"));
                    }
                    if OTHER_STREAMS.contains(&name.as_str()) {
                        continue;
                    }
                    if text.to_ascii_lowercase().contains("strip_prefix(\"content-length:\")") {
                        bad.push(format!("{name}: parses Content-Length by hand; use test_http::read"));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "fake servers read requests through crate::test_http:\n{}", bad.join("\n"));
    }
}
