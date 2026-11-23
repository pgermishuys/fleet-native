//! Which machine a call goes to (component 17, piece D): `machines-open-in-place.test.ts` and `machines-switch.test.ts` under
//! Fleet's names, and the live half of the contract against two small fake Fleets: a call about a session on a machine reaches
//! that machine, with that machine's token, and nothing reaches the other.
use crate::fleet::link::{Link, Post};
use crate::fleet::machines::parse_machine_list;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::mock::{Session, Status};
use crate::tests::app;
use serde_json::json;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MINI: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const LAB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

/// Every request a fake Fleet heard: its first line and the bearer token (or the hub's `access_token`) it carried.
type Heard = Arc<Mutex<Vec<(String, Option<String>)>>>;

fn fake_fleet() -> (String, Heard) {
    let listener = crate::test_http::bind().unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let heard: Heard = Default::default();
    let log = heard.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let text = crate::test_http::read(&mut stream).raw();
            let line = text.lines().next().unwrap_or("").to_string();
            let header = text.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("authorization: bearer ").map(str::to_string));
            let query = line.split("access_token=").nth(1).and_then(|t| t.split([' ', '&']).next()).map(str::to_string);
            log.lock().unwrap().push((line.clone(), header.or(query)));
            let body = if line.contains("/negotiate") { r#"{"connectionToken":"abc","negotiateVersion":1}"# } else { "{}" };
            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(reply.as_bytes());
        }
    });
    (base, heard)
}

fn machine(id: &str, name: &str, base: &str, token: &str) -> serde_json::Value {
    json!({"id": id, "name": name, "baseUrl": base, "token": token})
}

/// An app whose home is `home` and whose machine list is `machines`, read the way `machine_poll` reads it.
fn app_with(home: &str, machines: Vec<serde_json::Value>) -> rune::Runtime<crate::State> {
    let mut rt = app();
    let body = json!({ "machines": machines }).to_string();
    let link = Link::start(home.to_string(), Post::new(|_| {}));
    rt.update(move |s, _| {
        s.link = Some(link);
        s.machines.apply_list(parse_machine_list(200, &body));
    });
    rt
}

fn row(id: &str) -> Session {
    crate::mock::machines::listed(id, "A session", Status::Idle, "now", None, |_| {})
}

fn heard_with(heard: &Heard, what: &str) -> Vec<(String, Option<String>)> {
    heard.lock().unwrap().iter().filter(|(line, _)| line.contains(what)).cloned().collect()
}

fn wait_for(heard: &Heard, what: &str) -> bool {
    let end = Instant::now() + Duration::from_secs(10);
    while Instant::now() < end {
        if !heard_with(heard, what).is_empty() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

const HOME: &str = "http://127.0.0.1:9";

#[test]
fn asks_the_machine_a_session_is_on_the_live_machine_for_its_own_and_unknown_ones() {
    let mut rt = app_with(HOME, vec![machine(MINI, "mini", "http://127.0.0.1:1", "t-mini")]);
    rt.update(|s, _| {
        s.machines.remember_sessions(MINI, &["mini-1".to_string()]);
        s.machines.remember_sessions(HOME_MACHINE_KEY, &["home-1".to_string()]);
        let t = s.target_of("mini-1").unwrap();
        assert_eq!((t.key.as_str(), t.base.as_str()), (MINI, "http://127.0.0.1:1"));
        assert!(s.target_of("home-1").unwrap().is_home());
        assert!(s.target_of("nobody-knows").unwrap().is_home());
        assert_eq!(s.target_of("home-1").unwrap().base, HOME);
        assert_eq!(s.link_for("mini-1").unwrap().key, MINI);
        assert!(s.link_for("home-1").unwrap().is_home());
    });
}

#[test]
fn knows_the_machine_of_a_session_started_or_forked_there_in_place_before_any_list_has_it() {
    let mut rt =
        app_with(HOME, vec![machine(MINI, "mini", "http://127.0.0.1:1", "t-mini"), machine(LAB, "lab", "http://127.0.0.1:2", "t-lab")]);
    rt.update(|s, _| {
        s.machines.remember_sessions(MINI, &["moved".to_string()]);
        s.store.upsert_elsewhere(LAB, row("moved"));
        assert_eq!(s.target_of("moved").unwrap().key, LAB);
    });
}

#[test]
fn asks_the_live_machine_again_for_the_sessions_of_a_machine_thats_forgotten_and_drops_their_rows() {
    let mut rt = app_with(HOME, vec![machine(MINI, "mini", "http://127.0.0.1:1", "t-mini")]);
    rt.update(|s, _| {
        s.machines.remember_sessions(MINI, &["mini-1".to_string()]);
        s.store.upsert_elsewhere(MINI, row("mini-1"));
        // Starting its link, as a call about the session does.
        assert_eq!(s.link_for("mini-1").unwrap().key, MINI);
        assert_eq!(s.links.keys(), [MINI]);
        s.machines.forget(MINI);
        s.store.forget_elsewhere(MINI);
        s.prune_links();
        assert!(s.target_of("mini-1").unwrap().is_home());
        assert!(s.links.keys().is_empty(), "its link goes with it");
        assert_eq!(s.store.machine_of("mini-1"), None);
    });
}

#[test]
fn gives_a_target_for_any_listed_machine_the_live_one_for_one_that_isnt() {
    let mut rt = app_with(HOME, vec![machine(MINI, "mini", "http://127.0.0.1:1", "t-mini")]);
    rt.update(|s, _| {
        assert_eq!(s.target_of_key(MINI).unwrap().base, "http://127.0.0.1:1");
        assert!(s.target_of_key(HOME_MACHINE_KEY).unwrap().is_home());
        assert!(s.target_of_key("dddddddddddddddddddddddddddddddd").unwrap().is_home());
    });
}

#[test]
fn has_no_target_without_a_fleet() {
    let rt = app();
    assert!(rt.state.target_of("x").is_none() && rt.state.link_for("x").is_none() && rt.state.open_link().is_none());
}

#[test]
fn keeps_a_machines_link_between_calls_and_starts_a_new_one_when_its_address_moves() {
    let mut rt = app_with(HOME, vec![machine(MINI, "mini", "http://127.0.0.1:1", "t-mini")]);
    rt.update(|s, _| {
        s.machines.remember_sessions(MINI, &["mini-1".to_string()]);
        assert_eq!(s.link_for("mini-1").unwrap().base, "http://127.0.0.1:1");
        assert_eq!(s.links.keys().len(), 1);
        let body = json!({ "machines": [machine(MINI, "mini", "http://127.0.0.1:3", "t-mini")] }).to_string();
        s.machines.apply_list(parse_machine_list(200, &body));
        assert_eq!(s.link_for("mini-1").unwrap().base, "http://127.0.0.1:3");
        assert_eq!(s.links.keys().len(), 1);
    });
}

#[test]
fn archives_and_stops_a_session_opened_in_place_on_its_own_machine_from_outside_its_views() {
    // Home's token is the process's: take it from the tests that set one, and start with none.
    let _held = super::auth::HOME_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    crate::fleet::auth::set(None);
    let (home, at_home) = fake_fleet();
    let (mini, at_mini) = fake_fleet();
    let (lab, at_lab) = fake_fleet();
    let mut rt = app_with(&home, vec![machine(MINI, "mini", &mini, "token-of-mini"), machine(LAB, "lab", &lab, "token-of-lab")]);
    rt.update(|s, _| {
        s.machines.remember_sessions(MINI, &["mini-1".to_string()]);
        s.machines.remember_sessions(LAB, &["lab-1".to_string()]);
        s.link_for("mini-1").unwrap().abort("mini-1");
        s.link_for("lab-1").unwrap().abort("lab-1");
        s.link_for("home-1").unwrap().abort("home-1");
    });
    assert!(wait_for(&at_mini, "/api/sessions/mini-1/abort"));
    assert!(wait_for(&at_lab, "/api/sessions/lab-1/abort"));
    assert!(wait_for(&at_home, "/api/sessions/home-1/abort"));
    // Each reached only its own machine, with that machine's token.
    assert_eq!(
        heard_with(&at_mini, "/abort"),
        [("POST /api/sessions/mini-1/abort HTTP/1.1".to_string(), Some("token-of-mini".to_string()))]
    );
    assert_eq!(heard_with(&at_lab, "/abort"), [("POST /api/sessions/lab-1/abort HTTP/1.1".to_string(), Some("token-of-lab".to_string()))]);
    assert!(heard_with(&at_home, "/abort").iter().all(|(l, t)| l.contains("home-1") && t.is_none()));
}

#[test]
fn a_prompt_and_the_hub_of_a_session_go_to_its_machine_with_its_token() {
    let (home, at_home) = fake_fleet();
    let (mini, at_mini) = fake_fleet();
    let mut rt = app_with(&home, vec![machine(MINI, "mini", &mini, "token-of-mini")]);
    rt.update(|s, _| {
        s.machines.remember_sessions(MINI, &["mini-1".to_string()]);
        // The conversation's own hub, and a prompt sent into it.
        s.open_on_link("mini-1");
        s.link_for("mini-1").unwrap().send_prompt("mini-1", "c-1", "hello", Default::default());
    });
    assert!(wait_for(&at_mini, "/api/sessions/mini-1/prompt"), "{:?}", at_mini.lock().unwrap());
    assert!(wait_for(&at_mini, "/hubs/session-events/negotiate"));
    for (line, token) in at_mini.lock().unwrap().iter().filter(|(l, _)| !l.is_empty()) {
        assert_eq!(token.as_deref(), Some("token-of-mini"), "{line}");
    }
    assert!(heard_with(&at_home, "mini-1").is_empty(), "nothing about the machine's session reaches home");
    // Home's own hub is its own business: it never carried the machine's token.
    assert!(at_home.lock().unwrap().iter().all(|(_, t)| t.as_deref() != Some("token-of-mini")));
}

/// The live check, by hand: `FLEET_NATIVE_LIVE_HOME=<home's address>` where home lists another scratch Fleet as a machine.
/// Reads the list as the app does, starts a link for the machine's first session, listens to its hub and sends it a prompt.
/// Run with `cargo test live_two_fleets -- --ignored --nocapture`; it prints what each hop did, never a token.
#[test]
#[ignore = "needs two scratch Fleets (docs/FLEET.md)"]
fn live_two_fleets() {
    use crate::fleet::link::{Connection, Job};
    use std::sync::mpsc::channel;
    let home = std::env::var("FLEET_NATIVE_LIVE_HOME").expect("FLEET_NATIVE_LIVE_HOME");
    crate::fleet::auth::bind_home(&home);
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    let link = Link::start(home.clone(), post);
    rt.update(move |s, _| s.link = Some(link));
    let pump = |rt: &mut rune::Runtime<crate::State>, until: &dyn Fn(&crate::State) -> bool| {
        let end = Instant::now() + Duration::from_secs(30);
        while Instant::now() < end {
            while let Ok(job) = queue.try_recv() {
                rt.update(job);
            }
            rt.frame(1.0 / 60.0);
            if until(&rt.state) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    };
    assert!(pump(&mut rt, &|s| s.machines.others.values().any(|m| !m.sessions.is_empty())), "the machine was never read");
    let (key, id, name) = {
        let s = &rt.state;
        let (key, m) = s.machines.others.iter().find(|(_, m)| !m.sessions.is_empty()).unwrap();
        (key.clone(), m.sessions[0].id.clone(), s.machines.machine(key).unwrap().name.clone())
    };
    println!("machine {name} lists session {id}");
    let k = key.clone();
    rt.update(move |s, _| {
        let t = s.target_of(&id).unwrap();
        println!("target of the session: {} at {}", if t.is_home() { "home" } else { "machine" }, t.base);
        assert_eq!(t.key, k);
        s.open_on_link(&id);
        s.link_for(&id).unwrap().send_prompt(&id, "live-1", "run-tests", Default::default());
    });
    assert!(pump(&mut rt, &|s| s.machines.hubs.get(&key) == Some(&Connection::Online)), "the machine's hub never came up");
    println!("the machine's hub is online; home's connection is {:?}", rt.state.connection);
    assert_eq!(rt.state.connection, Connection::Online);
}

#[test]
fn sends_another_machines_row_actions_to_that_machine_and_lists_that_machine_again() {
    let (home, at_home) = fake_fleet();
    let (mini, at_mini) = fake_fleet();
    let mut rt = app_with(&home, vec![machine(MINI, "mini", &mini, "token-of-mini")]);
    rt.update(|s, tx| {
        let mut one = row("mini-1");
        one.title = "Water the tomatoes".into();
        s.machines.upsert_session(MINI, one);
        s.machines.others.get_mut(MINI).unwrap().projects = vec![crate::fleet::groups::ProjectInfo {
            id: "p-garden".into(),
            name: "Garden".into(),
            kind: crate::fleet::groups::ProjectKind::User,
            position: 1,
        }];
        // Rename a session, then a project.
        s.start_machine_row_rename(MINI, "mini-1", tx);
        s.act.edit = rune::TextEdit::new("Feed the tomatoes".to_string());
        s.finish_row_rename(true, tx);
        s.start_machine_rename(MINI, "p-garden", tx);
        s.proj.edit = rune::TextEdit::new("Vegetable garden".to_string());
        s.finish_rename(true, tx);
    });
    assert!(wait_for(&at_mini, "PATCH /api/sessions/mini-1 "), "{:?}", at_mini.lock().unwrap());
    assert!(wait_for(&at_mini, "PATCH /api/projects/p-garden"), "{:?}", at_mini.lock().unwrap());
    // Both were read again on that machine, with its token: its sessions, projects and identity.
    // Then that machine's own list is read again (what the call's answer does), not home's.
    rt.update(|s, _| s.list_again_on(MINI));
    assert!(wait_for(&at_mini, "GET /api/sessions?"));
    for (line, token) in at_mini.lock().unwrap().iter().filter(|(l, _)| l.starts_with("PATCH") || l.starts_with("GET /api/sessions?")) {
        assert_eq!(token.as_deref(), Some("token-of-mini"), "{line}");
    }
    assert!(heard_with(&at_home, "mini-1").is_empty() && heard_with(&at_home, "p-garden").is_empty(), "nothing reaches home");
}

// session-calls-machine.test.ts: "reads the side conversation" (component 15): every side route goes to the session's own
// machine with that machine's token, and nothing about it reaches home.
#[test]
fn reads_the_side_conversation_on_the_sessions_machine() {
    let (home, at_home) = fake_fleet();
    let (mini, at_mini) = fake_fleet();
    let mut rt = app_with(&home, vec![machine(MINI, "mini", &mini, "token-of-mini")]);
    rt.update(|s, tx| {
        s.machines.remember_sessions(MINI, &["mini-1".to_string()]);
        // The load, a question, folding it, seeing an answer, discarding it and Undo.
        s.load_side("mini-1");
        s.ask_side("mini-1", "what changed?", Default::default(), tx, |_, _| {});
        s.side_state("mini-1").side = crate::fleet::side::Side::from_json(&json!({"sessionId": "side-1", "minimized": false}));
        s.set_side_minimized("mini-1", true, tx);
        s.mark_side_seen("mini-1", "a1");
        s.discard_side("mini-1");
        s.undo_side_discard("mini-1");
    });
    for route in [
        "GET /api/sessions/mini-1/side ",
        "POST /api/sessions/mini-1/side ",
        "PUT /api/sessions/mini-1/side/minimized",
        "PUT /api/sessions/mini-1/side/seen",
        "DELETE /api/sessions/mini-1/side",
    ] {
        assert!(wait_for(&at_mini, route), "{route}: {:?}", at_mini.lock().unwrap());
    }
    for (line, token) in at_mini.lock().unwrap().iter().filter(|(l, _)| l.contains("/side")) {
        assert_eq!(token.as_deref(), Some("token-of-mini"), "{line}");
    }
    assert!(heard_with(&at_home, "/side").is_empty(), "nothing about the machine's side conversation reaches home");
}
