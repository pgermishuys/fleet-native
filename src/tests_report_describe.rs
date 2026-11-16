//! Report a problem, piece Q of docs/parity/09-app-frame.md: `ProblemReportDialog.test.ts` cases 1 and 2 under their names,
//! and the rows Fleet's tests don't name (the entry points, Esc and the overlay, the fields, the ticked parts, Retake, a
//! screenshot that couldn't be taken). A screenshot is handed in as a test hook: Rune can't take one (`docs/RUNE.md`).
use crate::fleet::report::{Kind, Source, Stage};
use crate::fleet::report_form::Item;
use crate::mock::report::{Case, shot};
use crate::tests::{FRAME, app, click};
use rune::{ElementInfo, Key, KeyEvent, Mods, Runtime};

type Rt = Runtime<crate::State>;

fn info(rt: &Rt, key: &str) -> ElementInfo {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}"))
}

fn all_text(rt: &Rt) -> String {
    rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

/// A session Fleet knows, open, titled as the Vue test's.
fn session(rt: &mut Rt) {
    rt.update(|s, _| {
        s.projects[0].sessions[0].id = "s1".into();
        s.projects[0].sessions[0].title = "Fix invoice rounding".into();
        s.route = crate::fleet::store::Route::Session("s1".into());
    });
    rt.frame(FRAME);
}

/// `opened()`: the report opened from Help with a screenshot taken.
fn opened(description: &str) -> Rt {
    let mut rt = app();
    session(&mut rt);
    rt.update(|s, _| {
        s.report_open(Source::Help, None, "");
        s.report_set_screenshot(Some(shot()));
    });
    if !description.is_empty() {
        let text = description.to_string();
        rt.update(move |s, _| s.report_form = crate::fleet::report_form::ReportForm::new(&text));
    }
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn type_into(rt: &mut Rt, key: &str, words: &str) {
    click(rt, key);
    rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

// ---- ProblemReportDialog.test.ts -------------------------------------------------------------------------------------------

#[test]
fn opens_on_describe_with_the_screenshot_and_every_part_ticked_about_the_open_session() {
    let mut rt = app();
    session(&mut rt);
    rt.update(|s, _| {
        s.report_open(Source::Help, None, "");
        s.report_set_screenshot(Some(shot()));
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    let text = all_text(&rt);
    assert!(text.contains("Nothing leaves your machine until you send it."));
    assert!(rt.state.images.ready(crate::fleet::report_form::SHOT_KEY).is_some(), "the thumbnail's picture is decoded");
    assert!(info(&rt, "report-thumb").rect[2] > 0.0);
    assert_eq!(info(&rt, "report-count").text.as_deref(), Some("5 of 5"));
    assert_eq!(info(&rt, "report-about").text.as_deref(), Some("About: Fix invoice rounding"));
    // Review is off until something is written.
    assert!(info(&rt, "report-review").opacity < 1.0);
    rt.state.report_review();
    assert_eq!(rt.state.report.busy, None, "a blank description is not reviewed");
}

#[test]
fn starts_the_description_with_the_text_it_was_opened_with() {
    let mut rt = app();
    session(&mut rt);
    rt.update(|s, tx| s.report_turn_failure("overloaded", tx));
    rt.frame(FRAME);
    assert_eq!(rt.state.report_form.description.text, "This turn stopped early: overloaded\n\n");
    assert_eq!(rt.state.report.opening.as_ref().map(|o| o.from), Some(Source::Failure));
    assert!(info(&rt, "report-review").opacity == 1.0, "words are there, so Review is on");
}

// ---- the entry points -------------------------------------------------------------------------------------------------------

#[test]
fn help_report_a_problem_opens_the_dialog_and_closes_the_menu() {
    let mut rt = app();
    click(&mut rt, "rail-help");
    click(&mut rt, "help-pop-item0");
    assert!(rt.state.report.open);
    assert_eq!(rt.state.report.opening.as_ref().map(|o| o.from), Some(Source::Help));
    assert_eq!(rt.state.ui.open, None);
    assert!(rt.find("report-dialog").is_some());
}

#[test]
fn the_palette_row_opens_it_and_puts_the_palette_away() {
    let mut rt = app();
    rt.update(crate::ui::palette::toggle);
    rt.update(|s, tx| crate::ui::commands::run(s, tx, "report-problem"));
    rt.frame(FRAME);
    assert!(rt.state.report.open);
    assert_eq!(rt.state.report.opening.as_ref().map(|o| o.from), Some(Source::Palette));
    assert_eq!(rt.state.ui.open, None);
}

#[test]
fn the_failure_cards_report_this_opens_it_about_that_session() {
    let mut rt = app();
    rt.update(|s, _| s.show_failure(crate::mock::failure::Case::Stopped, false));
    rt.frame(FRAME);
    click(&mut rt, "turn-failure-a1-report");
    assert!(rt.state.report.open);
    assert!(rt.state.report_form.description.text.starts_with("This turn stopped early: "));
}

#[test]
fn opening_it_again_while_it_is_open_does_nothing() {
    let mut rt = opened("");
    type_into(&mut rt, "report-description", "Half a thought");
    assert!(!rt.state.report_open(Source::Palette, None, "other"));
    assert_eq!(rt.state.report_form.description.text, "Half a thought");
}

// ---- the window --------------------------------------------------------------------------------------------------------------

#[test]
fn the_window_is_820_wide_in_a_big_window_and_leaves_16_px_in_a_small_one() {
    let mut rt = opened("");
    assert_eq!(info(&rt, "report-dialog").rect[2], 820.0);
    rt.update(|s, _| s.ui.sel.window = (700.0, 500.0));
    rt.frame(FRAME);
    rt.frame(FRAME);
    let r = info(&rt, "report-dialog").rect;
    assert_eq!(r[2], 668.0);
    assert!(r[3] <= 468.0 + 0.5, "at most the window less 32: {r:?}");
}

#[test]
fn esc_and_the_overlay_close_it_and_the_x_and_cancel_too() {
    let mut rt = opened("");
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.frame(FRAME);
    assert!(!rt.state.report.open, "Esc");
    for key in ["report-close", "report-cancel"] {
        let mut rt = opened("");
        click(&mut rt, key);
        assert!(!rt.state.report.open, "{key}");
        assert!(rt.state.images.ready(crate::fleet::report_form::SHOT_KEY).is_none(), "the screenshot goes with it");
    }
    let mut rt = opened("");
    // The scrim's middle is under the window: press its corner.
    rt.pointer_move(5.0, 5.0);
    rt.pointer_down(5.0, 5.0);
    rt.pointer_up(5.0, 5.0);
    rt.frame(FRAME);
    assert!(!rt.state.report.open, "the overlay");
}

#[test]
fn it_does_not_close_while_it_is_sending() {
    let mut rt = opened("");
    rt.update(|s, _| s.report.busy = Some(crate::fleet::report::Busy::Send));
    rt.frame(FRAME);
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.frame(FRAME);
    assert!(rt.state.report.open);
}

#[test]
fn the_steps_show_which_one_the_report_is_on() {
    let rt = opened("");
    assert_eq!(rt.state.report.stage, Stage::Describe);
    assert!(all_text(&rt).contains("1 Describe") && all_text(&rt).contains("2 Review"));
}

// ---- Describe ----------------------------------------------------------------------------------------------------------------

#[test]
fn the_kind_pills_pick_one_and_start_on_something_broke() {
    let mut rt = opened("");
    assert_eq!(rt.state.report_form.kind, Kind::Bug);
    click(&mut rt, "report-kind-idea");
    assert_eq!(rt.state.report_form.kind, Kind::Idea);
    click(&mut rt, "report-kind-looks-wrong");
    assert_eq!(rt.state.report_form.kind, Kind::LooksWrong);
}

#[test]
fn typing_a_description_turns_review_on() {
    let mut rt = opened("");
    assert!(info(&rt, "report-review").opacity < 1.0);
    type_into(&mut rt, "report-description", "It stopped updating.");
    assert_eq!(rt.state.report_form.description.text, "It stopped updating.");
    assert!(info(&rt, "report-review").opacity == 1.0);
    type_into(&mut rt, "report-expected", "It keeps going.");
    type_into(&mut rt, "report-contact", "@sam-example");
    assert_eq!(rt.state.report_form.expected.text, "It keeps going.");
    assert_eq!(rt.state.report_form.contact.text, "@sam-example");
}

#[test]
fn unticking_a_part_counts_it_out_and_ticking_it_again_counts_it_back() {
    let mut rt = opened("");
    click(&mut rt, "report-include-log");
    assert_eq!(info(&rt, "report-count").text.as_deref(), Some("4 of 5"));
    assert!(!rt.state.report_form.include.log);
    click(&mut rt, "report-include-screenshot");
    assert_eq!(info(&rt, "report-count").text.as_deref(), Some("3 of 5"));
    assert!(!rt.state.report_form.has_screenshot());
    click(&mut rt, "report-include-log");
    click(&mut rt, "report-include-screenshot");
    assert_eq!(info(&rt, "report-count").text.as_deref(), Some("5 of 5"));
}

#[test]
fn without_a_screenshot_it_says_so_counts_four_and_the_box_cannot_be_ticked() {
    let mut rt = app();
    session(&mut rt);
    rt.update(|s, _| {
        s.report_open(Source::Help, None, "");
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.state.report_form.screenshot.is_none(), "Rune can't give the app its own frame");
    assert!(all_text(&rt).contains("Couldn't take a screenshot. The report works without one."));
    assert_eq!(info(&rt, "report-count").text.as_deref(), Some("4 of 5"));
    rt.state.report_form.toggle(Item::Screenshot);
    assert!(rt.state.report_form.shot_on, "a box with nothing behind it stays as it was");
    assert!(!rt.state.report_form.has_screenshot());
}

#[test]
fn retake_takes_it_again_and_keeps_what_was_typed() {
    let mut rt = opened("");
    type_into(&mut rt, "report-description", "Kept");
    click(&mut rt, "report-retake");
    assert_eq!(rt.state.report_form.description.text, "Kept");
    assert!(rt.state.report.open);
    assert!(rt.state.report_form.screenshot.is_none(), "there is no frame to take, so Retake leaves none");
    assert!(all_text(&rt).contains("Couldn't take a screenshot"));
}

#[test]
fn the_screenshot_is_taken_at_and_as_big_as_it_says() {
    let rt = opened("");
    assert_eq!(info(&rt, "report-shot-taken").text.as_deref(), Some("Taken 02:03:10 PM · 1280×800"));
}

#[test]
fn the_screenshot_goes_to_the_server_as_a_data_url() {
    let url = shot().data_url();
    assert!(url.starts_with("data:image/png;base64,iVBORw0KGgo"), "a PNG's first bytes in base64");
    assert!(!url.contains('\n'));
    let tiny = crate::fleet::report_form::Shot { png: vec![1, 2, 3, 4], width: 1, height: 1, taken: String::new() };
    assert_eq!(tiny.data_url(), "data:image/png;base64,AQIDBA==");
}

#[test]
fn every_scene_of_it_draws() {
    for case in [Case::Fresh, Case::Filled, Case::NoShot] {
        let mut rt = app();
        rt.update(move |s, _| s.show_report(case));
        rt.frame(FRAME);
        rt.frame(FRAME);
        assert!(rt.find("report-dialog").is_some());
    }
}
