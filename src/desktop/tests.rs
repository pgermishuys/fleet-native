//! Piece A's tests: Fleet's `desktop/test/desktop.test.ts` "paths", "server environment", "restarts" and "ports" under the
//! same names, then the process tests against `examples/fake_fleet.rs`, which stands in for the server on every system.
use super::log::Log;
use super::paths::{Inputs, Os, ServerBin, candidates, find_server, not_found_message, staged_update_note};
use super::server::{Event, FleetServer, Options, can_listen, pick_port, restart_delay, server_env};
use crate::test_wait;
use std::collections::HashSet;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn bin(os: Os, dir: &str) -> ServerBin {
    ServerBin { binary: PathBuf::from(format!("{dir}/{}", os.server_name())), content_root: PathBuf::from(dir) }
}

/// Inputs for a system that isn't the one running; `present` are the files that exist.
fn run<R>(os: Os, env: &[(&str, &str)], present: &[&str], more: impl FnOnce(&mut Inputs), then: impl FnOnce(&Inputs) -> R) -> R {
    let env: Vec<(String, String)> = env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let present: HashSet<PathBuf> = present.iter().map(PathBuf::from).collect();
    let get = move |key: &str| env.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    let exists = move |p: &Path| present.contains(p);
    let mut inputs = Inputs { get: &get, exists: &exists, os, home: Some(PathBuf::from("/home/me")), exe_dir: None, path_dirs: Vec::new() };
    more(&mut inputs);
    then(&inputs)
}

fn binaries(os: Os, env: &[(&str, &str)], more: impl FnOnce(&mut Inputs)) -> Vec<PathBuf> {
    run(os, env, &[], more, |i| candidates(i).into_iter().map(|c| c.binary).collect())
}

mod paths {
    use super::*;

    #[test]
    fn shares_the_weave_folder_with_cli_installs() {
        let get = |k: &str| (k == "HOME").then(|| "/home/me".to_string());
        let dir = crate::fleet::instance::data_dir_for(false, &get).unwrap();
        assert_eq!(dir, PathBuf::from("/home/me/.weave"));
        assert_eq!(dir.join("fleet.db"), PathBuf::from("/home/me/.weave/fleet.db"));
        assert_eq!(dir.join("fleet.instance.json"), PathBuf::from("/home/me/.weave/fleet.instance.json"));
    }

    #[test]
    fn uses_the_server_bundled_beside_the_app() {
        run(
            Os::Linux,
            &[],
            &["/opt/app/fleet/WeaveFleet.Api"],
            |i| i.exe_dir = Some("/opt/app".into()),
            |i| {
                assert_eq!(find_server(i), Ok(bin(Os::Linux, "/opt/app/fleet")));
            },
        );
    }

    #[test]
    fn takes_overrides_for_scratch_runs() {
        let env = [("FLEET_NATIVE_SERVER_BIN", "/scratch/bin/WeaveFleet.Api")];
        run(
            Os::Linux,
            &env,
            &["/scratch/bin/WeaveFleet.Api"],
            |_| {},
            |i| {
                assert_eq!(find_server(i), Ok(bin(Os::Linux, "/scratch/bin")));
            },
        );
        // Fleet's own names work too, with a content root of their own.
        let env = [("FLEET_DESKTOP_SERVER_BIN", "/s/b/WeaveFleet.Api"), ("FLEET_DESKTOP_SERVER_CONTENT_ROOT", "/s/ui")];
        run(
            Os::Linux,
            &env,
            &["/s/b/WeaveFleet.Api"],
            |_| {},
            |i| {
                let found = find_server(i).unwrap();
                assert_eq!((found.binary, found.content_root), (PathBuf::from("/s/b/WeaveFleet.Api"), PathBuf::from("/s/ui")));
            },
        );
        // An override stands alone: when it isn't there, the app doesn't start some other Fleet instead.
        let env = [("FLEET_NATIVE_SERVER_BIN", "/scratch/gone/WeaveFleet.Api")];
        run(
            Os::Linux,
            &env,
            &["/opt/Fleet/resources/fleet/WeaveFleet.Api"],
            |_| {},
            |i| {
                assert_eq!(find_server(i), Err(vec![PathBuf::from("/scratch/gone/WeaveFleet.Api")]));
            },
        );
    }

    #[test]
    fn names_the_windows_binary_with_exe() {
        let places = binaries(Os::Windows, &[("LOCALAPPDATA", "C:\\Users\\me\\AppData\\Local")], |i| i.exe_dir = Some("C:\\Fleet".into()));
        assert!(!places.is_empty() && places.iter().all(|p| p.to_string_lossy().ends_with("WeaveFleet.Api.exe")), "{places:?}");
        assert!(places.iter().any(|p| p.to_string_lossy().contains("Programs")), "Fleet's own app is looked in: {places:?}");
    }

    #[test]
    fn looks_in_the_places_in_order() {
        let places = binaries(Os::Linux, &[], |i| {
            i.exe_dir = Some("/opt/app".into());
            i.path_dirs = vec!["/usr/bin".into()];
        });
        let want =
            ["/opt/app/fleet/WeaveFleet.Api", "/home/me/.weave/fleet/app/WeaveFleet.Api", "/opt/Fleet/resources/fleet/WeaveFleet.Api"];
        assert_eq!(places, want.map(PathBuf::from));
        // `fleet` on PATH adds its sibling `app` last, and only when the launcher is there.
        let with_launcher = run(
            Os::Linux,
            &[],
            &["/srv/fleet/bin/fleet"],
            |i| i.path_dirs = vec!["/srv/fleet/bin".into()],
            |i| candidates(i).last().cloned(),
        );
        assert_eq!(with_launcher.unwrap().binary, PathBuf::from("/srv/fleet/bin/../app/WeaveFleet.Api"));
        let install = binaries(Os::Linux, &[("WEAVE_FLEET_INSTALL_DIR", "/data/fleet")], |_| {});
        assert_eq!(install[0], PathBuf::from("/data/fleet/app/WeaveFleet.Api"));
    }

    #[test]
    fn the_first_that_exists_wins_and_the_mac_places_are_the_bundle_and_applications() {
        let found = run(
            Os::Mac,
            &[],
            &["/Applications/Fleet.app/Contents/Resources/fleet/WeaveFleet.Api", "/home/me/.weave/fleet/app/WeaveFleet.Api"],
            |i| i.exe_dir = Some("/Apps/Native.app/Contents/MacOS".into()),
            |i| (find_server(i), candidates(i).len()),
        );
        assert_eq!(found.0.unwrap().binary, PathBuf::from("/home/me/.weave/fleet/app/WeaveFleet.Api"));
        let places = binaries(Os::Mac, &[], |i| i.exe_dir = Some("/Apps/Native.app/Contents/MacOS".into()));
        assert!(places.contains(&PathBuf::from("/Apps/Native.app/Contents/MacOS/../Resources/fleet/WeaveFleet.Api")));
        assert!(places.contains(&PathBuf::from("/home/me/Applications/Fleet.app/Contents/Resources/fleet/WeaveFleet.Api")));
    }

    #[test]
    fn says_where_it_looked_when_there_is_no_server() {
        let one = not_found_message(&[PathBuf::from("/x/WeaveFleet.Api")]);
        assert_eq!(one, "Fleet isn't where the app expects it: /x/WeaveFleet.Api");
        let many = not_found_message(&[PathBuf::from("/a/S"), PathBuf::from("/b/S")]);
        assert_eq!(many, "Fleet isn't installed here. The app looked for its server in: /a/S, /b/S");
        assert!(not_found_message(&[]).contains("no server"));
    }

    #[test]
    fn mentions_a_staged_update_the_launcher_would_apply() {
        // Built with `join`, so the separators are the system's own.
        let update = Path::new("/h/.weave/fleet").join("update");
        let note = staged_update_note(&Path::new("/h/.weave/fleet").join("app").join("WeaveFleet.Api"), &|p| p == update);
        assert!(note.is_some_and(|n| n.contains(&update.display().to_string())));
        assert_eq!(staged_update_note(&Path::new("/h/.weave/fleet").join("app").join("WeaveFleet.Api"), &|_| false), None);
    }
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn lookup<'a>(env: &'a [(String, String)], key: &str) -> Option<&'a str> {
    env.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

mod server_environment {
    use super::*;

    #[test]
    fn runs_fleet_in_desktop_mode_on_the_shared_data_directory() {
        let env = server_env(pairs(&[("PATH", "/usr/bin"), ("HOME", "/home/me")]), Path::new("/home/me/.weave"), 5123);
        let d = |name: &str| Path::new("/home/me/.weave").join(name).display().to_string();
        assert_eq!(lookup(&env, "PATH"), Some("/usr/bin"));
        assert_eq!(lookup(&env, "HOME"), Some("/home/me"));
        assert_eq!(lookup(&env, "ASPNETCORE_ENVIRONMENT"), Some("Production"));
        assert_eq!(lookup(&env, "Fleet__Host"), Some("127.0.0.1"));
        assert_eq!(lookup(&env, "Fleet__Port"), Some("5123"));
        assert_eq!(lookup(&env, "Fleet__DatabasePath"), Some(d("fleet.db").as_str()));
        assert_eq!(lookup(&env, "Fleet__AnalyticsDatabasePath"), Some(d("fleet-analytics.db").as_str()));
        assert_eq!(lookup(&env, "Fleet__DataProtection__KeyPath"), Some(d("fleet-keys").as_str()));
        assert_eq!(lookup(&env, "Fleet__Desktop__Enabled"), Some("true"));
    }

    #[test]
    fn drops_electrons_variables_and_inherited_listen_urls() {
        let base = pairs(&[
            ("ELECTRON_RUN_AS_NODE", "1"),
            ("electron_no_asar", "1"),
            ("URLS", "http://0.0.0.0:80"),
            ("ASPNETCORE_URLS", "x"),
            ("dotnet_urls", "y"),
            ("ASPNETCORE_ENVIRONMENT", "Development"),
            ("FLEET_NATIVE_TOKEN", "a-key-for-another-fleet"),
            ("Fleet__Port", "1"),
            ("KEEP", "yes"),
        ]);
        let env = server_env(base, Path::new("/d"), 5000);
        for gone in ["ELECTRON_RUN_AS_NODE", "electron_no_asar", "URLS", "ASPNETCORE_URLS", "dotnet_urls", "FLEET_NATIVE_TOKEN"] {
            assert_eq!(lookup(&env, gone), None, "{gone}");
        }
        assert_eq!(lookup(&env, "ASPNETCORE_ENVIRONMENT"), Some("Production"));
        assert_eq!(lookup(&env, "KEEP"), Some("yes"));
        assert_eq!(env.iter().filter(|(k, _)| k == "Fleet__Port").count(), 1, "ours replaces an inherited one");
        assert_eq!(lookup(&env, "Fleet__Port"), Some("5000"));
    }
}

mod restarts {
    use super::*;
    const NOW: i64 = 1_000_000;

    #[test]
    fn restarts_straight_away_after_the_first_crash_then_backs_off() {
        assert_eq!(restart_delay(&[NOW], NOW), Some(0));
        assert_eq!(restart_delay(&[NOW - 3000, NOW], NOW), Some(1000));
        assert_eq!(restart_delay(&[NOW - 6000, NOW - 3000, NOW], NOW), Some(2000));
        assert_eq!(restart_delay(&[NOW - 9000, NOW - 6000, NOW - 3000, NOW], NOW), Some(4000));
    }

    #[test]
    fn gives_up_after_five_crashes_in_a_minute() {
        assert_eq!(restart_delay(&[NOW - 40_000, NOW - 30_000, NOW - 20_000, NOW - 10_000, NOW], NOW), None);
    }

    #[test]
    fn forgets_crashes_older_than_a_minute() {
        assert_eq!(restart_delay(&[NOW - 300_000, NOW - 200_000, NOW - 100_000, NOW - 90_000, NOW], NOW), Some(0));
    }
}

/// Holds a port the way a listening Fleet would, reading what connects through `test_http` (a probe sends nothing).
fn hold(address: &str) -> (u16, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind(address).unwrap();
    let port = listener.local_addr().unwrap().port();
    let thread = std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let _ = crate::test_http::read(&mut stream);
        }
    });
    (port, thread)
}

mod ports {
    use super::*;

    #[test]
    fn keeps_the_preferred_port_when_its_free() {
        let free = pick_port(&[]).unwrap();
        assert_eq!(pick_port(&[free]).unwrap(), free);
        assert!(can_listen(free));
    }

    #[test]
    fn tries_the_remembered_port_then_the_next_then_any() {
        let (taken, _held) = hold("127.0.0.1:0");
        let free = pick_port(&[]).unwrap();
        assert_eq!(pick_port(&[taken, free]).unwrap(), free);
        let any = pick_port(&[taken]).unwrap();
        assert_ne!(any, taken);
        assert_ne!(any, 0);
    }

    #[test]
    fn picks_another_port_when_the_preferred_one_is_taken() {
        let (taken, _held) = hold("127.0.0.1:0");
        assert!(!can_listen(taken));
        assert_ne!(pick_port(&[taken]).unwrap(), taken);
    }

    #[test]
    fn picks_another_port_when_the_preferred_one_is_held_on_all_addresses() {
        // On Windows a 127.0.0.1 bind succeeds beside a 0.0.0.0 holder, so a bind probe alone said "free".
        let (taken, _held) = hold("0.0.0.0:0");
        assert_ne!(pick_port(&[taken]).unwrap(), taken);
    }
}

// ---- Processes: the server started for real, with the stand-in ----

/// The stand-in server, compiled once per test run with the `rustc` on the path (cargo builds no example for `cargo test`,
/// and a test binary can't be its own fake: libtest refuses `--urls`). Plain std, so it takes under a second.
fn fake_binary() -> PathBuf {
    static BUILT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    BUILT
        .get_or_init(|| {
            let dir = std::env::temp_dir().join(format!("fleet-native-fake-fleet-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let out = dir.join(if cfg!(windows) { "fake_fleet.exe" } else { "fake_fleet" });
            let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples").join("fake_fleet.rs");
            let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
            let status = std::process::Command::new(rustc).args(["--edition", "2024", "-O", "-o"]).arg(&out).arg(&source).status().unwrap();
            assert!(status.success(), "the stand-in server compiles");
            out
        })
        .clone()
}

/// A scratch folder that goes when the test does.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("fleet-native-desktop-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
    fn data(&self) -> PathBuf {
        let d = self.0.join("data");
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn log(&self) -> PathBuf {
        self.0.join("logs").join("fleet-server.log")
    }
    fn log_text(&self) -> String {
        std::fs::read_to_string(self.log()).unwrap_or_default()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn options(scratch: &Scratch, mode: &str) -> Options {
    let mut options = Options::new(fake_binary(), scratch.0.join("content"), scratch.data(), scratch.log());
    options.preferred_ports = vec![];
    options.env.push(("FAKE_FLEET_MODE".into(), mode.into()));
    options.env.push(("ELECTRON_RUN_AS_NODE".into(), "1".into()));
    options.env.push(("FLEET_NATIVE_TOKEN".into(), "a-key-for-another-fleet".into()));
    options
}

fn next(server: &FleetServer, what: &str) -> Event {
    test_wait::recv(what, server.events())
}

/// The log's text once `needle` is in it (the pipes are read on their own threads).
fn log_has(scratch: &Scratch, needle: &str) {
    test_wait::until_true(&format!("{needle:?} in the log"), || scratch.log_text().contains(needle));
}

mod processes {
    use super::*;

    #[test]
    fn starts_the_server_in_desktop_mode_and_says_when_its_ready() {
        let scratch = Scratch::new("ready");
        let server = FleetServer::start(options(&scratch, "serve")).unwrap();
        let Event::Ready { url, port } = next(&server, "ready") else { panic!("not ready") };
        assert_eq!(url, server.url());
        assert_eq!(port, server.port());
        assert!(crate::fleet::instance::is_ready(&url, Duration::from_secs(5)));
        assert!(server.pid().is_some());
        // What the server was given.
        log_has(&scratch, "cwd=");
        let log = scratch.log_text();
        assert!(log.contains(&format!("args=--urls {url} --contentRoot")), "{log}");
        assert!(log.contains("env Fleet__Host=127.0.0.1"), "{log}");
        assert!(log.contains(&format!("env Fleet__Port={port}")), "{log}");
        assert!(log.contains("env Fleet__Desktop__Enabled=true"), "{log}");
        assert!(log.contains("env ELECTRON_RUN_AS_NODE=(unset)"), "{log}");
        assert!(log.contains("env FLEET_NATIVE_TOKEN=(unset)"), "{log}");
        assert!(scratch.data().join("fleet-keys").is_dir(), "the key folder is made first");
        server.stop(Duration::from_secs(20));
    }

    #[test]
    fn stopping_closes_its_input_and_returns_quickly() {
        let scratch = Scratch::new("stop");
        let server = FleetServer::start(options(&scratch, "serve")).unwrap();
        assert!(matches!(next(&server, "ready"), Event::Ready { .. }));
        let started = std::time::Instant::now();
        server.stop(Duration::from_secs(20));
        assert!(started.elapsed() < Duration::from_secs(15), "closing stdin stopped it, no kill: {:?}", started.elapsed());
        assert!(server.pid().is_none());
        assert!(!crate::fleet::instance::is_ready(server.url(), Duration::from_millis(500)));
        log_has(&scratch, "── Fleet exited (code 0, signal none)");
        // Nothing restarts it afterwards.
        assert!(server.events().recv_timeout(Duration::from_millis(600)).is_err());
    }

    #[test]
    fn a_server_that_ignores_its_closed_input_is_asked_then_killed() {
        let scratch = Scratch::new("deaf");
        let server = FleetServer::start(options(&scratch, "deaf")).unwrap();
        assert!(matches!(next(&server, "ready"), Event::Ready { .. }));
        server.stop(Duration::from_millis(300));
        assert!(server.pid().is_none());
        assert!(!crate::fleet::instance::is_ready(server.url(), Duration::from_millis(500)));
        log_has(&scratch, "── Fleet exited");
    }

    #[test]
    fn exit_75_means_another_fleet_took_the_data() {
        let scratch = Scratch::new("taken");
        let server = FleetServer::start(options(&scratch, "taken")).unwrap();
        assert_eq!(next(&server, "taken"), Event::Taken);
        log_has(&scratch, "── Fleet exited (code 75, signal none)");
        assert!(server.events().recv_timeout(Duration::from_millis(600)).is_err(), "no restart after Taken");
    }

    #[test]
    fn a_crash_restarts_it_and_ready_fires_again() {
        let scratch = Scratch::new("once");
        let server = FleetServer::start(options(&scratch, "crash-once")).unwrap();
        assert_eq!(next(&server, "restarting"), Event::Restarting { delay_ms: 0, exit_code: Some(1) });
        assert!(matches!(next(&server, "ready after the restart"), Event::Ready { .. }));
        let log = scratch.log_text();
        assert_eq!(log.matches(" starting ").count(), 2, "{log}");
        assert!(log.contains("── Fleet exited (code 1, signal none)"), "{log}");
        server.stop(Duration::from_secs(20));
    }

    #[test]
    fn five_crashes_in_a_minute_give_up() {
        let scratch = Scratch::new("crash");
        let server = FleetServer::start(options(&scratch, "crash")).unwrap();
        // 0 ms, 1 s, 2 s, 4 s, then no more.
        let mut delays = Vec::new();
        let failed = loop {
            match next(&server, "restarting or failed") {
                Event::Restarting { delay_ms, exit_code } => {
                    assert_eq!(exit_code, Some(1));
                    delays.push(delay_ms);
                }
                other => break other,
            }
        };
        assert_eq!(delays, vec![0, 1000, 2000, 4000]);
        assert_eq!(failed, Event::Failed("Fleet keeps stopping unexpectedly.".into()));
        assert_eq!(scratch.log_text().matches(" starting ").count(), 5);
    }

    #[test]
    fn a_server_that_never_answers_fails() {
        let scratch = Scratch::new("silent");
        let mut o = options(&scratch, "silent");
        o.ready_timeout = Duration::from_millis(800);
        let server = FleetServer::start(o).unwrap();
        assert_eq!(next(&server, "never answered"), Event::Failed("Fleet started but never answered.".into()));
        // It still runs, and stop ends it.
        assert!(server.pid().is_some());
        server.stop(Duration::from_secs(20));
        assert!(server.pid().is_none());
    }

    #[test]
    fn a_missing_binary_fails_with_where_it_looked() {
        let scratch = Scratch::new("missing");
        let mut o = options(&scratch, "serve");
        o.binary = scratch.0.join("nowhere").join("WeaveFleet.Api");
        let want = format!("Fleet isn't where the app expects it: {}", o.binary.display());
        let server = FleetServer::start(o).unwrap();
        assert_eq!(next(&server, "failed"), Event::Failed(want));
    }

    #[test]
    fn a_binary_that_cant_run_fails_and_the_log_says_so() {
        let scratch = Scratch::new("cantrun");
        let mut o = options(&scratch, "serve");
        // A file that exists and isn't a program.
        o.binary = scratch.0.join("WeaveFleet.Api");
        std::fs::write(&o.binary, "not a program").unwrap();
        let server = FleetServer::start(o).unwrap();
        let Event::Failed(message) = next(&server, "failed") else { panic!("not failed") };
        assert!(message.starts_with("Couldn't start Fleet: "), "{message}");
        log_has(&scratch, "── could not start Fleet: ");
    }

    #[test]
    fn the_log_has_its_header_the_servers_output_and_the_exit_line() {
        let scratch = Scratch::new("log");
        let server = FleetServer::start(options(&scratch, "taken")).unwrap();
        assert_eq!(next(&server, "taken"), Event::Taken);
        log_has(&scratch, "── Fleet exited");
        let log = scratch.log_text();
        let mut lines = log.lines().filter(|l| !l.is_empty());
        let header = lines.next().unwrap();
        assert!(
            header.starts_with("── 20") && header.contains("Z starting ") && header.ends_with(&format!(" on {}", server.url())),
            "{header}"
        );
        assert!(log.contains("fake fleet taken args=--urls"), "{log}");
        assert!(log.contains("fake fleet stderr line"), "stderr is logged too: {log}");
        assert!(log.trim_end().ends_with("── Fleet exited (code 75, signal none)"), "{log}");
    }

    #[test]
    fn the_log_is_appended_to_and_rolled_past_its_limit() {
        let scratch = Scratch::new("rotate");
        let file = scratch.log();
        let first = Log::open(&file, 100).unwrap();
        first.write("first start\n");
        drop(first);
        Log::open(&file, 100).unwrap().write("second start\n");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "first start\nsecond start\n");
        // Past the limit: the old one is kept as `.1`, and the new one starts empty.
        std::fs::write(&file, "x".repeat(101)).unwrap();
        Log::open(&file, 100).unwrap().write("fresh\n");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "fresh\n");
        let old = PathBuf::from(format!("{}.1", file.display()));
        assert_eq!(std::fs::read_to_string(&old).unwrap().len(), 101);
        // And once more: `.1` is replaced, not piled up.
        std::fs::write(&file, "y".repeat(101)).unwrap();
        Log::open(&file, 100).unwrap().write("again\n");
        assert_eq!(std::fs::read_to_string(&old).unwrap(), "y".repeat(101));
    }

    #[test]
    fn dropping_the_handle_closes_the_servers_input() {
        let scratch = Scratch::new("drop");
        let server = FleetServer::start(options(&scratch, "serve")).unwrap();
        assert!(matches!(next(&server, "ready"), Event::Ready { .. }));
        let url = server.url().to_string();
        drop(server);
        test_wait::until_true("the server to go once its handle is dropped", || {
            !crate::fleet::instance::is_ready(&url, Duration::from_millis(300))
        });
    }
}

/// Against a released server, by hand: `FLEET_NATIVE_LIVE_SERVER_DIR=<unpacked server package> FLEET_NATIVE_LIVE_SCRATCH=<an
/// empty folder> cargo test live_server -- --ignored --nocapture`, with `HOME` set to a scratch folder too. Never a Fleet
/// someone uses: its data folder is the scratch one, its port its own.
#[test]
#[ignore = "needs a released server package"]
fn live_server_starts_answers_and_stops_with_the_input() {
    let (Ok(package), Ok(work)) = (std::env::var("FLEET_NATIVE_LIVE_SERVER_DIR"), std::env::var("FLEET_NATIVE_LIVE_SCRATCH")) else {
        panic!("set FLEET_NATIVE_LIVE_SERVER_DIR and FLEET_NATIVE_LIVE_SCRATCH");
    };
    let (package, work) = (PathBuf::from(package), PathBuf::from(work));
    let data = work.join("data");
    let mut o =
        Options::new(package.join(Os::current().server_name()), package.clone(), data.clone(), work.join("logs").join("fleet-server.log"));
    o.preferred_ports = vec![];
    o.env.push(("FLEET_DESKTOP_DATA_DIR".into(), data.display().to_string()));
    let server = FleetServer::start(o).unwrap();
    let Event::Ready { url, .. } = next(&server, "the released server to answer /readyz") else { panic!("not ready") };
    println!("ready at {url}, pid {:?}", server.pid());
    let instance = crate::fleet::instance::read_instance(&data.join(crate::fleet::instance::FILE)).expect("the instance file");
    assert!(instance.desktop, "started in desktop mode");
    println!("{}", crate::fleet::http::get(&url, "/api/desktop/status").unwrap());
    server.stop(Duration::from_secs(20));
    assert!(!data.join(crate::fleet::instance::FILE).exists(), "the server removed its instance file");
    assert!(!crate::fleet::instance::is_ready(&url, Duration::from_millis(500)));
}
