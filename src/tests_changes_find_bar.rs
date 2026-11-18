//! Find in Changes, the bar, the marks and the keys (component 12, piece K): `ChangesReview.test.ts` "find in changes", the cases that
//! are about what a person presses and sees. The logic under them is piece J's (`tests_changes_find.rs`, same fake Fleet).
use crate::State;
use crate::fleet::changes_find::{FIELD, MARK};
use crate::tests::{FRAME, click};
use crate::tests_changes_find::{Rt, Search, found, one, scene};
use rune::{Key, KeyEvent, Mods};

fn press(rt: &mut Rt, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(FRAME);
}

fn ctrl() -> Mods {
    // As the window sends Ctrl+letter: the command key's flag (Cmd on a Mac, where Ctrl is not held) with Ctrl on the others.
    if cfg!(target_os = "macos") { Mods { cmd: true, ..Mods::default() } } else { Mods { cmd: true, ctrl: true, ..Mods::default() } }
}

fn shift() -> Mods {
    Mods { shift: true, ..Mods::default() }
}

fn focused(rt: &mut Rt) -> Option<String> {
    let mut f = None;
    rt.update(|_, tx| f = tx.focused().map(str::to_string));
    f
}

fn has(rt: &Rt, key: &str) -> bool {
    rt.find(key).is_some()
}

/// Ctrl+F with focus in the diff, then the query typed into the field.
fn open_with(rt: &mut Rt, query: &str) {
    click(rt, "review-scroll");
    press(rt, Key::Command('f'), ctrl());
    rt.key(KeyEvent { key: Key::Text(query.into()), mods: Mods::default() });
    rt.frame(FRAME);
}

fn consts() -> Vec<crate::tests_changes_find::Changed> {
    let lines: String = (0..6).map(|i| format!("const v{i} = {i};\n")).collect();
    vec![one("notes.ts", "const f = 1;\n", &lines)]
}

// ChangesReview.test.ts: "opens on Ctrl+F inside Changes, keeps the browser's find out, and closes on Esc without reaching the app"
#[test]
fn opens_on_ctrl_f_inside_changes_keeps_the_conversations_find_out_and_closes_on_esc_without_reaching_the_app() {
    let (mut rt, _queue, _asked) = scene(consts(), Search::Missing);
    let status = rt.state.session().status;
    click(&mut rt, "review-scroll");
    press(&mut rt, Key::Command('f'), ctrl());
    assert!(rt.state.changes_find.open && has(&rt, "changes-find"));
    assert!(!rt.state.find.open, "the conversation's bar stays shut");
    assert_eq!(focused(&mut rt).as_deref(), Some(FIELD), "the field takes focus");

    press(&mut rt, Key::Escape, Mods::default());
    assert!(!rt.state.changes_find.open && !has(&rt, "changes-find"));
    assert_eq!(focused(&mut rt).as_deref(), Some("review-scroll"), "focus goes back to Changes");
    assert_eq!(rt.state.session().status, status, "Esc did not stop the agent");
}

// 11.3: with focus anywhere else the key is not Changes'
#[test]
fn ctrl_f_with_focus_outside_changes_does_not_open_the_bar() {
    let (mut rt, _queue, _asked) = scene(consts(), Search::Missing);
    rt.update(|_, tx| tx.focus("composer", false));
    press(&mut rt, Key::Command('f'), ctrl());
    assert!(!rt.state.changes_find.open);
}

// ChangesReview.test.ts: "counts matches across files, steps with Enter and Shift+Enter, and wraps"
#[test]
fn counts_matches_across_files_steps_with_enter_and_shift_enter_and_wraps() {
    let (mut rt, queue, _asked) = scene(consts(), Search::Missing);
    open_with(&mut rt, "Const");
    found(&mut rt, &queue, "seven matches", 7);
    rt.frame(FRAME);
    assert_eq!(rt.state.changes_find_count_label(), "1 of 7");
    assert!(has(&rt, MARK), "the current match is marked");

    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.changes_find_count_label(), "2 of 7");
    press(&mut rt, Key::Enter, shift());
    press(&mut rt, Key::Enter, shift());
    assert_eq!(rt.state.changes_find_count_label(), "7 of 7");
    click(&mut rt, "changes-find-next");
    assert_eq!(rt.state.changes_find_count_label(), "1 of 7");
}

// ChangesReview.test.ts: "says No matches, and steps on F3 and Ctrl+G"
#[test]
fn says_no_matches_and_steps_on_f3_and_ctrl_g() {
    let (mut rt, queue, _asked) = scene(consts(), Search::Missing);
    open_with(&mut rt, "zzz");
    found(&mut rt, &queue, "no matches", 0);
    assert_eq!(rt.state.changes_find_count_label(), "No matches");
    assert!(rt.find("changes-find-next").is_some());

    // Select the field's text and type over it.
    press(&mut rt, Key::Command('f'), ctrl());
    rt.key(KeyEvent { key: Key::Text("const".into()), mods: Mods::default() });
    rt.frame(FRAME);
    found(&mut rt, &queue, "seven matches", 7);
    // Focus is in Changes but not in the field: the keys still step.
    click(&mut rt, "review-scroll");
    press(&mut rt, Key::Function(3), Mods::default());
    press(&mut rt, Key::Command('g'), ctrl());
    assert_eq!(rt.state.changes_find_count_label(), "3 of 7");
    press(&mut rt, Key::Function(3), shift());
    assert_eq!(rt.state.changes_find_count_label(), "2 of 7");
}

// ChangesReview.test.ts: "opens and closes from the Find button in the header"
#[test]
fn opens_and_closes_from_the_find_button_in_the_header() {
    let (mut rt, _queue, _asked) = scene(consts(), Search::Missing);
    click(&mut rt, "review-find");
    assert!(has(&rt, "changes-find"));
    assert_eq!(focused(&mut rt).as_deref(), Some(FIELD));
    click(&mut rt, "review-find");
    assert!(!has(&rt, "changes-find"));
}

// 11.1: previous and next are off with no matches
#[test]
fn previous_and_next_are_off_with_no_matches() {
    let (mut rt, queue, _asked) = scene(consts(), Search::Missing);
    open_with(&mut rt, "zzz");
    found(&mut rt, &queue, "no matches", 0);
    rt.frame(FRAME);
    let before = rt.state.changes_find.current;
    click(&mut rt, "changes-find-next");
    assert_eq!(rt.state.changes_find.current, before);
}

// ChangesReview.test.ts: "counts an unchanged line once in split view and marks both of its copies"
#[test]
fn counts_an_unchanged_line_once_in_split_view_and_marks_the_current_match_there() {
    let (mut rt, queue, _asked) = scene(vec![one("a.ts", "keep me\nold\n", "keep me\nnew\n")], Search::Missing);
    rt.update(|s, _| s.set_review_layout(crate::fleet::review::Layout::Split));
    rt.frame(FRAME);
    open_with(&mut rt, "keep me");
    found(&mut rt, &queue, "one match", 1);
    rt.frame(FRAME);
    assert_eq!(rt.state.changes_find_count_label(), "1 of 1");
    assert!(has(&rt, MARK), "the match is marked in Split");
}

// ChangesReview.test.ts: "keeps its marks when the layout switches"
#[test]
fn keeps_its_marks_when_the_layout_switches() {
    let (mut rt, queue, _asked) = scene(vec![one("a.ts", "x\n", "const newName = 1;\n")], Search::Missing);
    open_with(&mut rt, "newName");
    found(&mut rt, &queue, "one match", 1);
    rt.frame(FRAME);
    assert!(has(&rt, MARK));
    rt.update(|s: &mut State, _| s.set_review_layout(crate::fleet::review::Layout::Split));
    rt.frame(FRAME);
    assert!(has(&rt, MARK), "marked again in Split");
}
