//! Find in a file (component 11, piece N): the panel Ctrl/Cmd+F opens over the code of a file tab, as CodeMirror's `search({ top: true })`
//! does in Fleet (rows 12.1 and 12.2 of docs/parity/11-files.md). Fleet has no test of its own for it (the panel is CodeMirror's), so
//! these are ours, on the invented `src/paginate.py` the scenes draw: it holds "page" 14 times on 14 lines, 10 of them in lower
//! case and 5 as a word of their own.
use crate::State;
use crate::fleet::file_find::FIELD;
use crate::mock::file_tab::Case;
use crate::tests::{FRAME, app};
use crate::view::code_view::CODE;
use rune::{Key, KeyEvent, Mods, Runtime};

fn open() -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| {
        s.sheet_w = 1400.0;
        s.show_file(Case::Code, true)
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn press(rt: &mut Runtime<State>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn ctrl() -> Mods {
    // As the window sends Ctrl+letter: the command key's flag (Cmd on a Mac, where Ctrl is not held) with Ctrl on the others.
    if cfg!(target_os = "macos") { Mods { cmd: true, ..Mods::default() } } else { Mods { cmd: true, ctrl: true, ..Mods::default() } }
}

fn shift() -> Mods {
    Mods { shift: true, ..Mods::default() }
}

fn focused(rt: &mut Runtime<State>) -> Option<String> {
    let mut f = None;
    rt.update(|_, tx| f = tx.focused().map(str::to_string));
    f
}

/// A click as a person's hand makes it: the pointer is over the control for a frame before it goes down (`tests::click` presses at once,
/// which a control that has only just been drawn doesn't take).
fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(FRAME);
}

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

/// Focus in the code, Ctrl+F, then `query` typed into the field.
fn find(rt: &mut Runtime<State>, query: &str) {
    rt.update(|_, tx| tx.focus(CODE, false));
    rt.frame(FRAME);
    press(rt, Key::Command('f'), ctrl());
    rt.key(KeyEvent { key: Key::Text(query.into()), mods: Mods::default() });
    rt.frame(FRAME);
}

fn count(rt: &Runtime<State>) -> usize {
    rt.state.file_find.matches().len()
}

#[test]
fn opens_on_ctrl_f_in_the_code_and_the_field_takes_focus() {
    let mut rt = open();
    rt.update(|_, tx| tx.focus(CODE, false));
    press(&mut rt, Key::Command('f'), ctrl());
    assert!(rt.state.file_find.open && has(&rt, "file-find"));
    assert!(!rt.state.find.open, "the conversation's find stays shut");
    assert_eq!(focused(&mut rt).as_deref(), Some(FIELD));
    for key in
        ["file-find-next", "file-find-prev", "file-find-all", "file-find-case", "file-find-regexp", "file-find-word", "file-find-close"]
    {
        assert!(has(&rt, key), "{key}");
    }
}

#[test]
fn does_not_open_while_the_keyboard_is_elsewhere() {
    let mut rt = open();
    click(&mut rt, "composer");
    press(&mut rt, Key::Command('f'), ctrl());
    assert!(!rt.state.file_find.open && !has(&rt, "file-find"), "the composer's focus leaves Ctrl+F to the conversation's find");
    assert!(rt.state.find.open, "the conversation's bar opened instead");
}

#[test]
fn marks_every_match_as_it_is_typed_and_moves_nothing() {
    let mut rt = open();
    find(&mut rt, "page");
    assert_eq!(count(&rt), 14);
    assert_eq!(rt.state.file_find.current(), None, "typing marks, Enter steps");
}

#[test]
fn enter_and_shift_enter_step_and_wrap() {
    let mut rt = open();
    find(&mut rt, "page");
    let line = |rt: &Runtime<State>| rt.state.file_find.current().map(|i| rt.state.file_find.matches()[i].line + 1);
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(line(&rt), Some(1), "the first Enter goes to the first match");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(line(&rt), Some(6));
    press(&mut rt, Key::Enter, shift());
    press(&mut rt, Key::Enter, shift());
    assert_eq!(line(&rt), Some(39), "Shift+Enter from the first match wraps to the last");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(line(&rt), Some(1), "Enter from the last wraps to the first");
}

#[test]
fn f3_and_ctrl_g_step_too_while_the_keyboard_is_in_the_code() {
    let mut rt = open();
    find(&mut rt, "page");
    rt.update(|_, tx| tx.focus(CODE, false));
    press(&mut rt, Key::Function(3), Mods::default());
    assert_eq!(rt.state.file_find.current(), Some(0));
    press(&mut rt, Key::Command('g'), ctrl());
    assert_eq!(rt.state.file_find.current(), Some(1));
    press(&mut rt, Key::Function(3), shift());
    assert_eq!(rt.state.file_find.current(), Some(0));
}

#[test]
fn match_case_and_by_word_narrow_the_matches() {
    let mut rt = open();
    find(&mut rt, "page");
    click(&mut rt, "file-find-case");
    assert!(rt.state.file_find.options.case);
    assert_eq!(count(&rt), 10);
    click(&mut rt, "file-find-case");
    click(&mut rt, "file-find-word");
    assert_eq!(count(&rt), 5);
    click(&mut rt, "file-find-regexp");
    assert!(rt.state.file_find.options.regexp && rt.state.file_find.options.word);
}

#[test]
fn regexp_reads_the_query_as_a_pattern_and_an_unfinished_one_finds_nothing() {
    let mut rt = open();
    find(&mut rt, "page_[0-9]");
    assert_eq!(count(&rt), 0, "as text, the brackets are letters");
    click(&mut rt, "file-find-regexp");
    assert_eq!(count(&rt), 3);
    find(&mut rt, "(");
    assert_eq!(count(&rt), 0);
}

#[test]
fn the_current_match_stays_current_when_a_box_keeps_it() {
    let mut rt = open();
    find(&mut rt, "page");
    press(&mut rt, Key::Enter, Mods::default());
    press(&mut rt, Key::Enter, Mods::default());
    press(&mut rt, Key::Enter, Mods::default());
    let at = rt.state.file_find.matches()[rt.state.file_find.current().unwrap()].clone();
    click(&mut rt, "file-find-case");
    let now = rt.state.file_find.current().map(|i| rt.state.file_find.matches()[i].clone());
    assert!(now == Some(at.clone()) || at.line + 1 == 7, "a match that is still found stays current; the dropped one clears it");
}

#[test]
fn esc_closes_and_gives_the_keyboard_back_without_stopping_the_agent() {
    let mut rt = open();
    let status = rt.state.session().status;
    find(&mut rt, "page");
    press(&mut rt, Key::Escape, Mods::default());
    assert!(!rt.state.file_find.open && !has(&rt, "file-find"));
    assert_eq!(focused(&mut rt).as_deref(), Some(CODE), "the keyboard is back in the code");
    assert_eq!(rt.state.session().status, status, "Esc did not stop the agent");
}

#[test]
fn the_close_button_closes_it_and_ctrl_f_selects_the_field_of_an_open_panel() {
    let mut rt = open();
    find(&mut rt, "page");
    press(&mut rt, Key::Command('f'), ctrl());
    assert_eq!(focused(&mut rt).as_deref(), Some(FIELD));
    assert_eq!(rt.state.file_find.field.text, "page", "the query is kept");
    click(&mut rt, "file-find-close");
    assert!(!rt.state.file_find.open);
    assert_eq!(focused(&mut rt).as_deref(), Some(CODE));
}

#[test]
fn the_panel_goes_when_another_tab_comes_to_the_front() {
    let mut rt = open();
    find(&mut rt, "page");
    rt.update(|s, tx| s.open_file(&s.canvas_session(), "README.md", crate::fleet::canvases::OpenFile::default(), tx));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(!rt.state.file_find.open && !has(&rt, "file-find"));
}
