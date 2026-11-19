//! Component 16, piece Y: `HarnessWeaveGroup.test.ts` and the Weave cases of `HarnessesSection.test.ts`, under their names,
//! against a fake Fleet that answers the harness list, the preferences, `/api/weave` and the plugin requests, and records them.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::harness_weave::{Group, WeaveView};
use crate::tests::{FRAME, app};
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(crate) struct Fake {
    /// The harnesses the list has, in order; the first is the default.
    pub harnesses: Vec<&'static str>,
    /// What Fleet says each harness loads of Weave (`harnesses` of `GET /api/weave`).
    pub weave: Vec<Value>,
    pub source: &'static str,
    /// The first read of `/api/weave` waits until this is false.
    pub hold: bool,
    /// Reads of `/api/weave` refused until this many have been.
    pub refuse_reads: usize,
    /// What a plugin change answers: `(status, body)`; the config is filled in from `weave` when it is `None`.
    pub plugin: Option<(u16, Value)>,
    /// Held back: a plugin change waits until this is false.
    pub hold_plugin: bool,
    /// The `/api/weave` paths asked, query included, and the plugin requests `(method, path)`.
    pub reads: Vec<String>,
    pub plugin_calls: Vec<(String, String)>,
}
pub(crate) type Shared = Arc<Mutex<Fake>>;

fn view(f: &Fake) -> Value {
    json!({"source": if f.source.is_empty() { "own" } else { f.source }, "files": {}, "updatedAt": null, "apply": null, "harnesses": f.weave})
}

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
    let req = crate::test_http::read(&mut stream);
    if req.method.is_empty() {
        return;
    }
    let (method, path) = (req.method.clone(), req.path.clone());
    if path.starts_with("/api/weave") {
        while fake.lock().unwrap().hold && method == "GET" || fake.lock().unwrap().hold_plugin && method != "GET" {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    let mut f = fake.lock().unwrap();
    let (status, reply) = if method == "GET" && (path == "/api/harnesses" || path == "/api/harnesses?fresh=true") {
        let list: Vec<Value> = f
            .harnesses
            .iter()
            .enumerate()
            .map(|(at, kind)| {
                json!({"type": kind, "displayName": kind, "available": true, "userEnabled": true, "state": "ready", "isDefault": at == 0,
                    "capabilities": {}, "presentation": {"eyebrow": "CLI harness", "description": "The runtime."}, "settings": []})
            })
            .collect();
        (200, Value::Array(list))
    } else if method == "GET" && path == "/api/preferences" {
        (200, json!({}))
    } else if method == "GET" && path == "/api/agent-browser/settings" {
        (200, json!({"enabled": true}))
    } else if method == "GET" && path.starts_with("/api/weave") {
        f.reads.push(path.clone());
        if f.refuse_reads > 0 {
            f.refuse_reads -= 1;
            (500, json!({"error": "HTTP 500"}))
        } else {
            (200, view(&f))
        }
    } else if path.starts_with("/api/weave/harnesses/") {
        f.plugin_calls.push((method.clone(), path.clone()));
        match f.plugin.clone() {
            Some((status, body)) => (status, body),
            None => {
                (200, json!({"configPath": "/cfg/opencode.json", "entry": "e", "loaded": true, "message": "Done.", "config": view(&f)}))
            }
        }
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

pub(crate) const PACKAGE: &str = "@weaveio/weave-adapter-opencode";

pub(crate) fn weave_install() -> Value {
    json!({"flavor": "weave", "package": PACKAGE, "entry": format!("{PACKAGE}@0.2.0-next.1"), "acceptsFleetConfig": true})
}

pub(crate) fn v2_install() -> Value {
    json!({"flavor": "weave", "package": "@weaveio/weave-adapter-opencode2", "entry": "@weaveio/weave-adapter-opencode2@0.2.0-next.3",
        "acceptsFleetConfig": true, "addedByFleet": true})
}

pub(crate) fn old_legacy() -> Value {
    json!({"flavor": "legacy", "package": "@opencode_weave/weave", "entry": "@opencode_weave/weave@0.8.2", "acceptsFleetConfig": false})
}

pub(crate) fn detection(kind: &str, name: &str, installs: Vec<Value>, add_to: Option<&str>) -> Value {
    json!({"harnessType": kind, "harnessName": name, "checked": true, "installs": installs, "addTo": add_to})
}

pub(crate) struct Rig {
    pub rt: Runtime<State>,
    pub queue: Receiver<Job>,
    pub fake: Shared,
}

pub(crate) fn rig(mut fake: Fake) -> Rig {
    if fake.harnesses.is_empty() {
        fake.harnesses = vec!["opencode", "opencode2", "pi"];
    }
    let fake: Shared = Arc::new(Mutex::new(fake));
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
        s.rail = Rail::Settings;
        s.settings.set("harnesses", None);
    });
    let mut r = Rig { rt, queue, fake };
    r.rt.frame(FRAME);
    r.wait("the harness list and the preferences", |s| s.harness_list().all.len() == 3 && s.pickers.prefs.fetched);
    r
}

impl Rig {
    pub fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        let fake = self.fake.clone();
        test_wait::drive(&mut self.rt, &self.queue, what, done, move |s| {
            format!("{:?}; reads {:?}", s.settings.harnesses.weave, fake.lock().map(|f| f.reads.clone()))
        });
        self.frames();
    }
    pub fn frames(&mut self) {
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }
    pub fn has(&self, key: &str) -> bool {
        self.rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }
    pub fn words(&self) -> String {
        self.rt.elements().iter().filter_map(|e| e.text.clone().or_else(|| e.label.clone())).collect::<Vec<_>>().join("\n")
    }
    pub fn text_of(&self, key: &str) -> String {
        self.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text).unwrap_or_default()
    }
    pub fn disabled(&self, key: &str) -> bool {
        self.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).is_some_and(|e| e.opacity < 0.9)
    }
    pub fn click(&mut self, key: &str) {
        crate::tests::click(&mut self.rt, key);
        self.frames();
    }
    pub fn tab(&mut self, kind: &str) {
        self.click(&format!("harness-tab-{kind}"));
    }
    pub fn answered(&mut self) {
        self.wait("Fleet's answer about Weave", |s| s.settings.harnesses.weave.view.is_some());
    }
    pub fn reads(&self) -> usize {
        self.fake.lock().unwrap().reads.len()
    }
}

fn opencode(installs: Vec<Value>) -> Value {
    detection("opencode", "OpenCode", installs, None)
}

// ─── HarnessWeaveGroup.test.ts ──────────────────────────────────────────────────

#[test]
fn shows_which_weave_this_harness_loads_and_only_this_harness_s() {
    let mut r = rig(Fake {
        weave: vec![
            opencode(vec![weave_install()]),
            json!({"harnessType": "claude-code", "harnessName": "Claude Code", "checked": false, "installs": [],
                "note": "Fleet doesn't hand Weave a config in Claude Code yet."}),
        ],
        ..Fake::default()
    });
    r.answered();
    assert_eq!(r.text_of("settings-row-weave-plugin"), "");
    assert!(r.words().contains(&format!("{PACKAGE}@0.2.0-next.1")));
    assert!(!r.words().contains("Fleet doesn't hand Weave"));
    assert!(r.has("settings-group-weave"));
    assert!(r.has("weave-pill"));
}

#[test]
fn is_absent_for_a_harness_fleet_can_t_ask_about_weave() {
    let mut r = rig(Fake { weave: vec![opencode(vec![weave_install()])], ..Fake::default() });
    r.answered();
    r.tab("pi");
    assert!(!r.has("settings-group-weave"));
    assert!(!r.has("settings-group-weave-loading"));
}

#[test]
fn says_it_s_asking_the_harnesses_until_fleet_answers() {
    let mut r = rig(Fake { hold: true, weave: vec![opencode(vec![])], ..Fake::default() });
    r.wait("the ask to be on its way", |s| s.settings.harnesses.weave.loading);
    assert!(r.has("settings-group-weave-loading"));
    assert!(r.words().contains("Asking your harnesses which plugins they load\u{2026}"));
    r.fake.lock().unwrap().hold = false;
    r.answered();
    assert!(!r.has("weave-asking"));
}

#[test]
fn asks_only_once_the_group_is_seen_since_asking_can_start_a_harness() {
    // No Weave group is drawn on the Pi tab, so the page has not asked about Weave by the time it is drawn.
    let mut r = rig(Fake { weave: vec![opencode(vec![])], ..Fake::default() });
    r.answered();
    assert_eq!(r.reads(), 1, "asked when the group was first drawn, once, and not again by every frame");
    r.frames();
    r.frames();
    assert_eq!(r.reads(), 1);
}

#[test]
fn doesn_t_ask_again_on_its_own_after_asking_failed_check_again_does() {
    let mut r = rig(Fake { refuse_reads: 1, weave: vec![opencode(vec![weave_install()])], ..Fake::default() });
    r.wait("the refusal", |s| s.settings.harnesses.weave.error.is_some());
    r.frames();
    r.tab("opencode2");
    r.tab("opencode");
    assert_eq!(r.reads(), 1, "a tab switch does not retry");
    r.click("weave-recheck");
    r.answered();
    assert_eq!(r.fake.lock().unwrap().reads, ["/api/weave", "/api/weave?redetect=true"]);
}

#[test]
fn doesn_t_ask_again_when_fleet_has_answered_already() {
    let mut r =
        rig(Fake { weave: vec![opencode(vec![weave_install()]), detection("opencode2", "OpenCode 2", vec![], None)], ..Fake::default() });
    r.answered();
    r.tab("opencode2");
    r.tab("opencode");
    r.frames();
    assert_eq!(r.reads(), 1);
}

#[test]
fn says_why_it_can_t_be_asked_when_fleet_couldn_t_load_weave() {
    let mut r = rig(Fake { refuse_reads: 1, weave: vec![opencode(vec![])], ..Fake::default() });
    r.wait("the refusal", |s| s.settings.harnesses.weave.error.is_some());
    assert!(r.words().contains("Couldn't ask the harnesses about Weave"));
    assert_eq!(r.text_of("settings-row-weave-error"), "");
    assert!(r.words().contains("HTTP 500"));
    assert!(matches!(r.rt.state.settings.harnesses.weave.group("opencode"), Group::Failed(_)));
}

#[test]
fn says_weave_isn_t_running_in_any_harness_what_it_adds_and_offers_nothing_to_edit() {
    let mut r = rig(Fake {
        weave: vec![
            detection("opencode2", "OpenCode 2", vec![], Some("/cfg/opencode2.json")),
            detection("opencode", "OpenCode", vec![], Some("/cfg/opencode.json")),
        ],
        ..Fake::default()
    });
    r.answered();
    r.tab("opencode2");
    assert!(r.has("weave-none"));
    assert!(r.words().contains("Weave isn't running in any harness"));
    assert!(r.words().contains("Add Weave above puts it there for you."));
    assert!(r.has("weave-docs"));
    assert!(!r.has("weave-edit-config"));
}

#[test]
fn asks_for_the_config_dialog_from_edit_and_says_whose_config_weave_reads() {
    let mut r = rig(Fake { source: "fleet", weave: vec![opencode(vec![weave_install()])], ..Fake::default() });
    r.answered();
    assert!(r.words().contains("Kept in Fleet. Weave reads it in Fleet sessions."));
    r.click("weave-edit-config");
    assert!(r.rt.state.settings.harnesses.weave.edit_asked);
    let mut own = rig(Fake { weave: vec![opencode(vec![weave_install()])], ..Fake::default() });
    own.answered();
    assert!(own.words().contains("Weave reads your own file in Fleet sessions."));
}

#[test]
fn says_why_check_again_failed_even_with_what_fleet_found_before_still_shown() {
    let mut r = rig(Fake { weave: vec![opencode(vec![weave_install()])], ..Fake::default() });
    r.answered();
    r.fake.lock().unwrap().refuse_reads = 1;
    r.click("weave-recheck");
    r.wait("the refusal", |s| s.settings.harnesses.weave.error.is_some());
    assert!(r.words().contains("Couldn't ask the harnesses again: HTTP 500"));
    assert!(r.rt.elements().iter().any(|e| e.key.as_deref() == Some("weave-error") && e.role == "alert"));
    assert!(r.words().contains(&format!("{PACKAGE}@0.2.0-next.1")), "the old answer stays");
}

#[test]
fn asks_for_an_update_only_on_the_tab_whose_harness_loads_the_old_weave() {
    let mixed =
        vec![detection("opencode2", "OpenCode 2", vec![v2_install()], None), detection("opencode", "OpenCode", vec![old_legacy()], None)];
    let mut r = rig(Fake { harnesses: vec!["opencode2", "opencode", "pi"], weave: mixed, ..Fake::default() });
    r.answered();
    assert!(!r.has("weave-outdated-legacy"));
    r.tab("opencode");
    assert!(r.has("weave-outdated-legacy"));
}

#[test]
fn holds_add_weave_while_another_harness_s_plugin_list_is_being_changed() {
    let without = vec![
        detection("opencode2", "OpenCode 2", vec![], Some("/cfg/opencode2.json")),
        detection("opencode", "OpenCode", vec![], Some("/cfg/opencode.json")),
    ];
    let mut r = rig(Fake { weave: without, ..Fake::default() });
    r.answered();
    r.rt.update(|s, _| s.settings.harnesses.weave.changing = Some("opencode".into()));
    r.tab("opencode2");
    assert!(r.disabled("weave-add-opencode2"));
    assert!(r.words().contains("Add Weave"));
    assert!(r.disabled("weave-recheck"), "Check again waits for the change too");
}

#[test]
fn asks_for_an_update_when_the_installed_weave_can_t_read_fleet_s_folder() {
    let mut r = rig(Fake { weave: vec![opencode(vec![old_legacy()])], ..Fake::default() });
    r.answered();
    assert!(r.words().contains("Update Weave Legacy to 0.9.0 to use a config kept in Fleet."));
    assert!(r.words().contains("~/.config/opencode/weave-opencode.jsonc"));
}

#[test]
fn says_where_add_weave_writes() {
    let mut r = rig(Fake {
        weave: vec![detection("opencode2", "OpenCode 2", vec![], Some("/cfg/opencode2.json")), opencode(vec![])],
        ..Fake::default()
    });
    r.answered();
    r.tab("opencode2");
    assert!(r.words().contains("Weave isn't in its plugin list. Add Weave puts it in /cfg/opencode2.json."));
    assert!(r.has("weave-add-opencode2"));
    assert!(!r.has("weave-add-opencode"));
}

#[test]
fn adds_weave_to_the_harness_and_says_what_happened() {
    let mut r = rig(Fake {
        weave: vec![detection("opencode2", "OpenCode 2", vec![], Some("/cfg/opencode2.json"))],
        harnesses: vec!["opencode2", "opencode", "pi"],
        ..Fake::default()
    });
    r.answered();
    // After the add Fleet answers with the harness loading Weave.
    let after =
        json!({"source": "own", "files": {}, "apply": null, "harnesses": [detection("opencode2", "OpenCode 2", vec![v2_install()], None)]});
    r.fake.lock().unwrap().plugin = Some((
        200,
        json!({
            "configPath": "/cfg/opencode2.json", "entry": "e", "loaded": true,
            "message": "Added e to /cfg/opencode2.json. OpenCode 2 loaded it.", "config": after,
        }),
    ));
    r.click("weave-add-opencode2");
    r.wait("the change", |s| s.settings.harnesses.weave.notices.contains_key("opencode2"));
    assert_eq!(r.fake.lock().unwrap().plugin_calls, [("POST".to_string(), "/api/weave/harnesses/opencode2/plugin".to_string())]);
    assert_eq!(r.text_of("weave-plugin-notice"), "Added e to /cfg/opencode2.json. OpenCode 2 loaded it.");
    assert!(r.has("weave-remove-opencode2"));
    assert!(!r.has("weave-add-opencode2"));
}

#[test]
fn shows_a_busy_line_and_a_spinner_while_the_harness_downloads_weave() {
    let mut r = rig(Fake {
        weave: vec![detection("opencode2", "OpenCode 2", vec![], Some("/cfg/opencode2.json"))],
        harnesses: vec!["opencode2", "opencode", "pi"],
        hold_plugin: true,
        ..Fake::default()
    });
    r.answered();
    r.click("weave-add-opencode2");
    r.wait("the change to start", |s| s.settings.harnesses.weave.changing.is_some());
    assert!(r.words().contains("Adding\u{2026}"));
    assert!(r.words().contains("The first time, the harness downloads Weave from npm, which can take a minute."));
    assert!(r.disabled("weave-add-opencode2"));
    r.fake.lock().unwrap().hold_plugin = false;
    r.wait("the answer", |s| s.settings.harnesses.weave.changing.is_none());
    assert!(!r.words().contains("which can take a minute"));
}

#[test]
fn shows_why_an_add_was_refused() {
    let mut r = rig(Fake {
        weave: vec![detection("opencode2", "OpenCode 2", vec![], Some("/cfg/opencode2.json"))],
        harnesses: vec!["opencode2", "opencode", "pi"],
        plugin: Some((502, json!({"error": "Fleet couldn't look up @weaveio/weave-adapter-opencode2 on npm."}))),
        ..Fake::default()
    });
    r.answered();
    r.click("weave-add-opencode2");
    r.wait("the refusal", |s| s.settings.harnesses.weave.notices.contains_key("opencode2"));
    assert_eq!(r.text_of("weave-plugin-notice"), "Fleet couldn't look up @weaveio/weave-adapter-opencode2 on npm.");
    assert!(r.rt.elements().iter().any(|e| e.key.as_deref() == Some("weave-plugin-notice") && e.role == "alert"));
    assert!(r.has("weave-add-opencode2"), "the old answer stays, and it can be tried again");
}

#[test]
fn offers_remove_only_for_the_entry_fleet_added() {
    let both = vec![detection("opencode2", "OpenCode 2", vec![v2_install()], None), opencode(vec![weave_install()])];
    let mut r = rig(Fake { harnesses: vec!["opencode2", "opencode", "pi"], weave: both, ..Fake::default() });
    r.answered();
    assert!(r.has("weave-remove-opencode2"));
    r.tab("opencode");
    assert!(!r.has("weave-remove-opencode"));
    assert!(!r.has("weave-add-opencode"));
    r.tab("opencode2");
    let without = json!({"source": "own", "files": {}, "apply": null, "harnesses": [detection("opencode2", "OpenCode 2", vec![], Some("/cfg/o.json"))]});
    r.fake.lock().unwrap().plugin =
        Some((200, json!({"configPath": "/cfg/o.json", "entry": "e", "loaded": true, "message": "Took it out.", "config": without})));
    r.click("weave-remove-opencode2");
    r.wait("the removal", |s| s.settings.harnesses.weave.notices.contains_key("opencode2"));
    assert_eq!(r.fake.lock().unwrap().plugin_calls, [("DELETE".to_string(), "/api/weave/harnesses/opencode2/plugin".to_string())]);
}

#[test]
fn says_why_a_listed_weave_didn_t_load() {
    let broken = json!({"flavor": "weave", "package": "@weaveio/weave-adapter-opencode2", "entry": "@weaveio/weave-adapter-opencode2@9.9.9",
        "acceptsFleetConfig": false, "error": "No matching version found", "addedByFleet": true});
    let mut r = rig(Fake {
        harnesses: vec!["opencode2", "opencode", "pi"],
        weave: vec![detection("opencode2", "OpenCode 2", vec![broken], None)],
        ..Fake::default()
    });
    r.answered();
    assert!(r.words().contains("@weaveio/weave-adapter-opencode2@9.9.9 \u{b7} didn't load: No matching version found"));
    assert!(r.words().contains("Weave \u{b7} didn't load"));
    assert!(!r.has("weave-outdated-weave"), "it didn't load, so its version isn't what's wrong");
}

#[test]
fn says_both_weaves_are_loaded() {
    let legacy =
        json!({"flavor": "legacy", "package": "@opencode_weave/weave", "entry": "@opencode_weave/weave@0.9.1", "acceptsFleetConfig": true});
    let mut r = rig(Fake { weave: vec![opencode(vec![weave_install(), legacy])], ..Fake::default() });
    r.answered();
    assert!(r.has("weave-both"));
    assert!(r.words().contains("Weave + Weave Legacy"));
}

// ─── HarnessesSection.test.ts (the Weave ones) ──────────────────────────────────

#[test]
fn the_old_weave_id_opens_the_default_harness_s_tab_when_weave_is_asked_about_there() {
    let mut r = rig(Fake { weave: vec![opencode(vec![]), detection("opencode2", "OpenCode 2", vec![], None)], ..Fake::default() });
    r.rt.update(|s, _| {
        s.settings.set("weave", None);
    });
    r.wait("the tab", |s| s.settings.harness.is_some());
    assert_eq!(r.rt.state.harness_tab().unwrap().kind, "opencode");
    assert!(r.has("settings-group-weave"));
}

#[test]
fn the_old_weave_id_falls_to_the_first_harness_with_weave_when_the_default_has_none() {
    let mut r = rig(Fake {
        harnesses: vec!["pi", "opencode2", "opencode"],
        weave: vec![detection("opencode2", "OpenCode 2", vec![], None)],
        ..Fake::default()
    });
    r.rt.update(|s, _| {
        s.settings.set("weave", None);
    });
    r.wait("the tab", |s| s.settings.harness.is_some());
    assert_eq!(r.rt.state.harness_tab().unwrap().kind, "opencode2");
}

#[test]
fn asks_the_harnesses_about_weave_once_not_on_every_visit() {
    let mut r = rig(Fake { weave: vec![opencode(vec![])], ..Fake::default() });
    r.answered();
    r.rt.update(|s, _| s.settings.set("general", None));
    r.frames();
    r.rt.update(|s, _| s.settings.set("harnesses", None));
    r.frames();
    r.frames();
    assert_eq!(r.reads(), 1);
    assert!(r.has("settings-group-weave"), "the last answer shows at once");
}

#[test]
fn holds_add_weave_on_every_tab_while_one_harness_s_plugin_list_is_being_changed() {
    let mut r = rig(Fake {
        weave: vec![
            detection("opencode", "OpenCode", vec![], Some("/cfg/a.json")),
            detection("opencode2", "OpenCode 2", vec![], Some("/cfg/b.json")),
        ],
        hold_plugin: true,
        ..Fake::default()
    });
    r.answered();
    r.click("weave-add-opencode");
    r.wait("the change to start", |s| s.settings.harnesses.weave.changing.is_some());
    r.tab("opencode2");
    assert!(r.disabled("weave-add-opencode2"));
    r.fake.lock().unwrap().hold_plugin = false;
    r.wait("the change to end", |s| s.settings.harnesses.weave.changing.is_none());
    for _ in 0..30 {
        r.rt.frame(FRAME);
    }
    assert!(!r.disabled("weave-add-opencode2"));
}

#[test]
fn shows_a_weave_group_only_on_harnesses_weave_supports() {
    let mut r = rig(Fake { weave: vec![opencode(vec![])], ..Fake::default() });
    r.answered();
    assert!(r.has("settings-group-weave"));
    r.tab("pi");
    assert!(!r.has("settings-group-weave"));
}

#[test]
fn a_notice_and_the_answer_belong_to_the_page_not_a_tab() {
    let mut r = rig(Fake {
        harnesses: vec!["opencode2", "opencode", "pi"],
        weave: vec![detection("opencode2", "OpenCode 2", vec![], Some("/c.json"))],
        ..Fake::default()
    });
    r.answered();
    r.click("weave-add-opencode2");
    r.wait("the change", |s| s.settings.harnesses.weave.notices.contains_key("opencode2"));
    r.tab("opencode");
    r.tab("opencode2");
    assert_eq!(r.text_of("weave-plugin-notice"), "Done.");
}

#[test]
fn the_view_reads_what_fleet_sends() {
    let body = json!({"source": "fleet", "files": {"config.weave": "x"}, "apply": {"folders": [{"directory": "/a", "reloaded": false}], "error": null},
        "harnesses": [opencode(vec![weave_install()])]});
    let v = WeaveView::parse(&body).expect("a view");
    assert!(v.fleet_source);
    assert!(v.apply_pending());
    assert_eq!(v.detection("opencode").unwrap().installs[0].package, PACKAGE);
}

// ─── lib/weave-flavors.ts (no test file of its own in Fleet) ────────────────────

mod flavors {
    use super::*;
    use crate::fleet::settings::harness_weave::{
        Detection, Flavor, Tone, added_by_fleet, describe_install, loaded_flavors, outdated_flavors, pill, usable_flavors,
    };

    fn one(v: Value) -> Detection {
        WeaveView::parse(&json!({"files": {}, "harnesses": [v]})).unwrap().harnesses.remove(0)
    }

    #[test]
    fn describes_what_a_harness_loads() {
        let not_checked = json!({"harnessType": "pi", "harnessName": "Pi", "checked": false, "installs": [], "note": "No config for Pi."});
        assert_eq!(describe_install(&one(not_checked)), "No config for Pi.");
        assert_eq!(describe_install(&one(opencode(vec![]))), "Weave isn't in its plugin list.");
        assert_eq!(
            describe_install(&one(detection("o", "O", vec![], Some("/c.json")))),
            "Weave isn't in its plugin list. Add Weave puts it in /c.json."
        );
        let local = json!({"flavor": "weave", "package": "@weaveio/x", "entry": "file:///src/x", "acceptsFleetConfig": true});
        assert_eq!(describe_install(&one(opencode(vec![local]))), "@weaveio/x \u{b7} local build at /src/x");
        let two = opencode(vec![weave_install(), old_legacy()]);
        assert_eq!(describe_install(&one(two)), format!("{PACKAGE}@0.2.0-next.1 \u{b7} @opencode_weave/weave@0.8.2"));
    }

    #[test]
    fn the_pill_by_state() {
        let not_checked = json!({"harnessType": "pi", "harnessName": "Pi", "checked": false, "installs": []});
        assert_eq!(pill(&one(not_checked)), (Tone::Off, "Not yet".into()));
        assert_eq!(pill(&one(opencode(vec![]))), (Tone::Off, "No Weave".into()));
        assert_eq!(pill(&one(opencode(vec![weave_install()]))), (Tone::Ok, "Weave".into()));
        assert_eq!(pill(&one(opencode(vec![old_legacy()]))), (Tone::Warn, "Weave Legacy \u{b7} update".into()));
        assert_eq!(pill(&one(opencode(vec![weave_install(), old_legacy()]))), (Tone::Warn, "Weave + Weave Legacy \u{b7} update".into()));
        let broken = json!({"flavor": "weave", "package": "p", "entry": "p@1", "acceptsFleetConfig": true, "error": "no"});
        assert_eq!(pill(&one(opencode(vec![broken]))), (Tone::Warn, "Weave \u{b7} didn't load".into()));
    }

    #[test]
    fn which_weaves_are_loaded_outdated_and_usable() {
        let all = vec![one(opencode(vec![old_legacy()])), one(detection("opencode2", "OpenCode 2", vec![v2_install()], None))];
        assert_eq!(loaded_flavors(&all), [Flavor::Weave, Flavor::Legacy]);
        assert_eq!(outdated_flavors(&all), [Flavor::Legacy]);
        assert_eq!(usable_flavors(&all), [Flavor::Weave]);
        // One harness loading the new Legacy makes it current for everyone.
        let new_legacy = json!({"flavor": "legacy", "package": "p", "entry": "p@0.9.0", "acceptsFleetConfig": true});
        let mixed = vec![all[0].clone(), one(detection("o3", "O3", vec![new_legacy], None))];
        assert!(outdated_flavors(&mixed).is_empty());
        assert!(added_by_fleet(&all[1]));
        assert!(!added_by_fleet(&all[0]));
    }
}
