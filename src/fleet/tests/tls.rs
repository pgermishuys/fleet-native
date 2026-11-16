//! https and wss (`fleet::tls`, `http`, `link::connect`): a TLS server on loopback with a certificate made for the test
//! (`tests/tls/*.der`, valid for 127.0.0.1, never used outside tests). The test trusts it by adding it to the roots; the app has no
//! way to be told to trust anything, and no switch that turns verification off.
use crate::fleet::link::{connect, host_of};
use crate::fleet::tls::test_roots;
use crate::fleet::{auth, http};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;
use tungstenite::Message;

const TRUSTED: (&[u8], &[u8]) = (include_bytes!("tls/trusted.cert.der"), include_bytes!("tls/trusted.key.der"));
const OTHER: (&[u8], &[u8]) = (include_bytes!("tls/other.cert.der"), include_bytes!("tls/other.key.der"));

fn server_config((cert, key): (&[u8], &[u8])) -> Arc<ServerConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![CertificateDer::from(cert.to_vec())], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.to_vec())))
        .unwrap();
    Arc::new(config)
}

/// Answers `connections` TLS connections: a POST gets `{"connectionToken":"tok"}`, a GET `{"ok":true}`, an upgrade becomes a
/// WebSocket that answers the hub handshake and then waits to be closed. Reports each request line it saw.
#[allow(clippy::result_large_err)]
fn serve(cert: (&[u8], &[u8]), connections: usize) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("https://{}", listener.local_addr().unwrap());
    let config = server_config(cert);
    let handle = thread::spawn(move || {
        let mut seen = Vec::new();
        for _ in 0..connections {
            let (tcp, _) = listener.accept().unwrap();
            let conn = ServerConnection::new(config.clone()).unwrap();
            let mut tls = StreamOwned::new(conn, tcp);
            // The request head, read to the blank line (bodies are small and ignored).
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") {
                match tls.read(&mut byte) {
                    Ok(1) => head.push(byte[0]),
                    _ => break,
                }
            }
            let text = String::from_utf8_lossy(&head).to_string();
            seen.push(text.lines().next().unwrap_or("").to_string());
            if text.to_ascii_lowercase().contains("upgrade: websocket") {
                // Replay the head to tungstenite's server side through a stream that has already read it.
                let mut ws = tungstenite::accept_hdr(
                    Replay { head: head.clone(), at: 0, inner: tls },
                    |_: &_, r: tungstenite::handshake::server::Response| Ok::<_, tungstenite::handshake::server::ErrorResponse>(r),
                )
                .unwrap();
                let first = ws.read().unwrap();
                assert!(matches!(first, Message::Text(ref t) if t.contains("\"protocol\":\"json\"")));
                ws.send(Message::text("{}\u{1e}")).unwrap();
                let _ = ws.read();
            } else {
                let body = if text.starts_with("POST") { r#"{"connectionToken":"tok"}"# } else { r#"{"ok":true}"# };
                let reply = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = tls.write_all(reply.as_bytes());
                let _ = tls.flush();
                tls.conn.send_close_notify();
                let _ = tls.flush();
            }
        }
        seen
    });
    (base, handle)
}

struct Replay<S> {
    head: Vec<u8>,
    at: usize,
    inner: S,
}
impl<S: Read> Read for Replay<S> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.at < self.head.len() {
            let n = buf.len().min(self.head.len() - self.at);
            buf[..n].copy_from_slice(&self.head[self.at..self.at + n]);
            self.at += n;
            return Ok(n);
        }
        self.inner.read(buf)
    }
}
impl<S: Write> Write for Replay<S> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[test]
fn an_https_server_answers_http_calls_when_its_certificate_is_trusted() {
    test_roots::trust(TRUSTED.0);
    let (base, server) = serve(TRUSTED, 1);
    assert_eq!(http::get(&base, "/readyz").unwrap()["ok"], true);
    assert_eq!(server.join().unwrap(), ["GET /readyz HTTP/1.1"]);
}

#[test]
fn an_https_server_with_a_certificate_nobody_trusts_is_refused() {
    test_roots::trust(TRUSTED.0);
    let (base, _server) = serve(OTHER, 1);
    let why = http::get(&base, "/readyz").unwrap_err();
    assert!(why.contains("invalid peer certificate"), "{why}");
    // The hub too: the negotiate call is the first thing it says.
    let (base, _server) = serve(OTHER, 1);
    let why = connect(&base).err().expect("refused");
    assert!(why.contains("invalid peer certificate"), "{why}");
}

#[test]
fn the_hub_connects_over_wss_and_the_token_goes_to_its_own_address_only() {
    test_roots::trust(TRUSTED.0);
    let (base, server) = serve(TRUSTED, 2);
    auth::set_for(&base, Some("hub-secret-123"));
    let ws = connect(&base).expect("wss connects and the hub answers the handshake");
    drop(ws);
    let seen = server.join().unwrap();
    assert_eq!(seen.len(), 2);
    assert!(seen[0].starts_with("POST /hubs/session-events/negotiate"), "{seen:?}");
    assert!(seen[1].starts_with("GET /hubs/session-events?id=tok&access_token=hub-secret-123 "), "{seen:?}");
}

#[test]
fn an_address_says_whether_it_is_secure_and_where_to_connect() {
    use crate::fleet::link::is_secure;
    assert!(is_secure("https://falcon.tail1234.ts.net") && !is_secure("http://127.0.0.1:6411"));
    assert_eq!(host_of("https://[::1]").unwrap(), "[::1]:443");
}

/// Live check (not part of `cargo test`): a real Fleet behind a TLS proxy whose certificate is `FLEET_LIVE_CERT` (a DER file),
/// at `FLEET_LIVE_BASE` (an https:// address). The certificate is trusted by this test alone.
#[test]
#[ignore = "needs a Fleet behind a TLS proxy: see docs/FLEET.md"]
fn live_a_fleet_behind_a_tls_proxy() {
    let base = std::env::var("FLEET_LIVE_BASE").expect("FLEET_LIVE_BASE");
    test_roots::trust(&std::fs::read(std::env::var("FLEET_LIVE_CERT").expect("FLEET_LIVE_CERT")).unwrap());
    assert!(http::ready(&base, std::time::Duration::from_secs(5)));
    let sessions = http::get(&base, "/api/sessions").unwrap();
    println!("https: GET /api/sessions answered, {} bytes", sessions.to_string().len());
    let mut ws = connect(&base).expect("the hub over wss");
    ws.send(Message::text(r#"{"type":6}"#.to_string() + "\u{1e}")).unwrap();
    println!("wss: the hub answered the handshake and took a ping");
}

/// Live check: real certificates verify. `https://github.com/` over ureq with the public roots (no test certificate trusted).
#[test]
#[ignore = "needs the internet"]
fn live_a_public_https_host_verifies() {
    let status = ureq::get("https://github.com/").call().map(|r| r.status().as_u16());
    println!("https://github.com/ -> {status:?}");
    assert!(matches!(status, Ok(200)));
}
