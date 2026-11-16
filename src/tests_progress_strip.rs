//! The progress strip (component 8, piece K). Fleet has no test for `SessionMetadataHeader.vue`, so the names say what it does.
use crate::fleet::context_tab::PanelTab;
use crate::fleet::progress_line::{self as line, PROGRESS_TAB};
use crate::fleet::row_chips::Progress;
use crate::tests::{FRAME, app, click};
use rune::Runtime;

fn todos(done: u32, total: u32, current: Option<&str>) -> Progress {
    Progress { kind: "todos".into(), done, total, current: current.map(str::to_string) }
}

fn shown(done: u32, total: u32, current: Option<&str>) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.show_progress_strip(done, total, current, true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

#[test]
fn heads_the_line_with_todos_and_names_the_todo_in_hand() {
    let p = todos(3, 7, Some("Write the migration"));
    assert_eq!(line::heading(&p), "Todos");
    assert_eq!(line::current_line(&p), "Write the migration");
}

#[test]
fn says_every_todo_is_done_when_none_is_named() {
    assert_eq!(line::current_line(&todos(7, 7, None)), "Every todo is done");
}

#[test]
fn labels_the_strip_with_the_counts_the_line_and_where_it_leads() {
    assert_eq!(line::label(&todos(3, 7, Some("Write the migration"))), "3 of 7 todos done. Write the migration. Open the Progress tab");
}

#[test]
fn reaches_as_far_along_the_top_as_the_share_done() {
    assert_eq!(line::fraction(&todos(0, 4, None)), 0.0);
    assert_eq!(line::fraction(&todos(1, 4, None)), 0.25);
    assert_eq!(line::fraction(&todos(9, 4, None)), 1.0);
    assert_eq!(line::fraction(&todos(0, 0, None)), 0.0);
}

#[test]
fn is_hidden_with_no_items_and_on_the_progress_tab() {
    let p = todos(1, 2, None);
    assert!(line::visible(Some(&p), PanelTab::Context).is_some());
    assert!(line::visible(Some(&p), PROGRESS_TAB).is_none());
    assert!(line::visible(Some(&todos(0, 0, None)), PanelTab::Context).is_none());
    assert!(line::visible(None, PanelTab::Context).is_none());
}

#[test]
fn draws_the_strip_under_the_context_tab_with_its_bar() {
    let rt = shown(3, 7, Some("Write the migration"));
    assert!(rt.find("progress-strip").is_some());
    assert!(rt.find("progress-strip-bar").is_some());
}

#[test]
fn draws_no_bar_when_nothing_is_done() {
    let rt = shown(0, 3, Some("Start"));
    assert!(rt.find("progress-strip").is_some());
    assert!(rt.find("progress-strip-bar").is_none());
}

#[test]
fn a_click_opens_the_progress_tab_and_the_strip_goes_away() {
    let mut rt = shown(3, 7, Some("Write the migration"));
    click(&mut rt, "progress-strip");
    rt.frame(FRAME);
    assert_eq!(rt.state.context.tab, PROGRESS_TAB);
    assert!(rt.find("progress-strip").is_none());
}
