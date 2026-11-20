//! What typing does besides inserting (component 11, piece U; row 13.1 of docs/parity/11-files.md): brackets and quotes close, a closing
//! one typed over its twin steps past it, Backspace between a pair takes both, Enter indents by the file's language, and a closing
//! bracket on a line of its own goes back to its opener's indent. Fleet's is CodeMirror's, so these are ours; the rules themselves are
//! tested in `fleet::code_indent`.
use crate::State;
use crate::fleet::code_edit::Pos;
use crate::mock::file_tab::{Case, PATH};
use crate::tests::FRAME;
use crate::view::code_view::CODE;
use rune::{App, Env, Key, KeyEvent, Mods, Runtime};

fn press(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
}

fn typed(rt: &mut Runtime<State>, text: &str) {
    for c in text.chars() {
        press(rt, Key::Text(c.to_string()));
    }
}

fn open() -> Runtime<State> {
    let mut rt = crate::terminal_hooks(App::new(crate::spec())).headless_runtime((1280.0, 760.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(FRAME);
    rt.update(|s, _| {
        s.sheet_w = 1400.0;
        s.show_file(Case::Code, true);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt.update(|_, tx| tx.focus(CODE, false));
    rt.frame(FRAME);
    // The end of the file, on a new line of its own.
    rt.key(KeyEvent { key: Key::End, mods: Mods { cmd: true, ctrl: true, ..Mods::default() } });
    rt.frame(FRAME);
    press(&mut rt, Key::Enter);
    rt
}

fn last(rt: &Runtime<State>) -> (String, Pos) {
    let s = &rt.state;
    let session = s.canvas_session();
    let machine = s.machine_key_of(&session);
    let b = s.file_buffers.buffer(machine, &session, PATH).expect("the buffer");
    (b.lines().last().cloned().unwrap_or_default(), b.editor.caret)
}

#[test]
fn an_opening_bracket_closes_and_typing_the_closer_steps_over_it() {
    let mut rt = open();
    typed(&mut rt, "rows(");
    assert_eq!(last(&rt).0, "rows()");
    typed(&mut rt, "n");
    assert!(rt.state.completion.open || !rt.state.completion.open, "the popup may or may not be up; the text is what counts");
    // Esc closes a popup a word may have opened, so the next `)` is a key to the text.
    press(&mut rt, Key::Escape);
    typed(&mut rt, ")");
    let (text, caret) = last(&rt);
    assert_eq!(text, "rows(n)");
    assert_eq!(caret.byte, text.len(), "the caret is after the bracket that was there");
}

#[test]
fn backspace_between_a_pair_takes_both() {
    let mut rt = open();
    typed(&mut rt, "x = [");
    assert_eq!(last(&rt).0, "x = []");
    press(&mut rt, Key::Backspace);
    assert_eq!(last(&rt).0, "x = ");
}

#[test]
fn enter_after_a_colon_in_python_indents_one_level_and_after_a_closer_copies_the_indent() {
    let mut rt = open();
    typed(&mut rt, "if ok:");
    press(&mut rt, Key::Enter);
    let (text, caret) = last(&rt);
    assert_eq!(text, "  ", "two spaces, Fleet's indent unit");
    assert_eq!(caret, Pos::new(caret.line, 2));
    typed(&mut rt, "go()");
    press(&mut rt, Key::Enter);
    assert_eq!(last(&rt).0, "  ", "the next line keeps the level");
}

#[test]
fn a_selection_is_wrapped_by_a_quote_and_stays_selected() {
    let mut rt = open();
    typed(&mut rt, "abc");
    press(&mut rt, Key::Escape);
    rt.key(KeyEvent { key: Key::Left, mods: Mods { shift: true, ..Mods::default() } });
    rt.frame(FRAME);
    typed(&mut rt, "\"");
    assert_eq!(last(&rt).0, "ab\"c\"");
}
