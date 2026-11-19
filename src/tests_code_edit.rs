//! The editor spike (component 11, piece P): the code view of a file tab edited from the keyboard, the pointer, a paste and an input
//! method, on the invented `src/paginate.py` and a generated file of twenty thousand lines. Fleet has no test of its own for this (it
//! is CodeMirror's), so these are ours.
use crate::State;
use crate::fleet::code_edit::Pos;
use crate::mock::code_edit::{BIG_LINES, BIG_PATH};
use crate::mock::file_tab::{Case, PATH};
use crate::tests::FRAME;
use crate::view::code_view::CODE;
use rune::{App, Env, Ime, Key, KeyEvent, Mods, Runtime};

/// The app as `main` builds it (with the input method's hook), the file tab open on `src/paginate.py`, the spike on or off.
fn open(on: bool) -> Runtime<State> {
    let mut rt = crate::terminal_hooks(App::new(crate::spec())).headless_runtime((1280.0, 760.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(FRAME);
    rt.update(|s, _| {
        s.sheet_w = 1400.0;
        s.show_file(Case::Code, true);
        s.code_edit.on = on;
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt.update(|_, tx| tx.focus(CODE, false));
    rt.frame(FRAME);
    rt
}

fn lines(rt: &Runtime<State>, path: &str) -> Vec<String> {
    let s = &rt.state;
    let session = s.canvas_session();
    let machine = s.machine_key_of(&session);
    s.file_buffers.buffer(machine, &session, path).map(|b| b.lines().to_vec()).unwrap_or_default()
}

fn press(rt: &mut Runtime<State>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(FRAME);
}

fn typed(rt: &mut Runtime<State>, text: &str) {
    for c in text.chars() {
        press(rt, Key::Text(c.to_string()), Mods::default());
    }
}

fn ctrl() -> Mods {
    if cfg!(target_os = "macos") { Mods { cmd: true, ..Mods::default() } } else { Mods { cmd: true, ctrl: true, ..Mods::default() } }
}

fn caret(rt: &Runtime<State>) -> Pos {
    rt.state.code_edit.editor.caret
}

#[test]
fn typing_in_the_code_view_edits_the_file_and_undo_and_redo_take_it_back() {
    let mut rt = open(true);
    let before = lines(&rt, PATH);
    press(&mut rt, Key::Down, Mods::default());
    press(&mut rt, Key::End, Mods::default());
    typed(&mut rt, "  # ok");
    let after = lines(&rt, PATH);
    assert_eq!(after[1], "  # ok", "typed at the end of line 2");
    assert_eq!(caret(&rt), Pos::new(1, 6));
    assert!(rt.find("code-caret").is_some(), "the caret is drawn");
    press(&mut rt, Key::Command('z'), ctrl());
    assert_eq!(lines(&rt, PATH), before, "the typing undoes as one step");
    press(&mut rt, Key::Command('z'), Mods { shift: true, ..ctrl() });
    assert_eq!(lines(&rt, PATH), after);
}

#[test]
fn nothing_is_typed_while_the_spike_is_off() {
    let mut rt = open(false);
    let before = lines(&rt, PATH);
    typed(&mut rt, "xyz");
    assert_eq!(lines(&rt, PATH), before);
    assert!(rt.find("code-caret").is_none());
}

#[test]
fn enter_keeps_the_indent_and_backspace_joins_the_lines_again() {
    let mut rt = open(true);
    let before = lines(&rt, PATH);
    // Line 7 is `    start = 0 * size` (after the docstring of `page_1`).
    for _ in 0..7 {
        press(&mut rt, Key::Down, Mods::default());
    }
    press(&mut rt, Key::End, Mods::default());
    press(&mut rt, Key::Enter, Mods::default());
    let after = lines(&rt, PATH);
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(after[8], "    ");
    // Backspace takes the indent, then the line break.
    for _ in 0..5 {
        press(&mut rt, Key::Backspace, Mods::default());
    }
    assert_eq!(lines(&rt, PATH), before);
}

#[test]
fn shift_selects_and_a_cut_and_a_paste_move_the_text() {
    let mut rt = open(true);
    // `DEFAULT_SIZE = 20` on line 3: select its first word with Shift+Ctrl+Right (Option on a Mac) and cut it.
    press(&mut rt, Key::Down, Mods::default());
    press(&mut rt, Key::Down, Mods::default());
    let word = if cfg!(target_os = "macos") { Mods { alt: true, shift: true, ..Mods::default() } } else { Mods { shift: true, ..ctrl() } };
    press(&mut rt, Key::Right, word);
    assert_eq!(rt.state.code_edit.editor.selected_text(&lines(&rt, PATH)), "DEFAULT_SIZE");
    press(&mut rt, Key::Command('x'), ctrl());
    assert_eq!(lines(&rt, PATH)[2], " = 20");
    // What Ctrl+V reads from the clipboard (a headless run has none to read) goes in where a paste goes.
    rt.update(|s, tx| s.code_edit_paste("PAGE_SIZE", tx));
    rt.frame(FRAME);
    assert_eq!(lines(&rt, PATH)[2], "PAGE_SIZE = 20");
}

#[test]
fn a_press_on_the_code_puts_the_caret_there_and_a_drag_selects() {
    let mut rt = open(true);
    let (x, y) = rt.find("code-text-3").expect("line 3 is drawn");
    // The text's centre is in the middle of `DEFAULT_SIZE = 20`; the press lands on a character boundary near it.
    rt.pointer_move(x, y);
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(FRAME);
    let at = caret(&rt);
    assert_eq!(at.line, 2);
    assert!((7..=11).contains(&at.byte), "near the middle of the line, got {at:?}");
    // A drag from there down two lines selects across them.
    rt.pointer_down(x, y);
    rt.pointer_move(x, y + 40.0);
    rt.frame(FRAME);
    rt.pointer_up(x, y + 40.0);
    rt.frame(FRAME);
    assert_eq!(caret(&rt).line, 4);
    assert!(rt.state.code_edit.editor.has_selection());
}

#[test]
fn an_input_method_composes_at_the_caret_and_its_commit_is_typed() {
    let mut rt = open(true);
    press(&mut rt, Key::End, Mods::default());
    rt.ime(Ime::Preedit(" ペ".into(), None, Vec::new()));
    rt.frame(FRAME);
    assert_eq!(rt.state.code_edit.editor.preedit.as_ref().map(|p| p.0.as_str()), Some(" ペ"));
    assert!(!lines(&rt, PATH)[0].contains('ペ'), "the composition isn't in the text yet");
    rt.ime(Ime::Commit(" ページ".into()));
    rt.frame(FRAME);
    assert!(lines(&rt, PATH)[0].ends_with(" ページ"));
    assert!(rt.state.code_edit.editor.preedit.is_none());
    press(&mut rt, Key::Command('z'), ctrl());
    assert!(!lines(&rt, PATH)[0].contains('ペ'), "the commit undoes like typing");
}

#[test]
fn the_end_of_a_twenty_thousand_line_file_comes_into_view_and_takes_typing() {
    let mut rt = open(true);
    rt.update(|s, _| s.show_editor_spike(crate::mock::code_edit::Spike::Big, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    let count = lines(&rt, BIG_PATH).len();
    assert_eq!(count, BIG_LINES + 1, "the text ends with a line break, so the last line is empty");
    press(&mut rt, Key::End, ctrl());
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    assert_eq!(caret(&rt).line, count - 1);
    assert!(rt.find(&format!("code-line-{count}")).is_some(), "the last line is built");
    assert!(rt.find("code-line-100").is_none(), "the top of the file isn't");
    typed(&mut rt, "# end");
    assert_eq!(lines(&rt, BIG_PATH)[count - 1], "# end");
    // Ctrl+Home goes back to the top.
    press(&mut rt, Key::Home, ctrl());
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    assert!(rt.find("code-line-1").is_some());
}

#[test]
fn page_down_moves_the_caret_by_a_screen() {
    let mut rt = open(true);
    rt.update(|s, _| s.show_editor_spike(crate::mock::code_edit::Spike::Big, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    press(&mut rt, Key::Home, ctrl());
    let h = rt.state.code_edit.view_h;
    assert!(h > 200.0, "the view measured itself, {h}");
    press(&mut rt, Key::PageDown, Mods::default());
    assert_eq!(caret(&rt).line, (h / 20.0) as usize - 1);
}

#[test]
fn the_text_input_way_takes_typing_too() {
    let mut rt = open(true);
    rt.update(|s, _| s.show_editor_spike(crate::mock::code_edit::Spike::Input, true));
    for _ in 0..3 {
        rt.frame(FRAME);
    }
    typed(&mut rt, "page 1");
    let text = rt.state.code_edit.input.as_ref().map(|(_, e)| e.text.clone()).unwrap_or_default();
    assert!(text.contains("# the first page starts at page 1"), "the selection was replaced");
}
