//! Component 16, piece J: the Install panel, GitHub Copilot's path field and the update strip. `HarnessesSection.test.ts`'s cases
//! marked "D", the eight `HarnessUpdateStrip.test.ts` cases and the 2-second poll, under their names, against a fake Fleet that
//! answers the harness list and the preferences and records every update request. Nothing here installs or updates a real
//! harness: the fake answers for every one.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::harness_update::{Piece, Tone, strip, updating};
use crate::tests::{FRAME, app, click};
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fake {
    harnesses: Value,
    prefs: HashMap<String, String>,
    puts: Vec<(String, String)>,
    /// `(method, path)` of every update request, and what to answer them.
    updates: Vec<(String, String)>,
    update_answer: Option<(u16, Value)>,
    fresh: usize,
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
    let request = crate::tests::read_request(&mut stream);
    let mut first = request.lines().next().unwrap_or("").split(' ');
    let (method, path) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    let body = request.split("\r\n\r\n").nth(1).unwrap_or("");
    let mut f = fake.lock().unwrap();
    let (status, reply) = if method == "GET" && path.starts_with("/api/harnesses") {
        if path.contains("fresh=true") {
            f.fresh += 1;
        }
        (200, f.harnesses.clone())
    } else if method == "GET" && path == "/api/preferences" {
        (200, json!(f.prefs))
    } else if method == "GET" && path == "/api/agent-browser/settings" {
        (200, json!({"enabled": true}))
    } else if method == "PUT" && path.starts_with("/api/preferences/") {
        let key = path.trim_start_matches("/api/preferences/").replace("%2E", ".");
        let value = serde_json::from_str::<Value>(body).ok().and_then(|v| v["value"].as_str().map(str::to_string)).unwrap_or_default();
        f.puts.push((key.clone(), value.clone()));
        f.prefs.insert(key, value);
        (200, json!({}))
    } else if path.starts_with("/api/harnesses/") && path.ends_with("/update") {
        f.updates.push((method.to_string(), path.to_string()));
        f.update_answer.clone().unwrap_or((202, json!({})))
    } else {
        (200, json!({}))
    };
    let reply = reply.to_string();
    let out = format!(
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(out.as_bytes());
}

fn harness(kind: &str, name: &str, state: &str, extra: Value) -> Value {
    let mut h = json!({
        "type": kind, "displayName": name, "available": state == "ready", "userEnabled": true, "state": state,
        "capabilities": {}, "presentation": {"eyebrow": "CLI harness", "description": "A harness."}, "settings": [],
    });
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        h[k] = v;
    }
    h
}

const SEPARATE: &str = "curl -fsSL https://opencode.ai/v2/install | HOME=/home/you/.weave/harnesses/opencode2 bash -s -- --no-modify-path";
const COMMAND: &str = "/home/you/.opencode/bin/opencode upgrade 1.18.31";

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Shared,
}

fn rig(list: Value) -> Rig {
    let fake: Shared = Arc::new(Mutex::new(Fake { harnesses: list.clone(), ..Default::default() }));
    let base = serve(fake.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(
        base,
        Post::new(move |job| {
            let _ = jobs.lock().unwrap().send(job);
        }),
    );
    let mut rt = app();
    let first = list[0]["type"].as_str().unwrap_or("").to_string();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.pickers.prefs = Default::default();
        s.rail = Rail::Settings;
        s.settings.set("harnesses", None);
        s.select_harness_tab(&first);
    });
    let mut r = Rig { rt, queue, fake };
    r.rt.frame(FRAME);
    let n = list.as_array().map_or(0, Vec::len);
    r.wait("the harnesses and the preferences", move |s| {
        s.harness_list().all.len() == n && s.harness_prefs().fetched && !s.harnesses_checking()
    });
    r
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("{:?}", s.settings.harnesses));
        self.frames();
    }
    fn frames(&mut self) {
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }
    fn info(&self, key: &str) -> Option<rune::ElementInfo> {
        self.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }
    fn words(&self) -> String {
        self.rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
    }
    fn text_of(&self, key: &str) -> String {
        self.info(key).and_then(|e| e.text).unwrap_or_default()
    }
    fn click(&mut self, key: &str) {
        click(&mut self.rt, key);
        self.frames();
    }
    /// The setup wizard's rows are where the strip shows now: open them on the list the page read.
    fn open_rows(&mut self) {
        self.rt.update(|s, _| s.open_harness_setup());
        self.frames();
    }
    fn updates(&self) -> Vec<(String, String)> {
        self.fake.lock().unwrap().updates.clone()
    }
    fn first(&self) -> crate::fleet::new_session::harness_choice::HarnessEntry {
        self.rt.state.harness_list().all[0].clone()
    }
}

fn opencode2(state: &str, setup: Value, extra: Value) -> Value {
    let mut h = harness("opencode2", "OpenCode 2", state, json!({"setup": setup}));
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        h[k] = v;
    }
    h
}

// ─── HarnessesSection.test.ts: the Install panel ──────────────────────────────────

#[test]
fn shows_how_opencode_2_is_installed_in_place_of_no_settings_yet() {
    let setup = json!({
        "installCommand": SEPARATE,
        "signInCommand": "OPENCODE_CONFIG_DIR=/c OPENCODE_DB=/d opencode2 auth login",
        "mode": "In its own folder",
        "folders": [{"label": "Settings", "path": "/home/you/.weave/harnesses/opencode2/config"},
                    {"label": "Sessions", "path": "/home/you/.weave/harnesses/opencode2/data/opencode.db"}],
        "notes": ["OpenCode 2 keeps its own provider sign-ins here."],
    });
    let mut r = rig(json!([
        opencode2("ready", setup, json!({"version": "2.0.9", "executablePath": "/home/you/.weave/harnesses/opencode2/bin/opencode2"})),
        harness("pi", "Pi", "ready", json!({})),
    ]));
    assert_eq!(r.text_of("harness-install-mode"), "In its own folder");
    assert_eq!(r.text_of("harness-install-version"), "2.0.9");
    let words = r.words();
    assert!(words.contains("Sessions") && words.contains("/home/you/.weave/harnesses/opencode2/data/opencode.db"));
    assert!(words.contains("OpenCode 2 keeps its own provider sign-ins here."));
    assert!(r.text_of("harness-install-sign-in").contains("OPENCODE_CONFIG_DIR=/c OPENCODE_DB=/d"));
    // Pi says nothing about its install, so it keeps the placeholder.
    assert!(!words.contains("No settings yet"));
    r.click("harness-tab-pi");
    assert!(r.words().contains("No settings yet"));
    assert!(r.info("harness-install").is_none());
}

#[test]
fn offers_no_sign_in_before_opencode_2_is_installed_and_shows_its_installer() {
    let setup = json!({"installCommand": SEPARATE, "signInCommand": "opencode2 auth login", "mode": "In its own folder"});
    let mut r = rig(json!([opencode2("not-installed", setup, json!({}))]));
    assert!(r.words().contains("Not installed yet"));
    assert_eq!(r.text_of("harness-install-command"), SEPARATE);
    assert!(r.info("harness-install-sign-in").is_none());
    r.click("harness-install-set-up");
    assert!(r.rt.state.harness_setup.is_open, "Install here opens the setup at the harness step");
}

#[test]
fn shows_both_places_opencode_2_can_go_with_each_one_s_folders_and_installer() {
    let place = |id: &str, label: &str, text: &str, command: &str, recommended: bool, dir: &str| {
        json!({"id": id, "label": label, "description": text, "command": command, "recommended": recommended,
               "folders": [{"label": "Program", "path": dir}]})
    };
    let setup = json!({
        "installCommand": "separate-installer", "mode": "In its own folder",
        "folders": [{"label": "Program", "path": "/home/you/.weave/harnesses/opencode2/.opencode/bin"}],
        "installChoices": [
            place("separate", "In its own folder", "OpenCode 1 can be installed next to it at any time.", "separate-installer", true,
                  "/home/you/.weave/harnesses/opencode2/.opencode/bin"),
            place("default", "As your main opencode", "Installing OpenCode 1 later replaces OpenCode 2.", "default-installer", false,
                  "/home/you/.opencode/bin"),
        ],
    });
    let r = rig(json!([opencode2("not-installed", setup, json!({}))]));
    assert!(r.info("harness-install-mode").is_none(), "no mode chip when two places are offered");
    assert!(r.info("harness-install-command").is_none());
    assert!(r.info("harness-install-recommended-separate").is_some() && r.info("harness-install-recommended-default").is_none());
    assert_eq!(r.text_of("harness-install-choice-command-separate"), "separate-installer");
    assert_eq!(r.text_of("harness-install-choice-command-default"), "default-installer");
    let words = r.words();
    for need in [
        "OpenCode 1 can be installed next to it at any time.",
        "Installing OpenCode 1 later replaces OpenCode 2.",
        "/home/you/.weave/harnesses/opencode2/.opencode/bin",
        "/home/you/.opencode/bin",
    ] {
        assert!(words.contains(need), "{need}");
    }
}

#[test]
fn says_installed_again_when_what_is_there_stopped_working_and_hides_install_here_on_another_machine() {
    let setup = json!({"installCommand": SEPARATE, "mode": "In its own folder"});
    let mut r = rig(json!([opencode2("not-working", setup, json!({"executablePath": "/opt/oc2"}))]));
    assert!(r.words().contains("Needs installing again"));
    assert!(r.info("harness-install-set-up").is_some());
    let p = crate::fleet::settings::harness_install::panel(&r.first(), false, false).unwrap();
    assert!(!p.set_up, "another machine only shows the commands");
    r.click("harness-install-set-up");
}

// ─── the GitHub Copilot path field ────────────────────────────────────────────────

fn copilot(found: bool) -> Value {
    if found {
        harness("copilot", "GitHub Copilot", "ready", json!({"version": "weave-copilot 0.1.0", "executablePath": "/opt/weave-copilot"}))
    } else {
        harness(
            "copilot",
            "GitHub Copilot",
            "not-installed",
            json!({
                "reason": "GitHub Copilot isn't installed: Fleet couldn't find weave-copilot on PATH."
            }),
        )
    }
}

#[test]
fn saves_the_typed_path_and_checks_again_and_shows_what_fleet_found() {
    let mut r = rig(json!([copilot(false)]));
    assert!(r.text_of("harness-path-reason").contains("couldn't find weave-copilot"));
    r.fake.lock().unwrap().harnesses = json!([copilot(true)]);
    // The field sits below the first screen of Copilot's tab, where a click can't reach: type and press the way the keys do.
    assert!(r.info("harness-path-input").is_some() && r.info("harness-path-check").is_some());
    r.rt.update(|s, _| s.settings.harnesses.path.draft = rune::TextEdit::new("/opt/weave-copilot"));
    r.rt.update(|s, _| s.check_harness_path());
    r.frames();
    let fake = r.fake.clone();
    r.wait("the path to reach Fleet", move |_| !fake.lock().unwrap().puts.is_empty());
    r.wait("the check to end", |s| !s.settings.harnesses.path.checking);
    assert_eq!(r.fake.lock().unwrap().puts, [("copilot.path".to_string(), "/opt/weave-copilot".to_string())]);
    assert_eq!(r.text_of("harness-path-found"), "Found: weave-copilot 0.1.0 \u{b7} /opt/weave-copilot");
}

#[test]
fn checks_again_without_saving_when_the_path_did_not_change() {
    let mut r = rig(json!([copilot(false)]));
    let before = r.fake.lock().unwrap().fresh;
    r.rt.update(|s, _| s.check_harness_path());
    r.wait("the check to end", |s| !s.settings.harnesses.path.checking);
    assert!(r.fake.lock().unwrap().puts.is_empty());
    assert!(r.fake.lock().unwrap().fresh > before);
}

#[test]
fn replaces_what_was_typed_with_the_saved_path_when_it_arrives() {
    let mut r = rig(json!([copilot(true)]));
    r.fake.lock().unwrap().prefs.insert("copilot.path".into(), "/saved/weave-copilot".into());
    r.rt.update(|s, _| s.read_preferences(&s.harness_machine()));
    r.wait("the saved path", |s| s.settings.harnesses.path.draft.text == "/saved/weave-copilot");
}

#[test]
fn says_a_path_that_could_not_be_saved_in_words() {
    let mut r = rig(json!([copilot(false)]));
    r.rt.update(|s, _| {
        s.settings.harnesses.path.error = Some("Fleet couldn't save the path to weave-copilot.".into());
    });
    r.frames();
    assert!(r.info("harness-path-error").is_some() && r.info("harness-path-reason").is_none());
}

#[test]
fn the_path_field_is_not_on_other_harnesses_cards() {
    let r = rig(json!([harness("pi", "Pi", "ready", json!({}))]));
    assert!(r.info("harness-path-field").is_none());
}

// ─── HarnessUpdateStrip.test.ts ───────────────────────────────────────────────────

fn open_code(update: Value, extra: Value) -> crate::fleet::new_session::harness_choice::HarnessEntry {
    let mut h = json!({
        "type": "opencode", "displayName": "OpenCode", "available": true, "userEnabled": true, "state": "ready", "version": "1.18.20",
        "executablePath": "/home/you/.opencode/bin/opencode", "capabilities": {}, "update": update,
    });
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        h[k] = v;
    }
    crate::fleet::new_session::harness_choice::parse_harness_entries(&json!([h])).remove(0)
}

fn all_words(st: &crate::fleet::settings::harness_update::Strip) -> String {
    let rest = st.rest.iter().map(|p| match p {
        Piece::Words(w) | Piece::Code(w) => w.clone(),
    });
    [st.lead.clone(), st.after_lead.clone().unwrap_or_default()].into_iter().chain(rest).collect::<Vec<_>>().join(" ")
}

#[test]
fn the_strip_shows_nothing_when_the_harness_is_current() {
    let h = open_code(json!({"updateAvailable": false, "latestVersion": "1.18.20", "command": COMMAND}), json!({}));
    assert!(strip(&h).is_none());
}

#[test]
fn the_strip_offers_a_newer_version_and_starts_the_update() {
    let h = open_code(json!({"updateAvailable": true, "latestVersion": "1.18.31", "command": COMMAND}), json!({}));
    let st = strip(&h).unwrap();
    let words = all_words(&st);
    assert!(words.contains("OpenCode 1.18.31 is available. You have 1.18.20."));
    assert!(words.contains(&format!("Fleet runs {COMMAND} once no session is working.")));
    assert_eq!(st.primary.as_deref(), Some("Update to 1.18.31"));
    assert_eq!(st.tone, Tone::Plain);
    // The button posts, and the harnesses are listed again.
    let mut r = rig(json!([harness(
        "opencode",
        "OpenCode",
        "ready",
        json!({
            "version": "1.18.20", "update": {"updateAvailable": true, "latestVersion": "1.18.31", "command": COMMAND, "installPlan": "plan"}
        })
    )]));
    r.open_rows();
    // The setup rows list harnesses Fleet installs itself only; the request is the same one the button sends.
    let before = r.fake.lock().unwrap().fresh;
    r.rt.update(|s, _| s.send_harness_update(&s.harness_machine(), "opencode", "POST"));
    let fake = r.fake.clone();
    r.wait("the update request", move |_| !fake.lock().unwrap().updates.is_empty());
    assert_eq!(r.updates(), [("POST".to_string(), "/api/harnesses/opencode/update".to_string())]);
    let fake = r.fake.clone();
    r.wait("the list again", move |_| fake.lock().unwrap().fresh > before);
}

#[test]
fn the_strip_says_a_harness_that_s_too_old_can_t_start_sessions_until_it_s_updated() {
    let h = open_code(
        json!({"updateAvailable": true, "latestVersion": "1.18.31", "minimumVersion": "1.15.10", "command": COMMAND}),
        json!({"available": false, "state": "update-needed", "version": "1.12.0",
               "reason": "Fleet needs OpenCode 1.15.10 or newer. You have 1.12.0."}),
    );
    let st = strip(&h).unwrap();
    assert_eq!(st.tone, Tone::Warn);
    let words = all_words(&st);
    assert!(words.contains("Fleet needs OpenCode 1.15.10 or newer. You have 1.12.0."));
    assert!(words.contains("New sessions can't use OpenCode until you update."));
    assert!(words.contains("1.18.31 is the latest."));
}

#[test]
fn the_strip_waits_for_working_sessions_and_the_wait_can_be_cancelled() {
    let h = open_code(
        json!({"updateAvailable": true, "latestVersion": "1.18.31", "command": COMMAND,
               "job": {"phase": "waiting", "workingSessions": 2, "fromVersion": "1.18.20"}}),
        json!({}),
    );
    let st = strip(&h).unwrap();
    assert!(all_words(&st).contains("Waiting for 2 working sessions to finish."));
    assert!(st.cancel && st.busy && st.primary.is_none());
    let one = open_code(json!({"updateAvailable": true, "job": {"phase": "waiting", "workingSessions": 1}}), json!({}));
    assert!(strip(&one).unwrap().lead.contains("1 working session to finish."));
    let mut r = rig(json!([harness("opencode", "OpenCode", "ready", json!({}))]));
    r.rt.update(|s, _| s.send_harness_update(&s.harness_machine(), "opencode", "DELETE"));
    let fake = r.fake.clone();
    r.wait("the cancel", move |_| !fake.lock().unwrap().updates.is_empty());
    assert_eq!(r.updates(), [("DELETE".to_string(), "/api/harnesses/opencode/update".to_string())]);
}

#[test]
fn the_strip_shows_a_failed_update_s_reason_output_and_the_command_to_run_by_hand() {
    let message = format!("The update failed (exit code 1). OpenCode is still on 1.18.20. Run it yourself: {COMMAND}");
    let h = open_code(
        json!({"updateAvailable": true, "latestVersion": "1.18.31", "command": COMMAND,
               "job": {"phase": "failed", "workingSessions": 0, "message": message, "output": "npm error code EACCES"}}),
        json!({}),
    );
    let st = strip(&h).unwrap();
    assert_eq!(st.tone, Tone::Error);
    assert!(st.lead.contains("The update failed (exit code 1)."));
    assert_eq!(st.output.as_deref(), Some("npm error code EACCES"));
    assert_eq!(st.command.as_deref(), Some(COMMAND));
    assert_eq!((st.primary.as_deref(), st.dismiss), (Some("Try again"), Some("Dismiss")));
}

#[test]
fn the_strip_shows_a_finished_update_until_it_s_dismissed() {
    let h = open_code(
        json!({"updateAvailable": false, "latestVersion": "1.18.31", "command": COMMAND,
               "job": {"phase": "succeeded", "workingSessions": 0, "message": "Updated OpenCode from 1.18.20 to 1.18.31.", "fromVersion": "1.18.20"}}),
        json!({"version": "1.18.31"}),
    );
    let st = strip(&h).unwrap();
    assert_eq!(st.tone, Tone::Ok);
    assert!(all_words(&st).contains("Updated OpenCode from 1.18.20 to 1.18.31."));
    assert!(all_words(&st).contains("Idle OpenCode processes restarted. New sessions use the new version."));
    assert_eq!(st.dismiss, Some("Done"));
    assert!(st.primary.is_none());
}

fn copilot_row(update: Value) -> Value {
    harness(
        "copilot",
        "GitHub Copilot",
        "ready",
        json!({
            "version": "0.2.1",
            "setup": {"signInCommand": null},
            "update": update,
        }),
    )
}

#[test]
fn the_strip_shows_why_the_server_refused_an_update() {
    let mut r = rig(json!([copilot_row(json!({"updateAvailable": true, "latestVersion": "0.2.2", "installPlan": "Fleet downloads it."}))]));
    r.fake.lock().unwrap().update_answer = Some((409, json!({"error": "GitHub Copilot is already being updated."})));
    r.open_rows();
    r.click("harness-update-start-copilot");
    r.wait("the refusal", |s| s.settings.harnesses.strips.error.is_some());
    assert_eq!(r.text_of("harness-update-error-copilot"), "GitHub Copilot is already being updated.");
    // An answer with no words says its status.
    r.fake.lock().unwrap().update_answer = Some((500, json!({})));
    r.click("harness-update-start-copilot");
    r.wait("the second refusal", |s| s.settings.harnesses.strips.error.as_ref().is_some_and(|(_, e)| e.contains("HTTP 500")));
    assert_eq!(r.text_of("harness-update-error-copilot"), "The request failed (HTTP 500).");
}

#[test]
fn the_strip_offers_to_install_a_harness_fleet_downloads_itself_and_shows_the_download_s_progress() {
    let plan = "Fleet downloads weave-copilot 0.1.0 and the Copilot CLI 1.0.98 from GitHub into ~/.weave/runtimes/copilot.";
    let not_installed = json!({"state": "not-installed", "available": false, "version": null, "executablePath": null});
    let h = open_code(json!({"updateAvailable": false, "canInstall": true, "installPlan": plan}), not_installed.clone());
    let st = strip(&h).unwrap();
    assert_eq!(st.lead, "OpenCode isn't installed.");
    assert_eq!(st.rest, [Piece::Words(plan.into())]);
    assert_eq!(st.primary.as_deref(), Some("Install"));
    let running = open_code(
        json!({"updateAvailable": false, "canInstall": true, "installPlan": plan,
               "job": {"phase": "running", "message": "Downloading the Copilot CLI 1.0.98\u{2026} 42%", "workingSessions": 0}}),
        not_installed,
    );
    let st = strip(&running).unwrap();
    assert_eq!(st.lead, "Downloading the Copilot CLI 1.0.98\u{2026} 42%");
    assert!(st.busy && st.primary.is_none());
    // In the setup rows it is the same strip, with a button that posts.
    let mut r = rig(json!([harness(
        "copilot",
        "GitHub Copilot",
        "not-installed",
        json!({
            "update": {"updateAvailable": false, "canInstall": true, "installPlan": plan}, "setup": {"downloadUrl": null}
        })
    )]));
    r.open_rows();
    assert!(r.words().contains("GitHub Copilot isn't installed."));
    r.click("harness-update-start-copilot");
    let fake = r.fake.clone();
    r.wait("the install request", move |_| !fake.lock().unwrap().updates.is_empty());
    assert_eq!(r.updates(), [("POST".to_string(), "/api/harnesses/copilot/update".to_string())]);
}

#[test]
fn the_copy_button_of_a_failed_update_says_copied_for_a_moment() {
    let mut r = rig(json!([copilot_row(json!({
        "updateAvailable": true, "latestVersion": "0.2.2", "command": "weave-copilot update", "installPlan": "Fleet downloads it.",
        "job": {"phase": "failed", "workingSessions": 0, "message": "The update failed.", "output": "boom"},
    }))]));
    r.open_rows();
    assert!(r.info("harness-update-output-copilot").is_some());
    r.click("harness-update-copy-copilot");
    assert!(r.rt.state.harness_copied("weave-copilot update"));
    assert!(r.words().contains("Copied"));
}

// ─── the 2-second poll (use-harness-update-poll.ts) ───────────────────────────────

#[test]
fn reads_the_harnesses_every_two_seconds_while_an_update_waits_or_runs_and_stops_when_it_ends() {
    let running = json!({"updateAvailable": true, "job": {"phase": "running", "workingSessions": 0}});
    let mut r = rig(json!([copilot_row(running)]));
    assert!(updating(&r.rt.state.harness_list().all));
    let before = r.fake.lock().unwrap().fresh;
    // The first look only starts the wait; the next one, two seconds on, reads.
    let mut tx = rune::Tx::default();
    let machine = r.rt.state.harness_machine();
    r.rt.update(|s, _| s.settings.harnesses.strips.poll_due = None);
    r.rt.update(|s, _| assert!(!s.keep_update_poll(&machine, true, 10.0, &mut tx)));
    r.rt.update(|s, _| assert!(!s.keep_update_poll(&machine, true, 11.0, &mut tx)));
    r.rt.update(|s, _| assert!(s.keep_update_poll(&machine, true, 12.1, &mut tx)));
    let fake = r.fake.clone();
    r.wait("the poll's read", move |_| fake.lock().unwrap().fresh > before);
    // Off the page, or once nothing waits or runs, it stops.
    r.rt.update(|s, _| assert!(!s.keep_update_poll(&machine, false, 20.0, &mut tx)));
    assert!(r.rt.state.settings.harnesses.strips.poll_due.is_none());
    r.fake.lock().unwrap().harnesses = json!([copilot_row(json!({"updateAvailable": false}))]);
    r.rt.update(|s, _| s.check_harnesses_again());
    r.wait("the update's end", |s| !updating(&s.harness_list().all));
    r.rt.update(|s, _| assert!(!s.keep_update_poll(&machine, true, 30.0, &mut tx)));
    assert!(r.rt.state.settings.harnesses.strips.poll_due.is_none());
}
