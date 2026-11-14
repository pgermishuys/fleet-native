//! Keys to the bytes an xterm sends (xterm's ctlseqs: "PC-Style Function Keys"), as xterm.js
//! does with its defaults. Not the kitty keyboard protocol.
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
    let alt = |mut b: Vec<u8>| {
        if ev.mods.alt {
            b.insert(0, 0x1b);
        }
        b
    };
    Some(match &ev.key {
        // Off macOS, Ctrl and a character arrives as `Command`; on macOS that's ⌘, never the shell's.
        Key::Command(c) if !cfg!(target_os = "macos") || ev.mods.ctrl => alt(vec![ctrl(*c)?]),
        Key::Command(_) => return None,
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
