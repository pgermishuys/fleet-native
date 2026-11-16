//! The home dashboard's rules (component 9, piece C): `home-landing.test.ts`, `SummaryBar.archive.test.ts` ("countsWithoutArchiving")
//! under Fleet's names, and the greeting, status line, sections and order written from reading `FleetDashboard.vue` and
//! `lib/needs-you.ts`. (`home-landing`'s phone case is skipped: this app is desktop only.)
use super::state::app;
use crate::fleet::dashboard::{self, Bucket, Landing, MISSION_CONTROL_KEY};
use crate::fleet::sessions::parse_sessions_for;
use crate::fleet::store::{Retention, Route};
use crate::mock::Session;
use serde_json::{Value, json};
use std::collections::HashSet;

/// A list row: its status word, retention and (optionally) its parent, with `updated` as its time.
fn item(id: &str, status: &str, updated: i64) -> Value {
    json!({
        "session": {"id": id, "title": format!("Session {id}"), "time": {"created": 1, "updated": updated}},
        "sessionStatus": status, "activityStatus": "idle", "retentionStatus": "active", "isHidden": false,
        "workspaceDirectory": "/work/project", "projectId": "project-1", "projectName": "Api", "branch": "main",
    })
}

fn sessions(rows: Vec<Value>) -> Vec<Session> {
    parse_sessions_for(&Value::Array(rows), 0, Retention::All)
}

fn ids(list: &[&Session]) -> Vec<String> {
    list.iter().map(|s| s.id.clone()).collect()
}

fn none(_: &str) -> bool {
    false
}

// ---- routes/__tests__/home-landing.test.ts ----------------------------------------------------------------------

/// Fleet opening at `path` (the module's `landing`), the page shown once, the switch on or off.
fn open_at(path: &str, switch_on: bool) -> (Landing, bool) {
    let mut landing = Landing::at_start(path == "/");
    landing.home_shown();
    let lands = landing.lands(true, switch_on);
    (landing, lands)
}

#[test]
fn lands_on_fleet_when_fleet_opens_at_slash_with_the_switch_on() {
    assert!(open_at("/", true).1);
}

#[test]
fn stays_on_slash_with_the_switch_off() {
    assert!(!open_at("/", false).1);
}

#[test]
fn leaves_a_deep_link_alone_and_a_later_visit_to_slash_from_it() {
    let (mut landing, lands) = open_at("/sessions/abc", true);
    assert!(!lands);
    landing.home_left();
    landing.home_shown();
    assert!(!landing.lands(true, true));
}

#[test]
fn lets_sessions_reach_slash_after_landing_without_landing_again() {
    let (mut landing, lands) = open_at("/", true);
    assert!(lands);
    landing.home_left();
    landing.home_shown();
    assert!(!landing.lands(true, true));
}

#[test]
fn waits_for_the_preferences_to_load_before_landing() {
    let mut landing = Landing::at_start(true);
    landing.home_shown();
    assert!(!landing.lands(false, false), "the switch is not known yet");
    assert!(landing.lands(true, true));
    assert!(!landing.lands(true, true), "once");
}

#[test]
fn a_page_left_before_the_preferences_came_never_lands() {
    let mut landing = Landing::at_start(true);
    landing.home_shown();
    landing.home_left();
    assert!(!landing.lands(true, true));
}

#[test]
fn the_app_lands_once_the_preferences_say_so_and_only_from_its_first_home() {
    let mut rt = app();
    rt.update(|s, _| {
        s.go_live(false);
        s.go(Route::Home);
    });
    assert!(!rt.state.landing.landed, "the preferences have not come");
    rt.update(|s, _| {
        s.pickers.prefs.adopt(&json!({ MISSION_CONTROL_KEY: "true" }));
        s.landing_check();
    });
    assert!(rt.state.landing.landed);
    // Sessions and back: the chance is used up.
    rt.update(|s, _| {
        s.landing = Landing::at_start(false);
        s.go(Route::None);
        s.go(Route::Home);
    });
    assert!(!rt.state.landing.landed);
}

#[test]
fn a_start_with_sessions_goes_home_with_nothing_selected_and_the_rail_goes_back_there() {
    let mut rt = app();
    rt.update(|s, _| {
        s.go_live(false);
    });
    let rows = vec![item("s1", "idle", 5), item("s2", "idle", 4)];
    rt.update(move |s, tx| {
        let begin = s.store.begin(s.has_rows());
        s.on_sessions(begin, sessions(rows), Ok(vec![]), tx);
    });
    assert_eq!(rt.state.route, Route::Home);
    assert!(!rt.state.has_session());
    rt.update(|s, tx| s.open_session(0, 0, tx));
    assert!(rt.state.has_session());
    crate::tests::click(&mut rt, "rail-sessions");
    assert_eq!(rt.state.route, Route::Home, "Sessions in the rail goes to `/` with nothing selected");
}

#[test]
fn an_empty_list_still_goes_to_the_new_session_page() {
    let mut rt = app();
    rt.update(|s, _| s.go_live(false));
    rt.update(|s, tx| s.on_list(Vec::new(), tx));
    assert!(matches!(rt.state.route, Route::NewSession { .. }));
}

// ---- SummaryBar.archive.test.ts ---------------------------------------------------------------------------------

#[test]
fn counts_without_archiving_takes_a_pending_session_off_working_if_its_in_a_turn_else_off_idle() {
    let list = sessions(vec![item("a", "active", 3), item("b", "idle", 2), item("c", "waiting_input", 1)]);
    let pending: HashSet<&str> = ["a", "b"].into();
    assert_eq!(dashboard::counts_without_archiving((2, 1), &list, &|id| pending.contains(id)), (1, 0));
    assert_eq!(dashboard::counts_without_archiving((2, 1), &list, &none), (2, 1));
}

#[test]
fn counts_without_archiving_stops_at_zero_and_skips_sub_agents_and_archived_rows() {
    let mut sub = item("sub", "active", 3);
    sub["parentSessionId"] = json!("a");
    let mut archived = item("old", "active", 2);
    archived["retentionStatus"] = json!("archived");
    let list = sessions(vec![item("a", "active", 4), sub, archived]);
    let all = |_: &str| true;
    assert_eq!(dashboard::counts_without_archiving((0, 0), &list, &all), (0, 0));
    assert_eq!(dashboard::counts_without_archiving((5, 5), &list, &all), (4, 5), "only `a` counts");
}

// ---- the sections, the order and the filter ---------------------------------------------------------------------

#[test]
fn a_session_needs_you_when_waiting_or_errored_or_its_workflow_waits() {
    let list = sessions(vec![
        item("w", "waiting_input", 1),
        item("e", "error", 1),
        item("a", "active", 1),
        item("i", "idle", 1),
        item("s", "stopped", 1),
        item("f", "idle", 1),
    ]);
    let step = |id: &str| id == "f";
    let bucket: Vec<Bucket> = list.iter().map(|s| dashboard::session_bucket(s, &step)).collect();
    assert_eq!(
        bucket,
        [Bucket::NeedsYou, Bucket::NeedsYou, Bucket::Working, Bucket::Recent, Bucket::Recent, Bucket::NeedsYou],
        "in the list's own order (newest first)"
    );
}

#[test]
fn updated_at_falls_back_to_created_then_zero() {
    let mut list = sessions(vec![item("u", "idle", 50), item("c", "idle", 0)]);
    assert_eq!(dashboard::updated_at(&list[0]), 50);
    assert_eq!(dashboard::updated_at(&list[1]), 1, "created");
    list[1].row.created = 0;
    assert_eq!(dashboard::updated_at(&list[1]), 0);
}

#[test]
fn the_sections_are_newest_first_and_leave_out_sub_agents_and_rows_waiting_to_archive() {
    let mut sub = item("sub", "active", 99);
    sub["parentSessionId"] = json!("w");
    let list = sessions(vec![
        item("w-old", "waiting_input", 10),
        item("w-new", "error", 30),
        item("run", "active", 20),
        item("quiet", "idle", 5),
        item("going", "active", 40),
        sub,
    ]);
    let kept = dashboard::shown(&list, Retention::All, &|id| id == "going");
    assert_eq!(ids(&kept), ["w-new", "run", "w-old", "quiet"]);
    let [needs, working, recent] = dashboard::sections(&kept, &none);
    assert_eq!(
        (ids(&needs), ids(&working), ids(&recent)),
        (vec!["w-new".into(), "w-old".into()], vec!["run".into()], vec!["quiet".into()])
    );
}

#[test]
fn the_filter_shows_active_archived_or_all() {
    let mut old = item("old", "idle", 1);
    old["retentionStatus"] = json!("archived");
    let list = sessions(vec![item("live", "idle", 2), old]);
    assert_eq!(ids(&dashboard::shown(&list, Retention::Active, &none)), ["live"]);
    assert_eq!(ids(&dashboard::shown(&list, Retention::Archived, &none)), ["old"]);
    assert_eq!(ids(&dashboard::shown(&list, Retention::All, &none)), ["live", "old"]);
}

// ---- the words --------------------------------------------------------------------------------------------------

#[test]
fn the_greeting_changes_at_5_12_and_18() {
    let said: Vec<&str> = [0, 4, 5, 11, 12, 17, 18, 23].into_iter().map(dashboard::greeting).collect();
    assert_eq!(
        said,
        [
            "Working late",
            "Working late",
            "Good morning",
            "Good morning",
            "Good afternoon",
            "Good afternoon",
            "Good evening",
            "Good evening"
        ]
    );
}

#[test]
fn the_hour_is_the_local_one() {
    let noon_utc = 20_000 * 86_400_000 + 12 * 3_600_000;
    assert_eq!(dashboard::hour_of(noon_utc, 0), 12);
    assert_eq!(dashboard::hour_of(noon_utc, 2 * 3600), 14);
    assert_eq!(dashboard::hour_of(noon_utc, -13 * 3600), 23, "yesterday evening");
    assert_eq!(dashboard::hour_of(-1000, 0), 23, "before 1970 too");
}

#[test]
fn the_status_line_counts_who_needs_you_and_who_is_working() {
    let line = dashboard::status_line;
    assert_eq!(line(2, 3), "2 sessions need you, 3 working.");
    assert_eq!(line(1, 0), "1 session needs you.");
    assert_eq!(line(1, 1), "1 session needs you, 1 working.");
    assert_eq!(line(0, 1), "1 session working.");
    assert_eq!(line(0, 4), "4 sessions working.");
    assert_eq!(line(0, 0), "All quiet. Nothing needs you right now.");
}
