//! The home page (component 9, piece E): the row actions, the filter, "Show N more" and another machine's page. Fleet's
//! `FleetDashboard.archive.test.ts`, `FleetDashboard.row-archive-undo.test.ts` and `FleetDashboard.machine.test.ts` under
//! their names; the rest written from reading `DashboardSessionRow.vue` and `RetentionFilter.vue`.
use crate::fleet::archive_queue::UNDO_SECS;
use crate::fleet::dashboard_machine::{Other, UNREADABLE, path};
use crate::fleet::home::Scene;
use crate::fleet::machines::parse_machine_list;
use crate::fleet::store::Retention;
use crate::tests::{FRAME, app, click};
use crate::tests_archive_pins::{Fake, Rig};
use rune::Runtime;
use serde_json::json;

fn home(scene: Scene) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| {
        s.show_home_scene(scene, true);
        let mut n = 0;
        for p in &mut s.projects {
            for x in &mut p.sessions {
                n += 1;
                x.id = format!("s{n}");
            }
        }
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn has(rt: &Runtime<crate::State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn titles(rt: &Runtime<crate::State>) -> Vec<String> {
    rt.elements()
        .into_iter()
        .filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("dash-row-") && k.ends_with("-title")))
        .filter_map(|e| e.text.or(e.label))
        .collect()
}

// ---- FleetDashboard.archive.test.ts, FleetDashboard.row-archive-undo.test.ts --------------------------------------------------

#[test]
fn the_dashboards_recent_list_and_a_queued_archive_hides_a_session_as_soon_as_its_archived_and_shows_it_again_on_undo() {
    let mut rt = home(Scene::Three);
    assert!(has(&rt, "dash-row-s3"));
    rt.update(|s, tx| s.archive_sessions(&["s3".to_string()], None, tx));
    rt.frame(FRAME);
    assert!(!has(&rt, "dash-row-s3") && has(&rt, "dash-row-s1"));
    rt.update(|s, _| s.undo_archive());
    rt.frame(FRAME);
    assert!(has(&rt, "dash-row-s3"));
}

#[test]
fn the_dashboards_session_row_archive_goes_through_the_undo_queue_instead_of_archiving_at_once() {
    let mut rt = home(Scene::Three);
    click(&mut rt, "dash-row-s3-archive");
    rt.frame(FRAME);
    assert_eq!(rt.state.archive.pending.as_ref().map(|p| p.ids.clone()), Some(vec!["s3".to_string()]));
    assert!(!has(&rt, "dash-row-s3"), "the row is gone at once");
    assert!(!rt.state.projects[0].sessions[2].row.archived, "nothing is archived before the window closes");
    rt.frame(UNDO_SECS + 0.1);
    assert!(rt.state.projects[0].sessions[2].row.archived || rt.state.archive.pending.is_none());
}

#[test]
fn a_row_action_does_not_open_the_session() {
    let mut rt = home(Scene::Three);
    click(&mut rt, "dash-row-s3-archive");
    assert!(rt.state.on_home());
}

#[test]
fn delete_asks_first_in_fleets_words_and_deletes_on_confirm() {
    let mut rt = home(Scene::Three);
    click(&mut rt, "dash-row-s3-delete");
    let c = rt.state.ui.confirm.as_ref().expect("asks first");
    assert_eq!(
        (c.title.as_str(), c.subject.as_str(), c.body.as_str(), c.confirm_label.as_str()),
        ("Delete this session?", "Fix pagination", "It and its session data are deleted for good.", "Delete session")
    );
    assert!(has(&rt, "dash-row-s3"), "nothing is deleted before the answer");
    click(&mut rt, "confirm-action");
    rt.frame(FRAME);
    assert!(!has(&rt, "dash-row-s3"));
}

#[test]
fn a_session_that_cannot_archive_or_delete_has_no_such_button() {
    let mut rt = home(Scene::Three);
    rt.update(|s, _| {
        s.projects[0].sessions[2].row.capabilities.can_archive = Some(false);
        s.projects[0].sessions[2].row.capabilities.can_delete = Some(false);
    });
    rt.frame(FRAME);
    assert!(!has(&rt, "dash-row-s3-archive") && !has(&rt, "dash-row-s3-delete"));
    assert!(has(&rt, "dash-row-s1-archive") && has(&rt, "dash-row-s1-delete"));
}

#[test]
fn an_archived_row_offers_delete_but_no_archive() {
    let mut rt = home(Scene::Three);
    rt.update(|s, _| {
        s.projects[0].sessions[2].row.archived = true;
        s.store.retention = Retention::All;
    });
    rt.frame(FRAME);
    assert!(!has(&rt, "dash-row-s3-archive") && has(&rt, "dash-row-s3-delete"));
}

// ---- the filter ------------------------------------------------------------------------------------------------------------

#[test]
fn the_filter_opens_a_menu_of_active_archived_and_all_with_a_check_on_the_shown_one() {
    let mut rt = home(Scene::Three);
    click(&mut rt, "home-filter");
    rt.frame(FRAME);
    for key in ["retention-filter-option-active", "retention-filter-option-archived", "retention-filter-option-all"] {
        assert!(has(&rt, key), "{key}");
    }
    click(&mut rt, "retention-filter-option-archived");
    rt.frame(FRAME);
    assert_eq!(rt.state.store.retention, Retention::Archived);
    assert!(rt.state.ui.open.is_none(), "the menu closes");
    assert!(!has(&rt, "dash-row-s3"), "an active session isn't in the archived list");
    let label = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("home-filter")).and_then(|e| e.label);
    assert_eq!(label.as_deref(), Some("Show: Archived"));
}

#[test]
fn the_filter_is_in_the_empty_state_too() {
    let mut rt = home(Scene::Empty);
    click(&mut rt, "home-filter");
    rt.frame(FRAME);
    click(&mut rt, "retention-filter-option-all");
    assert_eq!(rt.state.store.retention, Retention::All);
}

// ---- Show N more -----------------------------------------------------------------------------------------------------------

#[test]
fn recent_shows_eight_and_show_n_more_shows_the_rest_and_show_fewer_goes_back() {
    let mut rt = home(Scene::Many);
    assert_eq!(titles(&rt).len(), 2 + 8, "the two other sections' rows, and eight of Recent's eleven");
    assert!(has(&rt, "home-more"));
    let label =
        |rt: &Runtime<crate::State>| rt.elements().into_iter().find(|e| e.key.as_deref() == Some("home-more")).and_then(|e| e.label);
    assert_eq!(label(&rt).as_deref(), Some("Show 3 more"));
    // The button is below the fold of the test window: the press is what it runs.
    rt.update(|s, _| s.toggle_recent());
    rt.frame(FRAME);
    assert_eq!(titles(&rt).len(), 2 + 11);
    assert_eq!(label(&rt).as_deref(), Some("Show fewer"));
    // The button is below the fold of the test window: the press is what it runs.
    rt.update(|s, _| s.toggle_recent());
    rt.frame(FRAME);
    assert_eq!(titles(&rt).len(), 2 + 8);
}

#[test]
fn eight_or_fewer_have_no_show_more() {
    let rt = home(Scene::Three);
    assert!(!has(&rt, "home-more"));
}

// ---- another machine's page (FleetDashboard.machine.test.ts) ---------------------------------------------------------------

const FALCON: &str = "ffffffffffffffffffffffffffffffff";

fn rows(path: &str) -> String {
    let row = |id: &str, title: &str| {
        json!({"session": {"id": id, "title": title, "directory": "/repos/lighthouse", "time": {"created": 1, "updated": 2}},
               "instanceId": format!("inst-{id}"), "sessionStatus": "idle", "activityStatus": "idle",
               "retentionStatus": "active", "lifecycleStatus": "completed", "workspaceDirectory": "/repos/lighthouse"})
    };
    let _ = path;
    json!([row("f-1", "Lighthouse sensor feed")]).to_string()
}

/// The app with home at one fake and falcon at another, the window working on falcon.
fn on_falcon(falcon: &Fake) -> (Rig, Fake) {
    let body =
        json!({ "machines": [{"id": FALCON, "name": "falcon", "baseUrl": falcon.base, "token": "falcon-token-0123456789"}] }).to_string();
    let listed = body.clone();
    let home = Fake::start(move |_, path| if path == "/api/machines" { (200, listed.clone()) } else { (200, "[]".into()) });
    let mut r = Rig::home(&home);
    r.rt.update(move |s, _| {
        s.machines.apply_list(parse_machine_list(200, &body));
        s.machines.set_current(FALCON);
        s.go(crate::fleet::store::Route::Home);
        s.projects.clear();
        s.projects.push(crate::mock::projects().remove(0));
        s.projects[0].sessions[0].title = "Tide chart caching".into();
    });
    r.rt.frame(FRAME);
    (r, home)
}

fn falcon_ok() -> Fake {
    Fake::start(|_, path| if path.starts_with("/api/sessions?") { (200, rows(path)) } else { (200, "{}".into()) })
}

#[test]
fn the_dashboard_on_another_machine_lists_that_machines_sessions_asks_only_it_offers_no_archive_or_delete_and_opens_a_session_on_it() {
    let falcon = falcon_ok();
    let (mut r, home) = on_falcon(&falcon);
    r.wait("falcon's sessions", |s| s.dash.other.sessions.len() == 1);
    r.rt.frame(FRAME);
    assert_eq!(titles(&r.rt), ["Lighthouse sensor feed"], "that machine's sessions alone");
    let asked = falcon.calls("/api/sessions?limit=100");
    let own: Vec<_> = asked.iter().filter(|c| c.1 == path(Retention::Active)).collect();
    assert_eq!(own.len(), 1, "read once: {asked:?}");
    assert_eq!(own[0].3.as_deref(), Some("falcon-token-0123456789"), "with its own token");
    assert!(home.calls("limit=100").iter().all(|c| c.1.contains("offset=0")), "home isn't asked for falcon's page");
    assert!(!has(&r.rt, "dash-row-f-1-archive") && !has(&r.rt, "dash-row-f-1-delete"));
    click(&mut r.rt, "dash-row-f-1");
    assert!(!r.state().on_home());
    assert_eq!(r.state().session().title, "Lighthouse sensor feed");
    assert_eq!(r.state().machines.session_machines.get("f-1").map(String::as_str), Some(FALCON));
}

#[test]
fn it_says_its_loading_that_machines_sessions_from_the_first_frame_not_that_it_has_none() {
    let falcon = Fake::start(|_, _| (200, "[]".into()));
    let (mut r, _home) = on_falcon(&falcon);
    assert!(!has(&r.rt, "home-empty"));
    assert!(has(&r.rt, "home-loading"));
    r.wait("the answer", |s| s.dash.other.key.is_some() && !s.dash.other.loading);
    r.rt.frame(FRAME);
    assert!(has(&r.rt, "home-empty"));
}

#[test]
fn a_failed_load_says_it_failed_not_that_there_are_no_sessions() {
    let falcon = Fake::start(|_, path| if path == path_of_dash() { (503, "{}".into()) } else { (200, "[]".into()) });
    let (mut r, _home) = on_falcon(&falcon);
    r.wait("the failure", |s| s.dash.other.error.is_some());
    r.rt.frame(FRAME);
    assert!(!has(&r.rt, "home-empty"));
    assert!(has(&r.rt, "home-load-error"));
    assert_eq!(r.state().dash.other.error.as_deref(), Some(UNREADABLE));
}

fn path_of_dash() -> &'static str {
    "/api/sessions?limit=100"
}

#[test]
fn an_answer_a_newer_read_overtook_is_dropped_and_a_failure_keeps_what_was_shown() {
    let mut o = Other::default();
    o.mount("m");
    let first = o.begin(0.0, Retention::Active);
    let second = o.begin(1.0, Retention::Active);
    assert!(!o.finish(first, Err("late".into())), "stale");
    assert!(o.error.is_none() && o.loading);
    let row = crate::mock::projects().remove(0).sessions.remove(0);
    assert!(o.finish(second, Ok(vec![row])));
    assert_eq!(o.sessions.len(), 1);
    let third = o.begin(2.0, Retention::Active);
    assert!(!o.loading, "loading shows only while nothing is held");
    assert!(o.finish(third, Err("down".into())));
    assert_eq!((o.sessions.len(), o.error.as_deref()), (1, Some("down")));
}

#[test]
fn it_reads_again_every_15_seconds_and_when_the_filter_changes_and_not_once_left() {
    let mut o = Other::default();
    assert!(!o.wants_read(0.0, Retention::Active), "not mounted");
    o.mount("m");
    assert!(o.wants_read(0.0, Retention::Active), "the first read is due at once");
    o.begin(10.0, Retention::Active);
    assert!(!o.wants_read(24.9, Retention::Active));
    assert!(o.wants_read(25.0, Retention::Active));
    assert!(o.wants_read(11.0, Retention::Archived), "a new filter reads at once");
    o.unmount();
    assert!(!o.wants_read(99.0, Retention::Active));
}

#[test]
fn the_path_leaves_the_retention_out_for_the_active_list() {
    assert_eq!(path(Retention::Active), "/api/sessions?limit=100");
    assert_eq!(path(Retention::Archived), "/api/sessions?limit=100&retentionStatus=archived");
    assert_eq!(path(Retention::All), "/api/sessions?limit=100&retentionStatus=all");
}
