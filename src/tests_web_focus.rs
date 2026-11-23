//! A focused web view counts as the window focused (row 2.88, 02 row 6.6): Fleet's `focus`/`blur` also fire when focus moves into an
//! iframe of the same page, so the server must not hear "not looking" while the keyboard is in a page.
use crate::tests::app;

#[test]
fn a_page_with_the_keyboard_keeps_the_window_focused() {
    let mut rt = app();
    rt.update(|s, tx| {
        // The page took the keys: Rune's half goes quiet, the window is still focused.
        s.window_focus_changed(true, true, tx);
        s.window_focus_changed(false, false, tx);
        assert!(s.quiet.focused, "a page with the keys is the window with the keys");
    });
}

#[test]
fn the_window_is_unfocused_once_neither_half_has_the_keyboard() {
    let mut rt = app();
    rt.update(|s, tx| {
        s.window_focus_changed(true, true, tx);
        s.window_focus_changed(false, false, tx);
        // The page's window lost it too: another app has it now.
        s.window_focus_changed(true, false, tx);
        assert!(!s.quiet.focused);
        // A click in Rune brings it back.
        s.window_focus_changed(false, true, tx);
        assert!(s.quiet.focused);
    });
}

#[test]
fn a_page_taking_the_keys_while_rune_is_blurred_focuses_the_window() {
    let mut rt = app();
    rt.update(|s, tx| {
        s.window_focus_changed(false, false, tx);
        assert!(!s.quiet.focused);
        s.window_focus_changed(true, true, tx);
        assert!(s.quiet.focused);
    });
}
