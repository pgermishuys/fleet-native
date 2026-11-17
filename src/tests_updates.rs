//! Component 16, piece U: the calls behind Settings → Updates & about against a fake Fleet. What the page reads and from which
//! machine, the rollout (start, refusal, a pushed `update.rollout`), the restart of this Fleet and the wait for it to come
//! back, and the two rule preferences. The words and the stores are tested without a server in `fleet::updates`.
//! Nothing here updates a real Fleet or harness: the fake answers every call.
use crate::fleet::events::{Event, Kind};
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::state::Topic;
use crate::tests::app;
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fake {
    /// Every call heard: method, path and body.
    heard: Vec<(String, String, String)>,
    machines: Vec<Value>,
    version: String,
    refuse_rollout: Option<(u16, String)>,
    /// `GET /api/machine` fails with a 503 this many times first (the Fleet is restarting).
    down_for: u32,
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

fn rollout_json(status: &str) -> Value {
    json!({ "id": "r1", "status": status, "startedAt": "2026-10-20T09:00:00Z", "steps": [
        { "machineId": "m-harbor", "name": "harbor", "state": "waiting", "waitingSessions": 2, "toVersion": "0.59.1" }] })
}

fn answer(mut stream: std::net::TcpStream, fake: Shared) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let (mut head_end, mut length) = (None, 0usize);
    loop {
        let n = stream.read(&mut chunk).unwrap_or(0);
        buf.extend_from_slice(&chunk[..n]);
        if head_end.is_none() {
            head_end = buf.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4);
            if let Some(end) = head_end {
                let head = String::from_utf8_lossy(&buf[..end]).to_lowercase();
                length = head.lines().find_map(|l| l.strip_prefix("content-length:")?.trim().parse().ok()).unwrap_or(0);
            }
        }
        if n == 0 || head_end.is_some_and(|end| buf.len() >= end + length) {
            break;
        }
    }
    let end = head_end.unwrap_or(buf.len());
    let head = String::from_utf8_lossy(&buf[..end]).to_string();
    let body = String::from_utf8_lossy(&buf[end..]).to_string();
    let mut first = head.lines().next().unwrap_or("").split(' ');
    let (method, path) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
    let mut f = fake.lock().unwrap();
    f.heard.push((method.clone(), path.clone(), body));
    let (status, reply) = match (method.as_str(), path.as_str()) {
        ("GET", "/api/machines") => (200, json!({ "machines": f.machines }).to_string()),
        ("GET", "/api/machine") if f.down_for > 0 => {
            f.down_for -= 1;
            (503, "{}".to_string())
        }
        ("GET", "/api/update/status") => (200, json!({ "currentVersion": f.version, "status": "staged", "latestVersion": "0.59.1" }).to_string()),
        ("GET", "/api/update/releases") => {
            (200, json!({ "releases": [{ "version": "0.59.1", "publishedAt": null, "body": "x", "url": "u" }], "fetchedAt": null, "error": null }).to_string())
        }
        ("GET", "/api/update/rollout") => (200, rollout_json("running").to_string()),
        ("POST", "/api/update/rollout") => f.refuse_rollout.clone().unwrap_or((200, rollout_json("running").to_string())),
        _ => (200, "{}".to_string()),
    };
    let out = format!(
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(out.as_bytes());
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    home: Shared,
}

fn link_to(base: String) -> (Link, Receiver<Job>) {
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    (Link::start(base, Post::new(move |job| drop(jobs.lock().unwrap().send(job)))), queue)
}

/// A connected app on Updates & about, with `other` listed as a second machine when given.
fn rig(version: &str, other: Option<(&Shared, &str)>, pick_other: bool) -> Rig {
    let home: Shared = Default::default();
    home.lock().unwrap().version = version.into();
    if let Some((_, base)) = other {
        home.lock().unwrap().machines = vec![json!({ "id": "m-harbor", "name": "harbor", "baseUrl": base, "token": "t-harbor" })];
    }
    let (link, queue) = link_to(serve(home.clone()));
    let mut rt = app();
    let base = other.map(|(_, b)| b.to_string());
    rt.update(move |s, _| {
        s.link = Some(link);
        if let Some(base) = base {
            s.show_settings_machines(true);
            let machine = s.machines.list.iter_mut().find(|m| m.id == "m-harbor").expect("harbor");
            machine.base_url = base;
            machine.addresses.clear();
        }
        s.rail = Rail::Settings;
        s.open_settings("updates", None);
        if pick_other {
            s.settings.choice.pick("m-harbor");
        }
    });
    Rig { rt, queue, home }
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("{:?}", s.updates));
    }
    fn heard(&self, fake: &Shared) -> Vec<(String, String, String)> {
        fake.lock().unwrap().heard.clone()
    }
    fn did(&self, fake: &Shared, method: &str, path: &str) -> bool {
        self.heard(fake).iter().any(|(m, p, _)| m == method && p == path)
    }
}

#[test]
fn reads_the_status_the_notes_and_the_rollout_of_the_machine_settings_shows_once_the_page_opens() {
    let mut r = rig("0.59.0", None, false);
    r.wait("the page to be read", |s| {
        s.updates.status.get("home").is_some_and(|x| x.status.is_some())
            && s.updates.releases.get("home").is_some_and(|x| x.loaded)
            && s.updates.rollout.is_some()
    });
    let status = r.rt.state.updates.status["home"].status.clone().expect("status");
    assert!(status.is_staged() && status.current_version == "0.59.0");
    assert_eq!(r.rt.state.updates.releases["home"].releases.len(), 1);
    assert!(r.rt.state.updates.rollout.as_ref().is_some_and(|x| x.running()));
    // Asking again is for the page's own timers: the notes are read once.
    let before = r.heard(&r.home).iter().filter(|(_, p, _)| p == "/api/update/releases").count();
    for _ in 0..30 {
        r.rt.frame(1.0 / 60.0);
    }
    assert_eq!(r.heard(&r.home).iter().filter(|(_, p, _)| p == "/api/update/releases").count(), before);
}

#[test]
fn asks_another_machines_status_and_notes_when_settings_shows_it_never_home() {
    let other: Shared = Default::default();
    other.lock().unwrap().version = "0.58.0".into();
    let other_base = serve(other.clone());
    let mut r = rig("0.59.0", Some((&other, &other_base)), true);
    assert_eq!(r.rt.state.settings.choice.key(&r.rt.state.machines), "m-harbor");
    // The rollout read is home's and goes out on its own request, so wait for it too: asserting it was heard
    // right after harbor's answers raced it on a slow runner.
    r.wait("harbor's status and notes, and home's rollout", |s| {
        s.updates.status.get("m-harbor").is_some_and(|x| x.status.is_some())
            && s.updates.releases.get("m-harbor").is_some_and(|x| x.loaded)
            && s.updates.rollout.is_some()
    });
    assert_eq!(r.rt.state.updates.status["m-harbor"].status.as_ref().map(|s| s.current_version.as_str()), Some("0.58.0"));
    assert!(r.did(&other, "GET", "/api/update/status") && r.did(&other, "GET", "/api/update/releases"));
    assert!(!r.did(&r.home, "GET", "/api/update/status") && !r.did(&r.home, "GET", "/api/update/releases"), "home was not asked");
    // The rollout is always home's, whichever machine the switch names.
    assert!(r.did(&r.home, "GET", "/api/update/rollout") && !r.did(&other, "GET", "/api/update/rollout"));
}

#[test]
fn update_machines_now_posts_the_rollout_to_home_and_follows_what_the_hub_pushes() {
    let mut r = rig("0.59.0", None, false);
    r.rt.update(|s, _| s.start_rollout(Vec::new()));
    r.wait("the rollout post", |s| s.updates.rollout.is_some());
    let posts: Vec<_> = r.heard(&r.home).into_iter().filter(|(m, p, _)| m == "POST" && p == "/api/update/rollout").collect();
    assert_eq!(posts.len(), 1);
    assert_eq!(serde_json::from_str::<Value>(&posts[0].2).unwrap(), json!({ "machineIds": [] }));
    // A push replaces it whole.
    let mut done = rollout_json("done");
    done["steps"][0]["state"] = json!("done");
    r.rt.update(move |s, _| s.dispatch(&Topic::Sessions, &[Event::Other(Kind::UpdateRollout, done)]));
    let got = r.rt.state.updates.rollout.clone().expect("rollout");
    assert!(!got.running() && got.steps[0].state == crate::fleet::updates::StepState::Done);
    // On another topic it isn't ours.
    r.rt.update(|s, _| s.dispatch(&Topic::Session("x".into()), &[Event::Other(Kind::UpdateRollout, rollout_json("running"))]));
    assert!(!r.rt.state.updates.rollout.as_ref().unwrap().running());
}

#[test]
fn a_restart_of_one_other_machine_is_a_rollout_of_that_machine_alone() {
    let mut r = rig("0.59.0", None, false);
    r.rt.update(|s, _| s.restart_machine("m-harbor", None, None));
    r.wait("the post", |s| s.updates.rollout.is_some());
    let body = r.heard(&r.home).into_iter().find(|(m, p, _)| m == "POST" && p == "/api/update/rollout").expect("a post").2;
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap(), json!({ "machineIds": ["m-harbor"] }));
}

#[test]
fn shows_the_servers_words_when_it_refuses_a_rollout() {
    let mut r = rig("0.59.0", None, false);
    r.home.lock().unwrap().refuse_rollout = Some((409, r#"{"error":"A rollout is already running."}"#.into()));
    r.rt.update(|s, _| s.start_rollout(Vec::new()));
    r.wait("the refusal", |s| s.updates.action_error.is_some());
    assert_eq!(r.rt.state.updates.action_error.as_deref(), Some("A rollout is already running."));
}

#[test]
fn restart_on_home_posts_apply_and_waits_for_it_to_go_down_and_come_back() {
    let mut r = rig("0.59.0", None, false);
    r.home.lock().unwrap().down_for = 2;
    r.rt.update(|s, _| s.restart_machine("home", Some("0.59.0".into()), Some("0.59.1".into())));
    r.wait("apply to be sent", |s| s.updates.restarting.is_some());
    // `restarting` is set before the request leaves; wait until home has heard it.
    let home = r.home.clone();
    r.wait("home to hear the apply", move |_| home.lock().unwrap().heard.iter().any(|(m, p, _)| m == "POST" && p == "/api/update/apply"));
    // Down twice (503), then up: the page lets go of the wait and reads everything again.
    r.wait("the Fleet to come back", |s| s.updates.restarting.is_none() && s.updates.action_error.is_none());
    let asked = r.heard(&r.home).iter().filter(|(m, p, _)| m == "GET" && p == "/api/machine").count();
    assert!(asked >= 3, "asked {asked} times");
}

#[test]
fn a_rule_choice_writes_fleet_update_rule_and_a_machines_override_writes_the_overrides_and_default_removes_it() {
    let mut r = rig("0.59.0", None, false);
    r.rt.update(|s, _| s.choose_update_rule(Some(crate::fleet::updates::UpdateRule::Overnight)));
    r.wait("the PUT", |s| s.pickers.prefs.get("FleetUpdateRule") == Some("overnight"));
    r.rt.update(|s, _| s.override_update_rule("m-harbor", Some(crate::fleet::updates::UpdateRule::Ask)));
    assert_eq!(r.rt.state.pickers.prefs.get("FleetUpdateRules"), Some(r#"{"m-harbor":"ask"}"#));
    r.rt.update(|s, _| s.override_update_rule("m-harbor", None));
    assert_eq!(r.rt.state.pickers.prefs.get("FleetUpdateRules"), Some("{}"));
    r.rt.update(|s, _| s.choose_update_rule(None));
    assert_eq!(r.rt.state.pickers.prefs.get("FleetUpdateRule"), Some(""));
    // Each write reached home as a PUT of the key.
    let home = r.home.clone();
    r.wait("the writes to arrive", move |_| {
        home.lock().unwrap().heard.iter().filter(|(m, p, _)| m == "PUT" && p.starts_with("/api/preferences/FleetUpdateRule")).count() >= 4
    });
}
