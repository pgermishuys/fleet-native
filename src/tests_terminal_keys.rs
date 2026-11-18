//! Component 13, piece E, without a server: `TerminalDrawer.test.ts` "moves between tabs with the arrow keys" and "resizes from the
//! keyboard, within its limits", `StatusBar.test.ts` "hides the terminal from the terminal's own hints, and leaves the rest as
//! text", and the grip's drag and double-click. (The key owner and the key table have their tests beside them; what reaches a
//! shell is in `tests_terminal_live`.)
use crate::tests::{FRAME, app, click};
use rune::{Key, KeyEvent, Mods, Runtime};

fn press(rt: &mut Runtime<crate::State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
}

/// The invented session with its drawer open on the pretend shell and a second tab, as the app with no server does.
fn two_tabs() -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.set_session_id((0, 0), "s1"));
    rt.frame(FRAME);
    click(&mut rt, "terminal");
    click(&mut rt, "terminal-new");
    rt
}

fn focus(rt: &mut Runtime<crate::State>, key: &str) {
    let key = key.to_string();
    rt.update(move |_, tx| tx.focus(key, true));
    rt.frame(FRAME);
}

fn focused(rt: &Runtime<crate::State>) -> Option<String> {
    rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key)
}

fn tab_ids(rt: &Runtime<crate::State>) -> Vec<String> {
    rt.state.drawer_tabs().into_iter().map(|t| t.0).collect()
}

// ---- TerminalDrawer.test.ts --------------------------------------------------------------------------------------------------

#[test]
fn moves_between_tabs_with_the_arrow_keys() {
    let mut rt = two_tabs();
    let ids = tab_ids(&rt);
    assert_eq!(rt.state.drawer_active().as_deref(), Some(ids[1].as_str()), "the new tab shows");
    focus(&mut rt, &crate::view::terminal_drawer::tab_key(&ids[1]));

    // Right wraps to the first, Left wraps back; the tab that shows has the focus.
    press(&mut rt, Key::Right);
    assert_eq!(rt.state.drawer_active().as_deref(), Some(ids[0].as_str()));
    assert_eq!(focused(&rt).as_deref(), Some(crate::view::terminal_drawer::tab_key(&ids[0]).as_str()));
    press(&mut rt, Key::Left);
    assert_eq!(rt.state.drawer_active().as_deref(), Some(ids[1].as_str()));
    press(&mut rt, Key::Home);
    assert_eq!(rt.state.drawer_active().as_deref(), Some(ids[0].as_str()));
    press(&mut rt, Key::End);
    assert_eq!(rt.state.drawer_active().as_deref(), Some(ids[1].as_str()));
}

#[test]
fn delete_closes_the_active_tab_and_the_neighbour_takes_the_focus() {
    let mut rt = two_tabs();
    let ids = tab_ids(&rt);
    focus(&mut rt, &crate::view::terminal_drawer::tab_key(&ids[1]));
    press(&mut rt, Key::Delete);
    assert_eq!(tab_ids(&rt), [ids[0].clone()]);
    assert_eq!(focused(&rt).as_deref(), Some(crate::view::terminal_drawer::tab_key(&ids[0]).as_str()));
}

#[test]
fn resizes_from_the_keyboard_within_its_limits() {
    let mut rt = two_tabs();
    let before = rt.state.terminals.height();
    focus(&mut rt, crate::view::terminal_drawer::GRIP);

    press(&mut rt, Key::Up);
    assert_eq!(rt.state.terminals.height(), before + 24);
    for _ in 0..20 {
        press(&mut rt, Key::Down);
    }
    assert_eq!(rt.state.terminals.height(), 140);
}

#[test]
fn the_keyboard_cant_make_the_drawer_taller_than_60_percent_of_the_column() {
    let mut rt = two_tabs();
    rt.update(|s, _| s.sheet_h = 500.0);
    focus(&mut rt, crate::view::terminal_drawer::GRIP);
    for _ in 0..20 {
        press(&mut rt, Key::Up);
    }
    assert_eq!(crate::view::terminal_drawer::height(&rt.state), 300.0);
}

#[test]
fn dragging_the_grip_up_makes_it_taller_and_double_clicking_puts_it_back_to_272() {
    let mut rt = two_tabs();
    assert_eq!(rt.state.terminals.height(), 272);
    let (x, y) = rt.find(crate::view::terminal_drawer::GRIP).expect("the grip");
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_move(x, y - 80.0);
    rt.frame(FRAME);
    assert!(rt.state.terminal.dragging, "the bar is lit while it is held");
    assert_eq!(rt.state.terminals.height(), 352);
    rt.pointer_move(x, y + 400.0);
    rt.frame(FRAME);
    assert_eq!(rt.state.terminals.height(), 140, "never shorter than the minimum");
    rt.pointer_up(x, y + 400.0);
    rt.frame(FRAME);
    assert!(!rt.state.terminal.dragging);

    let (x, y) = rt.find(crate::view::terminal_drawer::GRIP).expect("the grip");
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(FRAME);
    assert_eq!(rt.state.terminals.height(), 272);
}

// ---- StatusBar.test.ts -------------------------------------------------------------------------------------------------------

#[test]
fn hides_the_terminal_from_the_terminals_own_hints_and_leaves_the_rest_as_text() {
    let mut rt = two_tabs();
    rt.update(|s, _| s.terminal.focused = true);
    rt.frame(FRAME);
    assert!(rt.find("terminal-keyboard-hint").is_some(), "the variant is drawn");
    assert!(rt.find("terminal-owner").is_some());
    assert!(rt.find("palette-hint").is_none(), "the ordinary hints are not");
    // One button in it; the rest is text.
    assert!(rt.find("status-hint-hide-terminal").is_some());
    assert!(rt.find("terminal-copy-hint").is_some());

    click(&mut rt, "status-hint-hide-terminal");
    assert!(rt.find("terminal-drawer").is_none(), "it ran toggle-terminal");
}
