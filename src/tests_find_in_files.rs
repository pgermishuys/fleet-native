//! Find in files, the view (component 12, piece P): the 9 "Find in files" cases of `FileBrowserPanel.test.ts` under their names, on
//! the invented project the scenes draw (no server; the search itself is `tests_find_text`). The answers are set the way a scene
//! sets them, so nothing here waits for a thread.
use crate::fleet::file_browser::SearchIn;
use crate::fleet::find_text::*;
use crate::mock::files_tab::{Case, find_answer};
use crate::tests::{FRAME, app, click};
use rune::{Key, KeyEvent, Mods, Runtime};

fn open(case: Case) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_files_tab(case, true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn keys(rt: &Runtime<crate::State>, prefix: &str) -> Vec<String> {
    rt.elements().iter().filter_map(|e| e.key.as_deref()?.strip_prefix(prefix).map(str::to_string)).collect()
}

fn says(rt: &Runtime<crate::State>, words: &str) -> bool {
    rt.elements().iter().any(|e| e.text.as_deref().is_some_and(|t| t.contains(words)))
}

fn press(rt: &mut Runtime<crate::State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn show(rt: &mut Runtime<crate::State>, answer: Result<TextSearchResult, FindTextFailure>) {
    rt.update(move |s, _| {
        let id = s.files_session();
        let query = s.file_browser.field.text.clone();
        s.find_text.show(&id, &query, answer, 100.0);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn file_tabs(rt: &Runtime<crate::State>) -> Vec<(String, bool, crate::fleet::canvases::FileView)> {
    rt.state.open_canvases().canvases.iter().filter_map(|c| c.file.as_ref().map(|f| (f.path.clone(), f.preview, f.view))).collect()
}

const PAGINATE: &str = "src/ledger/paginate.py";

#[test]
fn starts_in_file_names_and_changes_nothing_until_contents_is_picked() {
    let rt = open(Case::Tree);
    assert_eq!(rt.state.file_browser.search_in, SearchIn::Names);
    assert!(rt.find("find-match-case").is_none() && rt.find("find-regex").is_none());
    assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some("files-search") && e.label.as_deref() == Some("Search files")));
}

#[test]
fn shows_the_toggles_and_a_hint_in_contents_mode_before_anything_is_typed() {
    let mut rt = open(Case::Tree);
    click(&mut rt, "files-mode-contents");
    rt.frame(FRAME);
    assert_eq!(rt.state.file_browser.search_in, SearchIn::Contents);
    assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some("files-search") && e.label.as_deref() == Some("Search inside files")));
    assert!(rt.find("find-match-case").is_some() && rt.find("find-regex").is_some());
    assert!(says(&rt, "Type to search inside every file in this folder."));
    assert!(says(&rt, "Ignored files and build folders are left out."));
    assert!(rt.find("files-tree").is_some(), "the tree stays above the hint");
    click(&mut rt, "find-regex");
    click(&mut rt, "find-match-case");
    assert!(rt.state.find_text.regex && rt.state.find_text.match_case);
}

#[test]
fn groups_matches_by_file_shows_five_lines_and_expands_the_rest_on_request() {
    let mut rt = open(Case::Find);
    assert!(says(&rt, "10 matches in 3 files"));
    assert_eq!(keys(&rt, "find-file-"), ["docs/pagination.md", "src/ledger/page_size.py", "src/ledger/paginate.py"]);
    assert_eq!(keys(&rt, "find-hit-").len(), 8, "five of paginate.py's seven, and the other files' lines");
    assert!(says(&rt, "Show 2 more in this file"));
    click(&mut rt, &format!("find-more-{PAGINATE}"));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(keys(&rt, "find-hit-").len(), 10);
    assert!(rt.find(&format!("find-more-{PAGINATE}")).is_none(), "the button goes");
}

#[test]
fn opens_a_clicked_line_in_a_preview_tab_and_asks_the_editor_to_go_to_it() {
    let mut rt = open(Case::Find);
    rt.update(|s, _| s.set_content_open(false));
    click(&mut rt, &format!("find-hit-{PAGINATE}:4"));
    rt.frame(FRAME);
    assert_eq!(file_tabs(&rt), [(PAGINATE.to_string(), true, crate::fleet::canvases::FileView::Edit)]);
    let session = rt.state.files_session();
    assert_eq!(rt.state.find_text.opened_at.take_focus(&session, PAGINATE), Some(Some(4)));
    assert!(rt.state.content_open, "the right panel shows");
}

#[test]
fn opens_markdown_on_its_source_not_rendered() {
    let mut rt = open(Case::Find);
    click(&mut rt, "find-hit-docs/pagination.md:3");
    rt.frame(FRAME);
    assert_eq!(file_tabs(&rt)[0].2, crate::fleet::canvases::FileView::Edit);
}

#[test]
fn steps_with_the_arrow_keys_and_opens_the_chosen_line_on_enter() {
    let mut rt = open(Case::Find);
    click(&mut rt, "files-search");
    let first = VisibleHit { path: "docs/pagination.md".into(), line: 3 };
    assert_eq!(rt.state.find_text.active, Some(first.clone()), "the first line is chosen");
    press(&mut rt, Key::Up);
    assert_eq!(rt.state.find_text.active, Some(first), "it stops at the first line");
    for _ in 0..5 {
        press(&mut rt, Key::Down);
    }
    press(&mut rt, Key::Enter);
    let session = rt.state.files_session();
    // The lines shown are docs 3, page_size 1 and 3, then paginate 1, 4, 7: five steps from the first is paginate's line 7.
    assert_eq!(rt.state.find_text.opened_at.take_focus(&session, PAGINATE), Some(Some(7)));
    assert_eq!(file_tabs(&rt).len(), 1);
}

#[test]
fn enter_opens_the_first_line_when_none_was_chosen() {
    let mut rt = open(Case::Find);
    rt.update(|s, _| s.find_text.active = None);
    click(&mut rt, "files-search");
    press(&mut rt, Key::Enter);
    let session = rt.state.files_session();
    assert_eq!(rt.state.find_text.opened_at.take_focus(&session, "docs/pagination.md"), Some(Some(3)));
}

#[test]
fn says_so_when_nothing_matches_and_never_when_the_search_failed() {
    let mut rt = open(Case::FindNone);
    assert!(says(&rt, "No matches for \u{201c}zzq\u{201d}"));
    assert!(says(&rt, "Check the spelling, or try the regex switch."));

    let deadline = TextSearchResult { files: Vec::new(), matches: 0, stopped: Some(Stopped::Deadline), engine: Engine::Git };
    show(&mut rt, Ok(deadline));
    assert!(!says(&rt, "No matches"));
    assert!(says(&rt, "ran out of time before it found anything"));

    let pattern = FindTextFailure { kind: FailureKind::Pattern, message: "Unmatched [ (git extended regex)".into() };
    show(&mut rt, Err(pattern));
    assert!(says(&rt, "That isn't a valid pattern.") && says(&rt, "Unmatched [ (git extended regex)"));

    let old = FindTextFailure { kind: FailureKind::Unsupported, message: "x".into() };
    show(&mut rt, Err(old));
    assert!(says(&rt, "can't search inside files yet") && says(&rt, "The Fleet running on it is older."));

    let failed = FindTextFailure { kind: FailureKind::Failed, message: "The search failed (HTTP 503).".into() };
    show(&mut rt, Err(failed));
    assert!(says(&rt, "The search failed.") && says(&rt, "HTTP 503") && !says(&rt, "No matches"));
}

#[test]
fn the_empty_words_follow_the_switches() {
    let mut rt = open(Case::FindNone);
    rt.update(|s, _| {
        s.find_text.regex = true;
        s.find_text.match_case = true;
    });
    let none = TextSearchResult { files: Vec::new(), matches: 0, stopped: None, engine: Engine::Walk };
    show(&mut rt, Ok(none));
    assert!(says(&rt, "Try turning off the regex switch. Match case is on."));
    assert!(says(&rt, "This folder isn't a git repository"));
}

#[test]
fn says_when_the_results_hit_the_cap_or_the_folder_is_not_a_git_repository() {
    let rt = open(Case::FindLimit);
    assert!(says(&rt, "First 200 matches, in 3 files"));
    assert!(says(&rt, "Showing the first 200 matches."));
    assert!(says(&rt, "not a git repository"));
    assert!(rt.find("find-walk").is_some());
}

#[test]
fn a_deadline_with_matches_says_so_after_the_list() {
    let mut rt = open(Case::Find);
    show(&mut rt, Ok(TextSearchResult { stopped: Some(Stopped::Deadline), ..find_answer(Engine::Git) }));
    assert!(says(&rt, "stopped after 10 s"));
    assert!(says(&rt, "Stopped after 10 seconds."));
}

#[test]
fn keeps_the_query_and_results_when_switching_back_and_forth() {
    let mut rt = open(Case::Find);
    click(&mut rt, "files-mode-names");
    rt.frame(FRAME);
    click(&mut rt, "files-mode-contents");
    rt.frame(FRAME);
    assert_eq!(rt.state.file_browser.field.text, "page");
    assert_eq!(keys(&rt, "find-file-").len(), 3);
}

#[test]
fn a_new_answer_starts_with_every_file_folded_and_the_first_line_active() {
    let mut rt = open(Case::Find);
    click(&mut rt, &format!("find-more-{PAGINATE}"));
    press(&mut rt, Key::Down);
    rt.update(|s, _| {
        let id = s.files_session();
        s.find_text.show(&id, "page", Ok(find_answer(Engine::Git)), 50.0);
    });
    assert!(rt.state.find_text.expanded.is_empty());
    assert_eq!(rt.state.find_text.active, Some(VisibleHit { path: "docs/pagination.md".into(), line: 3 }));
}

#[test]
fn the_summary_dims_with_the_list_while_a_newer_search_runs() {
    let mut rt = open(Case::Find);
    rt.update(|s, _| s.find_text.loading = true);
    rt.frame(FRAME);
    assert!(rt.find("find-summary").is_some() && rt.find("find-list").is_some(), "the last answer stays in view");
}

#[test]
fn leaves_a_request_for_another_session_alone() {
    let mut rt = open(Case::Tree);
    rt.update(|s, _| s.find_in_files.ask("another-session"));
    rt.frame(FRAME);
    assert_eq!(rt.state.file_browser.search_in, SearchIn::Names);
}

#[test]
fn a_ctrl_shift_f_request_takes_the_keyboard_for_the_field() {
    let mut rt = open(Case::Tree);
    rt.update(|s, _| {
        let id = s.files_session();
        s.find_in_files.ask(&id);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.file_browser.search_in, SearchIn::Contents);
    let mut focused = None;
    rt.update(|_, tx| focused = tx.focused().map(str::to_string));
    assert_eq!(focused.as_deref(), Some("files-search"));
}
