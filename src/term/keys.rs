//! Keys to the bytes an xterm sends (xterm's ctlseqs: "PC-Style Function Keys"), as xterm.js
//! does with its defaults. Not the kitty keyboard protocol. (Piece A of component 13 made the table; piece E checked every row.)
//! Rune has no key for the numeric keypad, so xterm's application keypad mode has nothing to answer.
use rune::{Key, KeyEvent};

/// xterm's modifier parameter: 1 + Shift 1 + Alt 2 + Ctrl 4.
fn modifier(ev: &KeyEvent) -> u8 {
    1 + ev.mods.shift as u8 + 2 * ev.mods.alt as u8 + 4 * ev.mods.ctrl as u8
}

/// The control character Ctrl and `c` make, as xterm maps them.
fn ctrl(c: char) -> Option<u8> {
    Some(match c.to_ascii_lowercase() {
        c @ 'a'..='z' => c as u8 - b'a' + 1,
        '@' | ' ' | '2' => 0,
        '[' | '3' => 27,
        '\\' | '4' => 28,
        ']' | '5' => 29,
        '^' | '6' => 30,
        '_' | '-' | '/' | '7' => 31,
        '8' | '?' => 127,
        _ => return None,
    })
}

/// `CSI 1 ; m <final>` with modifiers, else `CSI <final>`, or `SS3 <final>` in application
/// cursor mode (arrows, Home, End) and for F1 to F4.
fn letter(ev: &KeyEvent, last: char, ss3: bool) -> Vec<u8> {
    match modifier(ev) {
        1 if ss3 => format!("\x1bO{last}"),
        1 => format!("\x1b[{last}"),
        m => format!("\x1b[1;{m}{last}"),
    }
    .into_bytes()
}

/// `CSI n ~`, or `CSI n ; m ~` with modifiers.
fn tilde(ev: &KeyEvent, n: u8) -> Vec<u8> {
    match modifier(ev) {
        1 => format!("\x1b[{n}~"),
        m => format!("\x1b[{n};{m}~"),
    }
    .into_bytes()
}

/// The bytes for `ev`, or None when it isn't the shell's (a macOS Command shortcut).
pub fn encode(ev: &KeyEvent, app_cursor: bool) -> Option<Vec<u8>> {
    // Alt prefixes ESC off a Mac. On a Mac Option types characters (xterm's `macOptionIsMeta` is off, and Fleet doesn't set it):
    // the text that arrives is already what Option made.
    let alt = |mut b: Vec<u8>| {
        if ev.mods.alt && !cfg!(target_os = "macos") {
            b.insert(0, 0x1b);
        }
        b
    };
    Some(match &ev.key {
        // Off macOS, Ctrl and a character arrives as `Command`; on macOS that's ⌘, never the shell's.
        Key::Command(c) if !cfg!(target_os = "macos") || ev.mods.ctrl => alt(vec![ctrl(*c)?]),
        Key::Command(_) => return None,
        // Another ⌘ shortcut (on a Mac it arrives as text with the Command modifier) goes nowhere.
        Key::Text(_) if cfg!(target_os = "macos") && ev.mods.cmd && !ev.mods.ctrl => return None,
        Key::Text(t) if ev.mods.ctrl && t.chars().count() == 1 => alt(vec![ctrl(t.chars().next()?)?]),
        Key::Text(t) => alt(t.as_bytes().to_vec()),
        Key::Enter => alt(b"\r".to_vec()),
        Key::Backspace if ev.mods.ctrl => alt(vec![0x08]),
        Key::Backspace => alt(vec![0x7f]),
        Key::Tab if ev.mods.shift => b"\x1b[Z".to_vec(),
        Key::Tab => alt(b"\t".to_vec()),
        Key::Escape => alt(vec![0x1b]),
        Key::Up => letter(ev, 'A', app_cursor),
        Key::Down => letter(ev, 'B', app_cursor),
        Key::Right => letter(ev, 'C', app_cursor),
        Key::Left => letter(ev, 'D', app_cursor),
        Key::Home => letter(ev, 'H', app_cursor),
        Key::End => letter(ev, 'F', app_cursor),
        Key::Insert => tilde(ev, 2),
        Key::Delete => tilde(ev, 3),
        Key::PageUp => tilde(ev, 5),
        Key::PageDown => tilde(ev, 6),
        Key::Function(n @ 1..=4) => letter(ev, (b'P' + n - 1) as char, true),
        Key::Function(n @ 5..=12) => tilde(ev, [15, 17, 18, 19, 20, 21, 23, 24][*n as usize - 5]),
        // Ctrl+Break: what Windows consoles take as an interrupt.
        Key::Pause if ev.mods.ctrl => vec![0x03],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::Mods;

    const NONE: Mods = Mods { shift: false, cmd: false, alt: false, ctrl: false, logo: false };

    fn bytes(key: Key, mods: Mods, app: bool) -> Vec<u8> {
        encode(&KeyEvent { key, mods }, app).unwrap_or_default()
    }

    fn ctrl_key(c: char) -> KeyEvent {
        let mods = Mods { ctrl: true, cmd: !cfg!(target_os = "macos"), ..NONE };
        KeyEvent { key: if cfg!(target_os = "macos") { Key::Text(c.to_string()) } else { Key::Command(c) }, mods }
    }

    #[test]
    fn text_enter_backspace_tab_and_escape() {
        assert_eq!(bytes(Key::Text("é".into()), NONE, false), "é".as_bytes());
        assert_eq!(bytes(Key::Enter, NONE, false), b"\r");
        assert_eq!(bytes(Key::Backspace, NONE, false), b"\x7f");
        assert_eq!(bytes(Key::Backspace, Mods { ctrl: true, ..NONE }, false), b"\x08");
        assert_eq!(bytes(Key::Tab, NONE, false), b"\t");
        assert_eq!(bytes(Key::Tab, Mods { shift: true, ..NONE }, false), b"\x1b[Z");
        assert_eq!(bytes(Key::Escape, NONE, false), b"\x1b");
    }

    #[test]
    fn ctrl_and_a_letter_or_one_of_the_symbols_makes_a_control_character() {
        let want = [('a', 1), ('c', 3), ('z', 26), ('@', 0), (' ', 0), ('[', 27), ('\\', 28), (']', 29), ('^', 30), ('_', 31), ('/', 31)];
        for (c, byte) in want {
            assert_eq!(encode(&ctrl_key(c), false), Some(vec![byte]), "Ctrl {c:?}");
        }
        // Something Ctrl makes nothing of is nobody's.
        assert_eq!(encode(&ctrl_key('é'), false), None);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn alt_puts_escape_in_front_off_a_mac() {
        assert_eq!(bytes(Key::Text("b".into()), Mods { alt: true, ..NONE }, false), b"\x1bb");
        assert_eq!(bytes(Key::Backspace, Mods { alt: true, ..NONE }, false), b"\x1b\x7f");
        assert_eq!(bytes(Key::Enter, Mods { alt: true, ..NONE }, false), b"\x1b\r");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn option_types_characters_and_other_command_keys_go_nowhere_on_a_mac() {
        assert_eq!(bytes(Key::Text("\u{222b}".into()), Mods { alt: true, ..NONE }, false), "\u{222b}".as_bytes());
        assert_eq!(encode(&KeyEvent { key: Key::Text("k".into()), mods: Mods { cmd: true, ..NONE } }, false), None);
        assert_eq!(encode(&KeyEvent { key: Key::Command('k'), mods: Mods { cmd: true, ..NONE } }, false), None);
    }

    #[test]
    fn arrows_home_and_end_in_normal_and_application_mode_with_modifier_parameters() {
        for (key, last) in [(Key::Up, 'A'), (Key::Down, 'B'), (Key::Right, 'C'), (Key::Left, 'D'), (Key::Home, 'H'), (Key::End, 'F')] {
            assert_eq!(bytes(key.clone(), NONE, false), format!("\x1b[{last}").into_bytes());
            assert_eq!(bytes(key.clone(), NONE, true), format!("\x1bO{last}").into_bytes());
            // Shift 2, Alt 3, Ctrl 5, Ctrl+Shift 6, Ctrl+Alt 7; the parameter form holds in both modes.
            for (mods, m) in [
                (Mods { shift: true, ..NONE }, 2),
                (Mods { alt: true, ..NONE }, 3),
                (Mods { ctrl: true, ..NONE }, 5),
                (Mods { ctrl: true, shift: true, ..NONE }, 6),
                (Mods { ctrl: true, alt: true, ..NONE }, 7),
            ] {
                let want = format!("\x1b[1;{m}{last}").into_bytes();
                assert_eq!(bytes(key.clone(), mods, false), want);
                assert_eq!(bytes(key.clone(), mods, true), want);
            }
        }
    }

    #[test]
    fn insert_delete_page_keys_and_the_function_keys() {
        assert_eq!(bytes(Key::Insert, NONE, false), b"\x1b[2~");
        assert_eq!(bytes(Key::Delete, NONE, false), b"\x1b[3~");
        assert_eq!(bytes(Key::PageUp, NONE, false), b"\x1b[5~");
        assert_eq!(bytes(Key::PageDown, NONE, false), b"\x1b[6~");
        assert_eq!(bytes(Key::Delete, Mods { ctrl: true, ..NONE }, false), b"\x1b[3;5~");
        assert_eq!(bytes(Key::PageUp, Mods { alt: true, ..NONE }, false), b"\x1b[5;3~");
        let tildes = [(5, 15), (6, 17), (7, 18), (8, 19), (9, 20), (10, 21), (11, 23), (12, 24)];
        for (f, n) in tildes {
            assert_eq!(bytes(Key::Function(f), NONE, false), format!("\x1b[{n}~").into_bytes(), "F{f}");
            assert_eq!(bytes(Key::Function(f), Mods { shift: true, ..NONE }, false), format!("\x1b[{n};2~").into_bytes());
        }
        for (f, last) in [(1, 'P'), (2, 'Q'), (3, 'R'), (4, 'S')] {
            assert_eq!(bytes(Key::Function(f), NONE, false), format!("\x1bO{last}").into_bytes(), "F{f}");
            assert_eq!(bytes(Key::Function(f), Mods { ctrl: true, ..NONE }, false), format!("\x1b[1;5{last}").into_bytes());
        }
        assert_eq!(encode(&KeyEvent { key: Key::Function(13), mods: NONE }, false), None);
    }

    #[test]
    fn ctrl_break_is_an_interrupt_and_a_lone_pause_is_nothing() {
        assert_eq!(bytes(Key::Pause, Mods { ctrl: true, ..NONE }, false), vec![3]);
        assert_eq!(encode(&KeyEvent { key: Key::Pause, mods: NONE }, false), None);
    }

    fn key(key: Key, mods: Mods) -> Option<Vec<u8>> {
        encode(&KeyEvent { key, mods }, false)
    }

    #[test]
    fn ctrl_letters_are_control_characters() {
        let ctrl = Mods { ctrl: true, cmd: !cfg!(target_os = "macos"), ..Mods::default() };
        let c = if cfg!(target_os = "macos") { Key::Text("c".into()) } else { Key::Command('c') };
        assert_eq!(key(c, ctrl), Some(vec![3]));
    }

    #[test]
    fn arrows_follow_application_cursor_mode() {
        let up = KeyEvent { key: Key::Up, mods: Mods::default() };
        assert_eq!(encode(&up, false).unwrap(), b"\x1b[A");
        assert_eq!(encode(&up, true).unwrap(), b"\x1bOA");
        let shift = KeyEvent { key: Key::Up, mods: Mods { shift: true, ..Mods::default() } };
        assert_eq!(encode(&shift, true).unwrap(), b"\x1b[1;2A");
    }
}
