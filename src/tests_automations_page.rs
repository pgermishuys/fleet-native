//! Component 19, piece D: the Automations page and its list. Fleet's three D cases of `AutomationsPage.test.ts` under their
//! names ("shows loading, not “No automations yet”, until the first load finishes", "says a failed load failed, not that
//! there are no automations, and loads on Try again", "shows each automation's next run, Running, Failed or Off in the
//! sidebar, like session rows"), against a fake Fleet; then the rules the native page keeps: a row opens its automation, New
//! automation starts create, built-ins come first, the machine switch reads that machine's list.
use crate::fleet::automations::nav::ViewMode;
use crate::fleet::automations::schedule::next_short;
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::mock::automations::NOW_MS;
use crate::mock::machines::Case;
use crate::tests::{FRAME, app, click};
use crate::tests_automations_store::{Fake, automation, fake};
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    home: Fake,
    harbor: Fake,
}

/// The app connected to a fake Fleet, with `m-harbor` answering at a second fake. The page isn't open yet.
fn rig(home: Vec<Value>, harbor: Vec<Value>) -> Rig {
    let (home, harbor) = (fake(home), fake(harbor));
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link =
        Link::start_machine(HOME_MACHINE_KEY.to_string(), home.base.clone(), Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app();
    let base = harbor.base.clone();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.now_ms = Some(NOW_MS);
        s.show_machines_only(Case::Several);
        s.machines.list.iter_mut().find(|m| m.id == "m-harbor").expect("harbor is listed").base_url = base;
    });
    rt.frame(FRAME);
    Rig { rt, queue, home, harbor }
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| {
            let a = s.shown_automations();
            format!(
                "list {:?}, loading {}, error {:?}, nav {:?}",
                a.list.iter().map(|x| &x.id).collect::<Vec<_>>(),
                a.is_loading,
                a.error,
                s.automations_nav
            )
        });
    }

    /// Opens the page and waits for its first read to have been heard.
    fn open(&mut self) {
        self.rt.update(|s, _| s.press_rail(Rail::Automations));
        self.frames();
        let heard = self.home.handle();
        self.wait("the list to be asked for", move |_| heard("GET", "/api/automations") > 0);
    }

    fn frames(&mut self) {
        while let Ok(job) = self.queue.try_recv() {
            self.rt.update(job);
        }
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    fn has(&self, key: &str) -> bool {
        self.rt.find(key).is_some()
    }

    fn words(&self) -> String {
        self.rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
    }

    fn texts_of(&self, key: &str) -> Vec<String> {
        self.rt.elements().iter().filter(|e| e.key.as_deref() == Some(key)).filter_map(|e| e.text.clone()).collect()
    }

    /// The list's row names, top to bottom.
    fn rows(&self) -> Vec<String> {
        self.rt
            .elements()
            .iter()
            .filter(|e| e.role == "button" && e.key.as_deref().is_some_and(|k| k.starts_with("automation-row-")))
            .filter_map(|e| e.label.clone())
            .collect()
    }
}

fn running(id: &str, state: &str) -> Value {
    json!({"id": format!("run-{id}"), "automationId": id, "trigger": "schedule", "startedAt": "2026-09-15T09:00:00Z", "state": state})
}

fn weekly(id: &str, name: &str, extra: Value) -> Value {
    let mut extra = extra;
    extra["name"] = json!(name);
    extra["nextRunAt"] = extra.get("nextRunAt").cloned().unwrap_or(json!("2026-09-18T07:00:00Z"));
    extra["timeZone"] = json!("UTC");
    automation(id, extra)
}

// ─── AutomationsPage.test.ts: "Automations screen" (the D cases) ────────────────────────────────────────────────────

#[test]
fn shows_loading_not_no_automations_yet_until_the_first_load_finishes() {
    let mut r = rig(vec![], vec![]);
    r.home.hold_list.store(true, Ordering::SeqCst);
    r.open();

    assert!(!r.words().contains("No automations yet"), "{}", r.words());
    assert!(r.has("automation-loading"), "the detail says it's loading");
    assert!(r.has("automations-loading"), "the list says it's loading");

    r.home.hold_list.store(false, Ordering::SeqCst);
    r.wait("the list", |s| !s.shown_automations().is_loading);
    r.frames();

    assert!(!r.has("automation-loading") && !r.has("automations-loading"));
    assert!(r.words().contains("No automations yet"), "{}", r.words());
}

#[test]
fn says_a_failed_load_failed_not_that_there_are_no_automations_and_loads_on_try_again() {
    let mut r = rig(vec![], vec![]);
    r.open();
    r.wait("the list", |s| !s.shown_automations().is_loading);
    r.frames();
    assert!(r.words().contains("No automations yet"));

    *r.home.refuse_next_list.lock().unwrap() = Some("Fleet is restarting".into());
    r.rt.update(|s, tx| s.automations_refresh(HOME_MACHINE_KEY, tx, None));
    r.wait("the refusal", |s| s.shown_automations().error.is_some());
    r.frames();

    assert!(!r.words().contains("No automations yet"), "{}", r.words());
    assert!(r.has("automations-load-error") && r.has("automation-load-error"), "the list and the detail both say it");
    let words = r.words();
    assert!(words.contains("Fleet is restarting") && words.contains("Couldn't load automations"), "{words}");

    // The server has one now; Try again reads it.
    let made = json!({"name": "Weekly PR digest", "prompt": "Summarise", "triggerType": "schedule", "triggerConfig": "0 9 * * 1"});
    let (status, _) = crate::fleet::http::text_with("POST", &r.home.base, "/api/automations", Some(&made)).expect("a create");
    assert_eq!(status, 201);
    let heard = r.home.handle();
    let before = heard("GET", "/api/automations");
    click(&mut r.rt, "automations-load-error-retry");
    r.wait("the new read", move |_| heard("GET", "/api/automations") > before);
    r.wait("the list", |s| !s.shown_automations().list.is_empty());
    r.frames();

    assert!(!r.has("automations-load-error") && !r.has("automation-load-error"));
    assert_eq!(r.rows(), ["Weekly PR digest"]);
    assert!(!r.rt.state.automations_nav.retrying);
}

#[test]
fn shows_each_automation_s_next_run_running_failed_or_off_in_the_sidebar_like_session_rows() {
    let list = vec![
        weekly("a1", "Weekly PR digest", json!({})),
        weekly("a2", "Nightly dependency check", json!({"lastRun": running("a2", "running")})),
        weekly("a3", "Tidy stale worktrees", json!({"lastRun": running("a3", "failed")})),
        weekly("a4", "Old digest", json!({"isEnabled": false, "nextRunAt": null})),
    ];
    let mut r = rig(list, vec![]);
    r.open();
    r.wait("the list", |s| !s.shown_automations().is_loading);
    r.frames();

    let tz = |ms: i64| crate::conv::time::local_offset_at(ms);
    let next = next_short(crate::fleet::work::parse_iso_ms("2026-09-18T07:00:00Z").unwrap(), NOW_MS, &tz);
    assert_eq!(r.rows(), ["Weekly PR digest", "Nightly dependency check", "Tidy stale worktrees", "Old digest"]);
    assert_eq!(r.texts_of("automation-row-status"), [next.as_str(), "Running", "Failed", "Off"]);
}

// ─── What the native page keeps ─────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_row_opens_its_automation_in_edit_mode_and_the_detail_names_it() {
    let mut r = rig(vec![weekly("a1", "Weekly PR digest", json!({})), weekly("a2", "Old digest", json!({}))], vec![]);
    r.open();
    r.wait("the list", |s| !s.shown_automations().is_loading);
    r.frames();
    assert_eq!(r.rt.state.automations_nav.mode, ViewMode::List);
    assert!(r.words().contains("Pick one from the list, or make a new one."), "{}", r.words());
    assert!(!r.has("automation-empty-create"), "no button once there are some");

    click(&mut r.rt, "automation-row-a2");
    r.frames();
    let nav = &r.rt.state.automations_nav;
    assert_eq!((nav.mode, nav.open_id()), (ViewMode::Edit, Some("a2")));
    assert_eq!(r.words().matches("Old digest").count(), 2, "its row and the detail's title: {}", r.words());
}

#[test]
fn new_automation_starts_create_with_a_draft_row_and_the_empty_state_s_button_does_too() {
    let mut r = rig(vec![], vec![]);
    r.open();
    r.wait("the list", |s| !s.shown_automations().is_loading);
    r.frames();
    assert!(r.words().contains("An automation runs a prompt or a workflow on a schedule"), "{}", r.words());
    assert!(!r.has("automation-draft-row"));

    click(&mut r.rt, "automation-empty-create");
    r.frames();
    assert_eq!(r.rt.state.automations_nav.mode, ViewMode::Create);
    assert!(r.has("automation-draft-row"), "the draft row shows while creating, even with no automations");
    let words = r.words();
    assert!(words.contains("Draft") && words.matches("New automation").count() >= 3, "{words}");

    r.rt.update(|s, _| s.automations_nav.clear_selection());
    r.frames();
    click(&mut r.rt, "new-automation");
    r.frames();
    assert_eq!((r.rt.state.automations_nav.mode, r.rt.state.automations_nav.open_id()), (ViewMode::Create, None));
}

#[test]
fn built_in_automations_come_first_with_their_pill_and_the_rest_keep_the_server_s_order() {
    let list = vec![
        weekly("a1", "Weekly PR digest", json!({})),
        weekly("a-dream", "Dreaming", json!({"builtin": "dreaming", "nextRunAt": "2026-09-16T03:00:00Z"})),
        weekly("a2", "Old digest", json!({})),
    ];
    let mut r = rig(list, vec![]);
    r.open();
    r.wait("the list", |s| !s.shown_automations().is_loading);
    r.frames();

    assert_eq!(r.rows(), ["Dreaming", "Weekly PR digest", "Old digest"]);
    assert_eq!(r.words().matches("Built in").count(), 1, "only the built-in one has the pill");
}

#[test]
fn the_machine_switch_reads_that_machine_s_list_and_the_page_opens_on_this_machine_again() {
    let mut r = rig(vec![weekly("a1", "Home digest", json!({}))], vec![weekly("b1", "Harbor digest", json!({}))]);
    r.open();
    r.wait("the list", |s| !s.shown_automations().is_loading);
    r.frames();
    assert_eq!(r.rows(), ["Home digest"]);

    click(&mut r.rt, "automations-machine-switch");
    r.frames();
    click(&mut r.rt, "automations-machine-m-harbor");
    let heard = r.harbor.handle();
    r.wait("harbor's list to be asked for", move |_| heard("GET", "/api/automations") > 0);
    r.wait("harbor's list", |s| !s.shown_automations().is_loading);
    r.frames();
    assert_eq!(r.rt.state.automations_machine(), "m-harbor");
    assert_eq!(r.rows(), ["Harbor digest"]);
    assert_eq!(r.home.list_calls(), 1, "home's list was not read again for it");

    // Leaving and coming back opens on this machine, and reads its list again.
    r.rt.update(|s, _| s.press_rail(Rail::Sessions));
    r.frames();
    r.rt.update(|s, _| s.press_rail(Rail::Automations));
    r.frames();
    assert_eq!(r.rt.state.automations_machine(), HOME_MACHINE_KEY);
    let home = Arc::new(r.home.handle());
    r.wait("home's list again", move |_| home("GET", "/api/automations") > 1);
    r.frames();
    assert_eq!(r.rows(), ["Home digest"]);
}
