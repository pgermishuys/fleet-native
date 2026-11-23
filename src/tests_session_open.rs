//! Row 2.50-2.54, 2.59, 2.60 of docs/parity/26-parity-audit.md (piece P): a session no list holds opens by its id
//! (`use-session-detail.ts`), "Session not found", home after a delete, the filter box's clear button, Esc and reset, Abort and Delete
//! waiting for an instance, and the fork form starting over. Fleet has no test of its own for the route (`sessions.$id.tsx`), so
//! these come from reading it; the requests are read from the fake server `tests_archive_pins` runs.
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::store::Route;
use crate::tests::{FRAME, app, click};
use crate::tests_archive_pins::{Fake, Rig};
use crate::{Rail, State};
use rune::{Key, KeyEvent, Mods, Runtime, TextEdit};

const RUN: &str = "run-9";

const DETAIL: &str = r#"{"id":"run-9","title":"Nightly index rebuild","workspaceDirectory":"/srv/work/orchard","branch":"main","lifecycleStatus":"running","activityStatus":"idle","retentionStatus":"active","instanceId":"inst-1","createdAt":"2026-11-01T10:00:00Z"}"#;

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn focused(rt: &Runtime<State>) -> Option<String> {
    rt.elements().iter().find(|e| e.focused).and_then(|e| e.key.clone())
}

fn listed(s: &State, id: &str) -> bool {
    s.projects.iter().any(|p| p.sessions.iter().any(|x| x.id == id))
}

fn open(r: &mut Rig, id: &'static str) {
    r.rt.update(move |s, tx| s.open_session_by_id(HOME_MACHINE_KEY, id, tx));
}

// ---- 2.50: a session no list holds opens and loads ---------------------------------------------------------------------------------

#[test]
fn a_session_no_list_holds_is_read_by_its_id_and_put_in_the_list() {
    let fake = Fake::start(|_, path| if path == "/api/sessions/run-9" { (200, DETAIL.into()) } else { (404, "{}".into()) });
    let mut r = Rig::home(&fake);
    open(&mut r, RUN);
    r.wait("the record to be filed", |s| listed(s, RUN));
    assert_eq!(r.state().open_id().as_deref(), Some(RUN));
    assert!(!r.state().session_missing());
    assert_eq!(r.state().session().title, "Nightly index rebuild");
    assert_eq!(r.state().rail, Rail::Sessions);
    assert!(!fake.calls("/api/sessions/run-9").is_empty());
}

#[test]
fn a_sub_agents_session_opens_by_its_id_and_keeps_its_parent() {
    let body = DETAIL.replace(r#""id":"run-9""#, r#""id":"run-9","parentSessionId":"s-parent""#);
    let fake = Fake::start(move |_, path| if path == "/api/sessions/run-9" { (200, body.clone()) } else { (404, "{}".into()) });
    let mut r = Rig::home(&fake);
    open(&mut r, RUN);
    r.wait("the record to be filed", |s| s.store.session_by_id(&s.projects, RUN).is_some());
    assert_eq!(r.state().session().listing.parent.as_deref(), Some("s-parent"));
    assert!(!listed(r.state(), RUN), "the list leaves a sub-agent's session out");
    assert_eq!(r.state().open_id().as_deref(), Some(RUN));
}

#[test]
fn a_list_that_does_not_hold_the_session_yet_leaves_it_open() {
    let mut rt = app();
    rt.update(|s, tx| s.open_session_by_id(HOME_MACHINE_KEY, RUN, tx));
    rt.update(|s, tx| {
        let again = std::mem::take(&mut s.projects);
        s.on_list(again, tx);
    });
    rt.frame(FRAME);
    assert_eq!(rt.state.route, Route::Session(RUN.into()), "the list that arrives first doesn't send it home");
}

#[test]
fn a_listed_session_opens_in_place_without_asking() {
    let fake = Fake::start(|_, _| (404, "{}".into()));
    let mut r = Rig::home(&fake);
    let id = r.state().projects[0].sessions[1].id.clone();
    r.rt.update(move |s, tx| s.open_session_by_id(HOME_MACHINE_KEY, &id, tx));
    r.rt.frame(FRAME);
    assert_eq!(r.state().open_at(), (0, 1));
    assert!(!r.state().session_missing());
    assert!(fake.calls("/api/sessions/mock").is_empty(), "nothing asks the server for a row the list holds");
}

// ---- 2.51: "Session not found" ----------------------------------------------------------------------------------------------------

#[test]
fn a_404_is_session_not_found_and_back_to_sessions_goes_home() {
    let fake = Fake::start(|_, _| (404, "{}".into()));
    let mut r = Rig::home(&fake);
    open(&mut r, "gone-1");
    r.wait("the 404", |s| s.session_missing());
    r.rt.frame(FRAME);
    assert!(has(&r.rt, "session-not-found") && has(&r.rt, "session-not-found-back"));
    assert!(!has(&r.rt, "session-title"), "the header and the conversation are not drawn");
    // A list read that arrives now leaves the page where it is.
    r.rt.update(|s, tx| {
        let again = std::mem::take(&mut s.projects);
        s.on_list(again, tx);
    });
    assert!(r.state().session_missing());
    click(&mut r.rt, "session-not-found-back");
    r.rt.frame(FRAME);
    assert_eq!(r.rt.state.route, Route::Home);
    assert!(!has(&r.rt, "session-not-found"));
}

#[test]
fn another_failure_is_not_session_not_found() {
    let fake = Fake::start(|_, _| (500, "{}".into()));
    let mut r = Rig::home(&fake);
    open(&mut r, "gone-1");
    r.wait("the answer", |_| !fake.calls("/api/sessions/gone-1").is_empty());
    r.rt.frame(FRAME);
    assert!(!r.state().session_missing(), "only a 404 says the session doesn't exist (`response.status === 404`)");
}

// ---- 2.52: after a delete, home -------------------------------------------------------------------------------------------------------

#[test]
fn a_deleted_open_session_leaves_the_window_at_home() {
    let mut rt = app();
    let id = rt.state.session().id.clone();
    rt.update(move |s, tx| s.forget_session(&id, tx));
    rt.frame(FRAME);
    assert_eq!(rt.state.route, Route::Home, "`/`, not another session");
    assert!(has(&rt, "home"));
}

// ---- 2.53, 2.54: the filter box -----------------------------------------------------------------------------------------------------

fn type_filter(rt: &mut Runtime<State>, text: &'static str) {
    rt.update(move |s, _| s.filter = TextEdit::new(text));
    rt.frame(FRAME);
}

#[test]
fn the_clear_button_shows_with_text_and_empties_the_box() {
    let mut rt = app();
    assert!(!has(&rt, "filter-clear"), "an empty box has none");
    type_filter(&mut rt, "cache");
    assert!(has(&rt, "filter-clear"));
    click(&mut rt, "filter-clear");
    rt.frame(FRAME);
    assert_eq!(rt.state.filter.text, "");
    assert!(!has(&rt, "filter-clear"));
    assert_eq!(focused(&rt).as_deref(), Some("filter"), "the box has the focus again");
}

#[test]
fn escape_in_the_box_clears_it_and_only_then() {
    let mut rt = app();
    type_filter(&mut rt, "cache");
    rt.update(|_, tx| tx.focus("filter", true));
    rt.frame(FRAME);
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.frame(FRAME);
    assert_eq!(rt.state.filter.text, "");
    // With the focus elsewhere, Esc leaves the text alone.
    type_filter(&mut rt, "cache");
    rt.update(|_, tx| tx.focus("new-session", true));
    rt.frame(FRAME);
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.frame(FRAME);
    assert_eq!(rt.state.filter.text, "cache");
}

#[test]
fn the_filter_is_lost_when_another_rail_takes_the_panel() {
    let mut rt = app();
    type_filter(&mut rt, "cache");
    rt.update(|s, _| s.press_rail(Rail::Settings));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.filter.text, "", "Fleet's panel unmounts and its `shallowRef` goes with it");
    rt.update(|s, _| s.press_rail(Rail::Sessions));
    rt.frame(FRAME);
    assert_eq!(rt.state.filter.text, "");
}

#[test]
fn the_filter_is_kept_while_the_sessions_rail_stays_up_and_a_session_opens() {
    let mut rt = app();
    type_filter(&mut rt, "cache");
    rt.update(|s, tx| s.open_session(0, 0, tx));
    rt.frame(FRAME);
    assert_eq!(rt.state.filter.text, "cache");
}

// ---- 3.72: Alt+Up/Down keep the keyboard on the moved row ----------------------------------------------------------------------

#[test]
fn a_moved_pinned_row_keeps_the_focus() {
    let fake = Fake::start(|_, _| (200, r#"{"pinOrder":0.5}"#.into()));
    let mut r = Rig::home(&fake);
    r.rt.update(|s, _| {
        s.projects[0].sessions[0].listing.pin_order = Some(1.0);
        s.projects[0].sessions[1].listing.pin_order = Some(2.0);
    });
    r.rt.frame(FRAME);
    r.rt.update(|_, tx| tx.focus("session-0-1", true));
    r.rt.frame(FRAME);
    r.rt.key(KeyEvent { key: Key::Up, mods: Mods { alt: true, ..Mods::default() } });
    r.wait("the move", |s| s.projects[0].sessions[1].listing.pin_order == Some(0.5));
    r.rt.frame(FRAME);
    assert_eq!(focused(&r.rt).as_deref(), Some("session-0-1"), "the row that moved still has the keyboard");
}

// ---- 2.55: an errored session is live ------------------------------------------------------------------------------------------------

#[test]
fn an_errored_session_counts_as_live_like_fleets_live_statuses() {
    use crate::fleet::row_status::is_live;
    assert!(is_live("active") && is_live("waiting_input") && is_live("error"));
    assert!(!is_live("idle") && !is_live("completed") && !is_live("stopped"));
}

// ---- 2.60: Abort and Delete wait for an instance --------------------------------------------------------------------------------

#[test]
fn abort_and_delete_wait_for_an_instance() {
    let mut rt = crate::tests_header_edit::scene(crate::mock::header::Case::Retry);
    rt.update(|s, _| s.projects[0].sessions[0].listing.instance_id = None);
    rt.frame(FRAME);
    assert!(!rt.state.has_instance());
    rt.update(|s, tx| {
        s.toolbar_abort(tx);
        s.toolbar_delete(tx);
    });
    assert!(rt.state.ui.confirm.is_none(), "Delete… asks nothing without an instance");
    assert!(rt.state.head.bar.aborting.is_none());
    assert!(
        crate::ui::menus::more(&rt.state)
            .iter()
            .any(|e| matches!(e, crate::ui::menu::Entry::Item(i) if i.label == "Delete…" && i.disabled)),
        "its menu row is off"
    );
    rt.update(|s, _| s.projects[0].sessions[0].listing.instance_id = Some("inst-1".into()));
    assert!(rt.state.has_instance());
}

// ---- 2.59: the fork form starts over when the source's title changes ----------------------------------------------------------

#[test]
fn the_fork_form_starts_over_when_the_source_title_changes() {
    let mut rt = crate::tests_header_edit::scene(crate::mock::header::Case::Fork);
    rt.update(|s, tx| s.toolbar_fork(tx));
    rt.frame(FRAME);
    rt.update(|s, _| {
        let d = s.head.bar.fork.as_mut().unwrap();
        d.title = TextEdit::new("my own words");
        d.error = Some("Failed to fork session".into());
    });
    rt.frame(FRAME);
    assert_eq!(rt.state.head.bar.fork.as_ref().unwrap().title.text, "my own words", "typing is kept while nothing changes");
    rt.update(|s, _| s.projects[0].sessions[0].title = "Smaller cache, again".into());
    rt.frame(FRAME);
    let d = rt.state.head.bar.fork.as_ref().expect("the dialog stays open");
    assert_eq!(d.title.text, "Fork of Smaller cache, again");
    assert_eq!(d.source_title, "Smaller cache, again");
    assert!(d.error.is_none());
}
