//! A stand-in for Fleet's server, for the desktop shell's process tests (`src/desktop/tests.rs`): it takes the arguments the
//! app gives the real server (`--urls <url> --contentRoot <dir>`) and acts as `FAKE_FLEET_MODE` says. Pure std, so it builds
//! and runs the same on Linux, macOS and Windows runners with no Fleet installed.
//!
//! Modes: `serve` (answers `/readyz`, exits 0 when its standard input closes, like the real server), `taken` (exit 75),
//! `crash` (exit 1), `crash-once` (exit 1 the first time in this folder, then `serve`), `silent` (runs, never listens),
//! `deaf` (serves and ignores a closed standard input).
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let url = args.iter().position(|a| a == "--urls").and_then(|i| args.get(i + 1)).cloned().unwrap_or_default();
    let mode = std::env::var("FAKE_FLEET_MODE").unwrap_or_else(|_| "serve".into());
    println!("fake fleet {mode} args={}", args[1..].join(" "));
    eprintln!("fake fleet stderr line");
    for key in
        ["Fleet__Host", "Fleet__Port", "Fleet__Desktop__Enabled", "ASPNETCORE_ENVIRONMENT", "ELECTRON_RUN_AS_NODE", "FLEET_NATIVE_TOKEN"]
    {
        println!("env {key}={}", std::env::var(key).unwrap_or_else(|_| "(unset)".into()));
    }
    println!("cwd={}", std::env::current_dir().map(|d| d.display().to_string()).unwrap_or_default());
    match mode.as_str() {
        "taken" => exit(75),
        "crash" => exit(1),
        "crash-once" => {
            if !std::path::Path::new("crashed-once").exists() {
                let _ = std::fs::write("crashed-once", "x");
                exit(1);
            }
            serve(&url, true);
        }
        "silent" => wait_for_stdin_end(),
        "deaf" => serve(&url, false),
        _ => serve(&url, true),
    }
}

fn wait_for_stdin_end() {
    let mut sink = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut sink);
    exit(0);
}

fn serve(url: &str, stop_on_stdin_end: bool) {
    let address = url.trim_start_matches("http://");
    let listener = TcpListener::bind(address).expect("the port the app gave is free");
    std::io::stdout().flush().ok();
    if stop_on_stdin_end {
        std::thread::spawn(wait_for_stdin_end);
    }
    for stream in listener.incoming().flatten() {
        let mut stream = stream;
        let mut seen = Vec::new();
        let mut buf = [0u8; 1024];
        while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
            match stream.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => seen.extend_from_slice(&buf[..n]),
            }
        }
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nHealthy");
    }
}
