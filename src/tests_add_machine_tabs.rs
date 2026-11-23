//! Add a machine's tabs, Found and On a new server (piece H of docs/parity/26-parity-audit.md): `FoundMachinesCard.test.ts`,
//! `NewServerCard.test.ts` and MachinesSection's tab cases, under Fleet's names. Home is a fake whose answers a test can change
//! as it goes (a machine joins, a code is made again). The clock only moves by explicit frames, so the looks every 3 s and 4 s
//! are counted, not raced. Every machine, address and code here is invented.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::machines::add::AddMode;
use crate::tests::{FRAME, click};
use crate::tests_machines_page::{Rt, exists, short_page, words};
use crate::{State, test_wait};
use rune::{Key, KeyEvent, Mods};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

/// 2026-11-19T10:00:00Z, Fleet's `NOW`.
const NOW: i64 = 1_795_082_400_000;
const MIN: i64 = 60_000;

type Routes = Arc<Mutex<HashMap<String, (u16, String)>>>;

/// A fake home: "METHOD /path" → status and body, changeable while it runs; every request is kept.
struct Home {
    base: String,
    routes: Routes,
    heard: Arc<Mutex<Vec<(String, Value)>>>,
}

impl Home {
    fn new(routes: &[(&str, u16, Value)]) -> Home {
        let listener = crate::test_http::bind().unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let home = Home { base, routes: Default::default(), heard: Default::default() };
        for (route, status, body) in routes {
            home.set(route, *status, body.clone());
        }
        let (table, log) = (home.routes.clone(), home.heard.clone());
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let (table, log) = (table.clone(), log.clone());
                std::thread::spawn(move || {
                    let heard = crate::test_http::read(&mut stream);
                    if heard.method.is_empty() {
                        return;
                    }
                    let route = format!("{} {}", heard.method, heard.route());
                    log.lock().unwrap().push((route.clone(), heard.json()));
                    let (status, body) =
                        table.lock().unwrap().get(&route).cloned().unwrap_or((404, json!({ "error": "no route" }).to_string()));
                    crate::test_http::respond_json(&mut stream, status, &body);
                });
            }
        });
        home
    }

    fn set(&self, route: &str, status: u16, body: Value) {
        self.routes.lock().unwrap().insert(route.to_string(), (status, body.to_string()));
    }

    fn heard(&self, route: &str) -> Vec<Value> {
        self.heard.lock().unwrap().iter().filter(|(r, _)| r == route).map(|(_, b)| b.clone()).collect()
    }

    /// The calls the tabs make (the page itself reads other things on its own clock).
    fn calls(&self) -> Vec<String> {
        let tabs = ["GET /api/machines", "GET /api/machines/found", "POST /api/machines/join-code"];
        self.heard.lock().unwrap().iter().map(|(r, _)| r.clone()).filter(|r| tabs.contains(&r.as_str())).collect()
    }
}

struct Rig {
    rt: Rt,
    jobs: Receiver<Job>,
}

impl Rig {
    fn new(home: &Home) -> Rig {
        let mut rt = short_page();
        let (tx, jobs) = channel::<Job>();
        let tx = Mutex::new(tx);
        let link = Link::start_machine("home".to_string(), home.base.clone(), Post::new(move |job| drop(tx.lock().unwrap().send(job))));
        rt.update(move |s, _| {
            s.link = Some(link);
            s.now_ms = Some(NOW);
        });
        rt.frame(FRAME);
        Rig { rt, jobs }
    }

    /// Applies what threads hand back and draws frames that don't move the clock, until `done` holds.
    fn wait(&mut self, what: &str, mut done: impl FnMut(&State) -> bool) {
        let jobs = &self.jobs;
        test_wait::until(
            &mut self.rt,
            what,
            |rt| {
                while let Ok(job) = jobs.try_recv() {
                    rt.update(job);
                }
                rt.frame(0.0);
                done(&rt.state)
            },
            |rt| format!("mode {:?}, added {:?}", rt.state.machine_page.add.mode, rt.state.machine_page.add.added),
        );
    }

    /// Moves the app's clock by `secs`, then lets whatever look that starts finish.
    fn advance(&mut self, secs: f64) {
        self.rt.frame(secs);
        self.wait("the looks to finish", |s| !s.machine_page.add.waiting.looking && !s.machine_page.add.server.looking);
    }

    fn open(&mut self) {
        click(&mut self.rt, "machines-add");
        assert!(exists(&self.rt, "machines-add-form"));
    }

    fn set_wall(&mut self, at: i64) {
        self.rt.update(move |s, _| s.now_ms = Some(at));
        self.rt.frame(0.0);
    }
}

/// Types into a found machine's code field, from empty.
fn type_in(rt: &mut Rt, field: &str, text: &str) {
    rt.update(|s, _| s.machine_page.add.waiting.rest.clear());
    rt.frame(FRAME);
    click(rt, field);
    rt.key(KeyEvent { key: Key::Text(text.into()), mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

/// A button drawn as off (`faded`, at 60%).
fn off(rt: &mut Rt, key: &str) -> bool {
    // The fade is a transition: let it end first.
    for _ in 0..3 {
        rt.frame(0.2);
    }
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no {key}")).opacity < 1.0
}

// ── Found ──────────────────────────────────────────────────────────────────────

const ATLAS: &str = "a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0";

fn atlas() -> Value {
    json!({ "machineId": ATLAS, "name": "atlas", "os": "linux", "baseUrl": "http://192.168.1.40:6262", "hint": "7K3M",
        "source": "Home network", "expiresAt": "2026-11-19T18:00:00.000Z" })
}

fn atlas_machine() -> Value {
    json!({ "id": ATLAS, "name": "atlas", "baseUrl": "http://192.168.1.40:6262", "os": "linux", "status": "online",
        "addedAt": "2026-11-19T17:00:00.000Z", "lastSeenAt": null, "token": "scratch-d1-secret" })
}

fn found_home() -> Home {
    Home::new(&[("GET /api/machines", 200, json!({ "machines": [] })), ("GET /api/machines/found", 200, json!({ "machines": [atlas()] }))])
}

fn on_found(home: &Home) -> Rig {
    let mut rig = Rig::new(home);
    rig.open();
    rig.wait("the waiting machines", |s| s.machine_data.found.as_ref().is_some_and(|l| !l.is_empty()));
    rig.rt.frame(FRAME);
    rig
}

const CODE: &str = "machines-found-code-a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0";
const ADD: &str = "machines-found-add-a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0";

#[test]
fn lists_a_waiting_machine_with_where_it_was_seen_and_the_start_of_its_code() {
    let home = found_home();
    let mut rig = on_found(&home);
    assert_eq!(words(&rig.rt, &format!("machines-found-name-{ATLAS}")), "atlas · Linux");
    assert!(words(&rig.rt, &format!("machines-found-row-{ATLAS}")).contains("On your home network"));
    assert_eq!(words(&rig.rt, &format!("machines-found-hint-{ATLAS}")), "7K3M-");
    assert!(off(&mut rig.rt, ADD));
}

#[test]
fn adds_the_machine_only_when_the_rest_of_the_code_is_typed_and_sends_the_whole_code() {
    let home = found_home();
    home.set("POST /api/machines/pair", 200, atlas_machine());
    let mut rig = on_found(&home);
    type_in(&mut rig.rt, CODE, "q9z");
    assert!(off(&mut rig.rt, ADD));
    type_in(&mut rig.rt, CODE, "q9zd");
    assert!(!off(&mut rig.rt, ADD));
    // The form's submit: Enter in the field.
    rig.rt.key(KeyEvent { key: Key::Enter, mods: Mods::default() });
    rig.wait("the machine added", |s| s.machine_page.add.added.is_some());
    assert_eq!(home.heard("POST /api/machines/pair"), vec![json!({ "baseUrl": "http://192.168.1.40:6262", "code": "7K3M-Q9ZD" })]);
    assert_eq!(rig.rt.state.machine_page.add.added.as_deref(), Some("atlas"));
}

#[test]
fn takes_the_whole_code_too_but_not_one_that_starts_with_something_else() {
    let home = found_home();
    let mut rig = on_found(&home);
    type_in(&mut rig.rt, CODE, "1111-2222");
    assert!(off(&mut rig.rt, ADD));
    type_in(&mut rig.rt, CODE, "7k3m-q9zd");
    assert!(!off(&mut rig.rt, ADD));
}

#[test]
fn says_why_a_machine_could_not_be_added_and_adds_nothing() {
    let home = found_home();
    home.set("POST /api/machines/pair", 400, json!({ "error": "That code has expired or was already used." }));
    let mut rig = on_found(&home);
    type_in(&mut rig.rt, CODE, "q9zd");
    click(&mut rig.rt, ADD);
    rig.wait("the refusal", |s| !s.machine_page.add.waiting.failure.is_empty());
    rig.rt.frame(FRAME);
    assert!(words(&rig.rt, &format!("machines-found-error-{ATLAS}")).contains("expired"));
    assert!(rig.rt.state.machine_page.add.added.is_none());
    assert!(exists(&rig.rt, "machines-add-form"));
}

#[test]
fn offers_the_pasted_link_when_nothing_is_waiting() {
    let home = found_home();
    home.set("GET /api/machines/found", 200, json!({ "machines": [] }));
    let mut rig = Rig::new(&home);
    rig.open();
    rig.wait("the empty list", |s| s.machine_data.found.is_some());
    rig.rt.frame(FRAME);
    assert!(!exists(&rig.rt, &format!("machines-found-row-{ATLAS}")));
    click(&mut rig.rt, "machines-found-paste");
    assert_eq!(rig.rt.state.machine_page.add.mode, AddMode::Link);
    assert!(exists(&rig.rt, crate::fleet::pairing::LINK_FIELD));
}

#[test]
fn found_looks_again_every_four_seconds_while_it_is_shown() {
    let home = found_home();
    let mut rig = on_found(&home);
    assert_eq!(home.heard("GET /api/machines/found").len(), 1);
    rig.advance(3.0);
    assert_eq!(home.heard("GET /api/machines/found").len(), 1, "not yet");
    rig.advance(1.5);
    assert_eq!(home.heard("GET /api/machines/found").len(), 2);
    click(&mut rig.rt, "machines-add-mode-server");
    rig.advance(10.0);
    assert_eq!(home.heard("GET /api/machines/found").len(), 2, "another tab: Found stopped looking");
}

// ── On a new server ────────────────────────────────────────────────────────────

const HANGAR: &str = "b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0";

fn hangar() -> Value {
    json!({ "id": HANGAR, "name": "hangar", "baseUrl": "http://100.64.0.7:6262", "os": "linux", "status": "online",
        "addedAt": "2026-11-19T09:00:00.000Z", "lastSeenAt": null, "token": "scratch-d2-secret" })
}

fn atlas_joined() -> Value {
    json!({ "id": ATLAS, "name": "atlas", "baseUrl": "http://100.64.0.9:6262", "os": "linux", "status": "online",
        "addedAt": "2026-11-19T10:01:00.000Z", "lastSeenAt": null, "token": "scratch-d1-secret" })
}

fn join_code(expires_at: &str) -> Value {
    json!({ "code": "J8RT-4WXN", "expiresAt": expires_at, "addresses": ["http://100.64.0.2:6262", "http://192.168.1.20:6262"],
        "target": "http://100.64.0.2:6262,http://192.168.1.20:6262#J8RT-4WXN", "machineName": "studio" })
}

fn server_home() -> Home {
    Home::new(&[
        ("GET /api/machines", 200, json!({ "machines": [hangar()] })),
        ("GET /api/machines/found", 200, json!({ "machines": [] })),
        ("POST /api/machines/join-code", 200, join_code("2026-11-19T10:30:00.000Z")),
    ])
}

fn on_server(home: &Home) -> Rig {
    let mut rig = Rig::new(home);
    rig.open();
    click(&mut rig.rt, "machines-add-mode-server");
    rig.wait("the join code", |s| {
        let server = &s.machine_page.add.server;
        !server.making && (server.code.is_some() || server.failure.is_some())
    });
    rig.rt.frame(FRAME);
    rig
}

#[test]
fn shows_one_line_to_paste_with_this_machines_addresses_and_a_one_time_code() {
    let home = server_home();
    let rig = on_server(&home);
    assert_eq!(home.heard("POST /api/machines/join-code").len(), 1);
    assert_eq!(
        words(&rig.rt, "machines-new-server-line"),
        "curl -fsSL https://github.com/pgermishuys/weave-fleet/releases/latest/download/install.sh | sh -s -- --join \
         'http://100.64.0.2:6262,http://192.168.1.20:6262#J8RT-4WXN'"
    );
    let warning = words(&rig.rt, "machines-new-server-warning");
    assert!(warning.contains("Works once, for 30 minutes") && warning.contains("treat it like a password"), "{warning}");
    assert!(warning.contains("add a machine to studio"), "{warning}");
}

#[test]
fn counts_down_to_when_the_code_stops_working() {
    let home = server_home();
    let mut rig = on_server(&home);
    assert!(words(&rig.rt, "machines-new-server-waiting").contains("expires in 30 min"));
    rig.set_wall(NOW + 10 * MIN);
    rig.rt.frame(FRAME);
    assert!(words(&rig.rt, "machines-new-server-waiting").contains("expires in 20 min"));
    rig.set_wall(NOW + 30 * MIN - 30_000);
    rig.rt.frame(FRAME);
    assert!(words(&rig.rt, "machines-new-server-waiting").contains("expires in 30 s"));
}

#[test]
fn copies_the_whole_line() {
    let home = server_home();
    let mut rig = on_server(&home);
    let clipboard = rune::clipboard::memory();
    rig.rt.use_clipboard(&clipboard);
    click(&mut rig.rt, "machines-new-server-copy");
    rig.rt.frame(FRAME);
    let line = words(&rig.rt, "machines-new-server-line");
    assert_eq!(clipboard.text().as_deref().map(String::as_str), Some(line.as_str()));
    assert_eq!(words(&rig.rt, "machines-new-server-copy"), "Copied");
    rig.rt.frame(2.5);
    rig.rt.frame(FRAME);
    assert_eq!(words(&rig.rt, "machines-new-server-copy"), "Copy", "for 2 s");
}

#[test]
fn says_the_code_has_expired_stops_offering_the_line_and_makes_a_new_one_on_request() {
    let home = server_home();
    let mut rig = on_server(&home);
    rig.set_wall(NOW + 30 * MIN + 1000);
    rig.rt.frame(FRAME);
    assert!(words(&rig.rt, "machines-new-server-expired").contains("This code has expired"));
    assert!(off(&mut rig.rt, "machines-new-server-copy"));
    assert!(!exists(&rig.rt, "machines-new-server-waiting"));

    let mut again = join_code("2026-11-19T11:01:00.000Z");
    again["code"] = json!("K2MQ-9PDV");
    again["target"] = json!("http://100.64.0.2:6262#K2MQ-9PDV");
    home.set("POST /api/machines/join-code", 200, again);
    click(&mut rig.rt, "machines-new-server-renew");
    rig.wait("the new code", |s| s.machine_page.add.server.code.as_ref().is_some_and(|c| c.target.ends_with("#K2MQ-9PDV")));
    rig.rt.frame(FRAME);
    assert_eq!(home.heard("POST /api/machines/join-code").len(), 2);
    assert!(words(&rig.rt, "machines-new-server-line").contains("#K2MQ-9PDV"));
    assert!(!exists(&rig.rt, "machines-new-server-expired"));
    assert!(!off(&mut rig.rt, "machines-new-server-copy"));
}

#[test]
fn says_so_when_a_new_machine_joins_and_names_it() {
    let home = server_home();
    let mut rig = on_server(&home);
    rig.advance(3.5);
    assert!(rig.rt.state.machine_page.add.added.is_none());
    home.set("GET /api/machines", 200, json!({ "machines": [hangar(), atlas_joined()] }));
    rig.advance(3.5);
    rig.wait("the joined machine", |s| s.machine_page.add.added.is_some());
    assert_eq!(rig.rt.state.machine_page.add.added.as_deref(), Some("atlas"));
}

#[test]
fn does_not_take_a_machine_that_was_already_there_for_the_one_that_joined() {
    let home = server_home();
    let mut rig = on_server(&home);
    for _ in 0..3 {
        rig.advance(3.1);
    }
    assert!(home.heard("GET /api/machines").len() >= 3, "it looked");
    assert!(rig.rt.state.machine_page.add.added.is_none());
    assert!(exists(&rig.rt, "machines-add-form"));
}

#[test]
fn explains_a_machine_that_only_listens_on_itself_and_can_try_again() {
    let home = server_home();
    home.set(
        "POST /api/machines/join-code",
        400,
        json!({ "error": "studio only listens on this machine, so a new server can't reach it." }),
    );
    let mut rig = on_server(&home);
    assert!(!exists(&rig.rt, "machines-new-server-line"));
    assert!(words(&rig.rt, "machines-new-server-error").contains("only listens on this machine"));
    home.set("POST /api/machines/join-code", 200, join_code("2026-11-19T10:30:00.000Z"));
    click(&mut rig.rt, "machines-new-server-retry");
    rig.wait("the code", |s| s.machine_page.add.server.code.is_some());
    rig.rt.frame(FRAME);
    assert!(exists(&rig.rt, "machines-new-server-line"));
}

#[test]
fn stops_looking_when_it_is_closed() {
    let home = server_home();
    let mut rig = on_server(&home);
    click(&mut rig.rt, "machines-add-cancel");
    let calls = home.calls();
    for _ in 0..4 {
        rig.advance(5.0);
    }
    assert_eq!(home.calls(), calls, "nothing more was asked");
}

// ── MachinesSection ────────────────────────────────────────────────────────────

#[test]
fn opens_on_found_with_on_a_new_server_and_paste_a_pairing_link_as_the_other_choices() {
    let home =
        Home::new(&[("GET /api/machines", 200, json!({ "machines": [] })), ("GET /api/machines/found", 200, json!({ "machines": [] }))]);
    let mut rig = Rig::new(&home);
    rig.open();
    rig.wait("the empty list", |s| s.machine_data.found.is_some());
    rig.rt.frame(FRAME);
    let tabs: Vec<String> = rig.rt.elements().into_iter().filter(|e| e.role == "tab").filter_map(|e| e.label.clone()).collect();
    // "Set it up from here" (SSH) is the owner's decision 5, so it is not a tab here.
    assert_eq!(tabs, ["Found", "On a new server", "Paste a pairing link"]);
    assert_eq!(rig.rt.state.machine_page.add.mode, AddMode::Found);
    assert!(exists(&rig.rt, "machines-found-empty"));
    assert!(!exists(&rig.rt, crate::fleet::pairing::LINK_FIELD));
}

#[test]
fn on_a_new_server_shows_the_line_to_paste_and_closes_with_the_machines_name_when_one_joins() {
    let home = server_home();
    home.set("GET /api/machines", 200, json!({ "machines": [] }));
    home.set(
        "POST /api/machines/join-code",
        200,
        json!({ "code": "J8RT-4WXN", "expiresAt": "2026-11-19T10:30:00.000Z",
        "addresses": ["http://100.64.0.2:6262"], "target": "http://100.64.0.2:6262#J8RT-4WXN", "machineName": "studio" }),
    );
    let mut rig = on_server(&home);
    assert!(words(&rig.rt, "machines-new-server-line").contains("--join 'http://100.64.0.2:6262#J8RT-4WXN'"));
    assert!(!exists(&rig.rt, "machines-add-submit"));
    home.set("GET /api/machines", 200, json!({ "machines": [atlas_joined()] }));
    rig.advance(3.5);
    rig.wait("the joined machine", |s| s.machine_page.add.added.is_some());
    rig.rt.frame(FRAME);
    assert!(!exists(&rig.rt, "machines-add-form"));
    assert!(words(&rig.rt, "machines-added").contains("Added atlas."));
}
