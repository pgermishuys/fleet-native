//! A program that asks for the mouse (component 13, piece H; xterm modes 1000, 1002, 1003 with the default, UTF-8 and SGR
//! encodings) hears the pointer and the wheel in the box instead of the box selecting and scrolling. Shift (Option on a Mac)
//! takes the pointer back for a selection. The bytes are `term::mouse`.
use rune::{Drag, Phase, PointerButton, Tx, WheelEvent};

use crate::State;
use crate::term::fit::{CELL_H, CELL_W, PAD};
use crate::term::mouse::{self, Button, Kind, Report};

impl State {
    /// Shift forces a selection, and on a Mac so does Option (xterm.js's `macOptionClickForcesSelection` is off, but
    /// Option types characters there, so Option never reaches the program as Meta).
    fn forced_selection(&self) -> bool {
        let m = self.terminal.mods;
        m.shift || (cfg!(target_os = "macos") && m.alt)
    }

    fn mouse_asked(&self) -> bool {
        self.terminal.live.as_ref().is_some_and(|l| mouse::reporting(l.term.mode()))
    }

    /// The program asked for the mouse and the user hasn't taken it back.
    pub fn mouse_taken(&self) -> bool {
        self.mouse_asked() && !self.forced_selection()
    }

    /// The cell under a point in the box, kept inside the screen.
    fn mouse_cell(&self, x: f32, y: f32) -> (usize, usize) {
        let (cols, rows) = self.terminal.live.as_ref().map_or((1, 1), |l| l.term.size());
        let cell = |v: f32, pad: f32, size: f32, n: u16| (((v - pad) / size).floor().max(0.0) as usize).min(n.saturating_sub(1) as usize);
        (cell(x, PAD[3], CELL_W, cols), cell(y, PAD[0], CELL_H, rows))
    }

    fn send_report(&mut self, kind: Kind, cell: (usize, usize)) -> bool {
        let Some(live) = self.terminal.live.as_ref() else { return false };
        let m = self.terminal.mods;
        // The default encoding has no way to say Meta on a Mac, where Option types; xterm.js sends what the event holds.
        let r = Report { kind, col: cell.0, row: cell.1, shift: m.shift, alt: m.alt && !cfg!(target_os = "macos"), ctrl: m.ctrl };
        match mouse::encode(live.term.mode(), &r) {
            Some(bytes) => {
                self.terminal_write(bytes);
                true
            }
            None => false,
        }
    }

    /// A drag-gesture phase for the program. True when the program has the gesture (so the box doesn't select).
    pub fn report_drag(&mut self, d: &Drag, tx: &Tx) -> bool {
        let button = match tx.button() {
            PointerButton::Primary => Button::Left,
            PointerButton::Middle => Button::Middle,
            _ => return false,
        };
        let cell = self.mouse_cell(d.x, d.y);
        match d.phase {
            Phase::Down => {
                self.terminal.selecting = !self.mouse_taken();
                if self.terminal.selecting {
                    return false;
                }
                self.terminal.mouse_held = Some(button);
                self.terminal.mouse_cell = Some(cell);
                self.send_report(Kind::Press(button), cell);
                true
            }
            _ if self.terminal.selecting => false,
            Phase::Move => {
                let Some(held) = self.terminal.mouse_held else { return false };
                if self.terminal.mouse_cell.replace(cell) != Some(cell) {
                    self.send_report(Kind::Motion(Some(held)), cell);
                }
                true
            }
            Phase::Up => {
                let Some(held) = self.terminal.mouse_held.take() else { return false };
                self.send_report(Kind::Release(held), cell);
                true
            }
        }
    }

    /// The secondary button is a click to the program (a press and its release): Rune gives a press-and-release, not a drag.
    pub fn report_right_click(&mut self, x: f32, y: f32) {
        if self.mouse_taken() {
            let cell = self.mouse_cell(x, y);
            self.send_report(Kind::Press(Button::Right), cell);
            self.send_report(Kind::Release(Button::Right), cell);
        }
    }

    /// The pointer moved with no button held: mode 1003 hears it, once per cell.
    pub fn report_motion(&mut self, x: f32, y: f32) {
        if self.terminal.mouse_held.is_some() {
            return;
        }
        let cell = self.mouse_cell(x, y);
        if self.terminal.mouse_cell.replace(cell) != Some(cell) {
            self.send_report(Kind::Motion(None), cell);
        }
    }

    /// The wheel: a program that asked for the mouse hears it (one report per notch, per line for a trackpad); a full-screen
    /// program that didn't ask gets arrow presses, as xterm.js sends them in the alternate screen; both before the box scrolls.
    /// True when taken.
    pub fn report_wheel(&mut self, e: &WheelEvent) -> bool {
        let Some(live) = self.terminal.live.as_ref() else { return false };
        let mode = live.term.mode();
        let asked = mouse::reporting(mode);
        let alt_screen = mode.contains(crate::term::Mode::ALT_SCREEN);
        if (asked && self.forced_selection()) || !(asked || alt_screen) {
            return false;
        }
        // Positive dy moves the content down, which is the wheel turned up (back through the history).
        let up = e.dy > 0.0;
        let lines = if e.precise { e.dy / CELL_H } else { e.dy.signum() };
        self.terminal.wheel_rem += lines;
        let whole = self.terminal.wheel_rem.trunc();
        self.terminal.wheel_rem -= whole;
        let count = whole.abs() as usize;
        if count == 0 {
            return true;
        }
        if asked {
            let cell = self.mouse_cell(e.x, e.y);
            for _ in 0..count {
                self.send_report(if up { Kind::WheelUp } else { Kind::WheelDown }, cell);
            }
        } else {
            let app = mode.contains(crate::term::APP_CURSOR);
            // A notch is three lines of arrows, as the scrollback's wheel moves three.
            let n = if e.precise { count } else { count * super::terminal_select::NOTCH_LINES as usize };
            self.terminal_write(mouse::wheel_arrows(up, n, app));
        }
        true
    }

    /// Text an input method composed goes to the shell (13.64). True when the box has the keyboard and took it.
    pub fn terminal_ime(&mut self, ime: &rune::Ime) -> bool {
        match ime {
            rune::Ime::Commit(text) if self.terminal.focused && !text.is_empty() => {
                if let Some(live) = self.terminal.live.as_ref() {
                    live.term.scroll_to_bottom();
                }
                self.terminal_write(text.as_bytes().to_vec());
                true
            }
            _ => false,
        }
    }
}
