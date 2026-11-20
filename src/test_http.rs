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
/// it could after a bind-then-drop) and each call is closed unanswered.
pub fn dead_address() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || for _ in listener.incoming() {});
    base
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

    /// Every fake server reads its requests through this module. A file that binds a `TcpListener` must say
    /// `test_http::` (or be listed in `OTHER_STREAMS` with the reason), and nothing else may parse Content-Length by hand.
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
                    if OTHER_STREAMS.contains(&name.as_str()) {
                        continue;
                    }
                    if text.contains("TcpListener::bind") && !text.contains("test_http::") {
                        bad.push(format!("{name}: binds a TcpListener but doesn't read through test_http"));
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
