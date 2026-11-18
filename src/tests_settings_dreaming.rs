//! Component 16, piece R: Settings → Memory's Dreaming panel. `DreamingMemory.test.ts`'s cases (the panel's, not the nightly
//! row's) under Fleet's names, then what the panel does that they leave to the template: the poll, a stale answer, the
//! error that clears, and one action at a time. The server is a fake that keeps one dream and changes it as the real one does.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::new_session::harness_choice::{HarnessEntry, Harnesses};
use crate::tests::{FRAME, click};
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fake {
    dreams: Vec<Value>,
    /// Answer the dreams list with a refusal this many times.
    refuse_lists: usize,
    memory_on: bool,
    /// What the nightly row's read gets; none is a refusal.
    nightly: Value,
    heard: Vec<(String, String, Value)>,
}

type Shared = Arc<Mutex<Fake>>;

fn change(id: &str, kind: &str, text: &str) -> Value {
    json!({ "id": id, "kind": kind, "list": "repository", "noteId": format!("n-{id}"), "text": text, "replaced": [], "why": "Seen in two sessions.",
            "sessions": [], "ofSessions": 0, "quote": null, "reason": null, "undone": false })
}

fn dream(status: &str, changes: Vec<Value>) -> Value {
    json!({ "id": "d1", "repository": "/work/lantern", "repositoryName": "lantern", "sessionId": "dream-session", "harnessType": "opencode2",
            "status": status, "error": null, "started": "2026-10-20T03:00:00Z", "finished": "2026-10-20T03:09:00Z",
            "readSessions": [{ "id": "s1", "title": "One", "harnessType": null }, { "id": "s2", "title": "Two", "harnessType": null }],
            "changes": changes, "runId": "run-1", "tokens": 48200, "since": null })
}

fn answer(mut stream: std::net::TcpStream, fake: Shared) {
    let req = crate::test_http::read(&mut stream);
    let body = req.json();
    let head = req.head;
    let mut first = head.lines().next().unwrap_or("").split(' ');
    let (method, path) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
    let mut f = fake.lock().unwrap();
    // The live-events link negotiates in the background at any moment; it is not what a test asks about.
    if !path.starts_with("/hubs/") {
        f.heard.push((method.clone(), path.clone(), body.clone()));
    }
    let route = path.split('?').next().unwrap_or("").to_string();
    let segments: Vec<&str> = route.trim_start_matches('/').split('/').collect();
    let (status, reply) = match (method.as_str(), segments.as_slice()) {
        ("GET", ["api", "memory"]) => (
            200,
            json!({ "enabled": f.memory_on, "repositories": [{ "path": "/work/lantern", "name": "lantern", "count": 1 }], "machineCount": 0,
                    "maxRepositoryNotes": 40, "maxMachineNotes": 15 }),
        ),
        ("GET", ["api", "memory", "notes"]) => {
            (200, json!({ "repository": "/work/lantern", "repositoryNotes": [], "machineNotes": [], "tokens": 0, "expiredNotes": [] }))
        }
        ("GET", ["api", "memory", "dreaming", "nightly"]) => (200, f.nightly.clone()),
        ("GET", ["api", "memory", "dreams"]) => {
            if f.refuse_lists > 0 {
                f.refuse_lists -= 1;
                (500, json!({}))
            } else {
                (200, json!({ "dreams": f.dreams }))
            }
        }
        ("POST", ["api", "memory", "dreams"]) => {
            let mut started = dream("running", vec![]);
            started["harnessType"] = body["harnessType"].clone();
            started["runId"] = Value::Null;
            f.dreams.insert(0, started.clone());
            (200, started)
        }
        ("POST", ["api", "memory", "dreams", _, "changes", change_id, "undo"]) => {
            for c in f.dreams[0]["changes"].as_array_mut().into_iter().flatten().filter(|c| c["id"] == *change_id) {
                c["undone"] = json!(true);
            }
            (200, f.dreams[0].clone())
        }
        _ => (200, json!({})),
    };
    let reply = reply.to_string();
    let out = format!(
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(out.as_bytes());
    drop(f);
    crate::tests::finish(&mut stream);
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Shared,
}

fn rig(setup: impl FnOnce(&mut Fake)) -> Rig {
    let fake: Shared = Default::default();
    {
        let mut f = fake.lock().unwrap();
        f.memory_on = true;
        f.nightly = json!({ "enabled": false, "time": "03:00", "budgetTokens": 300000 });
        setup(&mut f);
    }
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let served = fake.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let fake = served.clone();
            std::thread::spawn(move || answer(stream, fake));
        }
    });
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(base, Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = crate::tests::app_tall();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.rail = Rail::Settings;
        let entry = |kind: &str, name: &str| HarnessEntry { display_name: name.into(), ..HarnessEntry::ready(kind) };
        let all = vec![entry("opencode2", "OpenCode 2"), entry("claude-code", "Claude Code")];
        s.feeds.harnesses.entry("home".into()).or_default().harnesses = Harnesses { all, ..Default::default() };
        s.open_settings("memory", None);
    });
    let mut r = Rig { rt, queue, fake };
    r.settle("the page to be read", |s| !s.settings.memory.loading && s.settings.memory.notes.is_some());
    r
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("{:?}", s.settings.memory.dreaming));
    }
    fn settle(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        self.wait(what, done);
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }
    /// The panel has asked for the dream and had its answer.
    fn dreamed(&mut self) {
        self.settle("the dreams to be read", |s| {
            let d = &s.settings.memory.dreaming;
            d.asked.is_some() && !d.reading
        });
    }
    fn press(&mut self, key: &str) {
        click(&mut self.rt, key);
        self.settle("the action to finish", |s| {
            let d = &s.settings.memory.dreaming;
            !d.starting && d.undoing.is_none() && !s.settings.memory.saving
        });
    }
    fn exists(&self, key: &str) -> bool {
        self.rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }
    fn text(&self, key: &str) -> String {
        let all = self.rt.elements();
        let it = all.iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element keyed {key:?}"));
        all.iter().filter(|e| e.path.starts_with(&it.path)).filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("")
    }
    fn heard(&self, method: &str, prefix: &str) -> Vec<(String, Value)> {
        self.fake
            .lock()
            .unwrap()
            .heard
            .iter()
            .filter(|(m, p, _)| m == method && p.starts_with(prefix))
            .map(|(_, p, b)| (p.clone(), b.clone()))
            .collect()
    }
}

fn flag(id: &str) -> Value {
    let mut c = change(id, "flagged", "Run the suite with `make check`.");
    c["evidence"] = json!("The Makefile has no check target.");
    c["quote"] = json!("make: *** No rule to make target 'check'.");
    c["sessions"] = json!([{ "id": "s9", "title": "Fix the build", "harnessType": "claude-code" }]);
    c
}

#[test]
fn renders_nothing_and_asks_for_no_dreams_while_the_switch_is_off() {
    let mut r = rig(|f| {
        f.memory_on = false;
        f.dreams = vec![dream("done", vec![change("c1", "added", "A note.")])];
    });
    r.rt.frame(FRAME);
    assert!(!r.exists("dreaming-panel"));
    assert!(r.heard("GET", "/api/memory/dreams").is_empty());
}

#[test]
fn shows_a_confirmed_note_once_as_tested_and_counts_it_apart_from_the_notes_changed() {
    let mut confirmed = change("c2", "confirmed", "Releases are cut from `release/`.");
    confirmed["how"] = json!("read the release script");
    let mut r = rig(|f| f.dreams = vec![dream("done", vec![change("c1", "added", "Run just lint."), confirmed])]);
    r.dreamed();
    assert!(r.text("dream-heading").contains("1 note changed, 1 confirmed"), "{}", r.text("dream-heading"));
    assert_eq!(r.text("dream-how-c2"), "Tested · read the release script");
    assert!(!r.exists("dream-how-c1"));
}

#[test]
fn shows_the_latest_dreams_cards_evidence_and_the_skipped_group() {
    let mut why = change("c1", "rewritten", "New words.");
    why["replaced"] = json!([{ "id": "old", "text": "Old words." }]);
    why["evidence"] = json!("tests/render.rs shows it.");
    why["quote"] = json!("a quoted line");
    why["sessions"] = json!([{ "id": "s1", "title": "One", "harnessType": "claude-code" }]);
    why["ofSessions"] = json!(3);
    let mut r = rig(|f| {
        f.dreams = vec![
            dream("done", vec![why, change("c3", "skipped", "Seen once."), change("c4", "left", "Left alone.")]),
            dream("done", vec![change("older", "added", "An older dream.")]),
        ]
    });
    r.dreamed();
    assert!(r.exists("dream-change-c1") && !r.exists("dream-change-older"), "only the newest dream shows");
    assert!(r.text("dream-change-c1").contains("Old words.") && r.text("dream-change-c1").contains("now reads"));
    assert!(r.text("dream-change-c4").contains("Left alone."));
    assert!(!r.exists("dream-evidence-c1"), "Why starts closed");
    click(&mut r.rt, "dream-why-c1");
    r.rt.frame(FRAME);
    let evidence = r.text("dream-evidence-c1");
    assert!(evidence.contains("tests/render.rs shows it.") && evidence.contains("1 of 3 sessions") && evidence.contains("a quoted line"));
    assert!(evidence.contains("One · Claude Code"));
    assert!(!r.exists("dream-fold-list"));
    click(&mut r.rt, "dream-skipped");
    r.rt.frame(FRAME);
    assert_eq!(r.text("dream-skipped"), "Seen in one session only, not changed (1)");
    assert_eq!(r.text("dream-fold-list"), "Seen once.");
}

#[test]
fn undoes_a_change_shows_undone_and_reads_the_notes_again() {
    let mut r = rig(|f| f.dreams = vec![dream("done", vec![change("c1", "added", "Run just lint.")])]);
    r.dreamed();
    let reads = r.heard("GET", "/api/memory/notes").len();
    r.press("dream-undo-c1");
    assert_eq!(r.heard("POST", "/api/memory/dreams/d1/changes/c1/undo").len(), 1);
    assert_eq!(r.text("dream-undo-c1"), "Undone");
    let fake = r.fake.clone();
    r.wait("the notes to be read again", move |_| {
        fake.lock().unwrap().heard.iter().filter(|(m, p, _)| m == "GET" && p.starts_with("/api/memory/notes")).count() > reads
    });
    assert!(r.heard("GET", "/api/memory/notes").len() > reads, "the notes are read again");
    assert!(r.text("dream-heading").contains("0 notes changed"));
}

#[test]
fn shows_looks_out_of_date_with_the_evidence_and_counts_no_note_as_changed() {
    let mut r = rig(|f| f.dreams = vec![dream("done", vec![flag("f1")])]);
    r.dreamed();
    assert!(r.text("dream-heading").contains("0 notes changed"));
    let card = r.text("dream-flag-f1");
    assert!(card.contains("LOOKS OUT OF DATE") && card.contains("YOUR NOTE") && card.contains("Run the suite with `make check`."));
    assert!(card.contains("The Makefile has no check target.") && card.contains("make: *** No rule"));
    assert!(card.contains("Fix the build · Claude Code"));
}

#[test]
fn keep_dismisses_the_flag_and_changes_no_note() {
    let mut r = rig(|f| f.dreams = vec![dream("done", vec![flag("f1")])]);
    r.dreamed();
    let reads = r.heard("GET", "/api/memory/notes").len();
    r.press("dream-flag-keep");
    assert!(!r.exists("dream-flag-f1"));
    assert_eq!(r.heard("POST", "/api/memory/dreams/d1/changes/f1/undo").len(), 1);
    assert!(r.heard("DELETE", "/api/memory/notes").is_empty() && r.heard("PUT", "/api/memory/notes").is_empty());
    assert_eq!(r.heard("GET", "/api/memory/notes").len(), reads, "no note changed, so the notes are not read again");
}

#[test]
fn forget_removes_the_note_then_dismisses_the_flag() {
    let mut r = rig(|f| f.dreams = vec![dream("done", vec![flag("f1")])]);
    r.dreamed();
    r.press("dream-flag-forget");
    assert_eq!(r.heard("DELETE", "/api/memory/notes/n-f1").len(), 1);
    assert_eq!(r.heard("POST", "/api/memory/dreams/d1/changes/f1/undo").len(), 1);
    assert!(!r.exists("dream-flag-f1"));
    let order: Vec<String> = r.fake.lock().unwrap().heard.iter().map(|(m, p, _)| format!("{m} {p}")).collect();
    let at = |what: &str| order.iter().position(|o| o.starts_with(what)).expect(what);
    assert!(at("DELETE") < at("POST /api/memory/dreams/d1"));
}

#[test]
fn edit_saves_the_new_wording_then_dismisses_the_flag() {
    let mut r = rig(|f| f.dreams = vec![dream("done", vec![flag("f1")])]);
    r.dreamed();
    click(&mut r.rt, "dream-flag-edit");
    r.rt.frame(FRAME);
    assert_eq!(r.rt.state.settings.memory.dreaming.edit.text, "Run the suite with `make check`.", "the box starts with the note's words");
    r.rt.update(|s, _| s.settings.memory.dreaming.edit = rune::TextEdit::new(""));
    r.rt.frame(FRAME);
    r.rt.update(|s, _| s.settings.memory.dreaming.edit = rune::TextEdit::new("Run the suite with `just check`."));
    r.rt.frame(FRAME);
    r.press("dream-flag-save");
    assert_eq!(
        r.heard("PUT", "/api/memory/notes/n-f1"),
        vec![("/api/memory/notes/n-f1".to_string(), json!({ "text": "Run the suite with `just check`." }))]
    );
    assert_eq!(r.heard("POST", "/api/memory/dreams/d1/changes/f1/undo").len(), 1);
    assert!(!r.exists("dream-flag-f1") && r.rt.state.settings.memory.dreaming.editing.is_none());
}

#[test]
fn a_flag_with_no_note_cannot_be_edited_or_forgotten_and_an_empty_edit_cannot_be_saved() {
    let mut orphan = flag("f1");
    orphan["noteId"] = Value::Null;
    let mut r = rig(|f| f.dreams = vec![dream("done", vec![orphan])]);
    r.dreamed();
    click(&mut r.rt, "dream-flag-edit");
    click(&mut r.rt, "dream-flag-forget");
    r.rt.frame(FRAME);
    assert!(r.rt.state.settings.memory.dreaming.editing.is_none());
    assert!(r.heard("DELETE", "/api/memory/notes").is_empty());
    r.press("dream-flag-keep");
    assert!(!r.exists("dream-flag-f1"), "Keep still works");
}

#[test]
fn names_a_nights_dream_and_its_tokens_and_shows_a_kept_note_with_undo() {
    let mut r = rig(|f| f.dreams = vec![dream("done", vec![change("c1", "kept", "Releases come from `release/`.")])]);
    r.dreamed();
    assert_eq!(r.text("dream-heading"), "Last night · lantern · 1 note changed");
    assert!(r.text("dream-meta").starts_with("Read 2 sessions · 48.2k tokens · Oct 20, "), "{}", r.text("dream-meta"));
    assert!(r.text("dream-meta").ends_with("Open the dreaming session"));
    assert!(r.text("dream-change-c1").contains("KEPT FOR GOOD") && r.exists("dream-undo-c1"));
}

#[test]
fn starts_a_dream_on_the_chosen_harness_and_shows_it_running() {
    let mut r = rig(|_| {});
    r.dreamed();
    assert!(!r.exists("dream-running"));
    r.rt.update(|s, _| s.dreaming_pick_harness("claude-code"));
    r.press("dream-now");
    assert_eq!(
        r.heard("POST", "/api/memory/dreams"),
        vec![("/api/memory/dreams".to_string(), json!({ "repository": "/work/lantern", "harnessType": "claude-code" }))]
    );
    assert_eq!(r.text("dream-running"), "Dreaming… reading 2 sessionsOpen the dreaming session");
    click(&mut r.rt, "dream-now");
    r.rt.frame(FRAME);
    assert_eq!(r.heard("POST", "/api/memory/dreams").len(), 1, "Dream now is off while a dream is running");
}

#[test]
fn dream_now_follows_the_nightly_harness_until_one_is_picked_here() {
    let mut r = rig(|f| f.nightly = json!({ "enabled": true, "harnessType": "claude-code", "time": "03:00", "budgetTokens": 300000 }));
    r.dreamed();
    r.settle("the panel to hear the row's harness", |s| s.settings.memory.dreaming.nightly_harness.is_some());
    r.press("dream-now");
    assert_eq!(r.heard("POST", "/api/memory/dreams")[0].1["harnessType"], "claude-code");
}

#[test]
fn asks_again_every_three_seconds_while_busy_and_reads_the_notes_when_it_ends() {
    let mut r = rig(|f| f.dreams = vec![dream("running", vec![])]);
    r.dreamed();
    assert!(r.exists("dream-running"));
    let asked = r.heard("GET", "/api/memory/dreams").len();
    r.rt.frame(1.0);
    assert_eq!(r.heard("GET", "/api/memory/dreams").len(), asked, "not before three seconds");
    let reads = r.heard("GET", "/api/memory/notes").len();
    r.fake.lock().unwrap().dreams = vec![dream("done", vec![change("c1", "added", "A note.")])];
    r.rt.frame(3.5);
    r.settle("the finished dream to show", |s| {
        !s.settings.memory.dreaming.busy() && s.settings.memory.dreaming.dream.is_some() && !s.settings.memory.saving
    });
    assert!(r.heard("GET", "/api/memory/dreams").len() > asked);
    assert!(r.heard("GET", "/api/memory/notes").len() > reads, "the notes are read again when the dream stops being busy");
    assert!(r.exists("dream-change-c1"));
}

#[test]
fn a_failed_dream_says_why_and_with_no_reason_says_so() {
    let mut failed = dream("failed", vec![]);
    failed["error"] = json!("The harness stopped.");
    let mut r = rig(|f| f.dreams = vec![failed]);
    r.dreamed();
    assert_eq!(r.text("dream-failed"), "The last dream failed: The harness stopped.");
    r.fake.lock().unwrap().dreams = vec![dream("failed", vec![])];
    r.rt.update(|s, _| s.settings.memory.dreaming.asked = None);
    r.dreamed();
    r.settle("the new answer", |s| s.settings.memory.dreaming.dream.as_ref().is_some_and(|d| d.error.is_none()));
    assert_eq!(r.text("dream-failed"), "The last dream failed: no reason given.");
}

#[test]
fn a_failed_read_says_so_and_the_next_good_poll_clears_it() {
    let mut r = rig(|f| {
        f.refuse_lists = 1;
        f.dreams = vec![dream("running", vec![])];
    });
    r.settle("the failed read", |s| s.settings.memory.dreaming.error.is_some());
    assert_eq!(r.text("dreaming-error"), "Couldn't load the dreams.");
    r.rt.update(|s, _| s.settings.memory.dreaming.dream = r_running());
    r.rt.frame(3.5);
    r.settle("the next good read", |s| s.settings.memory.dreaming.error.is_none() && s.settings.memory.dreaming.dream.is_some());
    assert!(!r.exists("dreaming-error"));
}

fn r_running() -> Option<crate::fleet::settings::dreaming::Dream> {
    crate::fleet::settings::dreaming::parse_dream(&dream("running", vec![]))
}

#[test]
fn another_error_is_not_cleared_by_a_good_read() {
    let mut r = rig(|f| f.dreams = vec![dream("running", vec![])]);
    r.dreamed();
    r.rt.update(|s, _| s.settings.memory.dreaming.error = Some("Couldn't start the dream.".into()));
    r.rt.frame(3.5);
    r.settle("a read", |s| !s.settings.memory.dreaming.reading);
    assert_eq!(r.rt.state.settings.memory.dreaming.error.as_deref(), Some("Couldn't start the dream."));
}

#[test]
fn the_harness_pick_is_kept_for_the_install() {
    let mut r = rig(|_| {});
    r.dreamed();
    r.rt.update(|s, _| s.dreaming_pick_harness("claude-code"));
    assert_eq!(r.rt.state.sidebar.dreaming_harness(), Some("claude-code"));
    assert!(r.rt.state.sidebar.to_json().contains("\"dreamingHarness\": \"claude-code\""));
    let back = crate::fleet::settings::Sidebar::parse(&r.rt.state.sidebar.to_json());
    assert_eq!(back.dreaming_harness(), Some("claude-code"));
}
