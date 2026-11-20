//! Starting Fleet from the app (component 24, piece B): what `desktop/src/main.ts`'s `connect()` does. Look once for a Fleet
//! already running on this data; if none answers, start the server the app can find (`paths`, `server`) and connect to it
//! when it is ready. While that happens the window shows the splash, and a failure shows the error page
//! (`view::start_page`). All the blocking work runs on one thread of its own and hands each result to the UI as a job.
use super::log::{self, Log};
use super::paths::{self, Inputs};
use super::server::{self, Event, FleetServer, Stopper};
use crate::fleet::discover::{self, Found, Place};
use crate::fleet::link::Post;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// What the window shows of the start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Nothing to show: the shell is up (or the app never starts a Fleet).
    Idle,
    /// The splash, with its status line.
    Starting(String),
    /// The error page: what went wrong, and where the server's log is.
    Failed { message: String, log: String },
    /// The shell shows, and the Fleet it shows is this app's own.
    Owned,
}

/// The two things the error page's buttons do; the app sets them, a test replaces them.
#[derive(Clone)]
pub struct Actions {
    pub retry: Arc<dyn Fn() + Send + Sync>,
    pub open_logs: Arc<dyn Fn(&Path) + Send + Sync>,
}

pub struct StartState {
    pub phase: Phase,
    pub actions: Option<Actions>,
    /// The server this app started. Dropping it closes the server's input, which stops it; `stop` waits for that.
    pub owned: Option<Stopper>,
}

impl StartState {
    /// `starting`: the window opens on the splash.
    pub fn new(starting: bool) -> StartState {
        StartState { phase: if starting { Phase::Starting(STARTING.into()) } else { Phase::Idle }, actions: None, owned: None }
    }

    /// The splash or the error page covers the window.
    pub fn covers(&self) -> bool {
        matches!(self.phase, Phase::Starting(_) | Phase::Failed { .. })
    }

    /// Try again: `connect()` from the top.
    pub fn retry(&mut self) {
        if let Some(actions) = self.actions.clone()
            && matches!(self.phase, Phase::Failed { .. })
        {
            self.phase = Phase::Starting(STARTING.into());
            (actions.retry)();
        }
    }

    /// Open logs: the folder the log is in, in the system's file manager.
    pub fn open_logs(&self) {
        if let (Some(actions), Phase::Failed { log, .. }) = (&self.actions, &self.phase)
            && let Some(dir) = Path::new(log).parent()
        {
            (actions.open_logs)(dir);
        }
    }

    /// The app is quitting: an owned server gets its input closed and is waited for (20 s, then asked, then killed) on a
    /// thread of its own, so the quit itself never waits. The server stops by itself with the app anyway.
    pub fn stop_owned(&mut self) {
        if let Some(server) = self.owned.take() {
            let _ = std::thread::Builder::new().name("fleet-server-stop".into()).spawn(move || server.stop(STOP_GRACE));
        }
    }
}

pub const STARTING: &str = "Starting Fleet…";
const STOP_GRACE: Duration = Duration::from_secs(20);
/// Fleet's `main.ts`: another Fleet took the data, so look for it every 500 ms for 30 s.
const TAKEN_EVERY: Duration = Duration::from_millis(500);
const TAKEN_FOR: Duration = Duration::from_secs(30);
const TAKEN_WORDS: &str = "Another Fleet is using your data, but it isn't answering.";

/// Reads an environment variable (a test gives its own).
pub type Getter = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Takes the app's environment and returns it with the login shell's merged in.
pub type EnvMerge = Arc<dyn Fn(Vec<(String, String)>) -> Vec<(String, String)> + Send + Sync>;

/// Everything the flow needs from outside, so a test can give it a fake server, a scratch folder and no real Fleet.
#[derive(Clone)]
pub struct Config {
    pub post: Post,
    pub get: Getter,
    /// Where a running Fleet is looked for (the instance file, the saved port, 6262).
    pub places: Arc<dyn Fn() -> Vec<Place> + Send + Sync>,
    /// Connect the app to a Fleet that was found (adopts this machine's key, starts the link).
    pub connect: Arc<dyn Fn(Found) + Send + Sync>,
    /// No server to start anywhere: the app keeps looking for one somebody else starts, and says where it looked.
    pub keep_looking: Arc<dyn Fn(Vec<PathBuf>) + Send + Sync>,
    /// Added to the server's environment (a test picks its stand-in's mode here).
    pub env: Vec<(String, String)>,
    /// Adds the login shell's environment to what the server gets (`shell_env::merged`); a test gives its own so no real
    /// shell is asked. Runs on the start thread, so a slow shell only delays the splash, never the window.
    pub login_env: EnvMerge,
    /// Ports to try, in order; `None` is the remembered one, then Fleet's 6262.
    pub ports: Option<Vec<u16>>,
    pub ready_timeout: Duration,
    pub taken_every: Duration,
    pub taken_for: Duration,
}

impl Config {
    pub fn new(
        post: Post,
        get: Getter,
        places: Arc<dyn Fn() -> Vec<Place> + Send + Sync>,
        connect: Arc<dyn Fn(Found) + Send + Sync>,
        keep_looking: Arc<dyn Fn(Vec<PathBuf>) + Send + Sync>,
    ) -> Config {
        Config {
            post,
            get,
            places,
            connect,
            keep_looking,
            env: Vec::new(),
            login_env: Arc::new(super::shell_env::merged),
            ports: None,
            ready_timeout: Duration::from_secs(60),
            taken_every: TAKEN_EVERY,
            taken_for: TAKEN_FOR,
        }
    }
}

/// Whether this run starts a Fleet of its own when none runs (decision 4): a normal start only, and not when
/// `FLEET_NATIVE_START_SERVER=0` says the person starts Fleet themselves.
pub fn wanted(headless: bool, demo: bool, explicit_server: bool, get: &dyn Fn(&str) -> Option<String>) -> bool {
    !headless && !demo && !explicit_server && get("FLEET_NATIVE_START_SERVER").as_deref() != Some("0")
}

/// Begins `connect()` on a thread: the splash first.
pub fn begin(config: Arc<Config>) {
    config.post.update(|s, _| {
        if !matches!(s.start.phase, Phase::Starting(_)) {
            s.start.phase = Phase::Starting(STARTING.into());
        }
    });
    let spawned = std::thread::Builder::new().name("fleet-start".into()).spawn({
        let config = config.clone();
        move || connect(&config)
    });
    if let Err(e) = spawned {
        fail(&config, format!("Couldn't start Fleet: {e}"), String::new());
    }
}

fn fail(config: &Config, message: String, log: String) {
    config.post.update(move |s, _| s.start.phase = Phase::Failed { message, log });
}

fn idle(config: &Config) {
    config.post.update(|s, _| s.start.phase = Phase::Idle);
}

/// `connect()`: look, then start; every way out ends in the shell, the error page, or the old search.
fn connect(config: &Config) {
    if let Some(found) = discover::find_in(&(config.places)()) {
        eprintln!("fleet-native: found a Fleet at {} ({})", found.url, found.place.describe());
        idle(config);
        return (config.connect)(found);
    }
    let get = |k: &str| (config.get)(k);
    let found = paths::find_server(&Inputs::from_system(&get));
    let bin = match found {
        Ok(bin) => bin,
        Err(looked) => {
            idle(config);
            return (config.keep_looking)(looked);
        }
    };
    let (Some(data_dir), Some(app_dir)) = (crate::fleet::instance::data_dir(get), crate::composer::drafts::app_dir(get)) else {
        return fail(config, "Couldn't start Fleet: no folder for its data.".into(), String::new());
    };
    let log_file = log::log_file(&app_dir);
    let shown = log_file.display().to_string();
    let mut options = server::Options::new(bin.binary.clone(), bin.content_root, data_dir, log_file.clone());
    options.ready_timeout = config.ready_timeout;
    // The port this app's Fleet used last time keeps the client's local storage; then Fleet's own 6262.
    options.preferred_ports =
        config.ports.clone().unwrap_or_else(|| remembered_port(&app_dir).into_iter().chain([server::DEFAULT_PORT]).collect());
    options.env.extend(config.env.iter().cloned());
    options.env = (config.login_env)(std::mem::take(&mut options.env));
    let server = match FleetServer::start(options) {
        Ok(server) => server,
        Err(e) => return fail(config, format!("Couldn't start Fleet: {e}"), shown),
    };
    watch(config, server, &log_file, &shown);
}

fn remembered_port(app_dir: &Path) -> Option<u16> {
    crate::fleet::settings::load(app_dir).desktop_port()
}

/// What the server says, until it is gone for good.
fn watch(config: &Config, server: FleetServer, log_file: &Path, shown: &str) {
    let mut connected: Option<String> = None;
    while let Ok(event) = server.events().recv() {
        match event {
            // A restart after a crash says ready again with the same address: the window is on it already.
            Event::Ready { url, port } => {
                if connected.as_deref() == Some(url.as_str()) {
                    continue;
                }
                connected = Some(url.clone());
                let owned = server.stopper();
                config.post.update(move |s, _| {
                    s.start.owned = Some(owned);
                    s.start.phase = Phase::Owned;
                    s.sidebar.save_desktop_port(port);
                    s.save_settings();
                });
                (config.connect)(Found { url, place: Place::Port(port) });
            }
            Event::Taken => return taken(config, shown),
            Event::Restarting { delay_ms, exit_code } => {
                let code = exit_code.map_or("none".to_string(), |c| c.to_string());
                if let Ok(log) = Log::open(log_file, log::LIMIT) {
                    log.write(&format!("Fleet stopped (exit code {code}); restarting in {delay_ms} ms.\n"));
                }
            }
            Event::Failed(message) => {
                fail(config, message, shown.to_string());
                // It may still run (it never answered): it goes, and the next try starts clean.
                let _ = std::thread::Builder::new().name("fleet-server-stop".into()).spawn(move || server.stop(Duration::from_secs(2)));
                return;
            }
        }
    }
}

/// Exit 75: another Fleet holds the data. Use it once it answers; if it never does, say so.
fn taken(config: &Config, shown: &str) {
    let until = Instant::now() + config.taken_for;
    loop {
        if let Some(found) = discover::find_in(&(config.places)()) {
            idle(config);
            return (config.connect)(found);
        }
        if Instant::now() >= until {
            return fail(config, TAKEN_WORDS.into(), shown.to_string());
        }
        std::thread::sleep(config.taken_every);
    }
}

/// The system's own opener for a folder, on a thread: nothing waits for a file manager.
pub fn open_folder(dir: &Path) {
    let dir = dir.to_path_buf();
    let _ = std::thread::Builder::new().name("fleet-open-logs".into()).spawn(move || {
        let program = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(windows) {
            "explorer"
        } else {
            "xdg-open"
        };
        let _ = std::process::Command::new(program).arg(&dir).status();
    });
}
