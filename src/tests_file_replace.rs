//! The replace row of the find panel (component 11, piece U; row 12.3 of docs/parity/11-files.md): the replace field, **replace** and
//! **replace all**, over the invented `src/paginate.py`. Fleet's panel is CodeMirror's, so these are ours.
use crate::State;
use crate::conv::file_find::Options;
use crate::mock::file_tab::{Case, PATH};
use crate::tests::FRAME;
use crate::view::code_view::CODE;
use rune::{App, Env, Key, KeyEvent, Mods, Runtime};

fn open() -> Runtime<State> {
    let mut rt = crate::terminal_hooks(App::new(crate::spec())).headless_runtime((1280.0, 760.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(FRAME);
    rt.update(|s, _| {
        s.sheet_w = 1400.0;
        s.show_file(Case::Code, true);
        s.show_file_find("page_", Options { case: true, ..Default::default() }, 0);
        s.file_find.replace = rune::TextEdit::new("rows_");
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

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

fn lines(rt: &Runtime<State>) -> Vec<String> {
    let s = &rt.state;
    let session = s.canvas_session();
    let machine = s.machine_key_of(&session);
    s.file_buffers.buffer(machine, &session, PATH).map(|b| b.lines().to_vec()).unwrap_or_default()
}

#[test]
fn the_panel_of_an_editable_file_has_the_replace_row() {
    let rt = open();
    for key in ["file-find-replace-field", "file-find-replace", "file-find-replace-all"] {
        assert!(rt.find(key).is_some(), "{key}");
    }
}

#[test]
fn replace_takes_the_current_match_and_makes_the_next_one_current() {
    let mut rt = open();
    let before = lines(&rt);
    let total = rt.state.file_find.matches().len();
    assert!(total >= 3, "page_ is in the file several times: {total}");
    click(&mut rt, "file-find-next");
    assert_eq!(rt.state.file_find.current(), Some(0));
    click(&mut rt, "file-find-replace");
    let after = lines(&rt);
    let changed: Vec<usize> = (0..after.len()).filter(|&i| after[i] != before[i]).collect();
    assert_eq!(changed.len(), 1, "one line changed");
    assert!(after[changed[0]].contains("rows_") && !after[changed[0]].contains("rows_page_"));
    assert_eq!(rt.state.file_find.matches().len(), total - 1);
    assert_eq!(rt.state.file_find.current(), Some(0), "the next match is the one that is now first");
}

#[test]
fn the_first_replace_with_nothing_current_only_picks_the_first_match() {
    let mut rt = open();
    let before = lines(&rt);
    click(&mut rt, "file-find-replace");
    assert_eq!(lines(&rt), before);
    assert_eq!(rt.state.file_find.current(), Some(0));
}

#[test]
fn replace_all_replaces_every_match_and_one_undo_takes_them_all_back() {
    let mut rt = open();
    let before = lines(&rt);
    click(&mut rt, "file-find-replace-all");
    let after = lines(&rt);
    assert!(rt.state.file_find.matches().is_empty(), "nothing is left to find");
    assert!(!after.iter().any(|l| l.contains("page_")));
    assert!(after.iter().any(|l| l.contains("rows_1")), "page_1 became rows_1: {after:?}");
    assert_eq!(after.len(), before.len());
    rt.update(|_, tx| tx.focus(CODE, false));
    rt.frame(FRAME);
    rt.key(KeyEvent { key: Key::Command('z'), mods: Mods { cmd: true, ctrl: true, ..Mods::default() } });
    rt.frame(FRAME);
    assert_eq!(lines(&rt), before, "one undo");
}
