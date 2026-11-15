//! The session list's row (`components/__tests__/SessionItem.test.ts`, the parts of piece C of docs/parity/06-session-list.md),
//! driven headless over the `row-statuses` scene's fixed clock. Rows there are, in order: working, delegating, retrying,
//! waiting, error, then idle ones from 12 minutes to 120 days, done, offline, failed, cut off (twice), a limit retry,
//! the long title and a title of spaces.
use crate::fleet::row_status::{Dim, Tone};
use crate::tests::{app, click};
use crate::theme::Theme;
use crate::view::session_row::{meta_ink, title_ink};

const WORKING: usize = 0;
const DELEGATING: usize = 1;
const RETRYING: usize = 2;
const WAITING: usize = 3;
const ERROR: usize = 4;
const MINUTES: usize = 5;
const HOURS: usize = 6;
const TWO_DAYS: usize = 7;
const DONE: usize = 11;
const CUT_OFF: usize = 14;
const BLANK_TITLE: usize = 18;

fn scene() -> rune::Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.show_row_statuses(true));
    rt.frame(crate::tests::FRAME);
    rt
}

fn has(rt: &rune::Runtime<crate::State>, key: String) -> bool {
    rt.find(&key).is_some()
}

#[test]
fn sessionitem_renders_the_session_title_and_current_status() {
    let rt = scene();
    assert!(has(&rt, format!("session-title-0-{MINUTES}")));
    assert!(has(&rt, format!("session-status-0-{MINUTES}")), "an idle row says how long ago");
    assert!(has(&rt, format!("session-status-0-{WAITING}")), "a waiting row says Needs input");
    assert!(!has(&rt, format!("session-status-0-{WORKING}")), "a working row shows no word, the glyph animates");
}

#[test]
fn sessionitem_names_delegation_and_retries_on_the_glyph_and_shows_the_retry_attempt() {
    let rt = scene();
    assert!(has(&rt, format!("session-glyph-0-{DELEGATING}")));
    assert!(has(&rt, format!("session-glyph-0-{RETRYING}")));
    assert!(has(&rt, format!("session-status-0-{RETRYING}")), "the attempt, as Retry 3/10");
}

#[test]
fn sessionitem_shows_no_status_glyph_on_an_idle_row_and_keeps_its_slot_so_titles_line_up() {
    let rt = scene();
    assert!(!has(&rt, format!("session-glyph-0-{MINUTES}")));
    let slot = rt.find(&format!("session-slot-0-{MINUTES}")).expect("the idle row keeps the glyph's place");
    let live = rt.find(&format!("session-glyph-0-{WORKING}")).unwrap();
    assert_eq!(slot.0, live.0, "the glyph and its empty slot share a left edge");
}

#[test]
fn sessionitem_keeps_the_glyph_for_sessions_that_are_waiting_or_stopped_by_an_error() {
    let rt = scene();
    assert!(has(&rt, format!("session-glyph-0-{WAITING}")));
    assert!(has(&rt, format!("session-glyph-0-{ERROR}")));
    assert!(!has(&rt, format!("session-glyph-0-{DONE}")), "Done is a word, not a glyph");
}

#[test]
fn sessionitem_a_cut_off_idle_row_fills_the_slot_with_a_dot() {
    let rt = scene();
    assert!(has(&rt, format!("session-glyph-0-{CUT_OFF}")));
}

#[test]
fn sessionitem_leads_a_top_level_row_with_the_caret_so_the_status_dot_sits_beside_the_title() {
    let rt = scene();
    for i in [WORKING, MINUTES] {
        let caret = rt.find(&format!("session-caret-0-{i}")).unwrap();
        let glyph = rt.find(&format!("session-glyph-0-{i}")).or_else(|| rt.find(&format!("session-slot-0-{i}"))).unwrap();
        let title = rt.find(&format!("session-title-0-{i}")).unwrap();
        assert!(caret.0 < glyph.0 && glyph.0 < title.0, "caret, glyph, title in that order on row {i}");
    }
}

#[test]
fn sessionitem_an_untitled_row_reads_untitled_session() {
    let rt = scene();
    assert!(has(&rt, format!("session-title-0-{BLANK_TITLE}")));
    assert_eq!(rt.state.projects[0].sessions[BLANK_TITLE].title.trim(), "");
}

#[test]
fn sessionitem_emits_the_session_when_clicked() {
    let mut rt = scene();
    click(&mut rt, &format!("session-0-{HOURS}"));
    assert_eq!(rt.state.open_at(), (0, HOURS));
}

#[test]
fn sessionitem_dims_a_quiet_row_by_its_last_activity_but_not_a_live_or_open_one() {
    for dark in [true, false] {
        let t = Theme::of(dark);
        let (one, two) = if dark { (0.66, 0.51) } else { (0.77, 0.62) };
        assert_eq!(title_ink(&t, false, Dim::None), t.text.alpha(0.86));
        assert_eq!(title_ink(&t, false, Dim::One), t.text.alpha(one), "after a day, dark: {dark}");
        assert_eq!(title_ink(&t, false, Dim::Two), t.text.alpha(two), "after three days, dark: {dark}");
        assert_eq!(title_ink(&t, true, Dim::Two), t.text, "the open row never dims");
        // The age fades with the title only at the deepest dim.
        assert_eq!(meta_ink(&t, Tone::Quiet, Dim::One), Some(t.text.alpha(t.row_meta)));
        assert_eq!(meta_ink(&t, Tone::Quiet, Dim::Two), Some(t.text.alpha(two)));
    }
    // The open row of the scene is an idle one; it keeps full contrast, a row for two days does not.
    let rt = scene();
    assert_eq!(rt.state.open_at(), (0, MINUTES));
    let item = crate::fleet::row_status::RowItem::of(&rt.state.projects[0].sessions[TWO_DAYS], crate::mock::rows::NOW);
    assert_eq!(crate::fleet::row_status::row_dim(&item, crate::mock::rows::NOW), Dim::One);
    let item = crate::fleet::row_status::RowItem::of(&rt.state.projects[0].sessions[WORKING], crate::mock::rows::NOW);
    assert_eq!(crate::fleet::row_status::row_dim(&item, crate::mock::rows::NOW), Dim::None, "a live row never dims");
}

#[test]
fn the_row_colours_are_the_themes_shares_of_text() {
    let dark = Theme::of(true);
    let light = Theme::of(false);
    assert_eq!((dark.row_dim, dark.row_meta, dark.row_waiting), ([0.66, 0.51], 0.53, 1.0));
    assert_eq!((light.row_dim, light.row_meta, light.row_waiting), ([0.77, 0.62], 0.63, 0.55));
    // `--row-waiting` of 100% is the amber itself; at 55% it is mixed with the light theme's text.
    let amber = meta_ink(&dark, Tone::Retry, Dim::None).unwrap();
    assert!((amber.r - dark.idle.r).abs() < 1e-4 && (amber.g - dark.idle.g).abs() < 1e-4 && (amber.b - dark.idle.b).abs() < 1e-4);
    assert_ne!(meta_ink(&light, Tone::Retry, Dim::None), Some(light.idle));
    assert_eq!(meta_ink(&dark, Tone::Working, Dim::None), None);
}

#[test]
fn the_list_re_reads_the_clock_every_30_seconds_so_ages_move_on() {
    let mut rt = scene();
    let before = rt.state.row_tick;
    rt.update(|s, tx| {
        let first = s.row_tick.unwrap_or(0.0);
        assert!(!s.keep_row_ages(first + 10.0, tx), "too soon");
        assert!(s.keep_row_ages(first + 31.0, tx), "30 s on");
    });
    assert!(rt.state.row_tick > before);
}

// ─── piece D: progress, the running chip and the pull request badge (`rows` of `mock::rows::chips`) ───────────────

fn chips() -> rune::Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.show_row_chips(true));
    rt.frame(crate::tests::FRAME);
    rt
}

#[test]
fn sessionitem_shows_a_progress_ring_and_count_instead_of_the_status_word_while_working() {
    let rt = chips();
    assert!(has(&rt, "session-progress-0-0".into()));
    assert!(has(&rt, "session-progress-count-0-0".into()));
    assert!(!has(&rt, "session-status-0-0".into()));
}

#[test]
fn sessionitem_keeps_words_the_user_has_to_act_on_next_to_the_ring() {
    let rt = chips();
    assert!(has(&rt, "session-progress-0-3".into()));
    assert!(!has(&rt, "session-progress-count-0-3".into()));
    assert!(has(&rt, "session-status-0-3".into()), "Needs input stays");
}

#[test]
fn sessionitem_shows_no_ring_for_a_session_without_progress_or_with_an_empty_list() {
    let rt = chips();
    assert!(!has(&rt, "session-progress-0-7".into()), "no progress");
    assert!(!has(&rt, "session-progress-0-6".into()), "an empty list");
}

#[test]
fn sessionitem_keeps_the_ring_but_shows_the_age_instead_of_the_count_for_a_quiet_session_with_unfinished_items() {
    let rt = chips();
    assert!(has(&rt, "session-progress-0-4".into()));
    assert!(!has(&rt, "session-progress-count-0-4".into()));
    assert!(has(&rt, "session-status-0-4".into()), "the age");
}

#[test]
fn sessionitem_shows_no_progress_once_a_quiet_session_has_finished_its_list_and_its_status_label_comes_back() {
    let rt = chips();
    assert!(!has(&rt, "session-progress-0-5".into()));
    assert!(has(&rt, "session-status-0-5".into()));
}

#[test]
fn sessionitem_still_shows_the_count_while_a_session_works_through_its_last_item() {
    let rt = chips();
    assert!(has(&rt, "session-progress-count-0-2".into()), "7/7 while still working");
}

#[test]
fn sessionitem_shows_a_green_chip_counting_the_work_its_agent_has_running() {
    let rt = chips();
    assert!(has(&rt, "session-running-chip-0-1".into()));
    assert!(has(&rt, "session-running-chip-0-8".into()));
    assert!(!has(&rt, "session-running-chip-0-0".into()), "hidden at zero");
}

#[test]
fn sessionitem_shows_the_pull_request_badge_only_when_the_session_has_one() {
    let rt = chips();
    assert!(has(&rt, "session-pr-0-9".into()));
    assert!(!has(&rt, "session-pr-0-0".into()));
}
