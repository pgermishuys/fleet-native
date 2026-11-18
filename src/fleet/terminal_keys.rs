//! Ports `lib/terminal-keys.ts` (component 13, piece E): who gets a key pressed while a terminal has the keyboard.
//! The shell gets everything (Esc, Ctrl K, Ctrl B, Ctrl [ are everyday shell keys) except Fleet's own pass-through
//! shortcuts and the copy and paste keys (Ctrl C stops the running command, so copy is Ctrl Shift C off a Mac).
use rune::{Key, KeyEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    /// Fleet handles it: the terminal lets it through.
    Fleet,
    /// Copy the terminal's selection.
    Copy,
    /// Paste what the clipboard holds into the shell.
    Paste,
    /// The shell gets it, and Fleet never sees it.
    Shell,
}

/// One of Fleet's shortcuts (`GlobalShortcut`): the platform modifier (⌘ on a Mac, Ctrl elsewhere), a letter, and Shift.
#[derive(Clone, Copy, Debug)]
pub struct Shortcut {
    pub key: char,
    pub shift: bool,
}

/// Fleet's pass-through shortcuts as bound today: toggle-terminal (Mod+J) and toggle-right-panel (Mod+Shift+B). The app has
/// no rebinding yet, so this is the binding table.
pub const PASS_THROUGH: [Shortcut; 2] = [Shortcut { key: 'j', shift: false }, Shortcut { key: 'b', shift: true }];

/// The letter a key event types with the platform modifier or Ctrl held, lowercased.
fn letter(ev: &KeyEvent) -> Option<char> {
    match &ev.key {
        Key::Command(c) => Some(c.to_ascii_lowercase()),
        Key::Text(t) if t.chars().count() == 1 => t.chars().next().map(|c| c.to_ascii_lowercase()),
        _ => None,
    }
}

pub fn terminal_key_owner(ev: &KeyEvent, mac: bool, pass_through: &[Shortcut]) -> Owner {
    let m = ev.mods;
    let key = letter(ev);
    if pass_through.iter().any(|p| m.cmd && !m.alt && m.shift == p.shift && key == Some(p.key)) {
        return Owner::Fleet;
    }
    if mac {
        // ⌘ C copies and ⌘ V pastes; Ctrl Shift C is the shell's.
        if m.cmd && !m.ctrl && !m.shift && !m.alt {
            match key {
                Some('c') => return Owner::Copy,
                Some('v') => return Owner::Paste,
                _ => {}
            }
        }
    } else if m.ctrl && m.shift && !m.alt && !m.logo {
        match key {
            Some('c') => return Owner::Copy,
            Some('v') => return Owner::Paste,
            _ => {}
        }
    }
    Owner::Shell
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::Mods;

    /// Off a Mac, Ctrl and a letter arrive as `Command` with both `cmd` and `ctrl`; on a Mac ⌘ is `cmd` alone.
    fn press(c: char, ctrl: bool, shift: bool, mac: bool) -> KeyEvent {
        let mods = Mods { ctrl, shift, cmd: if mac { false } else { ctrl }, ..Mods::default() };
        KeyEvent { key: Key::Command(c), mods }
    }

    fn mac_cmd(c: char, shift: bool) -> KeyEvent {
        KeyEvent { key: Key::Command(c), mods: Mods { cmd: true, shift, ..Mods::default() } }
    }

    fn plain(key: Key) -> KeyEvent {
        KeyEvent { key, mods: Mods::default() }
    }

    fn owner(ev: &KeyEvent) -> Owner {
        terminal_key_owner(ev, false, &PASS_THROUGH)
    }

    #[test]
    fn gives_the_everyday_shell_keys_to_the_shell() {
        assert_eq!(owner(&plain(Key::Escape)), Owner::Shell);
        for c in ['k', 'b', '[', 'c', 'v'] {
            assert_eq!(owner(&press(c, true, false, false)), Owner::Shell, "Ctrl {c}");
        }
        assert_eq!(owner(&plain(Key::Text("a".into()))), Owner::Shell);
    }

    #[test]
    fn leaves_ctrl_j_and_ctrl_shift_b_to_fleet() {
        assert_eq!(owner(&press('j', true, false, false)), Owner::Fleet);
        assert_eq!(owner(&press('b', true, true, false)), Owner::Fleet);
        // Ctrl Shift J and Ctrl B are not the shortcuts.
        assert_eq!(owner(&press('j', true, true, false)), Owner::Shell);
        assert_eq!(owner(&press('b', true, false, false)), Owner::Shell);
    }

    #[test]
    fn copies_with_ctrl_shift_c_and_pastes_with_ctrl_shift_v() {
        assert_eq!(owner(&press('c', true, true, false)), Owner::Copy);
        assert_eq!(owner(&press('v', true, true, false)), Owner::Paste);
    }

    #[test]
    fn follows_a_rebound_terminal_shortcut() {
        let rebound = [Shortcut { key: '`', shift: false }];
        assert_eq!(terminal_key_owner(&press('`', true, false, false), false, &rebound), Owner::Fleet);
        assert_eq!(terminal_key_owner(&press('j', true, false, false), false, &rebound), Owner::Shell);
    }

    #[test]
    fn leaves_ctrl_shift_c_to_the_shell_since_cmd_c_copies() {
        assert_eq!(terminal_key_owner(&press('c', true, true, true), true, &PASS_THROUGH), Owner::Shell);
        assert_eq!(terminal_key_owner(&mac_cmd('c', false), true, &PASS_THROUGH), Owner::Copy);
        assert_eq!(terminal_key_owner(&mac_cmd('v', false), true, &PASS_THROUGH), Owner::Paste);
        assert_eq!(terminal_key_owner(&mac_cmd('j', false), true, &PASS_THROUGH), Owner::Fleet);
        // Other ⌘ keys are the shell's to ignore: Fleet never sees them either.
        assert_eq!(terminal_key_owner(&mac_cmd('k', false), true, &PASS_THROUGH), Owner::Shell);
    }
}
