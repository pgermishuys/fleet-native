//! The Files tab drawn and clicked (component 11, piece D): `FileBrowserTreeNode.test.ts` (2) and the Tree block of
//! `FileBrowserPanel.test.ts` (3), on the invented project the scenes draw (no server). The rules under them are in
//! `src/fleet/tests/file_browser.rs`.
use crate::fleet::canvases::FileView;
use crate::mock::files_tab::Case;
use crate::tests::{FRAME, app, click};
use rune::Runtime;

fn open(case: Case) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_files_tab(case, true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn has(rt: &Runtime<crate::State>, key: &str) -> bool {
    rt.find(key).is_some()
}

/// The file tabs, in the strip's order: path, whether a preview.
fn file_tabs(rt: &Runtime<crate::State>) -> Vec<(String, bool)> {
    rt.state.open_canvases().canvases.iter().filter_map(|c| c.file.as_ref().map(|f| (f.path.clone(), f.preview))).collect()
}

fn rows(rt: &Runtime<crate::State>) -> usize {
    rt.elements().iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("dir-node-") || k.starts_with("file-node-"))).count()
}

const PAGINATE: &str = "file-node-src/ledger/paginate.py";

// FileBrowserPanel.test.ts, Tree

#[test]
fn shows_every_root_entry_even_when_some_files_changed() {
    let mut rt = open(Case::Tree);
    rt.update(|s, _| {
        s.file_browser = Default::default();
        s.file_browser.follow(&s.canvas_session());
        s.file_browser.root_done(&s.canvas_session(), crate::mock::files_tab::browse(None));
    });
    rt.frame(FRAME);
    assert_eq!(rows(&rt), 6, "four folders and two files, though paginate.py and page_size.py changed");
}

#[test]
fn shows_an_empty_state_when_the_session_has_no_files() {
    let rt = open(Case::Empty);
    assert!(has(&rt, "files-empty"));
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("No files found")));
    assert_eq!(rows(&rt), 0);
}

#[test]
fn refreshes_the_tree_from_the_toolbar() {
    let mut rt = open(Case::Tree);
    assert_eq!(rows(&rt), 13, "the six at the root, and what src, ledger and docs hold");
    click(&mut rt, "files-refresh");
    rt.frame(FRAME);
    rt.frame(FRAME);
    // The mock server answers at once: cleared, the root read, then the three folders opened again in turn.
    assert_eq!(rt.state.file_browser.open_paths(), ["src", "src/ledger", "docs"]);
    assert!(!rt.state.file_browser.root_loading());
}

// The states of the body

#[test]
fn says_loading_files_while_the_first_read_is_on_its_way() {
    let mut rt = open(Case::Loading);
    assert!(has(&rt, "files-loading"));
    assert!(rt.state.file_browser.root_loading());
    // Refresh is off meanwhile.
    let before = rt.state.file_browser.root_loading();
    click(&mut rt, "files-refresh");
    assert_eq!(rt.state.file_browser.root_loading(), before);
}

#[test]
fn shows_the_error_of_a_failed_read() {
    let rt = open(Case::Failed);
    assert!(has(&rt, "files-error"));
    assert!(rt.elements().iter().any(|e| e.text.as_deref().is_some_and(|t| t.contains("500 Internal Server Error"))));
    assert!(!has(&rt, "files-tree"));
}

#[test]
fn file_contents_is_drawn_off_until_find_in_files_exists() {
    let mut rt = open(Case::Tree);
    assert!(has(&rt, "files-mode-names") && has(&rt, "files-mode-contents"));
    click(&mut rt, "files-mode-contents");
    assert_eq!(rt.state.file_browser.search_in, crate::fleet::file_browser::SearchIn::Names);
}

#[test]
fn a_typed_query_shows_the_clear_button_and_clearing_empties_the_box() {
    let mut rt = open(Case::Tree);
    assert!(!has(&rt, "files-search-clear"));
    rt.update(|s, _| s.file_browser.field = rune::TextEdit::new("pag"));
    rt.frame(FRAME);
    assert!(has(&rt, "files-search-clear"));
    click(&mut rt, "files-search-clear");
    assert_eq!(rt.state.file_browser.field.text, "");
    assert!(!has(&rt, "files-search-clear"));
}

#[test]
fn a_folder_row_opens_and_closes_its_folder() {
    let mut rt = open(Case::Tree);
    assert!(!has(&rt, "file-node-tests/test_paginate.py"));
    click(&mut rt, "dir-node-tests");
    assert!(has(&rt, "file-node-tests/test_paginate.py"), "the mock server answers at once");
    click(&mut rt, "dir-node-tests");
    assert!(!has(&rt, "file-node-tests/test_paginate.py"));
}

#[test]
fn a_changed_file_has_its_badge_and_counts_said_to_screen_readers() {
    let rt = open(Case::Tree);
    let labels: Vec<String> = rt
        .elements()
        .iter()
        .filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("file-marks-")))
        .filter_map(|e| e.label.clone())
        .collect();
    assert_eq!(labels, ["added, 5 additions", "modified, 2 additions, 2 deletions"]);
}

// FileBrowserTreeNode.test.ts

#[test]
fn opens_a_preview_tab_after_one_click_once_a_double_click_cant_follow() {
    let mut rt = open(Case::Tree);
    click(&mut rt, PAGINATE);
    assert!(file_tabs(&rt).is_empty(), "the tab waits for a second click");
    assert_eq!(rt.state.file_browser.selected.as_deref(), Some("src/ledger/paginate.py"), "but the file is selected");
    for _ in 0..14 {
        rt.frame(FRAME);
    }
    assert!(file_tabs(&rt).is_empty(), "under 250 ms");
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    assert_eq!(file_tabs(&rt), [("src/ledger/paginate.py".to_string(), true)]);
}

#[test]
fn a_double_click_opens_a_kept_tab_and_only_that() {
    let mut rt = open(Case::Tree);
    click(&mut rt, PAGINATE);
    click(&mut rt, PAGINATE);
    for _ in 0..40 {
        rt.frame(FRAME);
    }
    assert_eq!(file_tabs(&rt), [("src/ledger/paginate.py".to_string(), false)], "one tab, kept, and no preview after it");
    let tabs = rt.state.open_canvases();
    assert_eq!(tabs.active_id, "file:src/ledger/paginate.py");
    assert_eq!(tabs.canvases.iter().filter_map(|c| c.file.as_ref()).map(|f| f.view).collect::<Vec<_>>(), [FileView::Edit]);
}

#[test]
fn a_click_on_another_file_replaces_the_wait() {
    let mut rt = open(Case::Tree);
    click(&mut rt, PAGINATE);
    click(&mut rt, "file-node-src/ledger/totals.py");
    for _ in 0..40 {
        rt.frame(FRAME);
    }
    assert_eq!(file_tabs(&rt), [("src/ledger/totals.py".to_string(), true)], "only the last file opens");
    assert_eq!(rt.state.file_browser.selected.as_deref(), Some("src/ledger/totals.py"));
}

// FileBrowserPanel.test.ts, File selection, and the search around it (piece E)

use crate::fleet::file_browser::SearchIn;
use rune::{Key, KeyEvent, Mods};

fn type_in_box(rt: &mut Runtime<crate::State>, words: &str) {
    click(rt, "files-search");
    rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
    // The 80 ms wait, then the invented project answers at once.
    for _ in 0..12 {
        rt.frame(FRAME);
    }
}

fn press_key(rt: &mut Runtime<crate::State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn results(rt: &Runtime<crate::State>) -> Vec<String> {
    rt.elements().iter().filter_map(|e| e.key.as_deref()?.strip_prefix("file-result-").map(str::to_string)).collect()
}

#[test]
fn calls_selectfile_on_content_panel_and_file_browser_when_search_result_clicked() {
    let mut rt = open(Case::Tree);
    type_in_box(&mut rt, "paginate");
    assert_eq!(results(&rt), ["src/ledger/paginate.py", "tests/test_paginate.py"]);
    click(&mut rt, "file-result-src/ledger/paginate.py");
    assert_eq!(rt.state.file_browser.selected.as_deref(), Some("src/ledger/paginate.py"));
    assert_eq!(file_tabs(&rt), [("src/ledger/paginate.py".to_string(), true)], "a preview, at once (no double-click wait)");
}

#[test]
fn opens_the_first_search_result_as_a_kept_tab_on_enter() {
    let mut rt = open(Case::Tree);
    type_in_box(&mut rt, "paginate");
    press_key(&mut rt, Key::Enter);
    assert_eq!(file_tabs(&rt), [("src/ledger/paginate.py".to_string(), false)]);
    assert_eq!(rt.state.file_browser.selected.as_deref(), Some("src/ledger/paginate.py"));
}

#[test]
fn one_character_leaves_the_tree_two_replace_it_with_results_and_folders_are_left_out() {
    let mut rt = open(Case::Tree);
    rt.update(|s, _| s.file_browser.field = rune::TextEdit::new("p"));
    rt.frame(FRAME);
    assert!(has(&rt, "files-tree") && !has(&rt, "files-results"));
    rt.update(|s, _| s.file_browser.field = rune::TextEdit::new("le"));
    for _ in 0..12 {
        rt.frame(FRAME);
    }
    assert!(has(&rt, "files-results") && !has(&rt, "files-tree"));
    assert!(!results(&rt).iter().any(|p| p.ends_with('/')), "src/ledger/ matches \"le\" and is not listed");
}

#[test]
fn says_no_files_found_for_a_query_nothing_matches_and_clearing_brings_the_tree_back() {
    let mut rt = open(Case::SearchNone);
    assert!(has(&rt, "files-search-empty"));
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("Try a different search query")));
    click(&mut rt, "files-search-clear");
    rt.frame(FRAME);
    assert!(has(&rt, "files-tree") && !has(&rt, "files-search-empty"));
}

#[test]
fn escape_clears_the_query() {
    let mut rt = open(Case::Search);
    assert!(has(&rt, "files-results"));
    click(&mut rt, "files-search");
    press_key(&mut rt, Key::Escape);
    assert_eq!(rt.state.file_browser.field.text, "");
    assert!(has(&rt, "files-tree"));
}

#[test]
fn the_contents_mode_does_not_search_names() {
    let mut rt = open(Case::Search);
    rt.update(|s, _| s.file_browser.search_in = SearchIn::Contents);
    rt.frame(FRAME);
    assert!(!has(&rt, "files-results"), "Find in files is component 12's");
}
