//! Go to file (component 11, piece L): the 5 cases of `GoToFileDialog.test.ts` under their names, and the keys, the wrap, the click
//! and the shortcut, on the invented project the scenes draw. The search's answer is set the way a scene sets it, so nothing here
//! waits for a thread.
use crate::fleet::finder::{FileSearch, Scope, Step};
use crate::mock::go_to_file::Case;
use crate::tests::{FRAME, app, click};
use rune::{Key, KeyEvent, Mods, Runtime};

fn open(case: Case) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_go_to_file(case, true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn rows(rt: &Runtime<crate::State>) -> Vec<String> {
    rt.elements().iter().filter_map(|e| e.key.as_deref()?.strip_prefix("go-to-file-item-").map(str::to_string)).collect()
}

fn says(rt: &Runtime<crate::State>, words: &str) -> bool {
    rt.elements().iter().any(|e| e.text.as_deref().is_some_and(|t| t.contains(words)))
}

fn press(rt: &mut Runtime<crate::State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

#[test]
fn lists_the_files_already_open_when_nothing_is_typed() {
    let rt = open(Case::Open);
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("Open")));
    assert_eq!(rows(&rt), ["README.md", "src/ledger/paginate.py", "src/ledger/totals.py"]);
    assert!(rt.state.go_to_items().iter().all(|p| rt.find(&format!("go-to-file-item-{p}")).is_some()));
}

#[test]
fn filters_to_files_as_you_type_leaving_out_folders() {
    let mut search = FileSearch::default();
    let scope = Scope::Session("s1".into());
    search.want(Some(&scope), Some("auth"), 0.0);
    let Step::Send { generation, query } = search.want(Some(&scope), Some("auth"), 1.0) else { panic!("the search wasn't sent") };
    search.answered(generation, &query, Ok(["src/middleware/auth.ts", "docs/auth.md", "src/components/"].map(String::from).to_vec()));
    let mut dialog = crate::fleet::go_to_file::GoToDialog::default();
    dialog.search = search;
    dialog.query = rune::TextEdit::new("auth");
    assert_eq!(dialog.items(Vec::new()), ["src/middleware/auth.ts", "docs/auth.md"]);
    // The invented project's answer, drawn.
    let rt = open(Case::Typed);
    assert!(!rt.elements().iter().any(|e| e.text.as_deref() == Some("Open")), "the group label is for the open files");
    assert!(rows(&rt).iter().all(|p| !p.ends_with('/')) && rows(&rt).iter().any(|p| p.ends_with("paginate.py")));
}

#[test]
fn says_the_search_failed_instead_of_saying_no_file_matches() {
    let rt = open(Case::Failed);
    assert!(says(&rt, "Couldn't search for files."));
    assert!(says(&rt, "HTTP 500"));
    assert!(!says(&rt, "No files match."));
}

#[test]
fn enter_opens_the_chosen_file_as_a_kept_tab_and_closes() {
    let mut rt = open(Case::Typed);
    rt.update(|s, _| s.set_content_open(false));
    let listed = rt.state.go_to_items();
    click(&mut rt, "go-to-file-input");
    press(&mut rt, Key::Down);
    press(&mut rt, Key::Enter);
    let chosen = &listed[1];
    let tabs = rt.state.open_canvases();
    assert_eq!(tabs.active_id, format!("file:{chosen}"));
    let file = tabs.canvases.iter().find_map(|c| c.file.as_ref().filter(|f| f.path == *chosen)).expect("the file's tab");
    assert!(!file.preview, "a kept tab");
    assert_eq!(rt.state.ui.open, None);
    assert_eq!(rt.state.go_to_file.session, None);
    let session = rt.state.canvas_session();
    assert_eq!(rt.state.go_to_file.focus_on_open.as_ref().map(|f| (f.session.clone(), f.path.clone())), Some((session, chosen.clone())));
    assert!(rt.state.content_open, "the right panel shows");
    assert!(rt.find("go-to-file").is_none());
}

#[test]
fn says_so_when_nothing_matches() {
    let rt = open(Case::None);
    assert!(says(&rt, "No files match."));
    assert!(rows(&rt).is_empty());
}

#[test]
fn says_what_to_do_when_nothing_is_open_and_nothing_is_typed() {
    let mut rt = app();
    rt.update(|s, _| {
        s.go_to_file.show("s1");
        s.ui.open = Some(crate::ui::Open::GoToFile);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(says(&rt, "Type to find a file in this session."));
    assert!(rt.find("go-to-file-input").is_some());
}

#[test]
fn the_arrows_move_the_active_row_and_wrap_at_both_ends() {
    let mut rt = open(Case::Open);
    click(&mut rt, "go-to-file-input");
    assert_eq!(rt.state.go_to.active, 0, "it opens with the first row active");
    press(&mut rt, Key::Up);
    assert_eq!(rt.state.go_to.active, 2, "up from the first wraps to the last");
    press(&mut rt, Key::Down);
    assert_eq!(rt.state.go_to.active, 0, "down from the last wraps to the first");
    press(&mut rt, Key::Down);
    assert_eq!(rt.state.go_to.active, 1);
}

#[test]
fn escape_closes_it_and_a_click_on_a_row_opens_that_file() {
    let mut rt = open(Case::Open);
    press(&mut rt, Key::Escape);
    assert_eq!(rt.state.ui.open, None);
    assert_eq!(rt.state.go_to_file.session, None);
    let mut rt = open(Case::Open);
    click(&mut rt, "go-to-file-item-src/ledger/totals.py");
    assert_eq!(rt.state.open_canvases().active_id, "file:src/ledger/totals.py");
    assert_eq!(rt.state.ui.open, None);
}

#[test]
fn the_pointer_over_a_row_makes_it_active() {
    let mut rt = open(Case::Open);
    let (x, y) = rt.find("go-to-file-item-src/ledger/paginate.py").expect("a row");
    rt.pointer_move(x, y);
    rt.frame(FRAME);
    assert_eq!(rt.state.go_to.active, 1);
}

#[test]
fn a_new_list_puts_the_active_row_back_on_the_first() {
    let mut rt = open(Case::Open);
    rt.update(|s, _| s.go_to.active = 2);
    rt.update(|s, _| s.go_to.query = rune::TextEdit::new("zzq"));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.go_to.active, 0);
}

#[test]
fn the_command_and_its_shortcut_open_it_for_the_open_session() {
    let mut rt = app();
    assert!(crate::ui::commands::find(&rt.state, "go-to-file").is_some_and(|c| c.description == "Open a file from this session by name."));
    rt.key(KeyEvent { key: Key::Command('p'), mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.ui.open, Some(crate::ui::Open::GoToFile));
    assert!(rt.state.go_to_file.session.is_some());
    assert!(rt.find("go-to-file-input").is_some());
    assert!(rt.state.go_to.query.text.is_empty());
}

fn file_open() -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| {
        s.sheet_w = 1400.0;
        s.show_file(crate::mock::file_tab::Case::Code, true)
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

#[test]
fn a_file_opened_at_a_line_takes_the_keyboard_and_marks_the_line() {
    let mut rt = file_open();
    let session = rt.state.canvas_session();
    let path = crate::mock::file_tab::PATH;
    rt.update(|s, _| s.go_to_file.ask_focus(&session, path, Some(3)));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.state.go_to_file.focus_on_open.is_none(), "taken once");
    assert_eq!(rt.state.go_to_file.shown_line.as_ref().map(|l| l.2), Some(3));
    let mut focused = None;
    rt.update(|_, tx| focused = tx.focused().map(str::to_string));
    assert_eq!(focused.as_deref(), Some("file-code"));
}

#[test]
fn a_line_past_the_end_goes_to_the_last_line() {
    let mut rt = file_open();
    let session = rt.state.canvas_session();
    let count = rt
        .state
        .file_buffers
        .buffer(rt.state.machine_key_of(&session), &session, crate::mock::file_tab::PATH)
        .map_or(0, |b| b.lines().len());
    rt.update(|s, _| s.go_to_file.ask_focus(&session, crate::mock::file_tab::PATH, Some(9999)));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.go_to_file.shown_line.as_ref().map(|l| l.2 as usize), Some(count));
}

#[test]
fn a_request_for_another_file_waits_and_is_not_taken() {
    let mut rt = file_open();
    let session = rt.state.canvas_session();
    rt.update(|s, _| s.go_to_file.ask_focus(&session, "src/other.py", None));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.state.go_to_file.focus_on_open.is_some());
    assert!(rt.state.go_to_file.shown_line.is_none());
}
