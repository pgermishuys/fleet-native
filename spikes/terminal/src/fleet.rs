//! `--fleet <url>`: a Fleet server's terminal instead of a local shell. Opens Fleet's setup
//! terminal over HTTP and attaches to its socket (`TerminalSocket.cs`): binary frames are output
//! (the saved scrollback first, then `{"type":"ready"}`, then live), text frames are control.
//! Loopback and no token only, by hand over std's `TcpStream` so the spike adds no crate; the
//! app would use the HTTP and WebSocket crates slice B brings.
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};

use crate::engine::{Engine, Input};

fn fail(what: impl std::fmt::Display) -> io::Error {
    io::Error::other(what.to_string())
}

/// One client frame: FIN, `op`, masked as RFC 6455 requires of a client (a fixed mask: it's
/// there for proxies, not secrecy).
fn send(w: &mut impl Write, op: u8, payload: &[u8]) -> io::Result<()> {
    let n = payload.len();
    let mut f = vec![0x80 | op];
    match n {
        0..126 => f.push(0x80 | n as u8),
        126..65536 => f.extend([0x80 | 126].into_iter().chain((n as u16).to_be_bytes())),
        _ => f.extend([0x80 | 127].into_iter().chain((n as u64).to_be_bytes())),
    }
    let mask = [0x5a, 0x17, 0xc3, 0x2e];
    f.extend(mask);
    f.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    w.write_all(&f)
}

/// One server frame: (op, payload). Servers don't mask.
fn receive(r: &mut impl Read) -> io::Result<(u8, Vec<u8>)> {
    let mut h = [0u8; 2];
    r.read_exact(&mut h)?;
    let n = match h[1] & 0x7f {
        126 => {
            let mut b = [0; 2];
            r.read_exact(&mut b)?;
            u16::from_be_bytes(b) as usize
        }
        127 => {
            let mut b = [0; 8];
            r.read_exact(&mut b)?;
            u64::from_be_bytes(b) as usize
        }
        n => n as usize,
    };
    let mut payload = vec![0; n];
    r.read_exact(&mut payload)?;
    Ok((h[0] & 0x0f, payload))
}

pub fn attach(engine: &Engine, base: &str) -> io::Result<()> {
    let host = base.strip_prefix("http://").ok_or_else(|| fail("--fleet takes http://host:port"))?.trim_end_matches('/');
    let (cols, rows) = (engine.size.cols, engine.size.lines);

    let body = format!(r#"{{"cols":{cols},"rows":{rows}}}"#);
    let mut http = TcpStream::connect(host)?;
    write!(
        http,
        "POST /api/setup/terminals HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    let mut reply = String::new();
    http.read_to_string(&mut reply)?;
    let id = reply.split(r#""id":""#).nth(1).and_then(|r| r.split('"').next()).ok_or_else(|| fail(reply.lines().next().unwrap_or("")))?;

    let mut ws = TcpStream::connect(host)?;
    ws.set_nodelay(true)?;
    write!(
        ws,
        "GET /api/setup/terminals/{id}/socket?cols={cols}&rows={rows} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: c3Bpa2UtdGVybWluYWwtMQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
    )?;
    let mut reader = BufReader::new(ws.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    if !line.contains(" 101 ") {
        return Err(fail(line));
    }
    while line != "\r\n" {
        line.clear();
        reader.read_line(&mut line)?;
    }

    let writer = Arc::new(Mutex::new(ws));
    let out = writer.clone();
    *engine.shared.out.lock().unwrap() = Some(Box::new(move |input| {
        let mut w = out.lock().unwrap();
        let _ = match input {
            Input::Bytes(b) => send(&mut *w, 2, &b),
            Input::Resize(s) => send(&mut *w, 1, format!(r#"{{"type":"resize","cols":{},"rows":{}}}"#, s.num_cols, s.num_lines).as_bytes()),
        };
    }));

    let (term, shared) = (engine.term.clone(), engine.shared.clone());
    std::thread::spawn(move || {
        let mut parser: Processor<StdSyncHandler> = Processor::new();
        while let Ok((op, payload)) = receive(&mut reader) {
            match op {
                0 | 2 => {
                    parser.advance(&mut *term.lock(), &payload);
                    shared.bytes.fetch_add(payload.len() as u64, Ordering::Relaxed);
                }
                // {"type":"ready"} after the scrollback, {"type":"cleared"}, {"type":"exit",...}.
                1 => *shared.title.lock().unwrap() = String::from_utf8_lossy(&payload).into_owned(),
                9 => drop(send(&mut *writer.lock().unwrap(), 10, &payload)),
                // 1000: the shell ended; 4001: fell behind the output (Fleet's client reconnects).
                8 => {
                    let code = payload.get(..2).map_or(0, |c| u16::from_be_bytes([c[0], c[1]]));
                    *shared.title.lock().unwrap() =
                        format!("closed {code} {}", String::from_utf8_lossy(payload.get(2..).unwrap_or_default()));
                    break;
                }
                _ => {}
            }
            shared.wake();
        }
        shared.exited.store(true, Ordering::Release);
        shared.wake();
    });
    Ok(())
}
