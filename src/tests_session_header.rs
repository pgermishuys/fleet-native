//! The session header drawn (component 8, piece B): the cases of `SessionDetailHeader.test.ts` that the drawing decides,
//! under Fleet's names, and the banners of `routes/sessions.$id.tsx`. The data under it is tested in `fleet/tests/header.rs`.
use crate::fleet::header::Indicator;
use crate::mock::header::Case;
use crate::tests::{FRAME, app, click};
use rune::Runtime;

fn scene(case: Case, list: bool) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_header(case, true, list));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt
}

fn has(rt: &Runtime<crate::State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn width(rt: &Runtime<crate::State>, key: &str) -> f32 {
    rt.presented(key).map(|p| p.0[2]).unwrap_or_else(|| panic!("{key} isn't drawn"))
}

#[test]
fn shows_no_visible_status_while_a_sessions_list_is_on_screen() {
    let rt = scene(Case::Retry, true);
    assert!(!has(&rt, "session-header-glyph"));
    // The retry note is the header's own words about the retry, not a status glyph.
    assert!(has(&rt, "session-retry-note"));
}

#[test]
fn shows_the_rows_glyph_before_the_title_when_no_sessions_list_is_on_screen() {
    let mut rt = scene(Case::Retry, true);
    assert!(!has(&rt, "session-header-glyph"));
    rt.update(|s, _| s.sidebar_open = false);
    rt.frame(FRAME);
    assert!(has(&rt, "session-header-glyph"));
    assert_eq!(rt.state.header_model().glyph.word(), "active");
}

#[test]
fn names_a_retry_and_its_attempt_next_to_the_title() {
    let rt = scene(Case::Retry, true);
    let m = rt.state.header_model();
    assert_eq!(m.indicator, Indicator::Retry);
    assert!(m.retry_note.as_deref().unwrap().starts_with("Retrying · attempt 3 of 10"));
    assert!(has(&rt, "session-retry-note"));
    let quiet = scene(Case::Fork, true);
    assert!(!has(&quiet, "session-retry-note"));
}

#[test]
fn says_a_session_stopped_on_a_question_needs_input_with_the_rows_diamond() {
    let mut rt = scene(Case::Fork, false);
    rt.update(|s, _| s.projects[0].sessions[0].row.activity = "waiting_input".into());
    rt.frame(FRAME);
    let m = rt.state.header_model();
    assert_eq!((m.indicator, m.status_label.as_str(), m.glyph.word()), (Indicator::Waiting, "Needs input", "waiting_input"));
    assert!(has(&rt, "session-header-glyph"));
}

#[test]
fn says_the_machine_isnt_answering_instead_of_working() {
    let rt = scene(Case::MachineDown, true);
    let m = rt.state.header_model();
    assert_eq!(m.indicator, Indicator::NotAnswering);
    assert_eq!(m.status_label, "Not answering");
    assert_eq!(m.machine.as_ref().unwrap().label(), "harbor · not answering");
    assert!(has(&rt, "session-machine"));
}

#[test]
fn links_a_session_another_session_started_back_to_it_by_its_title() {
    let mut rt = scene(Case::Fork, true);
    assert!(has(&rt, "session-lineage-link"));
    let parent = rt.state.header_model().lineage.unwrap().0;
    assert_eq!(rt.state.parent_title(&parent).as_deref(), Some("Move the image cache to disk"));
    click(&mut rt, "session-lineage-link");
    assert_eq!(rt.state.open_id().as_deref(), Some("a-cache"), "a plain click opens the parent in place");
}

#[test]
fn says_forked_from_for_a_parent_the_list_doesnt_hold_and_names_it_once_read() {
    let mut rt = scene(Case::Fork, true);
    rt.update(|s, _| s.projects[0].sessions.truncate(1));
    assert_eq!(rt.state.parent_title("a-cache"), None, "not in the list, not read yet: the pill says another session");
    rt.update(|s, _| {
        s.parent_titles.insert("a-cache".into(), "An archived session".into());
    });
    assert_eq!(rt.state.parent_title("a-cache").as_deref(), Some("An archived session"));
    rt.frame(FRAME);
    assert!(has(&rt, "session-lineage-link"));
}

#[test]
fn has_no_link_for_a_session_the_user_started() {
    let rt = scene(Case::Archived, true);
    assert!(!has(&rt, "session-lineage-link"));
}

#[test]
fn a_subagents_session_gets_its_banner_and_never_the_pill() {
    let mut rt = scene(Case::Subagent, true);
    assert!(!has(&rt, "session-lineage-link"));
    assert!(has(&rt, "session-delegated-banner") && has(&rt, "session-parent-link"));
    click(&mut rt, "session-back-to-parent");
    assert_eq!(rt.state.open_id().as_deref(), Some("a-cache"));
}

#[test]
fn a_subagent_whose_parent_isnt_listed_says_it_was_opened_from_a_parent_session() {
    let mut rt = scene(Case::Subagent, true);
    rt.update(|s, _| s.projects[0].sessions.truncate(1));
    rt.frame(FRAME);
    assert!(has(&rt, "session-parent-label") && !has(&rt, "session-parent-link"));
}

#[test]
fn cuts_the_path_from_the_start_the_folders_own_name_is_drawn_apart_from_the_rest() {
    let m = scene(Case::Fork, true).state.header_model();
    assert_eq!(m.directory_head, "/srv/work/orchard-worktrees");
    assert_eq!(m.directory_leaf, "/fleet-session-4c2d1e90");
}

#[test]
fn a_long_title_truncates_the_pill_keeps_half() {
    let mut rt = scene(Case::LongText, true);
    assert!(has(&rt, "session-lineage-link"));
    // 1280 wide: the conversation column beside the 48px rail of the content panel.
    let (title, pill, header) = (width(&rt, "session-title"), width(&rt, "session-lineage-link"), width(&rt, "session-header"));
    assert!(title < header * 0.7, "the title gave way: {title}");
    assert!(pill > 100.0 && pill <= header / 2.0, "the pill kept a share of at most half: {pill} of {header}");
    // The path's leaf is never the part that is cut while its head still has text.
    rt.frame(FRAME);
    assert!(width(&rt, "session-directory") >= 24.0);
}

#[test]
fn says_a_stopped_or_lost_session_isnt_running_and_shows_nothing_for_a_live_one() {
    for (case, shown) in [(Case::Stopped, true), (Case::Disconnected, true), (Case::Fork, false), (Case::Retry, false)] {
        let rt = scene(case, true);
        assert_eq!(has(&rt, "session-stopped-banner"), shown, "{case:?}");
    }
    assert!(!has(&scene(Case::Fork, true), "session-banners"), "no banner, no column");
}

#[test]
fn shows_the_archived_badge_and_banner_with_restore() {
    let rt = scene(Case::Archived, true);
    assert!(has(&rt, "session-archived-badge") && has(&rt, "session-archived-banner") && has(&rt, "session-restore-button"));
    assert!(!has(&scene(Case::Fork, true), "session-archived-badge"));
}
