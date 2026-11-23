//! Component 16, piece S: Settings → Memory's "Dream each night" row and the morning dream notice. `DreamingMemory.test.ts`'s
//! three nightly cases and `use-dream-notices.test.ts`'s six, under Fleet's names, then what the row does that they leave to
//! the template. The server is a fake that keeps one nightly view and changes it as the real one does.
use crate::fleet::events::{Event, Kind};
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::new_session::harness_choice::{HarnessEntry, Harnesses};
use crate::fleet::state::Topic;
use crate::tests::{FRAME, app, click};
use crate::{Rail, State, test_wait};
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::{Value, json};
use std::io::Write;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fake {
    nightly: Value,
    /// Answer the nightly read with a refusal this many times.
    refuse_reads: usize,
    /// Answer the nightly save with a refusal.
    refuse_save: Option<String>,
    memory_on: bool,
    heard: Vec<(String, String, Value)>,
}

type Shared = Arc<Mutex<Fake>>;

fn nightly_view(fields: Value) -> Value {
    let mut v = json!({ "enabled": false, "automationId": null, "paused": false, "time": "03:00", "timeZone": null, "harnessType": null,
        "model": null, "budgetTokens": 300000, "nextRunAt": null, "recent": [] });
    for (k, val) in fields.as_object().expect("an object") {
        v[k] = val.clone();
    }
    v
}

fn change(kind: &str, undone: bool) -> Value {
    json!({ "id": format!("c-{kind}"), "kind": kind, "list": "repository", "noteId": "n1", "text": "A note.", "replaced": [], "why": "Seen twice.",
            "sessions": [], "ofSessions": 0, "quote": null, "reason": null, "undone": undone })
}

fn dream(id: &str, run: Option<&str>, changes: Vec<Value>) -> Value {
    let sessions: Vec<Value> =
        (0..14).map(|i| json!({ "id": format!("s{i}"), "title": format!("Session {i}"), "harnessType": null })).collect();
    json!({ "id": id, "repository": "/work/lantern", "repositoryName": "lantern", "sessionId": null, "harnessType": "opencode2",
            "status": "done", "error": null, "started": "2026-10-20T03:00:00Z", "finished": "2026-10-20T03:09:00Z",
            "readSessions": sessions, "changes": changes, "runId": run, "tokens": 1000, "since": null })
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
        ("GET" | "PUT", ["api", "memory"]) => (
            {
                if method == "PUT" {
                    f.memory_on = body["enabled"] == true;
                }
                200
            },
            json!({ "enabled": f.memory_on, "repositories": [{ "path": "/work/lantern", "name": "lantern", "count": 1 }], "machineCount": 0,
                    "maxRepositoryNotes": 40, "maxMachineNotes": 15 }),
        ),
        ("GET", ["api", "memory", "notes"]) => {
            (200, json!({ "repository": "/work/lantern", "repositoryNotes": [], "machineNotes": [], "tokens": 0, "expiredNotes": [] }))
        }
        ("GET", ["api", "harnesses"]) => (
            200,
            json!([
                { "type": "opencode2", "displayName": "OpenCode 2", "available": true, "userEnabled": true, "state": "ready" },
                { "type": "claude-code", "displayName": "Claude Code", "available": true, "userEnabled": true, "state": "ready" },
                { "type": "pi", "displayName": "Pi", "available": true, "userEnabled": true, "state": "ready" },
            ]),
        ),
        ("GET", ["api", "memory", "dreams"]) => (200, json!({ "dreams": [] })),
        ("GET", ["api", "memory", "dreaming", "nightly"]) => {
            if f.refuse_reads > 0 {
                f.refuse_reads -= 1;
                (500, json!({ "error": "Nightly dreaming is unavailable." }))
            } else {
                (200, f.nightly.clone())
            }
        }
        ("PUT", ["api", "memory", "dreaming", "nightly"]) => match f.refuse_save.clone() {
            Some(why) => (500, json!({ "error": why })),
            None => {
                for (k, v) in body.as_object().into_iter().flatten() {
                    // The server reads an empty model as "back to the harness's own".
                    f.nightly[k] = if k == "model" && v == "" { Value::Null } else { v.clone() };
                }
                if body.get("enabled") == Some(&json!(true)) {
                    f.nightly["automationId"] = json!("auto-1");
                }
                (200, f.nightly.clone())
            }
        },
        ("GET", ["api", "harnesses", _, "catalog"]) => (
            200,
            json!({ "supported": true, "agents": [], "providers": [{ "id": "anthropic", "name": "Anthropic",
                "models": [{ "id": "sonnet", "name": "Claude Sonnet" }, { "id": "haiku", "name": "Claude Haiku" }] }] }),
        ),
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

/// An app on Settings → Memory (or, with `page` false, on Sessions with Memory on) against the fake.
fn rig(page: bool, setup: impl FnOnce(&mut Fake)) -> Rig {
    let fake: Shared = Default::default();
    {
        let mut f = fake.lock().unwrap();
        f.memory_on = true;
        f.nightly = nightly_view(json!({}));
        setup(&mut f);
    }
    let listener = crate::test_http::bind().expect("a port");
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
    let mut rt = app();
    let on = fake.lock().unwrap().memory_on;
    rt.update(move |s, _| {
        s.link = Some(link);
        let entry = |kind: &str, name: &str| HarnessEntry { display_name: name.into(), ..HarnessEntry::ready(kind) };
        let all = vec![entry("opencode2", "OpenCode 2"), entry("claude-code", "Claude Code"), entry("pi", "Pi")];
        s.feeds.harnesses.entry("home".into()).or_default().harnesses = Harnesses { all, ..Default::default() };
        s.pickers.prefs.set("Memory", if on { "true" } else { "false" });
        if page {
            s.rail = Rail::Settings;
            s.open_settings("memory", None);
        }
    });
    let mut r = Rig { rt, queue, fake };
    if page {
        r.settle("the page to be read", |s| !s.settings.memory.loading && s.settings.memory.overview.is_some());
    }
    r
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("{:?}", s.settings.memory.nightly));
    }
    fn settle(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        self.wait(what, done);
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }
    /// The row has asked for its answer and had it.
    fn row(&mut self) {
        self.settle("the nightly answer", |s| {
            let n = &s.settings.memory.nightly;
            n.asked.is_some() && !n.reading && (n.nightly.is_some() || n.error.is_some())
        });
    }
    fn press(&mut self, key: &str) {
        click(&mut self.rt, key);
        self.settle("the save to finish", |s| !s.settings.memory.nightly.saving);
    }
    fn exists(&self, key: &str) -> bool {
        self.rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }
    fn text(&self, key: &str) -> String {
        let all = self.rt.elements();
        let it = all.iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| {
            let keys: Vec<_> = all.iter().filter_map(|e| e.key.clone()).filter(|k| k.contains("dream")).collect();
            panic!("no element keyed {key:?}; dream keys: {keys:?}")
        });
        all.iter().filter(|e| e.path.starts_with(&it.path)).filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("")
    }
    fn heard(&self, method: &str, prefix: &str) -> Vec<Value> {
        self.fake.lock().unwrap().heard.iter().filter(|(m, p, _)| m == method && p.starts_with(prefix)).map(|(_, _, b)| b.clone()).collect()
    }
    fn key(&mut self, key: Key) {
        self.rt.key(KeyEvent { key, mods: Mods::default() });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }
    fn type_time(&mut self, words: &str) {
        click(&mut self.rt, "dream-time");
        self.rt.update(|s, _| s.settings.memory.nightly.time.replace_text(""));
        self.rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }
    fn pick(&mut self, select: &str, option: &str) {
        click(&mut self.rt, select);
        click(&mut self.rt, option);
        self.settle("the save to finish", |s| !s.settings.memory.nightly.saving);
    }
}

fn zone() -> Value {
    crate::fleet::settings::dream_nightly::local_time_zone().map_or(Value::Null, Value::from)
}

// DreamingMemory.test.ts: "shows no nightly row and asks nothing while the switch is off"
#[test]
fn shows_no_nightly_row_and_asks_nothing_while_the_switch_is_off() {
    let mut r = rig(true, |f| f.memory_on = false);
    r.rt.frame(FRAME);
    assert!(!r.exists("dream-nightly"));
    assert!(r.heard("GET", "/api/memory/dreaming").is_empty());
}

// DreamingMemory.test.ts: "turns nightly dreaming on with the time zone, then saves the time and the budget"
#[test]
fn turns_nightly_dreaming_on_with_the_time_zone_then_saves_the_time_and_the_budget() {
    let mut r = rig(true, |_| {});
    r.row();
    assert!(r.exists("dream-nightly") && !r.exists("dream-time"), "the controls wait for the switch");
    assert!(r.text("dream-nightly").contains("Dream each night") && r.text("dream-nightly").contains("EXPERIMENTAL"));
    r.press("dream-nightly-switch");
    let saves = r.heard("PUT", "/api/memory/dreaming/nightly");
    assert_eq!(saves.last().unwrap()["enabled"], true);
    assert_eq!(saves.last().unwrap().get("timeZone").cloned().unwrap_or(Value::Null), zone());
    assert!(r.text("dream-budget").contains("Up to 300k tokens a night"), "{}", r.text("dream-budget"));
    r.type_time("04:30");
    r.key(Key::Enter);
    r.settle("the time to be saved", |s| !s.settings.memory.nightly.saving);
    let saves = r.heard("PUT", "/api/memory/dreaming/nightly");
    let last = saves.last().unwrap();
    assert_eq!(last["time"], "04:30");
    assert_eq!(last.get("timeZone").cloned().unwrap_or(Value::Null), zone());
    r.pick("dream-budget", "dream-budget-list-opt3");
    assert_eq!(r.heard("PUT", "/api/memory/dreaming/nightly").last().unwrap(), &json!({ "budgetTokens": 1000000 }));
    assert!(r.text("dream-budget").contains("Up to 1M tokens a night"));
}

// DreamingMemory.test.ts: "says when the automation is paused, and links to it"
#[test]
fn says_when_the_automation_is_paused_and_links_to_it() {
    let mut r = rig(true, |f| f.nightly = nightly_view(json!({ "enabled": true, "paused": true, "automationId": "a1" })));
    r.row();
    assert_eq!(r.text("dream-paused"), "Paused in Automations");
    click(&mut r.rt, "dream-automation");
    assert_eq!(r.rt.state.rail, Rail::Automations);
}

// DreamingMemory.test.ts (weave-fleet #1051): "says when the nightly read fails, and shows the row once Try again works"
#[test]
fn says_when_the_nightly_read_fails_and_shows_the_row_once_try_again_works() {
    let mut r = rig(true, |f| f.refuse_reads = 2);
    r.row();
    assert!(r.text("dream-nightly-error").contains("Nightly dreaming is unavailable."));
    assert!(!r.exists("dream-nightly"));
    click(&mut r.rt, "dream-nightly-retry");
    r.settle("the row to show", |s| s.settings.memory.nightly.nightly.is_some());
    assert!(!r.exists("dream-nightly-error"));
    assert!(r.exists("dream-nightly"));
}

#[test]
fn a_save_that_is_refused_says_why_inside_the_row_and_keeps_the_row() {
    let mut r = rig(true, |f| f.refuse_save = Some("Automations are switched off.".into()));
    r.row();
    r.press("dream-nightly-switch");
    assert_eq!(r.text("dream-nightly-save-error"), "Automations are switched off.");
    assert!(r.exists("dream-nightly") && !r.exists("dream-time"), "still off");
}

#[test]
fn offers_only_the_two_harnesses_and_a_pick_clears_the_model() {
    let mut r =
        rig(true, |f| f.nightly = nightly_view(json!({ "enabled": true, "harnessType": "opencode2", "model": "anthropic/sonnet" })));
    r.row();
    r.settle("the models", |s| s.nightly_models().is_some());
    assert_eq!(r.text("dream-nightly-model"), "Claude Sonnet");
    click(&mut r.rt, "dream-nightly-harness");
    assert!(r.exists("dream-nightly-harness-list-opt1") && !r.exists("dream-nightly-harness-list-opt2"), "Pi is not offered");
    click(&mut r.rt, "dream-nightly-harness-list-opt1");
    r.settle("the save", |s| !s.settings.memory.nightly.saving);
    assert_eq!(r.heard("PUT", "/api/memory/dreaming/nightly").last().unwrap(), &json!({ "harnessType": "claude-code", "model": "" }));
    r.settle("the new harness's models", |s| {
        s.settings.memory.nightly.catalog_for.as_deref() == Some("claude-code") && s.nightly_models().is_some()
    });
    assert_eq!(r.text("dream-nightly-model"), "Default model");
}

#[test]
fn a_model_pick_sends_provider_slash_model() {
    let mut r = rig(true, |f| f.nightly = nightly_view(json!({ "enabled": true, "harnessType": "opencode2" })));
    r.row();
    r.settle("the models", |s| s.nightly_models().is_some());
    r.pick("dream-nightly-model", "dream-nightly-model-list-opt2");
    assert_eq!(r.heard("PUT", "/api/memory/dreaming/nightly").last().unwrap(), &json!({ "model": "anthropic/haiku" }));
}

#[test]
fn the_time_box_says_when_it_is_not_a_time_and_an_empty_one_changes_nothing() {
    let mut r = rig(true, |f| f.nightly = nightly_view(json!({ "enabled": true, "time": "03:00" })));
    r.row();
    r.type_time("25:99");
    r.key(Key::Enter);
    assert_eq!(r.text("dream-time-error"), "Use the 24-hour form, like 03:30.");
    assert!(r.heard("PUT", "/api/memory/dreaming/nightly").is_empty());
    r.type_time("");
    r.key(Key::Enter);
    assert!(!r.exists("dream-time-error") && r.heard("PUT", "/api/memory/dreaming/nightly").is_empty());
    assert_eq!(r.rt.state.settings.memory.nightly.time.text, "03:00");
    r.type_time("3:15");
    r.key(Key::Enter);
    r.settle("the save", |s| !s.settings.memory.nightly.saving);
    assert_eq!(r.heard("PUT", "/api/memory/dreaming/nightly").last().unwrap()["time"], "03:15");
}

#[test]
fn dream_now_follows_the_nightly_harness_while_the_row_is_on() {
    let mut r = rig(true, |f| f.nightly = nightly_view(json!({ "enabled": true, "harnessType": "claude-code" })));
    r.row();
    r.settle("the panel to hear it", |s| s.settings.memory.dreaming.nightly_harness.is_some());
    assert_eq!(r.rt.state.dreaming_harness_type(), "claude-code");
    r.press("dream-nightly-switch");
    r.settle("the panel to let go", |s| s.settings.memory.dreaming.nightly_harness.is_none());
}

#[test]
fn goes_when_memory_goes_off_and_asks_again_when_it_comes_back() {
    let mut r = rig(true, |_| {});
    r.row();
    assert!(r.exists("dream-nightly"));
    click(&mut r.rt, "memory-switch");
    r.settle("memory to be off", |s| !s.settings.memory.enabled() && !s.settings.memory.saving);
    assert!(!r.exists("dream-nightly") && r.rt.state.settings.memory.nightly.nightly.is_none());
    r.fake.lock().unwrap().memory_on = true;
    click(&mut r.rt, "memory-switch");
    r.row();
    assert!(r.exists("dream-nightly"));
}

// ─── The dream notice ───────────────────────────────────────────────────────

fn dreamed(r: &mut Rig, payload: Value) {
    r.rt.update(move |s, _| s.dispatch(&Topic::Sessions, &[Event::Other(Kind::MemoryDreamed, payload)]));
    r.rt.frame(FRAME);
}

fn notice(r: &Rig, id: &str) -> Option<crate::fleet::notices::Notice> {
    r.rt.state.notices.notices().iter().find(|n| n.id == id).cloned()
}

fn recent_with(dreams: Vec<Value>) -> impl FnOnce(&mut Fake) {
    move |f| f.nightly = nightly_view(json!({ "enabled": true, "recent": dreams }))
}

fn caught_up(r: &mut Rig) {
    r.settle("the catch-up", |s| s.dream_notices_asked());
}

// use-dream-notices.test.ts: "posts one notice for a night's dream that changed notes"
#[test]
fn posts_one_notice_for_a_nights_dream_that_changed_notes() {
    let changes = ["added", "rewritten", "merged", "kept", "left"].map(|k| change(k, false)).to_vec();
    let mut r = rig(false, recent_with(vec![dream("d1", Some("r1"), changes)]));
    caught_up(&mut r);
    r.settle("the notice", |s| s.notices.has("dream-d1"));
    let n = notice(&r, "dream-d1").unwrap();
    assert_eq!(n.title, "Fleet updated lantern's notes");
    assert_eq!(n.body.as_deref(), Some("14 sessions read overnight · 1 added, 1 rewritten, 1 merged, 1 kept for good"));
    assert_eq!(r.rt.state.notices.notices().len(), 1);
}

// use-dream-notices.test.ts: "says nothing for a dream with no applied changes"
#[test]
fn says_nothing_for_a_dream_with_no_applied_changes() {
    let mut r = rig(false, recent_with(vec![dream("d1", Some("r1"), vec![change("left", false), change("added", true)])]));
    caught_up(&mut r);
    r.rt.frame(FRAME);
    assert!(r.rt.state.notices.notices().is_empty());
}

// use-dream-notices.test.ts: "tells of notes that look out of date when nothing else changed, and not once they're dealt with"
#[test]
fn tells_of_notes_that_look_out_of_date_when_nothing_else_changed_and_not_once_theyre_dealt_with() {
    let mut r = rig(
        false,
        recent_with(vec![dream("f1", Some("r1"), vec![change("flagged", false)]), dream("f2", Some("r1"), vec![change("flagged", true)])]),
    );
    caught_up(&mut r);
    r.settle("the notice", |s| s.notices.has("dream-f1"));
    let n = notice(&r, "dream-f1").unwrap();
    assert_eq!(n.title, "Some of lantern's notes look out of date");
    assert_eq!(n.body.as_deref(), Some("14 sessions read overnight · 1 looks out of date"));
    assert_eq!(n.actions[0].label, "Review");
    assert!(!r.rt.state.notices.has("dream-f2"));
}

// use-dream-notices.test.ts: "asks for nothing and shows nothing while the switch is off"
#[test]
fn asks_for_nothing_and_shows_nothing_while_the_switch_is_off() {
    let mut r = rig(false, |f| {
        f.memory_on = false;
        f.nightly = nightly_view(json!({ "recent": [dream("d1", Some("r1"), vec![change("added", false)])] }));
    });
    r.rt.frame(FRAME);
    dreamed(&mut r, dream("d2", Some("r1"), vec![change("added", false)]));
    assert!(r.heard("GET", "/api/memory/dreaming").is_empty());
    assert!(r.rt.state.notices.notices().is_empty());
}

// use-dream-notices.test.ts: "posts when a night's dream ends live, but not for Dream now"
#[test]
fn posts_when_a_nights_dream_ends_live_but_not_for_dream_now() {
    let mut r = rig(false, recent_with(vec![]));
    caught_up(&mut r);
    dreamed(&mut r, dream("manual", None, vec![change("added", false)]));
    assert!(r.rt.state.notices.notices().is_empty());
    dreamed(&mut r, dream("d2", Some("r1"), vec![change("added", false)]));
    assert!(r.rt.state.notices.has("dream-d2"));
    dreamed(&mut r, dream("d2", Some("r1"), vec![change("added", false)]));
    assert_eq!(r.rt.state.notices.notices().len(), 1, "the same dream twice is one notice");
}

// use-dream-notices.test.ts: "opens Settings → Memory on the repository from See what changed"
#[test]
fn opens_settings_memory_on_the_repository_from_see_what_changed() {
    let mut r = rig(false, recent_with(vec![dream("d1", Some("r1"), vec![change("added", false)])]));
    caught_up(&mut r);
    r.settle("the notice", |s| s.notices.has("dream-d1"));
    let action = notice(&r, "dream-d1").unwrap().actions[0].clone();
    assert_eq!(action.label, "See what changed");
    r.rt.update(move |s, _| s.notices.tell(&action.run));
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    assert_eq!(r.rt.state.rail, Rail::Settings);
    assert_eq!(r.rt.state.settings.page, crate::fleet::settings_nav::Page::Memory);
    r.settle("the page to choose the repository", |s| s.settings.memory.repository.is_some());
    assert_eq!(r.rt.state.settings.memory.repository.as_deref(), Some("/work/lantern"));
}

#[test]
fn a_failed_catch_up_says_nothing_and_a_dream_with_no_id_is_ignored() {
    let mut r = rig(false, |f| f.refuse_reads = 1);
    caught_up(&mut r);
    r.rt.frame(FRAME);
    assert!(r.rt.state.notices.notices().is_empty());
    dreamed(&mut r, json!({ "status": "done", "runId": "r1" }));
    assert!(r.rt.state.notices.notices().is_empty());
}

#[test]
fn a_dream_that_checked_notes_says_so_and_a_failed_one_says_nothing() {
    let mut r = rig(false, recent_with(vec![]));
    caught_up(&mut r);
    dreamed(&mut r, dream("c1", Some("r1"), vec![change("confirmed", false)]));
    let n = notice(&r, "dream-c1").unwrap();
    assert_eq!(n.title, "Fleet checked lantern's notes");
    assert_eq!(n.actions[0].label, "See what changed");
    let mut failed = dream("x1", Some("r1"), vec![change("added", false)]);
    failed["status"] = json!("failed");
    dreamed(&mut r, failed);
    assert!(!r.rt.state.notices.has("dream-x1"));
}
