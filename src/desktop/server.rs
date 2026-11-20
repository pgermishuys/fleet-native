//! Starting Fleet's server in desktop mode, keeping it running and stopping it (`desktop/src/server.ts`). The server
//! stops itself when its standard input closes, which is how the app dying any way at all frees the data folder.
//!
//! Everything blocking happens on the supervisor thread this starts; the app reads [`Event`]s from the channel. No
//! Rune, no async runtime.
use super::log::Log;
use std::io::Read;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Fleet's exit code when another Fleet holds its database lock (`FleetInstanceLock.InUseExitCode`).
pub const IN_USE_EXIT_CODE: i32 = 75;

/// The port Fleet's desktop app asks for first.
pub const DEFAULT_PORT: u16 = 6262;

/// Variables that must not reach Fleet: Electron's own, listen URLs that would override the port we pick, and (native
/// only) the key this app was given for another Fleet.
const DROPPED_PREFIXES: &[&str] = &["ELECTRON_"];
const DROPPED_NAMES: &[&str] = &["URLS", "ASPNETCORE_URLS", "DOTNET_URLS", "ASPNETCORE_ENVIRONMENT", "FLEET_NATIVE_TOKEN"];

/// The environment the server runs with: the app's own, under the shared data folder, in desktop mode. No login token:
/// Fleet signs in loopback requests itself, and only when it binds a loopback address, hence `Fleet__Host`.
pub fn server_env(base: impl IntoIterator<Item = (String, String)>, data_dir: &Path, port: u16) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = base
        .into_iter()
        .filter(|(key, _)| {
            let upper = key.to_uppercase();
            !DROPPED_NAMES.contains(&upper.as_str()) && !DROPPED_PREFIXES.iter().any(|p| upper.starts_with(p))
        })
        .collect();
    let path = |name: &str| data_dir.join(name).display().to_string();
    let ours = [
        ("ASPNETCORE_ENVIRONMENT", "Production".to_string()),
        ("Fleet__Host", "127.0.0.1".to_string()),
        ("Fleet__Port", port.to_string()),
        ("Fleet__DatabasePath", path("fleet.db")),
        ("Fleet__AnalyticsDatabasePath", path("fleet-analytics.db")),
        ("Fleet__DataProtection__KeyPath", path("fleet-keys")),
        ("Fleet__Desktop__Enabled", "true".to_string()),
    ];
    // Ours win over a variable of the same name in any case (Windows has no cases; .NET reads them without).
    env.retain(|(key, _)| !ours.iter().any(|(name, _)| name.eq_ignore_ascii_case(key)));
    env.extend(ours.into_iter().map(|(k, v)| (k.to_string(), v)));
    env
}

/// When to restart a Fleet that crashed (`recent` are the crashes' times in ms): straight away the first time, then
/// 1 s, 2 s, 4 s, and not at all after five inside a minute (`None`).
pub fn restart_delay(recent: &[i64], now: i64) -> Option<u64> {
    let in_last_minute = recent.iter().filter(|t| now - **t < 60_000).count();
    if in_last_minute >= 5 {
        return None;
    }
    Some(if in_last_minute <= 1 { 0 } else { 1000 * 2u64.pow(in_last_minute as u32 - 2) })
}

/// Whether nothing listens on the port, on any address. A bind on 127.0.0.1 alone isn't enough: on Windows it succeeds
/// beside a holder of 0.0.0.0, and loopback calls then reach the newcomer. So anything that accepts a loopback connection
/// owns the port, and only then is a bind tried.
pub fn can_listen(port: u16) -> bool {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    if TcpStream::connect_timeout(&addr, Duration::from_secs(1)).is_ok() {
        return false;
    }
    TcpListener::bind(addr).is_ok()
}

/// The first of `preferred` that is free (the port used last time, then 6262), otherwise any free one. A stable port keeps
/// the client's local storage between runs.
pub fn pick_port(preferred: &[u16]) -> std::io::Result<u16> {
    if let Some(port) = preferred.iter().copied().find(|p| *p != 0 && can_listen(*p)) {
        return Ok(port);
    }
    Ok(TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?.local_addr()?.port())
}

/// What the server tells the app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Fleet is listening at the URL. Fires again after each restart.
    Ready { url: String, port: u16 },
    /// Another Fleet took the data folder's lock first: look for that one instead.
    Taken,
    /// Fleet crashed and starts again after `delay_ms`.
    Restarting { delay_ms: u64, exit_code: Option<i32> },
    /// Fleet keeps crashing, never answered, or couldn't start; the words are the user's.
    Failed(String),
}

/// How a server is started.
#[derive(Clone, Debug)]
pub struct Options {
    pub binary: PathBuf,
    pub content_root: PathBuf,
    pub data_dir: PathBuf,
    pub log_file: PathBuf,
    /// Ports to try first, in order (see [`pick_port`]).
    pub preferred_ports: Vec<u16>,
    /// The app's own environment, which [`server_env`] adjusts.
    pub env: Vec<(String, String)>,
    /// How long `/readyz` may stay silent (Fleet: 60 s).
    pub ready_timeout: Duration,
    /// How long the log may grow before it is rolled (Fleet: 10 MB).
    pub log_limit: u64,
}

impl Options {
    pub fn new(binary: PathBuf, content_root: PathBuf, data_dir: PathBuf, log_file: PathBuf) -> Options {
        Options {
            binary,
            content_root,
            data_dir,
            log_file,
            preferred_ports: vec![DEFAULT_PORT],
            env: std::env::vars().collect(),
            ready_timeout: Duration::from_secs(60),
            log_limit: super::log::LIMIT,
        }
    }
}

#[derive(Default)]
struct Shared {
    stopping: AtomicBool,
    /// The supervisor has finished for good.
    done: AtomicBool,
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
}

/// A running (or restarting) server. Dropping it without `stop` closes the server's standard input, which stops it.
pub struct FleetServer {
    shared: Arc<Shared>,
    events: Receiver<Event>,
    url: String,
    port: u16,
}

impl FleetServer {
    /// Picks the port and starts the supervisor thread. Only picking the port can fail.
    pub fn start(options: Options) -> std::io::Result<FleetServer> {
        let port = pick_port(&options.preferred_ports)?;
        let (tx, events) = channel();
        let shared = Arc::new(Shared::default());
        let url = format!("http://127.0.0.1:{port}");
        let (s, u) = (shared.clone(), url.clone());
        std::thread::Builder::new().name("fleet-server".into()).spawn(move || supervise(&options, &s, &tx, port, &u))?;
        Ok(FleetServer { shared, events, url, port })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// The pid of the running server, if one runs now.
    pub fn pid(&self) -> Option<u32> {
        self.shared.child.lock().ok()?.as_ref().map(Child::id)
    }

    /// The channel of events.
    pub fn events(&self) -> &Receiver<Event> {
        &self.events
    }

    /// Closes the server's standard input, which stops it cleanly with its agents; asks, then kills, if it takes longer
    /// than `grace` (20 s in the app). Returns once it is gone. Blocking: not for the UI thread.
    pub fn stop(&self, grace: Duration) {
        self.shared.stopping.store(true, Ordering::SeqCst);
        drop(self.shared.stdin.lock().ok().and_then(|mut s| s.take()));
        if self.wait_gone(grace) {
            return;
        }
        if self.terminate() && self.wait_gone(Duration::from_secs(5)) {
            return;
        }
        if let Some(child) = self.shared.child.lock().ok().as_mut().and_then(|c| c.as_mut()) {
            let _ = child.kill();
        }
        self.wait_gone(Duration::from_secs(10));
    }

    fn wait_gone(&self, within: Duration) -> bool {
        let deadline = Instant::now() + within;
        while !self.shared.done.load(Ordering::SeqCst) {
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        true
    }

    /// SIGTERM, true if sent; Windows has no such thing, so its second step is the kill.
    fn terminate(&self) -> bool {
        #[cfg(unix)]
        if let Some(pid) = self.pid().and_then(|p| libc::pid_t::try_from(p).ok()) {
            // SAFETY: the supervisor reaps the child only after this process saw it exit, so the pid is still ours.
            return unsafe { libc::kill(pid, libc::SIGTERM) } == 0;
        }
        false
    }
}

impl Drop for FleetServer {
    fn drop(&mut self) {
        // The app going away closes the pipe, and the server stops by itself; do it now rather than at process exit.
        self.shared.stopping.store(true, Ordering::SeqCst);
        drop(self.shared.stdin.lock().ok().and_then(|mut s| s.take()));
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Sleeps `ms`, in slices so a stop is noticed; false if stopped.
fn pause(shared: &Shared, ms: u64) -> bool {
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        if shared.stopping.load(Ordering::SeqCst) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10).min(end - Instant::now()));
    }
    !shared.stopping.load(Ordering::SeqCst)
}

fn supervise(options: &Options, shared: &Arc<Shared>, tx: &Sender<Event>, port: u16, url: &str) {
    let mut crashes: Vec<i64> = Vec::new();
    loop {
        if shared.stopping.load(Ordering::SeqCst) {
            break;
        }
        let Some(code) = launch(options, shared, tx, port, url) else {
            break;
        };
        if shared.stopping.load(Ordering::SeqCst) {
            break;
        }
        if code == Some(IN_USE_EXIT_CODE) {
            let _ = tx.send(Event::Taken);
            break;
        }
        crashes.push(now_ms());
        let Some(delay) = restart_delay(&crashes, now_ms()) else {
            let _ = tx.send(Event::Failed("Fleet keeps stopping unexpectedly.".into()));
            break;
        };
        let _ = tx.send(Event::Restarting { delay_ms: delay, exit_code: code });
        if !pause(shared, delay) {
            break;
        }
    }
    shared.done.store(true, Ordering::SeqCst);
}

/// One run of the server: start it, say when it is ready, wait for it to exit. `None` when it never ran (the event
/// saying why is sent); else its exit code (none after a signal).
fn launch(options: &Options, shared: &Arc<Shared>, tx: &Sender<Event>, port: u16, url: &str) -> Option<Option<i32>> {
    if !options.binary.is_file() {
        let _ = tx.send(Event::Failed(super::paths::not_found_message(std::slice::from_ref(&options.binary))));
        return None;
    }
    let _ = std::fs::create_dir_all(options.data_dir.join("fleet-keys"));
    let log = Log::open(&options.log_file, options.log_limit).unwrap_or_else(|_| Log::sink());
    log.write(&format!("\n── {} starting {} on {url}\n", crate::fleet::session_commands::iso_utc(now_ms()), options.binary.display()));
    if let Some(note) = super::paths::staged_update_note(&options.binary, &|p| p.is_dir()) {
        log.write(&format!("── note: {note}\n"));
    }

    let mut command = Command::new(&options.binary);
    command
        .args(["--urls", url, "--contentRoot"])
        .arg(&options.content_root)
        .current_dir(&options.data_dir)
        .env_clear()
        .envs(server_env(options.env.iter().cloned(), &options.data_dir, port))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000); // CREATE_NO_WINDOW
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            log.write(&format!("── could not start Fleet: {error}\n"));
            let _ = tx.send(Event::Failed(format!("Couldn't start Fleet: {error}")));
            return None;
        }
    };
    let pumps: Vec<_> = [child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>), child.stderr.take().map(|s| Box::new(s) as _)]
        .into_iter()
        .flatten()
        .map(|mut pipe| {
            let log = log.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                while let Ok(n) = pipe.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    log.write_bytes(&buf[..n]);
                }
            })
        })
        .collect();
    *shared.stdin.lock().unwrap() = child.stdin.take();
    *shared.child.lock().unwrap() = Some(child);
    // A stop that came in while it started closed no stdin yet.
    if shared.stopping.load(Ordering::SeqCst) {
        drop(shared.stdin.lock().unwrap().take());
    }

    let deadline = Instant::now() + options.ready_timeout;
    let mut announced = false;
    let status = loop {
        if let Some(status) = shared.child.lock().unwrap().as_mut().and_then(|c| c.try_wait().ok().flatten()) {
            break status;
        }
        if !announced && Instant::now() < deadline {
            // `/readyz` every 250 ms, a second to answer; the wait between looks also watches for the exit.
            if crate::fleet::instance::is_ready(url, Duration::from_secs(1)) {
                announced = true;
                let _ = tx.send(Event::Ready { url: url.to_string(), port });
            }
        } else if !announced {
            announced = true;
            let _ = tx.send(Event::Failed("Fleet started but never answered.".into()));
        }
        std::thread::sleep(Duration::from_millis(if announced { 50 } else { 250 }));
    };
    shared.child.lock().unwrap().take();
    shared.stdin.lock().unwrap().take();
    for pump in pumps {
        let _ = pump.join();
    }
    #[cfg(unix)]
    let signal = std::os::unix::process::ExitStatusExt::signal(&status);
    #[cfg(not(unix))]
    let signal: Option<i32> = None;
    log.write(&format!(
        "── Fleet exited (code {}, signal {})\n",
        status.code().map_or("none".to_string(), |c| c.to_string()),
        signal.map_or("none".to_string(), |s| s.to_string())
    ));
    Some(status.code())
}
