//! Component 16, piece H: `HarnessProfilesPanel.test.ts`'s cases marked "B" under their names, against a fake Fleet that
//! answers the harness list, the preferences and the profile requests, and records every write.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::harness_profiles::{Editing, Load};
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
    pub profiles: Vec<Value>,
    /// Profile reads that fail until this many have been refused.
    pub refuse_reads: usize,
    /// Held back: the first read of the list waits until this is false.
    pub hold: bool,
    pub refuse_default: bool,
    /// A create, update or delete is refused with this.
    pub refuse_write: Option<String>,
    /// `presentation.profileNote`.
    pub note: Option<String>,
    /// What a check answers.
    pub check: Option<Value>,
    pub writes: Vec<(String, String, Value)>,
    pub reads: usize,
}
pub(crate) type Shared = Arc<Mutex<Fake>>;

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
    let sent = req.json();
    let (method, path) = (req.method, req.path);
    // A held read waits outside the lock.
    if method == "GET" && path.ends_with("/profiles") {
        while fake.lock().unwrap().hold {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    let mut f = fake.lock().unwrap();
    let (status, reply) = if method == "GET" && path == "/api/harnesses" || path == "/api/harnesses?fresh=true" {
        let list = json!([{
            "type": "opencode2", "displayName": "OpenCode 2", "available": true, "userEnabled": true, "state": "ready",
            "capabilities": {"supportsProfiles": true, "supportsPermissionLevels": true},
            "presentation": {"eyebrow": "CLI harness", "description": "The runtime.", "profileNote": f.note}, "settings": []
        }]);
        ("200 OK", list)
    } else if method == "GET" && path == "/api/preferences" {
        ("200 OK", json!({}))
    } else if method == "GET" && path == "/api/agent-browser/settings" {
        ("200 OK", json!({"enabled": true}))
    } else if method == "GET" && path == "/api/harnesses/opencode2/profiles" {
        f.reads += 1;
        if f.refuse_reads > 0 {
            f.refuse_reads -= 1;
            ("500 Internal Server Error", json!({"error": "no"}))
        } else {
            ("200 OK", Value::Array(f.profiles.clone()))
        }
    } else if path.starts_with("/api/harnesses/opencode2/profiles") {
        f.writes.push((method.clone(), path.clone(), sent.clone()));
        if path.ends_with("/default") {
            if f.refuse_default { ("500 Internal Server Error", json!({"error": "The default is locked."})) } else { ("200 OK", json!({})) }
        } else if path.ends_with("/check") {
            ("200 OK", f.check.clone().unwrap_or(json!({"ok": false, "error": "Unknown key", "details": ["line 2"]})))
        } else if let Some(why) = f.refuse_write.clone() {
            ("400 Bad Request", json!({"error": why}))
        } else if method == "POST" {
            ("200 OK", json!({"id": "made", "name": sent["name"], "content": sent["content"], "isDefault": false, "openSessions": 0}))
        } else if method == "PUT" {
            ("200 OK", json!({"id": "work", "name": sent["name"], "content": sent["content"], "isDefault": true, "openSessions": 1}))
        } else {
            ("204 No Content", json!({}))
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

pub(crate) fn profile(id: &str, name: &str, default: bool, open: u32) -> Value {
    json!({"id": id, "name": name, "content": format!("{{ \"model\": \"{id}/model\" }}"), "isDefault": default, "openSessions": open})
}

pub(crate) struct Rig {
    pub rt: Runtime<State>,
    pub queue: Receiver<Job>,
    pub fake: Shared,
}

pub(crate) fn rig(fake: Fake) -> Rig {
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
        // The invented data has profiles of its own; the machine is what is asked here.
        s.feeds.profiles.clear();
        s.rail = Rail::Settings;
        s.settings.set("harnesses", None);
    });
    let mut r = Rig { rt, queue, fake };
    r.rt.frame(FRAME);
    // The page also reads the preferences; a reply landing later would replace anything a test sets, so wait for it too.
    r.wait("the harness list and the preferences", |s| s.harness_list().all.len() == 1 && s.pickers.prefs.fetched);
    r
}

impl Rig {
    pub fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        let fake = self.fake.clone();
        test_wait::drive(&mut self.rt, &self.queue, what, done, move |s| {
            format!("{:?}; writes {:?}", s.settings.harnesses.profiles, fake.lock().map(|f| f.writes.clone()))
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
    pub fn click(&mut self, key: &str) {
        crate::tests::click(&mut self.rt, key);
        self.frames();
    }
    pub fn profiles(&self) -> Vec<(String, bool)> {
        self.rt.state.harness_profiles("opencode2").into_iter().map(|p| (p.id, p.is_default)).collect()
    }
}

#[test]
fn says_it_s_loading_not_no_profiles_yet_until_the_machine_answers() {
    let mut r = rig(Fake { hold: true, ..Fake::default() });
    r.frames();
    assert!(r.has("harness-profiles-loading"));
    assert!(!r.has("harness-profiles-empty"));
    r.fake.lock().unwrap().hold = false;
    r.wait("the answer", |s| s.profiles_load("opencode2") == Load::Loaded);
    assert!(!r.has("harness-profiles-loading"));
    assert!(r.has("harness-profiles-empty"));
}

#[test]
fn says_a_failed_load_failed_not_that_there_are_no_profiles_and_tries_again() {
    let mut r = rig(Fake { refuse_reads: 1, ..Fake::default() });
    r.wait("the refusal", |s| s.profiles_load("opencode2") == Load::Failed);
    assert!(!r.has("harness-profiles-empty"));
    assert!(r.words().contains("Couldn't load OpenCode 2's profiles."));
    r.click("harness-profiles-retry");
    r.wait("the retry", |s| s.profiles_load("opencode2") == Load::Loaded);
    assert!(!r.has("harness-profiles-load-error"));
    assert!(r.has("harness-profiles-empty"));
    assert_eq!(r.fake.lock().unwrap().reads, 2, "asked once at the start and once for Try again, not on every frame");
}

#[test]
fn explains_profiles_when_there_are_none() {
    let mut r = rig(Fake::default());
    r.wait("the list", |s| s.profiles_load("opencode2") == Load::Loaded);
    assert!(r.words().contains("No profiles yet"));
    assert!(r.words().contains("the new-session box has no profile chip"));
}

#[test]
fn lists_profiles_with_the_default_what_each_changes_and_who_uses_it() {
    let mut r = rig(Fake { profiles: vec![profile("work", "Work", true, 2), profile("local", "Local", false, 0)], ..Fake::default() });
    r.wait("the list", |s| s.profiles_load("opencode2") == Load::Loaded);
    assert!(r.has("harness-profile-default-pill-work"));
    assert_eq!(r.text_of("harness-profile-summary-work"), "work/model");
    assert_eq!(r.text_of("harness-profile-usage-work"), "Used by 2 open sessions");
    assert_eq!(r.text_of("harness-profile-usage-local"), "No open sessions");
    assert!(r.has("harness-profile-row-none"));
    assert!(!r.has("harness-profile-default-none"), "No profile is the default only when no profile is");
}

#[test]
fn makes_a_profile_the_default() {
    let mut r = rig(Fake { profiles: vec![profile("work", "Work", true, 0), profile("local", "Local", false, 0)], ..Fake::default() });
    r.wait("the list", |s| s.profiles_load("opencode2") == Load::Loaded);
    r.click("harness-profile-default-local");
    r.wait("the default", |s| s.settings.harnesses.profiles.notice.is_some());
    assert_eq!(
        r.fake.lock().unwrap().writes[0],
        ("PUT".into(), "/api/harnesses/opencode2/profiles/default".into(), json!({"profileId": "local"}))
    );
    assert_eq!(r.profiles(), [("work".into(), false), ("local".into(), true)]);
    assert_eq!(r.text_of("harness-profile-notice"), "New sessions start with Local.");
}

#[test]
fn clears_the_default_with_no_profile() {
    let mut r = rig(Fake { profiles: vec![profile("work", "Work", true, 0)], ..Fake::default() });
    r.wait("the list", |s| s.profiles_load("opencode2") == Load::Loaded);
    r.click("harness-profile-default-clear");
    r.wait("the answer", |s| s.settings.harnesses.profiles.notice.is_some());
    assert_eq!(r.fake.lock().unwrap().writes[0].2, json!({"profileId": null}));
    assert_eq!(r.profiles(), [("work".into(), false)]);
    assert_eq!(r.text_of("harness-profile-notice"), "New sessions start with no profile.");
    assert!(r.has("harness-profile-default-none"));
}

#[test]
fn says_why_the_default_couldn_t_change_and_leaves_the_list_alone() {
    let mut r = rig(Fake {
        profiles: vec![profile("work", "Work", true, 0), profile("local", "Local", false, 0)],
        refuse_default: true,
        ..Fake::default()
    });
    r.wait("the list", |s| s.profiles_load("opencode2") == Load::Loaded);
    r.click("harness-profile-default-local");
    r.wait("the refusal", |s| s.settings.harnesses.profiles.list_error.is_some());
    assert!(r.words().contains("The default is locked."));
    assert_eq!(r.profiles(), [("work".into(), true), ("local".into(), false)]);
}

#[test]
fn a_profile_made_here_is_the_one_the_new_session_chip_lists() {
    let mut r = rig(Fake { profiles: vec![profile("work", "Work", true, 0)], ..Fake::default() });
    r.wait("the list", |s| s.profiles_load("opencode2") == Load::Loaded);
    r.rt.update(|s, _| s.create_profile("opencode2", "Fresh", "{}", |_, _| {}));
    r.wait("the new profile", |s| s.harness_profiles("opencode2").len() == 2);
    let chip = r.rt.state.feeds.profiles_of("home", "opencode2").iter().map(|p| p.name.clone()).collect::<Vec<_>>();
    assert_eq!(chip, ["Work", "Fresh"]);
}

#[test]
fn an_update_a_delete_and_a_check_follow_the_stores_rules() {
    let mut r = rig(Fake { profiles: vec![profile("work", "Work", true, 0), profile("local", "Local", false, 0)], ..Fake::default() });
    r.wait("the list", |s| s.profiles_load("opencode2") == Load::Loaded);
    r.rt.update(|s, _| s.update_profile("opencode2", "work", "Work 2", "{}", |_, _| {}));
    r.wait("the update", |s| s.harness_profiles("opencode2")[0].open_sessions == 1);
    assert_eq!(r.rt.state.harness_profiles("opencode2")[0].name, "Work 2");
    r.rt.update(|s, _| s.delete_profile("opencode2", "local", |_, _| {}));
    r.wait("the delete", |s| s.harness_profiles("opencode2").len() == 1);
    let seen = Arc::new(Mutex::new(None));
    let out = seen.clone();
    r.rt.update(move |s, _| s.check_profile("opencode2", "{ nope }", move |_, c| *out.lock().unwrap() = Some(c)));
    let waiting = seen.clone();
    r.wait("the check", move |_| waiting.lock().unwrap().is_some());
    let check = seen.lock().unwrap().clone().unwrap().unwrap();
    assert_eq!((check.ok, check.error.as_deref(), check.details.as_deref()), (false, Some("Unknown key"), Some("line 2")));
}

#[test]
fn opens_an_editor_in_the_row_and_new_profile_is_disabled_while_the_new_one_is_open() {
    let mut r = rig(Fake { profiles: vec![profile("work", "Work", true, 0)], ..Fake::default() });
    r.wait("the list", |s| s.profiles_load("opencode2") == Load::Loaded);
    r.click("harness-profile-edit-work");
    assert!(r.has("harness-profile-editor"));
    assert!(!r.has("harness-profile-edit-work"), "the buttons of the row being edited are hidden");
    r.rt.update(|s, _| s.close_profile_editor("opencode2", Some("Saved Work.".into())));
    r.frames();
    assert!(!r.has("harness-profile-editor"));
    assert_eq!(r.text_of("harness-profile-notice"), "Saved Work.");
    r.click("harness-profile-new");
    assert!(r.has("harness-profile-row-new"));
    assert_eq!(r.rt.state.settings.harnesses.profiles.editing, Some(Editing::New(None)));
    assert!(r.rt.state.settings.harnesses.profiles.notice.is_none(), "opening an editor clears the notice");
}
