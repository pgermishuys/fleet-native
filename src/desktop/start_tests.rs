//! Piece B's tests: the app starts its own Fleet and shows it (`desktop::start`), against the stand-in server of piece A. A
//! headless app takes the jobs the flow posts, and the tests wait on what the window would show.
use super::server::{FleetServer, Options};
use super::start::{self, Actions, Config, Phase};
use super::tests::{Scratch, fake_binary};
use crate::fleet::discover::{Found, Place};
use crate::fleet::link::{Job, Link, Post};
use crate::{State, test_wait};
use rune::{App, Env, Runtime};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct Rig {
    rt: Runtime<State>,
    jobs: Receiver<Job>,
    scratch: Scratch,
    config: Arc<Config>,
    opened: Arc<Mutex<Vec<PathBuf>>>,
    connected: Arc<Mutex<Vec<String>>>,
    kept_looking: Arc<AtomicUsize>,
}

/// An app with nothing running, whose server is the stand-in in `mode`. `places` is where a running Fleet is looked for.
fn rig(name: &str, mode: &str, server_bin: Option<PathBuf>, places: Vec<Place>) -> Rig {
    let scratch = Scratch::new(name);
    let (tx, jobs) = channel::<Job>();
    let tx = Mutex::new(tx);
    let post = Post::new(move |job| {
        let _ = tx.lock().unwrap().send(job);
    });
    let bin = server_bin.unwrap_or_else(fake_binary);
    let env: Vec<(String, String)> = vec![
        ("FLEET_NATIVE_SERVER_BIN".into(), bin.display().to_string()),
        ("FLEET_DESKTOP_DATA_DIR".into(), scratch.data().display().to_string()),
        ("FLEET_NATIVE_DATA_DIR".into(), scratch.0.join("app").display().to_string()),
        ("HOME".into(), scratch.0.display().to_string()),
    ];
    let get: start::Getter = Arc::new(move |k| env.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()));
    let (connected, kept_looking) = (Arc::new(Mutex::new(Vec::new())), Arc::new(AtomicUsize::new(0)));
    let connect: Arc<dyn Fn(Found) + Send + Sync> = {
        let (post, connected) = (post.clone(), connected.clone());
        Arc::new(move |found| {
            connected.lock().unwrap().push(found.url.clone());
            let link = Link::start(found.url, post.clone());
            post.update(move |s, _| s.link = Some(link));
        })
    };
    let keep = kept_looking.clone();
    let keep_looking: Arc<dyn Fn(Vec<PathBuf>) + Send + Sync> = Arc::new(move |_| {
        keep.fetch_add(1, Ordering::SeqCst);
    });
    let mut config = Config::new(post, get, Arc::new(move || places.clone()), connect, keep_looking);
    config.env.push(("FAKE_FLEET_MODE".into(), mode.into()));
    config.ports = Some(vec![]);
    config.ready_timeout = Duration::from_secs(2);
    config.taken_for = Duration::from_millis(600);
    config.taken_every = Duration::from_millis(100);
    let config = Arc::new(config);
    let opened = Arc::new(Mutex::new(Vec::new()));
    let mut rt = App::new(crate::spec()).headless_runtime((1280.0, 760.0), Env::new(true, 1.0));
    let (again, seen) = (config.clone(), opened.clone());
    rt.update(move |s, _| {
        s.start = start::StartState::new(true);
        s.start.actions = Some(Actions {
            retry: Arc::new(move || start::begin(again.clone())),
            open_logs: Arc::new(move |dir: &Path| seen.lock().unwrap().push(dir.to_path_buf())),
        });
    });
    Rig { rt, jobs, scratch, config, opened, connected, kept_looking }
}

impl Rig {
    /// The server's log in the app's own folder, which is where the app writes it.
    fn log(&self) -> PathBuf {
        crate::desktop::log::log_file(&self.scratch.0.join("app"))
    }

    fn log_text(&self) -> String {
        std::fs::read_to_string(self.log()).unwrap_or_default()
    }

    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.jobs, what, done, |s| format!("{:?}, link {}", s.start.phase, s.link.is_some()));
    }
}

fn server_binary_starts(log: &str) -> usize {
    log.lines().filter(|l| l.contains(" starting ")).count()
}

#[test]
fn a_fake_server_reaches_the_shell_and_the_fleet_is_owned() {
    let mut r = rig("owned", "serve", None, vec![]);
    start::begin(r.config.clone());
    r.wait("the window to have its own Fleet", |s| s.start.phase == Phase::Owned && s.link.is_some());
    assert!(r.rt.state.start.owned.is_some(), "the app marks the Fleet as its own");
    assert!(!r.rt.state.start.covers(), "the shell shows, not the splash");
    let url = r.connected.lock().unwrap()[0].clone();
    assert!(url.starts_with("http://127.0.0.1:"), "{url}");
    // The port it came up on is kept for the next start (the client's local storage is keyed by address).
    let port: u16 = url.rsplit(':').next().unwrap().parse().unwrap();
    r.wait("the port to be remembered", |s| s.sidebar.desktop_port() == Some(port));
    r.rt.state.start.stop_owned();
    let log = r.log();
    r.wait("the server to be stopped", |_| std::fs::read_to_string(&log).is_ok_and(|t| t.contains("Fleet exited")));
}

#[test]
fn the_splash_shows_while_the_server_starts() {
    let mut r = rig("splash", "serve", None, vec![]);
    assert!(r.rt.state.start.covers());
    start::begin(r.config.clone());
    assert_eq!(r.rt.state.start.phase, Phase::Starting(start::STARTING.into()));
    r.wait("the Fleet", |s| s.start.phase == Phase::Owned);
    r.rt.state.start.stop_owned();
}

#[test]
fn a_server_that_never_answers_shows_the_error_page_with_fleets_words() {
    let mut r = rig("silent", "silent", None, vec![]);
    start::begin(r.config.clone());
    r.wait("the error page", |s| matches!(s.start.phase, Phase::Failed { .. }));
    let Phase::Failed { message, log } = r.rt.state.start.phase.clone() else { unreachable!() };
    assert_eq!(message, "Fleet started but never answered.");
    assert!(log.ends_with("fleet-server.log"), "the page says where the log is: {log}");
    assert!(Path::new(&log).exists(), "and it is there");
    r.rt.state.start.open_logs();
    assert_eq!(r.opened.lock().unwrap().as_slice(), [Path::new(&log).parent().unwrap().to_path_buf()], "Open logs opens its folder");
}

#[test]
fn a_server_that_cant_be_started_says_where_fleet_should_be() {
    let missing = PathBuf::from("/nowhere/WeaveFleet.Api");
    let mut r = rig("missing", "serve", Some(missing), vec![]);
    start::begin(r.config.clone());
    // No server anywhere: the page doesn't stay, and the old search for a Fleet someone else starts takes over.
    r.wait("the search to take over", |s| s.start.phase == Phase::Idle);
    test_wait::until_true("the search to be asked", || r.kept_looking.load(Ordering::SeqCst) == 1);
}

#[test]
fn another_fleet_on_the_data_that_never_answers_shows_the_error_page() {
    let mut r = rig("taken-silent", "taken", None, vec![]);
    start::begin(r.config.clone());
    r.wait("the error page", |s| matches!(s.start.phase, Phase::Failed { .. }));
    let Phase::Failed { message, .. } = r.rt.state.start.phase.clone() else { unreachable!() };
    assert_eq!(message, "Another Fleet is using your data, but it isn't answering.");
    assert!(r.rt.state.start.owned.is_none());
}

#[test]
fn another_fleet_that_took_the_data_is_used_once_it_answers() {
    // The stand-in "taken" exits 75 at once; the other Fleet is one that answers on a port of its own.
    let kept = Scratch::new("taken-other");
    let other = FleetServer::start(options(&kept, "serve")).unwrap();
    let place = Place::Port(other.port());
    test_wait::until_true("the other Fleet to answer", || crate::fleet::discover::find_in(std::slice::from_ref(&place)).is_some());
    let mut r = rig("taken-found", "taken", None, vec![]);
    // It isn't there when the app looks first, and is there when the app looks again.
    let places = Arc::new(Mutex::new(Vec::new()));
    let seen = places.clone();
    let mut config = (*r.config).clone();
    config.places = Arc::new(move || seen.lock().unwrap().clone());
    r.config = Arc::new(config);
    let later = places.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(250));
        later.lock().unwrap().push(place);
    });
    start::begin(r.config.clone());
    r.wait("the other Fleet to be used", |s| s.start.phase == Phase::Idle && s.link.is_some());
    assert!(r.rt.state.start.owned.is_none(), "a Fleet the app didn't start isn't its own");
    other.stop(Duration::from_secs(5));
}

#[test]
fn try_again_starts_again() {
    let mut r = rig("again", "silent", None, vec![]);
    start::begin(r.config.clone());
    r.wait("the error page", |s| matches!(s.start.phase, Phase::Failed { .. }));
    assert_eq!(server_binary_starts(&r.log_text()), 1);
    r.rt.state.start.retry();
    assert_eq!(r.rt.state.start.phase, Phase::Starting(start::STARTING.into()), "the splash is back at once");
    r.wait("the error page again", |s| matches!(s.start.phase, Phase::Failed { .. }));
    assert_eq!(server_binary_starts(&r.log_text()), 2, "a second start, not the same one");
}

#[test]
fn a_fleet_that_runs_already_is_used_and_not_started_again() {
    let kept = Scratch::new("found-other");
    let other = FleetServer::start(options(&kept, "serve")).unwrap();
    let place = Place::Port(other.port());
    test_wait::until_true("the running Fleet to answer", || crate::fleet::discover::find_in(std::slice::from_ref(&place)).is_some());
    let mut r = rig("found", "crash", None, vec![place]);
    start::begin(r.config.clone());
    r.wait("the running Fleet to be used", |s| s.start.phase == Phase::Idle && s.link.is_some());
    assert!(!r.log().exists(), "no server was started");
    assert!(r.rt.state.start.owned.is_none());
    // Quitting leaves it running: it isn't this app's to stop.
    r.rt.state.start.stop_owned();
    std::thread::sleep(Duration::from_millis(300));
    assert!(other.pid().is_some(), "the Fleet that was found keeps running");
    other.stop(Duration::from_secs(5));
}

#[test]
fn a_run_with_a_server_or_no_window_never_starts_a_fleet() {
    let none = |_: &str| None;
    assert!(start::wanted(false, false, false, &none), "a normal start does");
    assert!(!start::wanted(false, false, true, &none), "--server: that Fleet is the one");
    assert!(!start::wanted(true, false, false, &none), "headless: pictures and scripts stay the same every time");
    assert!(!start::wanted(false, true, false, &none), "--demo");
    let off = |k: &str| (k == "FLEET_NATIVE_START_SERVER").then(|| "0".to_string());
    assert!(!start::wanted(false, false, false, &off), "FLEET_NATIVE_START_SERVER=0: the person starts Fleet themselves");
}

#[test]
fn the_splashs_slider_follows_the_apps_clock() {
    use crate::view::start_page::{SWEEP_SECS, slider_x};
    let bar = 160.0 * 0.4;
    assert_eq!(slider_x(0.0), -bar, "it starts wholly left of the bar (translateX(-100%))");
    assert!((slider_x(SWEEP_SECS / 2.0) - (-bar + bar * 1.75)).abs() < 0.01, "ease-in-out is at half way at half the time");
    assert!(slider_x(SWEEP_SECS * 0.999) > bar * 2.4, "it ends at 250% of its own width");
    assert_eq!(slider_x(SWEEP_SECS), slider_x(0.0), "and begins again");
    assert!(slider_x(0.1) < slider_x(0.2) && slider_x(0.2) < slider_x(0.4), "it moves as the clock does");
    // The sweep is drawn from `State::clock`, which the poll keeps moving only while the splash covers the window.
    let mut r = rig("clock", "serve", None, vec![]);
    let before = r.rt.state.clock;
    r.rt.frame(0.5);
    r.rt.frame(0.5);
    assert!(r.rt.state.clock > before, "the clock runs under the splash: {} then {}", before, r.rt.state.clock);
}

#[test]
fn the_port_is_kept_in_the_settings_file() {
    let mut a = crate::fleet::settings::Sidebar::default();
    a.save_desktop_port(6411);
    let b = crate::fleet::settings::Sidebar::parse(&a.to_json());
    assert_eq!(b.desktop_port(), Some(6411));
    assert_eq!(crate::fleet::settings::Sidebar::parse(r#"{"desktopPort": 99999}"#).desktop_port(), None);
}

fn options(scratch: &Scratch, mode: &str) -> Options {
    let mut options = Options::new(fake_binary(), scratch.0.join("content"), scratch.data(), scratch.log());
    options.preferred_ports = vec![];
    options.env.push(("FAKE_FLEET_MODE".into(), mode.into()));
    options
}
