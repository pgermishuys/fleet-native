//! Component 16, piece V: Settings → Updates & about, the Fleet group (with the app's own row), the Harnesses group and the
//! menu's dot and badge. Fleet's `DesktopAppUpdates.test.ts` (6) and `HarnessUpdatesGroup.test.ts` (2) under their names, as
//! native cases; then the rows the plan adds. A fake Fleet answers every call; nothing here updates a real Fleet or app.
use crate::fleet::link::{Job, Link, Post};
use crate::tests::click;
use crate::update::manifest::Asset;
use crate::update::{Available, Phase};
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fake {
    heard: Vec<(String, String, String)>,
    prefs: serde_json::Map<String, Value>,
    status: String,
    harnesses: String,
}

type Shared = Arc<Mutex<Fake>>;

fn serve(fake: Shared) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let fake = fake.clone();
            std::thread::spawn(move || answer(stream, fake));
        }
    });
    base
}

fn answer(mut stream: std::net::TcpStream, fake: Shared) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let (head, mut body) = loop {
        let n = stream.read(&mut chunk).unwrap_or(0);
        buf.extend_from_slice(&chunk[..n]);
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break (String::from_utf8_lossy(&buf[..at]).to_string(), buf[at + 4..].to_vec());
        }
        if n == 0 {
            return;
        }
    };
    let wanted: usize =
        head.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse().ok())).unwrap_or(0);
    while body.len() < wanted {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    let mut first = head.lines().next().unwrap_or("").split(' ');
    let (method, path) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
    let text = String::from_utf8_lossy(&body).to_string();
    let mut f = fake.lock().unwrap();
    // The live-events link negotiates in the background at any moment; it is not what a test asks about.
    if !path.starts_with("/hubs/") {
        f.heard.push((method.clone(), path.clone(), text));
    }
    let reply = if method == "GET" && path == "/api/preferences" {
        Value::Object(f.prefs.clone()).to_string()
    } else if method == "PUT" && path.starts_with("/api/preferences/") {
        let key = path.trim_start_matches("/api/preferences/").replace("%2E", ".");
        let value = serde_json::from_slice::<Value>(&body).ok().and_then(|v| v["value"].as_str().map(str::to_string)).unwrap_or_default();
        f.prefs.insert(key, json!(value));
        "{}".to_string()
    } else if method == "GET" && path.starts_with("/api/harnesses") {
        f.harnesses.clone()
    } else if method == "GET" && path == "/api/update/status" {
        f.status.clone()
    } else if path == "/api/update/releases" {
        json!({ "releases": [], "fetchedAt": null, "error": null }).to_string()
    } else if path == "/api/update/rollout" {
        return drop(stream.write_all(b"HTTP/1.1 404 X\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"));
    } else {
        "{}".to_string()
    };
    let out =
        format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}", reply.len());
    let _ = stream.write_all(out.as_bytes());
}

fn status(kind: &str) -> String {
    json!({ "currentVersion": "0.25.0", "status": kind, "latestVersion": null, "checkedAt": null, "error": null,
            "downloadBytesReceived": null, "downloadBytesTotal": null })
    .to_string()
}

fn harness_list() -> String {
    json!([
        { "type": "opencode", "displayName": "OpenCode", "available": true, "userEnabled": true, "version": "1.18.20",
          "update": { "updateAvailable": true, "latestVersion": "1.19.0", "command": "npm i -g opencode", "canInstall": true } },
        { "type": "pi", "displayName": "Pi", "available": false, "userEnabled": true, "version": null,
          "update": { "updateAvailable": true, "latestVersion": "0.9.0", "command": "npm i -g pi", "canInstall": true } },
        { "type": "claude-code", "displayName": "Claude Code", "available": true, "userEnabled": false, "version": "2.1.0",
          "update": { "updateAvailable": true, "latestVersion": "2.2.0", "command": "npm i -g claude", "canInstall": true } },
    ])
    .to_string()
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Shared,
}

/// A connected app on Updates & about, the Fleet's status and harnesses read.
fn rig(status_kind: &str) -> Rig {
    let fake: Shared = Arc::new(Mutex::new(Fake { status: status(status_kind), harnesses: harness_list(), ..Default::default() }));
    let base = serve(fake.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(base, Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = crate::tests::app();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.rail = Rail::Settings;
        s.open_settings("updates", None);
    });
    let mut r = Rig { rt, queue, fake };
    r.wait("the status and the harnesses", |s| {
        s.updates.status.get("home").is_some_and(|x| x.status.is_some()) && !s.harness_list().all.is_empty() && s.harness_prefs().fetched
    });
    r.frames();
    // The wait above is on state; the rows are drawn from it by frames, and on a loaded runner the layout can lag the state
    // by more than two. Draw until the first harness's row is there, so a test reads a page that has been drawn.
    let first = r.rt.state.harness_list().all.first().map(|h| format!("settings-row-harness-version-{}", h.kind)).unwrap_or_default();
    test_wait::until(
        &mut r,
        "the harness rows to be drawn",
        |r| {
            r.frames();
            r.info(&first).is_some()
        },
        |_| format!("no element keyed {first:?}"),
    );
    r
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("{:?}", s.updates));
    }
    fn frames(&mut self) {
        self.rt.frame(1.0 / 60.0);
        self.rt.frame(1.0 / 60.0);
    }
    fn info(&self, key: &str) -> Option<rune::ElementInfo> {
        self.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }
    fn text(&self, key: &str) -> String {
        self.info(key).unwrap_or_else(|| panic!("no element keyed {key:?}")).text.unwrap_or_default()
    }
    fn words(&self) -> String {
        self.rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
    }
    fn heard(&self, method: &str, path: &str) -> usize {
        self.fake.lock().unwrap().heard.iter().filter(|(m, p, _)| m == method && p == path).count()
    }
    fn set_app(&mut self, f: impl FnOnce(&mut crate::update::Update) + 'static) {
        self.rt.update(move |s, _| f(&mut s.update));
        self.frames();
    }
}

fn offer(build: u64) -> Available {
    Available {
        build,
        date: "15 November 2026".into(),
        asset: Asset { name: "app.tar.gz".into(), size: 31_400_000, sha256: "0".repeat(64) },
    }
}

// ─── DesktopAppUpdates.test.ts (SystemSection), as native cases ─────────────────

#[test]
fn shows_the_app_s_updates_instead_of_the_server_s_when_the_app_runs_fleet() {
    let mut r = rig("managed");
    r.set_app(|u| (u.running, u.checked_ms) = (Some(210), Some(1)));
    assert!(r.info("app-update-row").is_some());
    assert_eq!(r.text("app-update-status"), "Up to date");
    assert!(!r.words().contains("Updates come from the Fleet app"), "a Fleet the app started is updated with it: no server row");
    assert!(r.info("fleet-update-row").is_none());
}

#[test]
fn offers_to_update_and_restart_into_a_newer_build_through_the_dialog() {
    let mut r = rig("managed");
    r.set_app(|u| {
        u.running = Some(210);
        u.available = Some(offer(214));
    });
    assert_eq!(r.text("app-update-status"), "Build 214 is available");
    click(&mut r.rt, "app-update-go");
    assert!(r.rt.state.update.dialog, "Update and restart opens the dialog; the row doesn't install by itself");
    assert_eq!(r.rt.state.update.phase, Phase::Idle);
}

#[test]
fn follows_the_app_s_update_as_it_downloads() {
    let mut r = rig("managed");
    r.set_app(|u| {
        u.running = Some(210);
        u.available = Some(offer(214));
        u.phase = Phase::Downloading(12_400_000, 31_400_000);
    });
    assert_eq!(r.text("app-update-status"), "Downloading build 214\u{2026}");
    assert!(r.info("app-update-bar").is_some());
    assert!(r.info("app-update-go").is_none() && r.info("app-update-check").is_none(), "nothing to press while it downloads");
}

#[test]
fn says_a_build_that_cannot_update_itself_and_offers_no_check() {
    let mut r = rig("managed");
    r.set_app(|u| u.running = None);
    assert_eq!(r.text("app-update-status"), "This build doesn't update itself");
    assert_eq!(r.text("app-update-build"), "dev build");
    assert!(r.info("app-update-check").is_none());
}

#[test]
fn keeps_the_server_s_row_when_the_app_is_connected_to_a_fleet_it_didn_t_start() {
    let mut r = rig("uptodate");
    r.set_app(|u| (u.running, u.checked_ms) = (Some(210), Some(1)));
    assert!(r.info("app-update-row").is_some());
    assert_eq!(r.text("fleet-update-status"), "Up to date");
    let checks = r.rt.elements().iter().filter(|e| e.text.as_deref() == Some("Check for updates") && e.key.is_none()).count();
    assert!(checks >= 2 || r.info("app-update-check").is_some() && r.info("fleet-update-check").is_some(), "two Check for updates buttons");
}

#[test]
fn check_for_updates_wakes_the_checker_and_waits_for_its_answer() {
    let mut r = rig("managed");
    let (wake, woken) = channel::<()>();
    r.set_app(move |u| {
        u.running = Some(210);
        u.checked_ms = Some(1);
        u.wake = Some(wake);
    });
    click(&mut r.rt, "app-update-check");
    assert!(woken.try_recv().is_ok(), "the checker was woken");
    assert!(r.rt.state.update.checking);
    assert_eq!(r.text("app-update-status"), "Checking for updates\u{2026}");
    assert!(r.info("app-update-check").unwrap().opacity < 1.0, "one look at a time");
    // The checker's answer: a newer build for this platform becomes the offer.
    r.rt.update(|s, _| s.update_checked(5_000, Err("The release isn't reachable.".into())));
    r.frames();
    assert_eq!(r.text("app-update-status"), "Update check failed");
    assert!(r.words().contains("The release isn't reachable."));
}

// ─── HarnessUpdatesGroup.test.ts ────────────────────────────────────────────────

#[test]
fn lists_each_harness_with_its_version_and_the_update_only_for_one_that_is_turned_on() {
    let r = rig("uptodate");
    assert!(r.text("settings-row-harness-version-opencode").contains("1.18.20") || r.words().contains("1.18.20"));
    assert!(r.words().contains("Not installed"), "Pi has no version and isn't available");
    let strips: Vec<String> =
        r.rt.elements()
            .iter()
            .filter_map(|e| e.key.clone())
            .filter(|k| k.starts_with("harness-update-") && k.ends_with("-opencode"))
            .collect();
    assert!(!strips.is_empty(), "OpenCode is on: its strip shows");
    assert!(r.rt.elements().iter().filter_map(|e| e.key.clone()).any(|k| k.starts_with("harness-update-") && k.ends_with("-pi")));
    assert!(
        !r.rt.elements().iter().filter_map(|e| e.key.clone()).any(|k| k.starts_with("harness-update-") && k.ends_with("-claude-code")),
        "Claude Code is turned off: no strip"
    );
}

#[test]
fn keeps_the_check_for_updates_switch_and_check_again() {
    let mut r = rig("uptodate");
    assert_eq!(r.info("harnesses-update-checks").unwrap().label.as_deref(), Some("Check for updates"));
    click(&mut r.rt, "harnesses-update-checks");
    let fake = r.fake.clone();
    r.wait("the switch to reach Fleet", move |_| fake.lock().unwrap().prefs.get("harnessUpdates.check") == Some(&json!("false")));
    let before = r.heard("GET", "/api/harnesses?fresh=true");
    // Turning it off asks the harnesses again; so does Check again.
    let fake = r.fake.clone();
    r.wait("the harnesses to be asked again", move |_| {
        fake.lock().unwrap().heard.iter().filter(|(_, p, _)| p == "/api/harnesses?fresh=true").count() > before
    });
    r.wait("that look to end", |s| !s.harnesses_checking());
    let mid = r.heard("GET", "/api/harnesses?fresh=true");
    r.frames();
    click(&mut r.rt, "harnesses-check-again");
    let fake = r.fake.clone();
    r.wait("Check again to reach Fleet", move |_| {
        fake.lock().unwrap().heard.iter().filter(|(_, p, _)| p == "/api/harnesses?fresh=true").count() > mid
    });
}

// ─── The rows the plan adds ─────────────────────────────────────────────────────

#[test]
fn installed_version_line_says_update_waiting_and_links_to_machines() {
    let mut r = rig("uptodate");
    assert_eq!(r.text("fleet-version-text"), "v0.25.0", "nothing waits");
    assert!(r.info("system-see-machines").is_none());
    r.fake.lock().unwrap().status = json!({ "currentVersion": "0.25.0", "status": "available", "latestVersion": "0.26.0" }).to_string();
    r.rt.update(|s, _| s.load_update_status("home", 1.0e9));
    r.wait("the new status", |s| s.updates.status["home"].status.as_ref().is_some_and(|x| x.is_available()));
    r.frames();
    assert!(r.words().contains("v0.25.0, update waiting"));
    assert_eq!(r.text("fleet-update-status"), "Update available: v0.26.0");
    click(&mut r.rt, "system-see-machines");
    assert_eq!(r.rt.state.settings.anchor.as_deref(), Some("machines"));
}

#[test]
fn download_shows_the_received_and_total_bytes() {
    let mut r = rig("uptodate");
    r.fake.lock().unwrap().status = json!({ "currentVersion": "0.25.0", "status": "downloading", "latestVersion": "0.26.0",
        "downloadBytesReceived": 5_242_880, "downloadBytesTotal": 20_971_520 })
    .to_string();
    r.rt.update(|s, _| s.load_update_status("home", 1.0e9));
    r.wait("the download", |s| s.updates.status["home"].status.as_ref().is_some_and(|x| x.download_bytes_total.is_some()));
    r.frames();
    assert_eq!(r.text("fleet-update-status"), "Downloading v0.26.0\u{2026}");
    assert_eq!(r.text("fleet-update-progress"), "5.0 MB / 20 MB");
    assert!(r.info("fleet-update-bar").is_some());
}

#[test]
fn the_staged_note_says_it_installs_the_next_time_fleet_starts() {
    let mut r = rig("uptodate");
    r.fake.lock().unwrap().status = json!({ "currentVersion": "0.25.0", "status": "staged", "latestVersion": "0.26.0" }).to_string();
    r.rt.update(|s, _| s.load_update_status("home", 1.0e9));
    r.wait("the staged status", |s| s.updates.status["home"].status.as_ref().is_some_and(|x| x.is_staged()));
    r.frames();
    assert_eq!(r.text("fleet-update-status"), "v0.26.0 is ready");
    assert!(r.info("fleet-update-check").is_none(), "no check once it is ready");
    assert!(r.words().contains("It's downloaded and installs the next time you start Fleet ("));
}

#[test]
fn the_menu_marks_an_update_waiting_with_a_dot_the_idle_colour_once_it_is_ready() {
    let mut r = rig("uptodate");
    assert!(r.info("updates-dot").is_none());
    r.fake.lock().unwrap().status = json!({ "currentVersion": "0.25.0", "status": "available", "latestVersion": "0.26.0" }).to_string();
    r.rt.update(|s, _| s.load_update_status("home", 1.0e9));
    r.wait("the new status", |s| s.updates.status["home"].status.as_ref().is_some_and(|x| x.is_available()));
    r.frames();
    assert_eq!(r.info("updates-dot").unwrap().label.as_deref(), Some("Update available"));
    assert!(r.info("updates-badge").is_none(), "no other machine waits");
}

#[test]
fn another_machines_status_is_asked_when_settings_shows_it_and_the_menu_dot_follows_it() {
    // The calls themselves are tested against two fake Fleets in `tests_updates`; here the dot reads the shown machine's.
    let mut r = rig("uptodate");
    r.rt.update(|s, _| {
        let mut other = crate::fleet::updates::feed::StatusStore::default();
        other.status = Some(crate::fleet::updates::UpdateStatus {
            current_version: "0.20.0".into(),
            status: crate::fleet::updates::Status::Staged,
            latest_version: Some("0.21.0".into()),
            checked_at: None,
            error: None,
            download_bytes_received: None,
            download_bytes_total: None,
        });
        s.updates.status.insert("m-harbor".into(), other);
    });
    r.frames();
    assert!(r.info("updates-dot").is_none(), "home is up to date");
    assert_eq!(
        crate::fleet::settings::updates_page::menu_dot(r.rt.state.updates.status["m-harbor"].status.as_ref()),
        Some(crate::fleet::settings::updates_page::Dot::Idle)
    );
}
