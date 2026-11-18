//! Component 16, piece P: Settings → Memory's model and page. `MemorySection.test.ts`'s first case and
//! `use-dreaming-feature.test.ts` under Fleet's names, then the words (`life_label` and the rest, which Fleet has no
//! tests for), the request bodies, the refusals, and that every request goes to the machine Settings shows.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::memory::{self as m, List, MemoryPage, Note};
use crate::tests::{FRAME, app, click};
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fake {
    enabled: bool,
    /// Every call heard: method and path.
    heard: Vec<(String, String)>,
    /// What `PUT /api/memory` answers instead of success.
    refuse: Option<(u16, String)>,
    /// What `GET /api/memory` answers instead of the overview.
    broken: Option<(u16, String)>,
    /// The other machines home lists (`GET /api/machines`), so its own poll keeps them listed.
    machines: Vec<Value>,
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
    let req = crate::test_http::read(&mut stream);
    let (method, path) = (req.method, req.path);
    let mut f = fake.lock().unwrap();
    // The live-events link negotiates in the background at any moment; it is not what a test asks about.
    if !path.starts_with("/hubs/") {
        f.heard.push((method.clone(), path.clone()));
    }
    let overview = |on: bool| {
        json!({ "enabled": on, "repositories": [{ "path": "/work/lantern", "name": "lantern", "count": 1 }], "machineCount": 0,
                "maxRepositoryNotes": 40, "maxMachineNotes": 15 })
    };
    let (status, reply) = match (method.as_str(), path.as_str()) {
        ("GET", "/api/memory") => match f.broken.clone() {
            Some((code, body)) => (code, body),
            None => (200, overview(f.enabled).to_string()),
        },
        ("GET", "/api/machines") => (200, json!({ "machines": f.machines }).to_string()),
        ("PUT", "/api/memory") => match f.refuse.clone() {
            Some((code, body)) => (code, body),
            None => {
                f.enabled = !f.enabled;
                (200, overview(f.enabled).to_string())
            }
        },
        ("GET", p) if p.starts_with("/api/memory/notes") => {
            (200, json!({ "repository": "/work/lantern", "repositoryNotes": [], "machineNotes": [], "tokens": 0 }).to_string())
        }
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
    fake: Shared,
}

fn link_to(base: String) -> (Link, Receiver<Job>) {
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    (Link::start(base, Post::new(move |job| drop(jobs.lock().unwrap().send(job)))), queue)
}

/// A connected app on Settings → Memory, the page not yet read.
fn rig(setup: impl FnOnce(&mut Fake)) -> Rig {
    let fake: Shared = Default::default();
    setup(&mut fake.lock().unwrap());
    let (link, queue) = link_to(serve(fake.clone()));
    let mut rt = app();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.rail = Rail::Settings;
        s.open_settings("memory", None);
    });
    Rig { rt, queue, fake }
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("{:?}", s.settings.memory));
    }
    fn read(&mut self) {
        self.wait("the page to be read", |s| !s.settings.memory.loading && s.settings.memory.loaded_for.is_some());
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }
    fn exists(&self, key: &str) -> bool {
        self.rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }
    fn text(&self, key: &str) -> String {
        let all = self.rt.elements();
        let it = all.iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element keyed {key:?}"));
        all.iter().filter(|e| e.path.starts_with(&it.path)).filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("")
    }
    /// The switch's state is the page's: Rune doesn't report a switch's checked flag to a test. The switch is drawn from it.
    fn checked(&self, key: &str) -> bool {
        assert!(self.exists(key), "no switch keyed {key:?}");
        self.rt.state.settings.memory.enabled()
    }
    fn heard(&self) -> Vec<(String, String)> {
        self.fake.lock().unwrap().heard.clone()
    }
}

// ─── MemorySection.test.ts ──────────────────────────────────────────────────────

#[test]
fn is_off_until_the_user_turns_it_on_and_says_what_turning_it_on_costs_and_who_reads_the_notes() {
    let mut r = rig(|_| {});
    r.read();
    assert!(!r.checked("memory-switch"));
    assert!(r.text("memory-state").contains("Off."));
    assert!(r.exists("memory-facts"));
    assert!(r.text("memory-facts").contains("Costs tokens."));
    assert!(r.text("memory-facts").contains("up to 40 repository and 15 machine notes"));
    assert!(r.text("memory-claude-warning").contains("Claude Code also keeps its own memory"));
    assert!(!r.exists("memory-list-machine"));

    click(&mut r.rt, "memory-switch");
    r.wait("the switch to come back on", |s| s.settings.memory.enabled() && !s.settings.memory.saving);
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    assert!(r.heard().contains(&("PUT".into(), "/api/memory".into())));
    assert!(r.checked("memory-switch"));
    assert!(r.text("memory-state").starts_with("On."));
    assert!(!r.exists("memory-facts"), "the four facts are for the off state");
    assert!(r.exists("memory-harnesses"), "the Harnesses card shows in every state");
}

// ─── use-dreaming-feature.test.ts ───────────────────────────────────────────────

#[test]
fn is_off_while_memory_is_off_whatever_a_leftover_dreaming_preference_says() {
    let mut s = State::default();
    s.pickers.prefs.set("Dreaming", "true");
    assert!(!s.dreaming_enabled());
}

#[test]
fn is_on_whenever_memory_is_on_with_no_dreaming_preference() {
    let mut s = State::default();
    s.pickers.prefs.set("Memory", "true");
    assert!(s.dreaming_enabled());
}

// ─── The page ───────────────────────────────────────────────────────────────────

#[test]
fn says_loading_until_the_overview_and_the_notes_came() {
    let mut r = rig(|_| {});
    r.rt.frame(FRAME);
    assert!(r.exists("memory-loading") || !r.rt.state.settings.memory.loading, "loading shows while the read is on its way");
    r.read();
    assert!(!r.exists("memory-loading"));
    assert!(r.rt.state.settings.memory.notes.is_some());
    assert_eq!(r.rt.state.settings.memory.repository.as_deref(), Some("/work/lantern"));
}

#[test]
fn shows_the_servers_message_when_memory_cant_be_read_and_nothing_else() {
    let mut r = rig(|f| f.broken = Some((500, json!({ "error": "The notes folder is locked." }).to_string())));
    r.read();
    assert_eq!(r.text("memory-error"), "The notes folder is locked.");
    assert!(!r.exists("memory-switch") && !r.exists("memory-harnesses"));
}

#[test]
fn says_couldnt_load_memory_when_the_server_gives_no_reason() {
    let mut r = rig(|f| f.broken = Some((500, "<html>".into())));
    r.read();
    assert_eq!(r.text("memory-error"), "Couldn't load memory.");
}

#[test]
fn a_refused_switch_says_why_under_the_row_and_leaves_the_switch_where_it_was() {
    let mut r = rig(|f| f.refuse = Some((409, json!({ "error": "Memory is managed by the host." }).to_string())));
    r.read();
    click(&mut r.rt, "memory-switch");
    r.wait("the refusal", |s| s.settings.memory.error.is_some());
    r.rt.frame(FRAME);
    assert_eq!(r.text("memory-error"), "Memory is managed by the host.");
    assert!(!r.checked("memory-switch"));
    assert!(!r.rt.state.settings.memory.saving, "the lock lets go");
}

#[test]
fn does_one_action_at_a_time_and_clears_the_last_failure_on_the_next() {
    let mut r = rig(|_| {});
    r.read();
    r.rt.update(|s, _| s.settings.memory.error = Some("old".into()));
    assert!(r.rt.state.begin_memory_action());
    assert!(r.rt.state.settings.memory.error.is_none());
    assert!(!r.rt.state.begin_memory_action(), "one at a time");
}

#[test]
fn reads_the_repository_open_memory_asked_for_if_the_machine_lists_it() {
    let mut r = rig(|_| {});
    r.rt.update(|s, _| s.settings.memory = MemoryPage::default());
    r.rt.update(|s, _| s.open_memory("/work/elsewhere"));
    r.read();
    assert_eq!(r.rt.state.settings.memory.repository.as_deref(), Some("/work/lantern"), "an unlisted one falls back to the first");
    assert!(r.rt.state.settings.repository.is_none(), "taken once");
}

#[test]
fn reads_again_when_the_page_is_left_and_come_back_to() {
    let mut r = rig(|_| {});
    r.read();
    let before = r.heard().iter().filter(|(_, p)| p == "/api/memory").count();
    r.rt.update(|s, _| s.open_settings("appearance", None));
    r.rt.frame(FRAME);
    assert_eq!(r.rt.state.settings.memory, MemoryPage::default());
    r.rt.update(|s, _| s.open_settings("memory", None));
    r.read();
    assert_eq!(r.heard().iter().filter(|(_, p)| p == "/api/memory").count(), before + 1);
}

#[test]
fn asks_the_machine_settings_shows_never_home() {
    let home: Shared = Default::default();
    let other: Shared = Default::default();
    other.lock().unwrap().enabled = true;
    let other_base = serve(other.clone());
    home.lock().unwrap().machines = vec![json!({ "id": "m-harbor", "name": "harbor", "baseUrl": other_base, "token": "t-harbor" })];
    let (link, queue) = link_to(serve(home.clone()));
    let mut rt = app();
    rt.update(move |s, _| {
        s.show_settings_machines(true);
        s.link = Some(link);
        let machine = s.machines.list.iter_mut().find(|m| m.id == "m-harbor").expect("harbor");
        machine.base_url = other_base;
        machine.addresses.clear();
        s.rail = Rail::Settings;
        s.open_settings("memory", None);
        s.settings.choice.pick("m-harbor");
    });
    let mut r = Rig { rt, queue, fake: home.clone() };
    assert_eq!(r.rt.state.settings.choice.key(&r.rt.state.machines), "m-harbor");
    assert_ne!(r.rt.state.settings_target().expect("a link").base, r.rt.state.link.as_ref().unwrap().base);
    r.wait("harbor's page", |s| s.settings.memory.enabled());
    assert!(other.lock().unwrap().heard.iter().any(|(_, p)| p == "/api/memory"), "harbor was asked");
    assert!(home.lock().unwrap().heard.iter().all(|(_, p)| !p.starts_with("/api/memory")), "home was not");
    // The switch goes to harbor too.
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    click(&mut r.rt, "memory-switch");
    r.wait("harbor to turn it off", |s| !s.settings.memory.enabled() && !s.settings.memory.saving);
    assert!(!other.lock().unwrap().enabled);
    assert!(home.lock().unwrap().heard.iter().all(|(_, p)| !p.starts_with("/api/memory")));
}

// ─── The model ──────────────────────────────────────────────────────────────────

fn note(kind: &str) -> Note {
    Note {
        id: "n1".into(),
        list: List::Machine,
        text: String::new(),
        kind: kind.into(),
        repository: None,
        session_id: None,
        session_title: None,
        created: String::new(),
        updated: String::new(),
        lifetime: None,
        days_left: None,
        expired: false,
        relearned: 0,
        how: None,
        in_search: false,
        pinned: false,
        found: 0,
        last_found: None,
    }
}

#[test]
fn life_label_says_each_branch() {
    assert_eq!(m::life_label(&note("from-you")), None, "a note from you never expires");
    assert_eq!(m::life_label(&note("added")), None);
    assert_eq!(m::life_label(&note("learned")).as_deref(), Some("Kept for good"));
    let mut n = note("learned");
    n.relearned = 2;
    assert_eq!(m::life_label(&n).as_deref(), Some("Kept for good · learned 3×"));
    n.lifetime = Some(30);
    n.days_left = Some(12);
    assert_eq!(m::life_label(&n).as_deref(), Some("12 more days of use · learned 3×"));
    n.relearned = 0;
    n.days_left = Some(1);
    assert_eq!(m::life_label(&n).as_deref(), Some("1 more day of use"));
    n.days_left = None;
    assert_eq!(m::life_label(&n).as_deref(), Some("30 more days of use"), "no count yet: the whole lifetime");
    n.days_left = Some(0);
    assert_eq!(m::life_label(&n).as_deref(), Some("Last day of use"));
    n.expired = true;
    assert_eq!(m::life_label(&n).as_deref(), Some("Expired after 30 days of use"));
    n.kind = "from-you".into();
    assert_eq!(m::life_label(&n).as_deref(), Some("Expired after 30 days of use"), "a lifetime wins over the kind");
}

#[test]
fn found_label_counts_times_and_gives_the_last_day() {
    let now = 1_791_337_903_000; // 7 October 2026
    let mut n = note("learned");
    assert_eq!(m::found_label(&n, now, 0), None);
    n.found = 1;
    assert_eq!(m::found_label(&n, now, 0).as_deref(), Some("found 1 time"));
    n.found = 3;
    n.last_found = Some("2026-10-02T09:12:00Z".into());
    assert_eq!(m::found_label(&n, now, 0).as_deref(), Some("found 3 times · last Oct 2"));
    n.last_found = Some("2025-12-24T09:12:00Z".into());
    assert_eq!(m::found_label(&n, now, 0).as_deref(), Some("found 3 times · last Dec 24, 2025"));
}

#[test]
fn estimates_a_note_at_four_characters_a_token_with_thirty_for_its_id() {
    assert_eq!(m::estimate_note_tokens(""), 8);
    assert_eq!(m::estimate_note_tokens(&"a".repeat(10)), 10);
    assert_eq!(m::estimate_note_tokens(&"a".repeat(90)), 30);
    assert_eq!(m::estimate_note_tokens("é😀"), 9, "counted in UTF-16 units, as JavaScript counts");
}

#[test]
fn labels_a_notes_kind_and_place() {
    assert_eq!(m::kind_label("from-you"), "From you");
    assert_eq!(m::kind_label("added"), "Added by you");
    assert_eq!(m::kind_label("learned"), "Learned");
    assert_eq!(m::kind_label("something-new"), "Learned");
    assert_eq!(m::memory_place(List::Machine, Some("lantern")), "this machine");
    assert_eq!(m::memory_place(List::Repository, Some("lantern")), "lantern");
    assert_eq!(m::memory_place(List::Repository, None), "this repository");
}

#[test]
fn builds_each_request_as_fleet_does() {
    let r = m::get_memory();
    assert_eq!((r.method, r.path.as_str(), r.fallback.as_str()), ("GET", "/api/memory", "Couldn't load memory."));
    let r = m::set_enabled(true);
    assert_eq!((r.method, r.body, r.fallback.as_str()), ("PUT", Some(json!({ "enabled": true })), "Couldn't turn memory on."));
    assert_eq!(m::set_enabled(false).fallback, "Couldn't turn memory off.");
    assert_eq!(m::list_notes(None).path, "/api/memory/notes");
    assert_eq!(m::list_notes(Some("/work/my repo")).path, "/api/memory/notes?repository=%2Fwork%2Fmy%20repo");
    let r = m::add_note(List::Repository, Some("/work/lantern"), "hi");
    assert_eq!(r.body, Some(json!({ "list": "repository", "repository": "/work/lantern", "text": "hi" })));
    assert_eq!(m::add_note(List::Machine, None, "hi").body, Some(json!({ "list": "machine", "repository": null, "text": "hi" })));
    assert_eq!((m::update_note("a/b", "x").method, m::update_note("a/b", "x").path), ("PUT", "/api/memory/notes/a%2Fb".to_string()));
    assert_eq!(m::keep_note("n1").path, "/api/memory/notes/n1/keep");
    assert_eq!(m::move_note("n1", "search").body, Some(json!({ "group": "search" })));
    assert_eq!(m::move_note("n1", "always").path, "/api/memory/notes/n1/place");
    assert_eq!((m::forget_note("n1").method, m::forget_note("n1").fallback), ("DELETE", "Couldn't forget the note.".to_string()));
    assert_eq!(m::clear("all", None).body, Some(json!({ "scope": "all", "repository": null })));
    assert_eq!(m::clear("repository", Some("/r")).fallback, "Couldn't clear the notes.");
}

#[test]
fn reads_a_refusal_as_fleet_does_and_falls_back_to_each_requests_own_words() {
    let said = |body: &str| m::memory_error(body, "Couldn't save the note.");
    assert_eq!(said(r#"{"error":"Too long."}"#), "Too long.");
    assert_eq!(said(r#"{"detail":"d","title":"t"}"#), "d");
    assert_eq!(said(r#"{"title":"Bad request"}"#), "Bad request");
    assert_eq!(said(r#"{"message":"m"}"#), "m");
    assert_eq!(said(r#"{"error":{"title":"nested"}}"#), "nested");
    assert_eq!(said(r#"{"status":400}"#), "Couldn't save the note.");
    assert_eq!(said("not json"), "Couldn't save the note.");
    assert_eq!(said(""), "Couldn't save the note.");
}

#[test]
fn reads_notes_and_the_overview_and_the_two_events() {
    let overview =
        m::parse_overview(&json!({ "enabled": true, "repositories": [{ "path": "/r", "name": "r", "count": 2 }, { "name": "no path" }],
        "machineCount": 1, "maxRepositoryNotes": 40, "maxMachineNotes": 15, "searchEnabled": true, "maxAlwaysMachineNotes": 5 }))
        .expect("an overview");
    assert!(overview.enabled && overview.search_enabled);
    assert_eq!(overview.repositories.len(), 1, "a repository without a path is dropped");
    assert_eq!((overview.max_always_machine_notes, overview.max_always_repository_notes), (Some(5), None));
    assert!(m::parse_overview(&Value::Null).is_none());
    let notes = m::parse_notes(&json!({ "repository": null, "repositoryNotes": [{ "id": "a", "list": "repository", "text": "t", "kind": "from-you", "pinned": true },
        { "text": "no id" }], "machineNotes": [], "tokens": 640, "expiredNotes": [{ "id": "e", "expired": true }] }))
    .expect("notes");
    assert_eq!((notes.repository_notes.len(), notes.expired_notes.len(), notes.tokens), (1, 1, 640));
    assert!(notes.repository_notes[0].pinned && notes.expired_notes[0].expired);
    let saved = crate::fleet::settings::memory_events::parse_saved(&json!({ "note": { "id": "n", "list": "machine", "text": "t" },
        "repositoryName": "lantern", "previous": { "id": "p", "text": "old" } }))
    .expect("a saved note");
    assert_eq!(
        (saved.note.id.as_str(), saved.repository_name.as_deref(), saved.previous.map(|p| p.text)),
        ("n", Some("lantern"), Some("old".into()))
    );
    assert!(crate::fleet::settings::memory_events::parse_saved(&json!({ "note": { "text": "no id" } })).is_none());
    assert_eq!(crate::fleet::settings::memory_events::dreamed_id(&json!({ "id": "d1" })).as_deref(), Some("d1"));
    assert!(crate::fleet::settings::memory_events::dreamed_id(&json!({})).is_none());
}
