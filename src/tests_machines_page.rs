//! Settings → Machines (piece J of docs/parity/17-machines.md): the cases of `MachinesSection.test.ts` and `stores/machines.test.ts`
//! under Fleet's names, driven with no window. The machines are the invented ones of `mock::machines`; the cases that write use a
//! fake Fleet that records each request (method, path, body, token).
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::machine_actions::{Access, AccessState};
use crate::fleet::machines::AgentWork;
use crate::tests::{FRAME, app, click};
use crate::{Rail, State};
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::{Value, json};
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

pub(crate) type Rt = Runtime<State>;
type Heard = Arc<Mutex<Vec<(String, String, Value, Option<String>)>>>;

pub(crate) fn page() -> Rt {
    let mut rt = app();
    rt.update(|s, _| s.show_machines_page(true, false));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

/// The page with only falcon beside home, so the access block fits on the screen to be clicked.
pub(crate) fn short_page() -> Rt {
    let mut rt = page();
    rt.update(|s, _| s.machines.list.retain(|m| m.id == "m-falcon"));
    rt.frame(FRAME);
    rt
}

pub(crate) fn words(rt: &Rt, key: &str) -> String {
    let all = rt.elements();
    let it = all.iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    let own = it.text.clone().unwrap_or_default();
    own + &all.iter().filter(|e| e.path.starts_with(&it.path) && e.path != it.path).filter_map(|e| e.text.clone()).collect::<String>()
}

pub(crate) fn exists(rt: &Rt, key: &str) -> bool {
    rt.find(key).is_some()
}

pub(crate) fn press(rt: &mut Rt, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

/// Manage on a machine's card (`openPage`): its own page replaces the list.
pub(crate) fn open(rt: &mut Rt, key: &str) {
    click(rt, &format!("machine-manage-{key}"));
}

pub(crate) fn card_of(rt: &Rt, key: &str) -> crate::fleet::machine_card::Card {
    rt.state.machine_cards().into_iter().find(|c| c.key == key).unwrap_or_else(|| panic!("no card {key}"))
}

// ── the fake Fleet ─────────────────────────────────────────────────────────────

/// Answers `routes` (method + path → status, body) and records everything else as a 404.
pub(crate) struct Fake {
    pub(crate) base: String,
    heard: Heard,
}

pub(crate) fn fake(routes: Vec<(&'static str, &'static str, u16, String)>) -> Fake {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let heard: Heard = Default::default();
    let log = heard.clone();
    let routes = Arc::new(routes);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (log, routes) = (log.clone(), routes.clone());
            std::thread::spawn(move || serve(stream, &log, &routes));
        }
    });
    Fake { base, heard }
}

fn serve(mut stream: std::net::TcpStream, log: &Heard, routes: &[(&'static str, &'static str, u16, String)]) {
    let heard = crate::test_http::read(&mut stream);
    if heard.method.is_empty() {
        return;
    }
    let bearer = heard.header("authorization").and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
    let first = heard.head.lines().next().unwrap_or("").to_string();
    let (_head, raw) = (heard.head, heard.body);
    let mut parts = first.split(' ');
    let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").split('?').next().unwrap_or("").to_string());
    log.lock().unwrap().push((method.clone(), path.clone(), serde_json::from_slice(&raw).unwrap_or(Value::Null), bearer));
    let (status, body) =
        routes.iter().find(|(m, p, ..)| *m == method && *p == path).map_or((404, "{}".to_string()), |(_, _, s, b)| (*s, b.clone()));
    let _ = stream.try_clone().unwrap().write_all(
        format!(
            "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    );
}

impl Fake {
    pub(crate) fn seen(&self, method: &str, path: &str) -> Vec<(Value, Option<String>)> {
        self.heard.lock().unwrap().iter().filter(|h| h.0 == method && h.1 == path).map(|h| (h.2.clone(), h.3.clone())).collect()
    }
}

pub(crate) struct Rig {
    pub(crate) rt: Rt,
    pub(crate) jobs: Receiver<Job>,
}

impl Rig {
    /// The page, with its home at `home`: the invented machines are shown and every write goes to the fake.
    pub(crate) fn new(home: &Fake) -> Rig {
        let mut rt = short_page();
        let (tx, jobs) = channel::<Job>();
        let tx = Mutex::new(tx);
        let link = Link::start_machine("home".to_string(), home.base.clone(), Post::new(move |job| drop(tx.lock().unwrap().send(job))));
        rt.update(move |s, _| s.link = Some(link));
        rt.frame(FRAME);
        Rig { rt, jobs }
    }

    pub(crate) fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        // The poll thread's own jobs would replace the invented machines with the fake's answer; only the page's are wanted.
        crate::test_wait::drive(&mut self.rt, &self.jobs, what, done, |s| {
            format!("page errors {:?}, busy {:?}", s.machine_page.errors, s.machine_page.busy)
        });
    }
}

fn token_json(token: &str) -> String {
    json!({ "token": token, "tokenSource": "saved", "host": "0.0.0.0", "port": 2113, "remoteReachable": true, "requiresToken": true, "addresses": [] }).to_string()
}

// ── asks before ────────────────────────────────────────────────────────────────

#[test]
fn asks_before_forgetting_a_machine_and_keeps_it_when_cancelled() {
    let mut rt = page();
    open(&mut rt, "m-falcon");
    click(&mut rt, "machine-forget-m-falcon");
    let c = rt.state.ui.confirm.as_ref().expect("asked");
    assert_eq!(
        (c.title.as_str(), c.confirm_label.as_str(), c.destructive),
        ("Forget falcon on every machine in your Fleet?", "Forget falcon", true)
    );
    click(&mut rt, "confirm-cancel");
    assert!(rt.state.ui.confirm.is_none());
    assert!(exists(&rt, "machine-about-m-falcon"), "cancelled: the machine and its page stay");

    click(&mut rt, "machine-forget-m-falcon");
    click(&mut rt, "confirm-action");
    // Forgotten: its page closes and the list no longer has it.
    assert!(!exists(&rt, "machine-about-m-falcon") && !exists(&rt, "machine-card-m-falcon"));
    rt.frame(FRAME);
    assert!(rt.state.machine_page.managing.is_none(), "the page is closed, not just hidden");
    assert!(rt.state.machines.machine("m-falcon").is_none());
}

#[test]
fn asks_before_replacing_this_machines_token_and_replaces_nothing_when_cancelled() {
    let old = token_json("old-token-0123456789");
    let new = token_json("new-token-9876543210");
    let home = fake(vec![("GET", "/api/machine/access", 200, old), ("POST", "/api/machine/access/token", 200, new)]);
    let mut rig = Rig::new(&home);
    rig.rt.update(|s, _| {
        s.machine_page.access =
            AccessState::Loaded(Access::from_json(&serde_json::from_str(&token_json("old-token-0123456789")).unwrap()).unwrap())
    });
    rig.rt.frame(FRAME);
    open(&mut rig.rt, "home");
    click(&mut rig.rt, "machine-replace-token");
    let c = rig.rt.state.ui.confirm.as_ref().expect("asked");
    assert_eq!((c.title.as_str(), c.confirm_label.as_str(), c.destructive), ("Replace this machine's token?", "Replace token", true));
    click(&mut rig.rt, "confirm-cancel");
    assert!(home.seen("POST", "/api/machine/access/token").is_empty(), "cancelled: nothing replaced");

    click(&mut rig.rt, "machine-replace-token");
    click(&mut rig.rt, "confirm-action");
    rig.wait("the new token", |s| matches!(&s.machine_page.access, AccessState::Loaded(a) if a.token == "new-token-9876543210"));
    assert_eq!(home.seen("POST", "/api/machine/access/token").len(), 1);
    assert!(rig.rt.state.machine_page.show_token, "the new token shows");
    assert!(words(&rig.rt, "machine-access-token").contains("new-token-9876543210"));
}

#[test]
fn hides_the_token_until_shown() {
    let mut rt = short_page();
    open(&mut rt, "home");
    assert_eq!(words(&rt, "machine-access-token"), Access::hidden_token());
    click(&mut rt, "machine-show-token");
    assert_eq!(words(&rt, "machine-access-token"), crate::mock::machines_page::SCENE_TOKEN);
    click(&mut rt, "machine-show-token");
    assert_eq!(words(&rt, "machine-access-token"), Access::hidden_token());
}

// ── what a card says ───────────────────────────────────────────────────────────

#[test]
fn shows_each_machines_ready_harnesses_and_how_many_sessions_work_there() {
    let rt = page();
    // Installed but switched off, and switched on but not installed, are not ready.
    assert_eq!(card_of(&rt, "home").capabilities.as_deref(), Some("Ready: OpenCode, Claude Code · 1 working · 2 need you"));
    assert_eq!(card_of(&rt, "home").versions.as_deref(), Some("OpenCode 1.18.32, Claude Code 2.1.0"));
    assert_eq!(card_of(&rt, "m-falcon").capabilities.as_deref(), Some("Ready: OpenCode, Claude Code · 0 working · 1 needs you"));
    assert!(words(&rt, "machine-capabilities-home").contains("2 need you"));
}

#[test]
fn shows_nothing_extra_for_a_machine_on_an_older_fleet_or_one_not_answering() {
    let mut rt = page();
    // An older Fleet says nothing about what it can run.
    rt.update(|s, _| drop(s.machines.identities.remove("m-falcon")));
    assert_eq!(card_of(&rt, "m-falcon").capabilities, None);
    // Harnesses not checked yet: only the sessions.
    rt.update(|s, _| {
        let info = s.machines.home.as_mut().unwrap();
        info.capabilities.as_mut().unwrap().harnesses = None;
        info.capabilities.as_mut().unwrap().needs_you = 0;
        info.capabilities.as_mut().unwrap().working = 3;
    });
    assert_eq!(card_of(&rt, "home").capabilities.as_deref(), Some("3 working"));
    // Not answering: its last numbers would be out of date.
    rt.update(|s, _| {
        s.machines.identities.insert("m-harbor".into(), crate::mock::machines_page::info_for_tests("m-harbor", "harbor"));
    });
    assert!(card_of(&rt, "m-harbor").capabilities.is_none());
}

#[test]
fn says_1_session_for_a_reachable_machine_with_one_session() {
    let mut rt = page();
    assert_eq!(card_of(&rt, "m-falcon").state, "reachable · 4 sessions");
    rt.update(|s, _| s.machines.others.get_mut("m-falcon").unwrap().sessions.truncate(1));
    assert_eq!(card_of(&rt, "m-falcon").state, "reachable · 1 session");
}

#[test]
fn shows_no_version_update_rule_or_restart_even_though_they_exist_they_are_in_updates_and_about() {
    let rt = page();
    let all: String = rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("|");
    for word in ["Restart", "0.72.0", "update rule", "Version"] {
        assert!(!all.contains(word), "{word} is drawn: {all}");
    }
}

#[test]
fn names_the_step_that_broke_on_the_card_with_the_steps_under_details() {
    let mut rt = page();
    assert_eq!(card_of(&rt, "m-harbor").state, "harbor isn't answering");
    open(&mut rt, "m-harbor");
    assert!(!exists(&rt, "machine-diagnosis-m-harbor"), "closed until Details is pressed");
    click(&mut rt, "machine-details-m-harbor");
    assert!(exists(&rt, "machine-diagnosis-m-harbor"));
    let steps = card_of(&rt, "m-harbor").diagnosis.unwrap().steps;
    assert_eq!(steps.len(), 6);
    assert!(words(&rt, "machine-diagnosis-m-harbor").contains("Port answers · not checked"));
    assert!(words(&rt, "machine-diagnosis-try-m-harbor").starts_with("Try"));
}

// ── agents sending work ────────────────────────────────────────────────────────

#[test]
fn asks_by_default_on_each_other_machines_card_says_its_level_and_saves_always_or_never() {
    let saved = json!({ "id": "m-falcon", "agentWork": "always" }).to_string();
    let home = fake(vec![("PUT", "/api/machines/m-falcon", 200, saved)]);
    let mut rig = Rig::new(&home);
    open(&mut rig.rt, "m-falcon");
    // This machine is where agents send work from: no setting on its card.
    assert!(card_of(&rig.rt, "home").agent_work.is_none());
    let (work, level) = card_of(&rig.rt, "m-falcon").agent_work.unwrap();
    assert_eq!((work, level.as_str()), (AgentWork::Ask, "Allow edits"));
    assert_eq!(words(&rig.rt, "machine-agent-work-level-m-falcon"), "Work there runs under falcon's level: Allow edits.");
    click(&mut rig.rt, "machine-agent-work-m-falcon-always");
    rig.wait("the setting saved", |s| s.machines.agent_work.get("m-falcon") == Some(&AgentWork::Always));
    assert_eq!(
        home.seen("PUT", "/api/machines/m-falcon").into_iter().map(|(b, _)| b).collect::<Vec<_>>(),
        [json!({ "agentWork": "always" })]
    );
}

#[test]
fn says_the_level_is_unknown_for_a_machine_on_a_fleet_too_old_to_say() {
    let mut rt = page();
    rt.update(|s, _| drop(s.machines.identities.remove("m-falcon")));
    rt.frame(FRAME);
    open(&mut rt, "m-falcon");
    assert_eq!(words(&rt, "machine-agent-work-level-m-falcon"), "Work there runs under falcon's level: unknown.");
}

// ── Work here, rename, token ───────────────────────────────────────────────────

#[test]
fn makes_the_machine_the_one_youre_on_and_goes_to_its_first_page_in_place() {
    let mut rt = page();
    click(&mut rt, "machine-open-m-falcon");
    assert_eq!(rt.state.machines.current_key(), "m-falcon");
    assert_eq!(rt.state.rail, Rail::Sessions);
    assert!(!exists(&rt, "machine-open-home"), "home is the live machine: no Work here on its card");
}

#[test]
fn renames_a_machine_on_the_machine_itself() {
    let node = fake(vec![("PUT", "/api/machine", 200, json!({ "id": "m-falcon", "name": "kestrel", "os": "linux" }).to_string())]);
    let home = fake(vec![("PUT", "/api/machines/m-falcon", 200, "{}".into()), ("GET", "/api/machines", 200, r#"{"machines":[]}"#.into())]);
    let mut rig = Rig::new(&home);
    let base = node.base.clone();
    rig.rt.update(move |s, _| {
        let m = s.machines.list.iter_mut().find(|m| m.id == "m-falcon").unwrap();
        m.base_url = base;
        crate::fleet::auth::set_for(&m.base_url, Some("falcon-scene-token"));
    });
    open(&mut rig.rt, "m-falcon");
    click(&mut rig.rt, "machine-rename-m-falcon");
    assert_eq!(rig.rt.state.machine_page.rename.text, "falcon");
    press(&mut rig.rt, Key::Text("kestrel".into()));
    press(&mut rig.rt, Key::Enter);
    rig.wait("the rename", |s| s.machine_page.renaming.is_none() && s.machines.machine("m-falcon").is_some_and(|m| m.name == "kestrel"));
    let sent = node.seen("PUT", "/api/machine");
    assert_eq!(sent, [(json!({ "name": "kestrel" }), Some("falcon-scene-token".to_string()))], "the machine itself, with its own token");
    assert_eq!(home.seen("PUT", "/api/machines/m-falcon")[0].0, json!({ "name": "kestrel" }), "then home keeps the name");
}

#[test]
fn escape_cancels_a_rename_and_a_refusal_shows_under_the_card() {
    let node = fake(vec![("PUT", "/api/machine", 403, json!({ "error": "Only the owner can rename it." }).to_string())]);
    let home = fake(vec![]);
    let mut rig = Rig::new(&home);
    let base = node.base.clone();
    rig.rt.update(move |s, _| s.machines.list.iter_mut().find(|m| m.id == "m-falcon").unwrap().base_url = base);
    open(&mut rig.rt, "m-falcon");
    click(&mut rig.rt, "machine-rename-m-falcon");
    press(&mut rig.rt, Key::Escape);
    assert!(rig.rt.state.machine_page.renaming.is_none());
    click(&mut rig.rt, "machine-rename-m-falcon");
    press(&mut rig.rt, Key::Enter);
    rig.wait("the refusal", |s| s.machine_page.errors.contains_key("m-falcon"));
    assert_eq!(words(&rig.rt, "machine-error-m-falcon"), "Only the owner can rename it.");
    assert!(rig.rt.state.machine_page.renaming.is_some(), "the field stays open to try again");
}

#[test]
fn saves_a_new_token_through_home_and_says_why_when_it_is_not_taken() {
    let home = fake(vec![("PUT", "/api/machines/m-falcon", 400, json!({ "error": "falcon didn't accept that token." }).to_string())]);
    let mut rig = Rig::new(&home);
    open(&mut rig.rt, "m-falcon");
    click(&mut rig.rt, "machine-token-m-falcon");
    press(&mut rig.rt, Key::Text("not-the-token".into()));
    press(&mut rig.rt, Key::Enter);
    rig.wait("the refusal", |s| s.machine_page.errors.contains_key("m-falcon"));
    assert_eq!(home.seen("PUT", "/api/machines/m-falcon")[0].0, json!({ "token": "not-the-token" }));
    assert_eq!(words(&rig.rt, "machine-error-m-falcon"), "falcon didn't accept that token.");
}

#[test]
fn the_token_never_shows_in_a_debug() {
    let a = Access::from_json(&serde_json::from_str(&token_json("secret-value-123")).unwrap()).unwrap();
    assert!(!format!("{a:?}").contains("secret-value-123"));
}

#[test]
fn the_access_is_read_once_per_visit_and_a_missing_one_is_said() {
    let home = fake(vec![("GET", "/api/machine/access", 404, "{}".into())]);
    let mut rig = Rig::new(&home);
    rig.rt.update(|s, _| {
        s.machine_page.opened = false;
        s.machine_page.access = AccessState::Loading;
    });
    rig.rt.frame(FRAME);
    rig.rt.frame(FRAME);
    rig.wait("the access answer", |s| s.machine_page.access == AccessState::Unavailable);
    open(&mut rig.rt, "home");
    assert!(words(&rig.rt, "machine-access-note").contains("doesn't hand out access tokens"));
}

// ── the list (26 F) ────────────────────────────────────────────────────────────

fn address(url: &str, kind: crate::fleet::machine::Kind) -> crate::fleet::machine::Address {
    crate::fleet::machine::Address { url: url.into(), kind, learned: true, last_result: None, last_tried_at: None }
}

fn reach_state(mode: crate::fleet::machines::reach::ReachMode) -> crate::fleet::machines::reach::ReachState {
    crate::fleet::machines::reach::ReachState {
        available: true,
        mode,
        own_address: None,
        own_port: 6262,
        listening: vec![],
        found: vec![],
        tailscale: None,
    }
}

#[test]
fn gives_every_machine_a_line_on_how_a_phone_reaches_it_and_takes_out_let_other_devices_reach() {
    use crate::fleet::machine::Kind;
    use crate::fleet::machines::reach::ReachMode;
    let mut rt = page();
    rt.update(|s, _| {
        if let Some(m) = s.machines.list.iter_mut().find(|m| m.id == "m-falcon") {
            m.addresses = vec![address("http://falcon.tail4f2a.ts.net:6262", Kind::Tailnet)];
        }
        s.machine_data.reach = Some(reach_state(ReachMode::Home));
    });
    rt.frame(FRAME);
    assert_eq!(words(&rt, "machine-reach-line-m-falcon"), "Only on Tailscale");
    assert_eq!(
        words(&rt, &format!("machine-reach-line-{}", crate::fleet::settings::HOME_MACHINE_KEY)),
        "Reaches your phone on the home network"
    );
    let all: String = rt.elements().iter().filter_map(|e| e.text.clone()).collect();
    assert!(all.contains(
        "A machine is anywhere Fleet runs: the desktop app, or a server running fleet node. Your phone and browsers look at them."
    ));
    assert!(all.contains("Your machines"), "the group is named for what it lists");
    assert!(!all.contains("Let other devices reach"), "the token card is Who can reach it, on home's own page");
}

#[test]
fn a_machine_that_stopped_answering_says_so_in_the_error_colour() {
    let mut rt = page();
    rt.update(|s, _| {
        s.machines.others.entry("m-falcon".into()).or_default().error = Some("down".into());
    });
    rt.frame(FRAME);
    let line = card_of(&rt, "m-falcon").reach.unwrap();
    assert!(line.text.starts_with("Off since "), "the last read's time, as Fleet says it: {}", line.text);
    rt.update(|s, _| s.machines.others.entry("m-falcon".into()).or_default().loaded_at = None);
    let line = card_of(&rt, "m-falcon").reach.unwrap();
    assert_eq!(line.text, "Not reachable");
    assert_eq!(line.tone, crate::fleet::machines::reach::Tone::Out);
}

#[test]
fn forgetting_a_machine_saying_it_reaches_every_machine() {
    let mut rt = page();
    open(&mut rt, "m-falcon");
    click(&mut rt, "machine-forget-m-falcon");
    let c = rt.state.ui.confirm.as_ref().expect("asked");
    assert_eq!(c.title, "Forget falcon on every machine in your Fleet?");
    assert_eq!(c.confirm_label, "Forget falcon");
    assert!(c.destructive);
    assert!(c.body.starts_with("Every machine drops it from its list and its keys stop working, even on machines that are off now"));
    click(&mut rt, "confirm-cancel");
    // An SSH machine keeps the old words: only this device lists it.
    rt.update(|s, _| {
        if let Some(m) = s.machines.list.iter_mut().find(|m| m.id == "m-falcon") {
            m.ssh_destination = Some("maya@falcon.example".into());
        }
    });
    rt.frame(FRAME);
    click(&mut rt, "machine-forget-m-falcon");
    let c = rt.state.ui.confirm.as_ref().expect("asked");
    assert_eq!((c.title.as_str(), c.confirm_label.as_str()), ("Forget falcon?", "Forget machine"));
}

#[test]
fn manage_opens_the_machines_own_page_and_work_here_is_not_on_the_live_machine() {
    let mut rt = page();
    let home = crate::fleet::settings::HOME_MACHINE_KEY;
    assert!(!exists(&rt, &format!("machine-open-{home}")), "the live machine has no Work here");
    assert!(exists(&rt, "machine-open-m-falcon"));
    click(&mut rt, "machine-manage-m-falcon");
    assert_eq!(rt.state.machine_page.managing.as_deref(), Some("m-falcon"));
}

// ── a machine's own page (26 G) ────────────────────────────────────────────────

#[test]
fn keeps_the_token_and_addresses_off_the_list_on_the_machines_own_page() {
    let mut rt = page();
    assert!(!exists(&rt, "machine-access-token"));
    assert!(!exists(&rt, "machine-address-0"));
    for gone in ["machine-rename-home", "machine-forget-m-falcon", "machine-token-m-falcon", "machine-agent-work-m-falcon"] {
        assert!(!exists(&rt, gone), "{gone} is on the machine's page, not the list");
    }

    open(&mut rt, "home");
    assert!(exists(&rt, "machine-access-token"));
    assert!(exists(&rt, "machine-address-0"), "home's addresses, with a kind chip and Copy");
    assert!(!exists(&rt, "machines-add"), "the page replaces the section");
}

#[test]
fn manage_opens_the_page_titled_with_the_machine_and_its_trail_leads_back() {
    let mut rt = page();
    open(&mut rt, "m-falcon");
    assert_eq!(rt.state.machine_page.managing.as_deref(), Some("m-falcon"));
    let header = words(&rt, "settings-header");
    assert!(
        header.starts_with("falcon") && header.ends_with("Everything about this machine: how it is reached, and what it keeps."),
        "{header}"
    );
    assert!(exists(&rt, "machine-page-m-falcon") && !exists(&rt, "machines-page-body"));
    click(&mut rt, "machine-page-back");
    assert!(rt.state.machine_page.managing.is_none());
    assert!(exists(&rt, "machines-page-body") && !exists(&rt, "machine-page-m-falcon"));
}

#[test]
fn leaving_the_page_drops_a_rename_or_token_left_open() {
    let mut rt = page();
    open(&mut rt, "m-falcon");
    click(&mut rt, "machine-token-m-falcon");
    assert!(rt.state.machine_page.token_for.is_some());
    click(&mut rt, "machine-page-back");
    assert!(rt.state.machine_page.token_for.is_none() && rt.state.machine_page.renaming.is_none());
}

#[test]
fn a_machine_that_leaves_closes_its_page() {
    let mut rt = page();
    open(&mut rt, "m-falcon");
    rt.update(|s, _| s.machines.list.retain(|m| m.id != "m-falcon"));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.state.machine_page.managing.is_none());
    assert!(exists(&rt, "machines-page-body"));
}

#[test]
fn another_machines_page_has_work_here_rename_forget_and_its_key_but_not_who_can_reach_it() {
    let mut rt = page();
    open(&mut rt, "m-falcon");
    for here in
        ["machine-open-m-falcon", "machine-rename-m-falcon", "machine-forget-m-falcon", "machine-token-m-falcon", "machine-key-note"]
    {
        assert!(exists(&rt, here), "{here}");
    }
    assert_eq!(words(&rt, "machine-key-note"), "This browser holds a key for falcon. If it stopped working, give it a new one.");
    assert!(!exists(&rt, "machine-access-token"), "who can reach it is home's");
    assert!(!exists(&rt, "machine-manage-m-falcon"));
}

#[test]
fn homes_page_has_who_can_reach_it_and_no_forget_work_here_or_key() {
    let mut rt = page();
    open(&mut rt, "home");
    assert!(exists(&rt, "machine-rename-home") && exists(&rt, "machine-access-block"));
    for gone in ["machine-forget-home", "machine-open-home", "machine-token-home", "machine-key-note"] {
        assert!(!exists(&rt, gone), "{gone}");
    }
}

#[test]
fn says_who_can_reach_home_in_the_words_of_its_mode() {
    use crate::fleet::machines::reach::ReachMode;
    let mut rt = page();
    open(&mut rt, "home");
    rt.update(|s, _| s.machine_data.reach = Some(reach_state(ReachMode::Off)));
    rt.frame(FRAME);
    assert_eq!(
        words(&rt, "machine-access-loopback"),
        "Reachable from this machine only. Turn it on to use atlas from your phone or another machine."
    );
    for (mode, label) in [
        (ReachMode::TailscaleAndHome, "Tailscale and home network"),
        (ReachMode::Home, "Home network only"),
        (ReachMode::Own, "Your own address"),
    ] {
        rt.update(move |s, _| s.machine_data.reach = Some(reach_state(mode)));
        rt.frame(FRAME);
        assert_eq!(words(&rt, "machine-access-reach"), format!("Reachable from other devices: {label}. Each one needs its own key."));
    }
}

#[test]
fn names_where_fleet_listens_when_only_this_machine_can_reach_it() {
    let mut rt = page();
    open(&mut rt, "home");
    rt.update(|s, _| {
        if let AccessState::Loaded(a) = &mut s.machine_page.access {
            a.remote_reachable = false;
            a.requires_token = false;
            a.host = "127.0.0.1".into();
            a.port = 2113;
        }
    });
    rt.frame(FRAME);
    assert!(
        words(&rt, "machine-access-listens")
            .starts_with("Only this machine can reach it: Fleet listens on 127.0.0.1:2113. To add it from another device")
    );
    rt.update(|s, _| {
        if let AccessState::Loaded(a) = &mut s.machine_page.access {
            a.requires_token = true;
        }
    });
    rt.frame(FRAME);
    assert!(words(&rt, "machine-access-proxy").starts_with("Fleet listens on 127.0.0.1:2113 and asks every request for the token"));
}

#[test]
fn masks_the_new_token_field() {
    let home = fake(vec![]);
    let mut rig = Rig::new(&home);
    open(&mut rig.rt, "m-falcon");
    click(&mut rig.rt, "machine-token-m-falcon");
    press(&mut rig.rt, Key::Text("not-the-token".into()));
    let page = &rig.rt.state.machine_page;
    assert!(page.token.is_secure(), "a secure edit draws bullets and keeps no undo or copy");
    assert_eq!(page.token.text, "not-the-token", "what is saved is what was typed");
}

#[test]
fn masks_the_access_token_field_of_the_add_a_machine_form() {
    let mut rt = page();
    assert!(!rt.state.machine_page.add.token.is_secure());
    click(&mut rt, "machines-add");
    // Add a machine opens on Found; the token is under the pasted link's tab.
    click(&mut rt, "machines-add-mode-link");
    click(&mut rt, crate::fleet::pairing::LINK_FIELD);
    press(&mut rt, Key::Text("100.64.90.72:2113".into()));
    click(&mut rt, "machines-add-advanced");
    click(&mut rt, crate::fleet::pairing::TOKEN_FIELD);
    press(&mut rt, Key::Text("scratch-master-0002".into()));
    assert!(rt.state.machine_page.add.token.is_secure());
    assert_eq!(rt.state.machine_page.add.token.text, "scratch-master-0002");
}
