//! Piece N: the harness setup wizard (`HarnessSetupWizard.test.ts`, rows C12 and C17.12-C17.15), against a fake Fleet that
//! answers the harness list and the preferences and records every `PUT /api/preferences/{key}`. The cases keep Fleet's names.
use crate::fleet::harness_setup::Step;
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::new_session::harness_choice::Load;
use crate::tests::{FRAME, app, click};
use crate::{State, test_wait};
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fake {
    harnesses: Vec<Value>,
    prefs: HashMap<String, String>,
    puts: Vec<(String, String)>,
}

type Shared = Arc<Mutex<Fake>>;

fn harness(kind: &str, name: &str, state: &str, enabled: bool) -> Value {
    let ready = state == "ready";
    json!({ "type": kind, "displayName": name, "available": ready, "userEnabled": enabled, "state": state,
        "reason": if ready { Value::Null } else { json!(format!("{name} isn't installed.")) }, "version": if ready { json!("1.18.30") } else { Value::Null },
        "isDefault": kind == "opencode2" })
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
    let (method, path) = (req.method.as_str(), req.path.as_str());
    let body = req.body;
    let mut f = fake.lock().unwrap();
    let reply = if method == "GET" && path.starts_with("/api/harnesses") {
        Value::Array(f.harnesses.clone())
    } else if method == "GET" && path == "/api/preferences" {
        json!(f.prefs)
    } else if method == "PUT" && path.starts_with("/api/preferences/") {
        let key = path.trim_start_matches("/api/preferences/").replace("%2E", ".");
        let value = serde_json::from_slice::<Value>(&body).ok().and_then(|v| v["value"].as_str().map(str::to_string)).unwrap_or_default();
        f.prefs.insert(key.clone(), value.clone());
        f.puts.push((key, value));
        json!({})
    } else {
        json!({})
    };
    let reply = reply.to_string();
    let out =
        format!("HTTP/1.1 200 X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}", reply.len());
    let _ = stream.write_all(out.as_bytes());
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Shared,
}

/// A Fleet with these harnesses and preferences, connected; nothing has been read yet.
fn rig(harnesses: Vec<Value>, prefs: &[(&str, &str)]) -> Rig {
    let fake: Shared = Arc::new(Mutex::new(Fake {
        harnesses,
        prefs: prefs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        puts: Vec::new(),
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
        // A real launch starts with nothing read: the invented lists and preferences the headless app shows are cleared.
        s.feeds.harnesses.clear();
        s.pickers.prefs = Default::default();
    });
    Rig { rt, queue, fake }
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("wizard {:?}", s.harness_setup));
    }

    /// The app reads the harness list and the preferences when it connects.
    fn connect(&mut self) {
        self.rt.update(|s, _| s.load_pickers());
        self.wait("the harnesses and the preferences", |s| s.pickers.prefs.fetched && s.setup_harnesses().load == Load::Done);
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    /// Waits until Fleet has been sent `PUT /api/preferences/{key}` with `value`.
    fn wait_put(&mut self, key: &str, value: &str) {
        let fake = self.fake.clone();
        let (key, value) = (key.to_string(), value.to_string());
        self.wait(&format!("{key} = {value} to reach Fleet"), move |_| {
            fake.lock().unwrap().puts.iter().any(|(k, v)| *k == key && *v == value)
        });
    }

    /// What the element's text says, joined.
    fn words(&self, key: &str) -> String {
        let all = self.rt.elements();
        let it = all.iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}"));
        all.iter().filter(|e| e.path.starts_with(&it.path)).filter_map(|e| e.text.clone()).collect::<String>()
    }

    /// The disabled button is drawn at half strength.
    fn continue_on(&self) -> bool {
        let all = self.rt.elements();
        all.iter().find(|e| e.key.as_deref() == Some("harness-setup-continue")).expect("Continue").opacity > 0.9
    }

    fn puts(&self) -> Vec<(String, String)> {
        self.fake.lock().unwrap().puts.clone()
    }

    fn is_open(&self) -> bool {
        self.rt.state.harness_setup.is_open
    }

    fn step(&self) -> Step {
        self.rt.state.harness_setup.step
    }

    fn set_harnesses(&self, harnesses: Vec<Value>) {
        self.fake.lock().unwrap().harnesses = harnesses;
    }
}

fn none_ready() -> Vec<Value> {
    vec![harness("opencode2", "OpenCode 2", "not-installed", true), harness("claude-code", "Claude Code", "not-installed", true)]
}

#[test]
fn opens_on_the_first_launch_when_no_harness_is_ready() {
    let mut r = rig(none_ready(), &[]);
    r.connect();
    assert!(r.is_open());
    assert_eq!(r.step(), Step::Welcome);
    assert!(r.rt.find("harness-setup-wizard").is_some());
    assert!(r.rt.find("harness-setup-get-started").is_some());
}

#[test]
fn stays_closed_once_setup_was_finished_or_skipped_in_cloud_mode_and_when_a_harness_is_ready() {
    // Finished or skipped before.
    let mut r = rig(none_ready(), &[("harnessSetup.done", "true")]);
    r.connect();
    assert!(!r.is_open());
    // Cloud mode.
    let mut r = rig(none_ready(), &[]);
    r.rt.update(|s, _| s.harness_setup.cloud = true);
    r.connect();
    assert!(!r.is_open());
    // A harness is ready.
    let mut r = rig(vec![harness("opencode2", "OpenCode 2", "ready", true)], &[]);
    r.connect();
    assert!(!r.is_open());
    assert!(r.rt.find("harness-setup-wizard").is_none());
}

#[test]
fn skip_setup_closes_it_and_remembers_so_it_doesnt_open_again() {
    let mut r = rig(none_ready(), &[]);
    r.connect();
    click(&mut r.rt, "harness-setup-skip");
    assert!(!r.is_open());
    r.wait_put("harnessSetup.done", "true");
    assert_eq!(r.puts(), vec![("harnessSetup.done".to_string(), "true".to_string())]);
    assert!(r.rt.state.harness_setup_done());
    // A later read of the same lists doesn't bring it back, and a new launch (the flag is on the server) stays closed too.
    r.rt.update(|s, _| s.load_pickers());
    r.wait("the lists again", |s| s.harness_setup_done());
    assert!(!r.is_open());
    let mut again = rig(none_ready(), &[("harnessSetup.done", "true")]);
    again.connect();
    assert!(!again.is_open());
}

#[test]
fn goes_on_only_once_a_harness_is_ready_then_starts_a_session() {
    let mut r = rig(none_ready(), &[]);
    r.connect();
    click(&mut r.rt, "harness-setup-get-started");
    assert_eq!(r.step(), Step::Harnesses);
    // Nothing is ready: Continue is off and the left button says "Skip for now".
    assert!(!r.continue_on());
    click(&mut r.rt, "harness-setup-continue");
    assert_eq!(r.step(), Step::Harnesses);
    assert_eq!(r.words("harness-setup-skip"), "Skip for now");
    // The user installs one; the list is asked again and Continue follows it. The harness is installed but turned off.
    r.set_harnesses(vec![harness("opencode2", "OpenCode 2", "ready", false), harness("claude-code", "Claude Code", "not-installed", true)]);
    r.rt.update(|s, _| s.read_harnesses(crate::fleet::settings::HOME_MACHINE_KEY, true));
    r.wait("the new list", |s| s.setup_harnesses().all.iter().any(|h| h.is_ready()));
    for _ in 0..30 {
        r.rt.frame(FRAME);
    }
    assert!(r.continue_on());
    assert_eq!(r.words("harness-setup-skip"), "Close");
    click(&mut r.rt, "harness-setup-continue");
    assert_eq!(r.step(), Step::Ready);
    // Setting it up here means wanting it on; the done flag waits for leaving.
    r.wait_put("opencode2.enabled", "true");
    assert!(!r.puts().iter().any(|(k, _)| k == "harnessSetup.done"));
    assert!(r.rt.state.setup_harnesses().all.iter().any(|h| h.display_name == "OpenCode 2"));
    click(&mut r.rt, "harness-setup-start-session");
    assert!(!r.is_open());
    assert!(r.rt.state.on_new_session_page());
    r.wait_put("harnessSetup.done", "true");
}

#[test]
fn opens_at_the_harness_step_from_the_banner_the_new_session_box_or_settings() {
    // Every entry point is `open_harness_setup`; the new-session box's fix is the one built so far (the dashboard banner is
    // component 23's, Settings component 16's). It ignores the done flag and opens at the harnesses.
    let mut r = rig(none_ready(), &[("harnessSetup.done", "true")]);
    r.connect();
    assert!(!r.is_open());
    click(&mut r.rt, "new-session");
    r.rt.frame(FRAME);
    click(&mut r.rt, "new-session-no-harness-fix");
    assert!(r.is_open());
    assert_eq!(r.step(), Step::Harnesses);
    assert!(r.rt.find("harness-setup-continue").is_some());
}

#[test]
fn click_outside_and_escape_do_nothing() {
    let mut r = rig(none_ready(), &[]);
    r.connect();
    r.rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    r.rt.frame(FRAME);
    assert!(r.is_open());
    r.rt.pointer_down(5.0, 5.0);
    r.rt.pointer_up(5.0, 5.0);
    r.rt.frame(FRAME);
    assert!(r.is_open());
    assert!(r.puts().is_empty());
}

#[test]
fn the_ready_line_names_each_ready_harness_with_its_version() {
    use crate::fleet::harness_setup::ready_line;
    use crate::fleet::new_session::harness_choice::{HarnessEntry, Harnesses};
    let one = HarnessEntry { display_name: "OpenCode".into(), version: Some("1.18.30".into()), ..HarnessEntry::ready("opencode") };
    let two = HarnessEntry { display_name: "Claude Code".into(), ..HarnessEntry::ready("claude-code") };
    let list = |all| Harnesses { all, ..Harnesses::default() };
    assert_eq!(
        ready_line(&list(vec![one.clone()])),
        "OpenCode 1.18.30 is ready. Add or update harnesses any time in Settings → Harnesses."
    );
    assert_eq!(
        ready_line(&list(vec![one, two])),
        "OpenCode 1.18.30 and Claude Code are ready. Add or update harnesses any time in Settings → Harnesses."
    );
}
