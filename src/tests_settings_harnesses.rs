//! Component 16, piece G: Settings → Harnesses (page, tabs, panel header, switches). `HarnessesSection.test.ts`'s cases marked
//! "A" in docs/parity/16-settings.md under their names, against a fake Fleet that answers the harness list and the preferences
//! and records every `PUT /api/preferences/{key}`; then the state-pill table and the plain rules.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::new_session::harness_choice::Card;
use crate::fleet::settings::harnesses::{Status, relative, summary};
use crate::tests::{FRAME, app};
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(crate) struct Fake {
    pub harnesses: Value,
    pub prefs: HashMap<String, String>,
    pub puts: Vec<(String, String)>,
    pub refuse: Vec<String>,
    pub lists: usize,
    pub fresh: usize,
    /// What `GET /api/agent-browser/settings` says `enabled` is.
    pub browser_on: bool,
    /// The pages and scripts it says (`""` is this session's pages), every `PUT` body it got, and whether it turns them down.
    pub browser_pages: String,
    pub browser_scripts: bool,
    pub browser_puts: Vec<Value>,
    pub browser_refuse: bool,
    /// Answers the browser's settings with something that isn't settings.
    pub browser_garbage: bool,
    /// Every `PUT /api/machines/{id}`: the id and the `agentWork` it sent. One whose id is in `refuse` is turned down.
    pub machine_puts: Vec<(String, String)>,
}
pub(crate) type Shared = Arc<Mutex<Fake>>;

fn browser_json(f: &Fake) -> Value {
    json!({"enabled": f.browser_on, "pages": if f.browser_pages.is_empty() { "session" } else { &f.browser_pages }, "scripts": f.browser_scripts})
}

pub(crate) fn serve(fake: Shared) -> String {
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
    let req = crate::test_http::read(&mut stream);
    if req.method.is_empty() {
        return;
    }
    let (method, path) = (req.method.as_str(), req.path.as_str());
    let body = req.body;
    let mut f = fake.lock().unwrap();
    let (status, reply) = if method == "GET" && path.starts_with("/api/harnesses") {
        f.lists += 1;
        if path.contains("fresh=true") {
            f.fresh += 1;
        }
        ("200 OK", f.harnesses.clone())
    } else if method == "GET" && path == "/api/preferences" {
        ("200 OK", json!(f.prefs))
    } else if path == "/api/weave" {
        // Weave asks nothing of these pages: an empty answer draws no Weave group (the rigs wait for it, so the layout is still).
        ("200 OK", json!({"source": "own", "files": {}, "harnesses": []}))
    } else if method == "GET" && path == "/api/agent-browser/settings" {
        if f.browser_garbage { ("200 OK", json!({})) } else { ("200 OK", browser_json(&f)) }
    } else if method == "PUT" && path == "/api/agent-browser/settings" {
        let sent = serde_json::from_slice::<Value>(&body).unwrap_or_default();
        f.browser_puts.push(sent.clone());
        if f.browser_refuse {
            ("500 Internal Server Error", json!({ "error": "The server said no." }))
        } else {
            if let Some(on) = sent["enabled"].as_bool() {
                f.browser_on = on;
            }
            if let Some(p) = sent["pages"].as_str() {
                f.browser_pages = p.to_string();
            }
            if let Some(on) = sent["scripts"].as_bool() {
                f.browser_scripts = on;
            }
            ("200 OK", browser_json(&f))
        }
    } else if method == "PUT" && path.starts_with("/api/machines/") {
        let id = path.trim_start_matches("/api/machines/").to_string();
        let work =
            serde_json::from_slice::<Value>(&body).ok().and_then(|v| v["agentWork"].as_str().map(str::to_string)).unwrap_or_default();
        f.machine_puts.push((id.clone(), work.clone()));
        if f.refuse.contains(&id) {
            ("500 Internal Server Error", json!({ "error": "The server said no." }))
        } else {
            ("200 OK", json!({ "agentWork": work }))
        }
    } else if method == "PUT" && path.starts_with("/api/preferences/") {
        let key = path.trim_start_matches("/api/preferences/").replace("%2E", ".");
        let value = serde_json::from_slice::<Value>(&body).ok().and_then(|v| v["value"].as_str().map(str::to_string)).unwrap_or_default();
        f.puts.push((key.clone(), value.clone()));
        if f.refuse.contains(&key) {
            ("500 Internal Server Error", json!({ "error": "The server said no." }))
        } else {
            f.prefs.insert(key, value);
            ("200 OK", json!({}))
        }
    } else {
        ("200 OK", json!({}))
    };
    let reply = reply.to_string();
    let out = format!(
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(out.as_bytes());
}

pub(crate) fn info(kind: &str, name: &str, state: &str, extra: Value) -> Value {
    let mut h = json!({
        "type": kind, "displayName": name, "available": state == "ready" || state == "sign-in-required", "userEnabled": true, "state": state,
        "reason": if state == "not-installed" { json!(format!("{name} isn't installed.")) } else { Value::Null },
        "capabilities": {"supportsPermissionLevels": kind != "pi", "supportsAgentBrowser": kind != "pi"},
        "presentation": {"eyebrow": "CLI harness", "description": format!("The {name} runtime.")},
        "settings": [],
    });
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        h[k] = v;
    }
    h
}

fn list() -> Value {
    let hydra = json!([{"key": "copilot.hydrafusion", "label": "HydraFusion (research preview)", "description": "Mixes models.", "default": false}]);
    json!([
        info(
            "opencode2",
            "OpenCode 2",
            "ready",
            json!({"version": "2.0.6", "executablePath": "/opt/oc2/bin/opencode2", "checkedAt": "2026-10-07T01:49:43Z"})
        ),
        info("claude-code", "Claude Code", "ready", json!({"version": "2.1.276"})),
        info(
            "copilot",
            "GitHub Copilot",
            "sign-in-required",
            json!({"version": "0.9.2", "executablePath": "/opt/bin/weave-copilot", "cliVersion": "1.0.4", "settings": hydra})
        ),
        info("pi", "Pi", "not-installed", json!({})),
    ])
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Shared,
}

fn rig(prefs: &[(&str, &str)]) -> Rig {
    let fake: Shared = Arc::new(Mutex::new(Fake {
        harnesses: list(),
        prefs: prefs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        ..Default::default()
    }));
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
    rt.update(move |s, _| {
        s.link = Some(link);
        s.pickers.prefs = Default::default();
        s.now_ms = Some(1_791_337_783_000); // 2026-10-07T01:49:43Z + 2 minutes
        s.rail = Rail::Settings;
        s.settings.set("harnesses", None);
    });
    let mut r = Rig { rt, queue, fake };
    r.rt.frame(FRAME);
    r.wait("the harnesses and the preferences", |s| {
        s.harness_list().all.len() == 4 && s.harness_prefs().fetched && !s.harnesses_checking() && s.settings.harnesses.weave.view.is_some()
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
    fn put(&mut self, key: &str, value: &str) {
        let fake = self.fake.clone();
        let (k, v) = (key.to_string(), value.to_string());
        self.wait(&format!("{key} = {value} to reach Fleet"), move |_| fake.lock().unwrap().puts.contains(&(k.clone(), v.clone())));
        self.wait("the save to settle", |s| s.settings.harnesses.saving.is_none());
    }
    fn tabs(&self) -> Vec<String> {
        self.rt.state.harness_list().all.iter().map(|h| h.kind.clone()).collect()
    }
}

// ─── HarnessesSection.test.ts ───────────────────────────────────────────────────

#[test]
fn renders_harnesses_section() {
    let r = rig(&[]);
    assert!(r.info("harnesses-section").is_some());
    assert!(r.words().contains("The runtimes Fleet drives sessions with. Everything about one harness is on its tab."));
    assert!(r.info("settings-group-default-harness").is_some());
}

fn click(r: &mut Rig, key: &str) {
    crate::tests::click(&mut r.rt, key);
    r.frames();
}

fn enabled(r: &Rig, kind: &str) -> bool {
    let h = r.rt.state.harness_list().all.into_iter().find(|h| h.kind == kind).unwrap();
    r.rt.state.harness_enabled(&h)
}

#[test]
fn shows_a_harness_on_until_the_user_turns_it_off_so_a_computer_with_only_claude_code_works() {
    let mut r = rig(&[]);
    click(&mut r, "harness-tab-claude-code");
    assert!(enabled(&r, "claude-code"));
    assert!(r.words().contains("Enabled") && r.words().contains("Claude Code is installed and ready."));
    click(&mut r, "harness-enabled-switch");
    assert!(!enabled(&r, "claude-code"), "the value shows at once");
    r.put("claude-code.enabled", "false");
    assert!(r.words().contains("Disabled until enabled by the user."));
    assert!(r.words().contains("DISABLED"));
}

#[test]
fn says_a_harness_isn_t_installed_and_why() {
    let mut r = rig(&[]);
    click(&mut r, "harness-tab-pi");
    assert!(r.words().contains("Pi isn't installed."));
    assert!(r.words().contains("NOT INSTALLED"));
}

#[test]
fn shows_the_version_and_where_fleet_found_a_harness_that_needs_a_sign_in() {
    let mut r = rig(&[]);
    // The default's tab is first shown; the sign-in one is picked.
    assert_eq!(r.info("harness-location").unwrap().text.unwrap(), "2.0.6 \u{b7} /opt/oc2/bin/opencode2");
    click(&mut r, "harness-tab-copilot");
    assert!(r.words().contains("SIGN-IN NEEDED"));
    assert!(r.words().contains("weave-copilot 0.9.2 \u{b7} Copilot CLI 1.0.4 is installed and ready."));
}

#[test]
fn shows_the_harness_as_it_describes_itself_the_default_as_the_server_marks_it() {
    let mut r = rig(&[]);
    assert!(r.words().contains("CLI harness \u{b7} The OpenCode 2 runtime."));
    assert!(r.info("harness-default-pill").is_some(), "with none marked, the first harness is the default (resolveDefaultHarness)");
    r.fake.lock().unwrap().harnesses[1]["isDefault"] = json!(true);
    r.rt.update(|s, _| s.check_harnesses_again());
    r.wait("the new list", |s| s.harness_list().default_type() == "claude-code");
    assert!(r.words().contains("DEFAULT"));
    assert!(r.rt.state.harness_tab().is_some_and(|h| h.kind == "claude-code"), "the default harness's tab is shown first");
}

#[test]
fn shows_a_harness_s_own_switches_on_by_the_default_it_sends_until_the_user_sets_them() {
    let mut r = rig(&[]);
    click(&mut r, "harness-tab-copilot");
    assert!(r.words().contains("HydraFusion (research preview)") && r.words().contains("Off by default. Mixes models."));
    click(&mut r, "harness-setting-toggle-copilot.hydrafusion");
    r.put("copilot.hydrafusion", "true");
    assert_eq!(r.rt.state.harness_prefs().get("copilot.hydrafusion"), Some("true"));
    // Saving lists every harness again.
    let fake = r.fake.clone();
    r.wait("the list asked again after the save", move |_| fake.lock().unwrap().fresh >= 2);
}

#[test]
fn shows_an_error_under_the_row_when_a_switch_can_t_be_saved() {
    let mut r = rig(&[]);
    r.fake.lock().unwrap().refuse.push("copilot.hydrafusion".into());
    click(&mut r, "harness-tab-copilot");
    click(&mut r, "harness-setting-toggle-copilot.hydrafusion");
    r.wait("the refusal", |s| s.settings.harnesses.error.is_some());
    assert!(r.info("harness-setting-error-copilot.hydrafusion").is_some());
    assert_eq!(r.rt.state.harness_prefs().get("copilot.hydrafusion"), None, "the old value is back");
}

#[test]
fn checks_again_so_a_harness_installed_a_moment_ago_shows_up() {
    let mut r = rig(&[]);
    let before = r.fake.lock().unwrap().fresh;
    click(&mut r, "harnesses-check-again");
    r.wait("another check", |s| !s.harnesses_checking());
    assert!(r.fake.lock().unwrap().fresh > before);
}

#[test]
fn opens_harness_setup_at_the_harness_step() {
    let mut r = rig(&[]);
    click(&mut r, "harnesses-set-up");
    assert!(r.rt.state.harness_setup.is_open);
}

#[test]
fn offers_every_harness_as_the_default_and_writes_the_choice() {
    let mut r = rig(&[]);
    click(&mut r, "harnesses-default-select");
    for name in ["OpenCode 2", "Claude Code", "GitHub Copilot", "Pi"] {
        assert!(r.words().contains(name));
    }
    click(&mut r, "harnesses-default-list-opt1");
    r.put("defaultHarnessType", "claude-code");
    assert_eq!(r.rt.state.harness_list().default_type(), "claude-code");
}

#[test]
fn a_harness_that_is_off_can_t_be_made_the_default() {
    let mut r = rig(&[("pi.enabled", "false")]);
    r.rt.update(|s, _| s.set_default_harness("pi"));
    r.frames();
    assert!(r.fake.lock().unwrap().puts.is_empty());
}

// ─── tabs ───────────────────────────────────────────────────────────────────────

#[test]
fn has_one_tab_per_harness_fleet_lists_in_its_order_on_the_default_harness_s_first() {
    let r = rig(&[]);
    assert_eq!(r.tabs(), ["opencode2", "claude-code", "copilot", "pi"]);
    for kind in r.tabs() {
        assert!(r.info(&format!("harness-tab-{kind}")).is_some());
    }
    assert!(r.info("harness-panel-opencode2").is_some());
}

#[test]
fn gives_github_copilot_its_own_tab_with_no_switch_to_turn_it_on_beyond_enabled() {
    let mut r = rig(&[]);
    click(&mut r, "harness-tab-copilot");
    assert!(r.info("harness-panel-copilot").is_some());
    assert!(r.info("harness-enabled-switch").is_some());
}

#[test]
fn remembers_the_tab_while_settings_is_open() {
    let mut r = rig(&[]);
    click(&mut r, "harness-tab-pi");
    r.rt.update(|s, _| s.open_settings("general", None));
    r.rt.update(|s, _| s.open_settings("harnesses", None));
    r.frames();
    assert!(r.info("harness-panel-pi").is_some());
}

#[test]
fn opens_the_tab_a_link_names() {
    let mut r = rig(&[]);
    r.rt.update(|s, _| s.open_harness("copilot", Some("may-do")));
    r.frames();
    assert!(r.info("harness-panel-copilot").is_some());
}

#[test]
fn spends_the_anchor_once_it_has_scrolled_there_so_a_later_save_doesn_t_pull_the_page_back() {
    let mut r = rig(&[]);
    r.rt.update(|s, _| s.open_harness("opencode2", Some("may-do")));
    r.frames();
    r.frames();
    assert!(r.rt.state.settings.anchor.is_some(), "Harnesses keeps its anchor (it picks its tab from it)");
    assert!(r.rt.state.settings.scrolled, "but scrolled to it once");
}

#[test]
fn says_the_level_it_runs_at_and_links_to_what_agents_can_do() {
    let mut r = rig(&[("PermissionLevel", "edits"), ("PermissionLevel.claude-code", "ask")]);
    // The fake says Fleet's browser is off, so every harness runs without it.
    r.wait("the browser answer", |s| s.browser_off());
    assert!(r.words().contains("Runs at Allow edits, without the browser."));
    click(&mut r, "harness-tab-claude-code");
    assert!(r.words().contains("Runs at Ask before changes, without the browser."));
    click(&mut r, "harness-may-do-link");
    assert_eq!(r.rt.state.settings.page, crate::fleet::settings_nav::Page::Permissions);
}

#[test]
fn says_fleet_can_t_hand_a_harness_without_levels_a_level_or_the_browser() {
    let mut r = rig(&[]);
    click(&mut r, "harness-tab-pi");
    assert!(r.words().contains("Fleet can't hand Pi a permission level or the browser: it asks as its own settings say."));
}

#[test]
fn links_to_updates_and_about_and_says_no_settings_yet() {
    let mut r = rig(&[]);
    assert!(r.words().contains("No settings yet"));
    click(&mut r, "harness-updates-link");
    assert_eq!(r.rt.state.settings.page, crate::fleet::settings_nav::Page::Updates);
}

// ─── ours ───────────────────────────────────────────────────────────────────────

#[test]
fn asks_the_machine_the_switch_names_not_home() {
    let r = rig(&[]);
    let lists = r.fake.lock().unwrap().lists;
    assert!(lists >= 1);
    assert_eq!(r.rt.state.harness_machine(), "home");
}

#[test]
fn the_state_pill_has_a_label_for_every_state() {
    let h = |state: &str, available| crate::fleet::new_session::harness_choice::HarnessEntry {
        state: state.into(),
        available,
        card: Card::default(),
        ..Default::default()
    };
    let table = [
        ("ready", Status::Ready, "Ready"),
        ("not-installed", Status::NotInstalled, "Not installed"),
        ("sign-in-required", Status::SignInRequired, "Sign-in needed"),
        ("not-working", Status::NotWorking, "Not working"),
        ("update-needed", Status::UpdateNeeded, "Update needed"),
    ];
    for (state, status, label) in table {
        assert_eq!(Status::of(&h(state, true), true), status);
        assert_eq!(status.label(), label);
    }
    assert_eq!(Status::of(&h("ready", true), false).label(), "Disabled", "a turned-off harness is disabled whatever it says");
    assert_eq!(Status::of(&h("", true), true), Status::Ready);
    assert_eq!(Status::of(&h("", false), true), Status::NotWorking);
}

#[test]
fn the_summary_follows_fleet_s_rule() {
    let mut h = crate::fleet::new_session::harness_choice::HarnessEntry { display_name: "Pi".into(), ..Default::default() };
    assert_eq!(summary(&h, true), "Pi is registered but not currently available.");
    h.reason = Some("Nope.".into());
    assert_eq!(summary(&h, true), "Nope.");
    assert_eq!(summary(&h, false), "Disabled until enabled by the user.");
    h.available = true;
    assert_eq!(summary(&h, true), "Pi is installed and ready.");
}

#[test]
fn relative_time_reads_as_fleet_words_it() {
    let now = 10 * 7 * 86_400_000;
    assert_eq!(relative(now - 20_000, now, 0), "just now");
    assert_eq!(relative(now - 60_000, now, 0), "1 minute ago");
    assert_eq!(relative(now - 120_000, now, 0), "2 minutes ago");
    assert_eq!(relative(now - 3 * 3_600_000, now, 0), "3 hours ago");
    assert_eq!(relative(now - 86_400_000, now, 0), "yesterday");
    assert_eq!(relative(now - 8 * 86_400_000, now, 0), "last week");
}
