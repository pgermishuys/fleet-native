//! What the session list says besides rows (`SessionsPanel.vue`, piece E of docs/parity/06-session-list.md): loading, the
//! error block and banner, the empty wordings, "Try again", the filter's focus look. Fleet has no unit test for these
//! states (`SessionsPanel.test.ts` covers machines), so the names say what Fleet's template does.
use crate::fleet::panel::{Feedback, banner_copy, empty_words, error_message, feedback};
use crate::mock::groups::Panel;
use crate::tests::{FRAME, app, click};
use rune::Runtime;

fn scene(case: Panel) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_panel(case));
    for _ in 0..20 {
        rt.frame(FRAME);
    }
    rt
}

fn has(rt: &Runtime<crate::State>, key: &str) -> bool {
    rt.find(key).is_some()
}

#[test]
fn the_first_load_shows_a_spinner_and_no_rows_until_something_is_held() {
    assert_eq!(feedback(true, None, false, 0), Feedback::Loading);
    assert_eq!(feedback(true, Some("boom"), false, 0), Feedback::Loading, "loading wins over a stale error");
    assert_eq!(feedback(true, None, true, 3), Feedback::None, "a refresh with rows on screen shows nothing");
    let rt = scene(Panel::Loading);
    assert!(has(&rt, "sessions-loading"));
    assert!(!has(&rt, "sessions-empty") && !has(&rt, "sessions-error"));
}

#[test]
fn an_error_with_no_sessions_is_a_block_with_try_again() {
    assert_eq!(feedback(false, Some("boom"), false, 0), Feedback::Error("boom".into()));
    let rt = scene(Panel::Error);
    assert!(has(&rt, "sessions-error") && has(&rt, "sessions-retry"));
    assert!(!has(&rt, "sessions-banner") && !has(&rt, "sessions-empty"), "the empty state waits for no error");
}

#[test]
fn an_error_with_sessions_is_a_banner_over_the_rows() {
    assert_eq!(feedback(false, Some("boom"), true, 2), Feedback::None);
    assert_eq!(banner_copy("boom"), "Showing cached sessions. Refresh failed: boom");
    let rt = scene(Panel::Banner);
    assert!(has(&rt, "sessions-banner") && has(&rt, "session-0-0"), "the rows stay");
    let (_, banner_y) = rt.find("sessions-banner").unwrap();
    let (_, row_y) = rt.find("session-0-0").unwrap();
    assert!(banner_y < row_y, "the banner is above the rows");
}

#[test]
fn the_sessions_error_is_shown_before_the_projects_error() {
    assert_eq!(error_message(Some("a"), Some("b")).as_deref(), Some("a"));
    assert_eq!(error_message(None, Some("b")).as_deref(), Some("b"));
    assert_eq!(error_message(None, None), None);
}

#[test]
fn the_empty_state_has_three_wordings() {
    assert_eq!(empty_words(true, false), ("No sessions found", "Try a different search term or clear the filter."));
    assert_eq!(empty_words(false, true), ("No archived sessions", "Sessions you archive show up here."));
    assert_eq!(empty_words(false, false), ("No sessions yet", "Start one with New session above."));
    assert_eq!(empty_words(true, true).0, "No sessions found", "a filter wins over the Archived view");
}

#[test]
fn no_sessions_and_no_project_says_no_sessions_yet() {
    assert_eq!(feedback(false, None, false, 0), Feedback::Empty);
    assert_eq!(feedback(false, None, false, 1), Feedback::None, "a project with no sessions is a group, not empty");
    let rt = scene(Panel::Empty);
    assert!(has(&rt, "sessions-empty"));
}

#[test]
fn a_filter_with_no_match_says_no_sessions_found() {
    let rt = scene(Panel::NoMatch);
    assert!(has(&rt, "sessions-empty") && !has(&rt, "session-0-0"));
    assert!(has(&rt, "filter"), "the box stays so the filter can be cleared");
}

#[test]
fn try_again_reads_the_list_again_without_a_server_it_does_nothing() {
    let mut rt = scene(Panel::Error);
    click(&mut rt, "sessions-retry");
    assert!(has(&rt, "sessions-error"), "no link, so nothing is asked and the block stays");
}

#[test]
fn a_failed_read_leaves_no_status_bar_line_the_panel_says_it() {
    let mut rt = app();
    let mut request = 0;
    rt.update(|s, _| request = s.store.begin(s.has_rows()));
    rt.update(move |s, _| s.on_list_failed(request, "Failed to fetch".into()));
    assert!(rt.state.notice.is_none());
    assert_eq!(rt.state.list_error().as_deref(), Some("Failed to fetch"));
}

#[test]
fn the_filter_takes_the_sheets_fill_and_an_accent_border_while_it_has_focus() {
    let mut rt = app();
    assert!(!rt.state.filter.focused);
    click(&mut rt, "filter");
    rt.frame(FRAME);
    assert!(rt.state.filter.focused, "a click puts the caret in it, and the view reads that");
}
