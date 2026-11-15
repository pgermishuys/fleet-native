//! Where a waiting card puts the keyboard, and how the arrows move it (`lib/focus-blocking-card.ts`, rows 3.14 and
//! 1.19 of `docs/parity/04-questions-and-permissions.md`). Plain rules: the views and the window's key handler ask,
//! and piece D's question card uses the same two.
use rune::Key;

/// `focusBlockingCard`: a card that blocks the turn takes the keyboard when it appears, unless the user is typing
/// somewhere else that already holds words. An empty composer that took focus as the session opened isn't typing.
pub fn takes_focus(focused: Option<&str>, composer_key: &str, composer_len: usize) -> bool {
    !(focused == Some(composer_key) && composer_len > 0)
}

/// `moveChoiceFocus`: Up and Left go back, Down and Right forward; any other key is not the card's.
pub fn step_of(key: &Key) -> Option<isize> {
    match key {
        Key::Down | Key::Right => Some(1),
        Key::Up | Key::Left => Some(-1),
        _ => None,
    }
}

/// The choice an arrow lands on, wrapping round. `None` when `from` isn't one of the `choices`.
pub fn next_choice<'a>(choices: &[&'a str], from: &str, step: isize) -> Option<&'a str> {
    let at = choices.iter().position(|c| *c == from)? as isize;
    let n = choices.len() as isize;
    choices.get((at + step).rem_euclid(n) as usize).copied()
}
