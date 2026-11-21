//! Component 23, piece A: the frame the Labs views share. `lib/__tests__/fleet-views.test.ts` and `jobs.test.ts` ("round-trips a
//! job through its route") under their names; `MissionControlPage.test.ts` ("with the switch off only says how to turn it on,
//! and asks no machine for anything") and `JobsScreen.test.ts` ("Jobs switched off") against a fake Fleet that counts what it
//! is asked; then what this app adds: the Board's own off notice, Turn on, the landing, Ctrl+B, the palette and Open.
use crate::fleet::labs::{
    FleetView, Labs, decode_component, fleet_view_from_path, fleet_view_path, is_same_fleet_view, job_id_from_path, job_path,
};
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::tests::{FRAME, app, click};
use crate::tests_workflow_runs::{Fake, fake};
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};

const PREFERENCES: &str = "/api/preferences";

// ─── fleet-views.test.ts ────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn round_trips_a_view_through_its_route() {
    assert_eq!(fleet_view_path(&FleetView::NeedsYou), "/fleet");
    assert_eq!(fleet_view_path(&FleetView::Machine("a b".into())), "/fleet/machines/a%20b");
    assert_eq!(fleet_view_from_path("/fleet/machines/a%20b"), FleetView::Machine("a b".into()));
    assert!(is_same_fleet_view(&FleetView::Machine("a".into()), &FleetView::Machine("a".into())));
    assert!(!is_same_fleet_view(&FleetView::Machine("a".into()), &FleetView::NeedsYou));
}

#[test]
fn shows_needs_you_for_the_bare_route_and_for_anything_stale() {
    for path in ["/fleet", "/fleet/", "/fleet/machines", "/fleet/other/x", "/fleet/machines/%E0%A4%A", "/fleet/machines/"] {
        assert_eq!(fleet_view_from_path(path), FleetView::NeedsYou, "{path}");
    }
    // Slashes and non-ASCII in a machine's id survive the round trip.
    let view = FleetView::Machine("f/é 1".into());
    assert_eq!(fleet_view_from_path(&fleet_view_path(&view)), view);
}

// ─── jobs.test.ts ───────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn round_trips_a_job_through_its_route() {
    assert_eq!(job_path("a b"), "/jobs/a%20b");
    assert_eq!(job_id_from_path("/jobs/a%20b").as_deref(), Some("a b"));
    assert_eq!(job_id_from_path("/jobs"), None);
    assert_eq!(job_id_from_path("/jobs/"), None);
    // Fleet's throws on a bad escape; a job that can't be named is none here.
    assert_eq!(job_id_from_path("/jobs/%E0%A4%A"), None);
    assert_eq!(decode_component("%zz"), None);
    assert_eq!(decode_component("%+1"), None);
}

// ─── the pages, against a fake Fleet ────────────────────────────────────────────────────────────────────────────────

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fleet: Fake,
}

/// The app connected to a fake Fleet whose preferences are `prefs`; nothing has been asked yet.
fn rig(prefs: Value) -> Rig {
    let fleet = fake();
    fleet.route("GET", PREFERENCES, 200, prefs);
    for key in ["Jobs", "features.mission-control.enabled", "features.board.enabled"] {
        fleet.route("PUT", &format!("{PREFERENCES}/{key}"), 200, json!({}));
    }
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link =
        Link::start_machine(HOME_MACHINE_KEY.to_string(), fleet.base.clone(), Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.pickers.prefs = Default::default();
    });
    rt.frame(FRAME);
    Rig { rt, queue, fleet }
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("preferences fetched: {}", s.pickers.prefs.fetched));
    }

    fn frames(&mut self) {
        while let Ok(job) = self.queue.try_recv() {
            self.rt.update(job);
        }
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    /// The preferences are read, and the page of `rail` is open.
    fn open(&mut self, rail: Rail) {
        self.rt.update(|s, _| s.load_preferences());
        self.wait("the preferences", |s| s.pickers.prefs.fetched);
        self.rt.update(move |s, _| s.press_rail(rail));
        self.frames();
    }

    fn has(&self, key: &str) -> bool {
        self.rt.find(key).is_some()
    }

    fn words(&self) -> String {
        self.rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
    }

    /// What the fake was asked under `prefixes`: the data of the views, not the preferences the frame reads itself nor what
    /// the app keeps up for its own sake (the update check, the events hub).
    fn asked(&self, prefixes: &[&str]) -> Vec<(String, String)> {
        self.fleet.calls().into_iter().filter(|(_, p)| prefixes.iter().any(|x| p.starts_with(x))).collect()
    }
}

#[test]
fn mission_control_with_the_switch_off_only_says_how_to_turn_it_on_and_asks_no_machine_for_anything() {
    let mut r = rig(json!({}));
    r.open(Rail::Fleet);
    assert!(r.has("fleet-off"), "{}", r.words());
    assert!(r.words().contains("Mission control is off"));
    assert!(r.words().contains("One screen for every running agent on every machine"));
    assert!(!r.has("fleet-menu") && !r.has("fleet-detail"), "no panel beside the notice");
    assert_eq!(r.asked(&["/api/sessions", "/api/fleet", "/api/jobs", "/api/boards"]), []);
}

#[test]
fn jobs_switched_off_shows_how_to_turn_it_on_and_asks_fleet_nothing_about_jobs() {
    let mut r = rig(json!({}));
    r.open(Rail::Jobs);
    assert!(r.words().contains("Jobs is off"), "{}", r.words());
    assert!(r.words().contains("Post jobs to one queue."));
    assert!(!r.has("jobs-list"));
    assert_eq!(r.asked(&["/api/jobs"]), []);
}

#[test]
fn the_board_with_the_switch_off_says_so_and_asks_for_no_board() {
    let mut r = rig(json!({}));
    r.open(Rail::Board);
    assert!(r.words().contains("Board is off"), "{}", r.words());
    assert!(r.words().contains("The board lays your sessions out in columns by status"));
    assert!(!r.has("board-controls"));
    assert_eq!(r.asked(&["/api/boards"]), []);
}

#[test]
fn says_nothing_until_the_switches_have_loaded_so_a_page_that_s_on_doesn_t_flash_is_off() {
    let mut r = rig(json!({"Jobs": "true"}));
    r.rt.update(|s, _| s.press_rail(Rail::Jobs));
    r.frames();
    assert!(!r.words().contains("is off") && !r.has("jobs-list"), "{}", r.words());
    r.open(Rail::Jobs);
    assert!(r.has("jobs-list") && r.has("jobs-detail") && !r.has("jobs-off"), "{}", r.words());
}

#[test]
fn turn_on_saves_the_preference_and_shows_the_view() {
    for (rail, key, off, shell) in [
        (Rail::Fleet, "features.mission-control.enabled", "fleet-off", "fleet-menu"),
        (Rail::Jobs, "Jobs", "jobs-off", "jobs-list"),
        (Rail::Board, "features.board.enabled", "board-off", "board-controls"),
    ] {
        let mut r = rig(json!({}));
        r.open(rail);
        assert!(r.has(off));
        click(&mut r.rt, &format!("{}-turn-on", off.trim_end_matches("-off")));
        let calls = r.fleet.calls_handle();
        let path = format!("{PREFERENCES}/{key}");
        r.wait("the preference to be saved", move |_| calls().iter().any(|(m, p)| m == "PUT" && *p == path));
        r.frames();
        assert_eq!(r.fleet.bodies("PUT", &format!("{PREFERENCES}/{key}")), [json!({"value": "true"})]);
        assert!(r.has(shell) && !r.has(off), "{key}: {}", r.words());
    }
}

// ─── what this app adds ─────────────────────────────────────────────────────────────────────────────────────────────

fn on(rt: &mut Runtime<State>, keys: &[&str]) {
    let body: serde_json::Map<String, Value> = keys.iter().map(|k| ((*k).to_string(), json!("true"))).collect();
    rt.update(|s, _| s.pickers.prefs.adopt(&body.into()));
    rt.frame(FRAME);
    rt.frame(FRAME);
}

/// The window comes to `/` (the session list arrived and none is open), as it does once at a start.
fn show_home(rt: &mut Runtime<State>) {
    rt.update(|s, _| s.go(crate::fleet::store::Route::Home));
    rt.frame(FRAME);
}

#[test]
fn the_first_slash_of_a_start_lands_on_needs_you_when_mission_control_is_on() {
    // The preferences came first, then the first `/`.
    let mut r = rig(json!({"features.mission-control.enabled": "true"}));
    assert_eq!(r.rt.state.rail, Rail::Sessions);
    r.rt.update(|s, _| s.load_preferences());
    r.wait("the preferences", |s| s.pickers.prefs.fetched);
    assert_eq!(r.rt.state.rail, Rail::Sessions, "not at `/` yet");
    show_home(&mut r.rt);
    r.frames();
    assert_eq!(r.rt.state.rail, Rail::Fleet);
    assert_eq!(r.rt.state.labs.fleet_view, FleetView::NeedsYou);
    assert!(r.has("fleet-menu") && r.has("fleet-detail"), "{}", r.words());
    // Only once: a later visit to Sessions' home stays there.
    click(&mut r.rt, "rail-sessions");
    show_home(&mut r.rt);
    r.rt.update(|s, _| s.load_preferences());
    r.frames();
    assert_eq!(r.rt.state.rail, Rail::Sessions);
}

#[test]
fn the_first_slash_waits_for_the_preferences_and_then_lands() {
    let mut r = rig(json!({"features.mission-control.enabled": "true"}));
    show_home(&mut r.rt);
    assert_eq!(r.rt.state.rail, Rail::Sessions, "the preferences haven't come");
    r.rt.update(|s, _| s.load_preferences());
    r.wait("the window to land", |s| s.rail == Rail::Fleet);
}

#[test]
fn a_start_on_a_deep_link_never_lands() {
    let mut r = rig(json!({"features.mission-control.enabled": "true"}));
    r.rt.update(|s, _| s.landing = crate::fleet::dashboard::Landing::at_start(false));
    r.rt.update(|s, _| s.load_preferences());
    r.wait("the preferences", |s| s.pickers.prefs.fetched);
    show_home(&mut r.rt);
    assert_eq!(r.rt.state.rail, Rail::Sessions);
}

#[test]
fn the_first_slash_stays_home_with_mission_control_off() {
    let mut r = rig(json!({}));
    r.rt.update(|s, _| s.load_preferences());
    r.wait("the preferences", |s| s.pickers.prefs.fetched);
    show_home(&mut r.rt);
    assert_eq!(r.rt.state.rail, Rail::Sessions);
    assert!(!r.rt.state.landing.landed);
}

#[test]
fn ctrl_b_hides_each_views_panel_and_leaves_the_page() {
    let mut rt = app();
    on(&mut rt, &["features.mission-control.enabled", "Jobs", "features.board.enabled"]);
    for (rail, panel, page) in
        [(Rail::Fleet, "fleet-menu", "fleet-detail"), (Rail::Jobs, "jobs-list", "jobs-detail"), (Rail::Board, "board-controls", "board")]
    {
        rt.update(move |s, _| s.press_rail(rail));
        rt.frame(FRAME);
        assert!(rt.find(panel).is_some() && rt.find(page).is_some(), "{panel} beside {page}");
        rt.key(rune::KeyEvent { key: rune::Key::Command('b'), mods: rune::Mods::default() });
        rt.frame(FRAME);
        assert!(rt.find(panel).is_none() && rt.find(page).is_some(), "{page} alone");
        rt.key(rune::KeyEvent { key: rune::Key::Command('b'), mods: rune::Mods::default() });
        rt.frame(FRAME);
    }
}

#[test]
fn go_to_board_opens_the_board_and_the_off_notice_while_it_is_off() {
    let mut rt = app();
    rt.update(|s, tx| crate::ui::commands::run(s, tx, "nav-board"));
    rt.frame(FRAME);
    assert_eq!(rt.state.rail, Rail::Board);
    assert!(rt.find("board-off").is_some(), "listed with the Board off, it opens the notice");
    let c = crate::ui::commands::all(&rt.state).into_iter().find(|c| c.id == "nav-board").expect("the command");
    assert_eq!(c.keywords, ["kanban", "overview", "board"]);
    assert_eq!(c.label, "Go to Board");
    on(&mut rt, &["features.board.enabled"]);
    assert!(rt.find("board-controls").is_some());
}

#[test]
fn the_rails_and_settings_open_a_views_bare_address() {
    let mut rt = app();
    rt.update(|st, _| {
        st.labs = Labs::default();
        st.labs.job = Some("j1".into());
        st.labs.fleet_view = FleetView::Machine("m1".into());
        st.press_rail(Rail::Jobs);
        assert_eq!(st.labs.job, None, "the rail button opens /jobs");
        st.open_jobs_address("/jobs/j2");
        assert_eq!(st.labs.job.as_deref(), Some("j2"));
        st.press_rail(Rail::Fleet);
        assert_eq!(st.labs.fleet_view, FleetView::NeedsYou, "the rail button opens /fleet");
        st.open_fleet_address("/fleet/machines/m2");
        assert_eq!(st.labs.fleet_view, FleetView::Machine("m2".into()));
    });
}
