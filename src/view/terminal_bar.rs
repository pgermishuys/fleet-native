//! xterm 6's scrollbar in the terminal's box (component 13, piece F; row 13.7): 8 px wide with a slider of radius 4, shown
//! while the pointer is over the box or the slider is held, and only while there is scrollback to move through.
use rune::{Cursor, El, el};

use crate::State;
use crate::term::fit::{CELL_H, PAD};
use crate::theme::Theme;

pub const BAR: &str = "terminal-scrollbar";
pub const WIDTH: f32 = 8.0;
/// VS Code's scrollable element, which xterm 6 uses, never makes the slider shorter than this.
const MIN_SLIDER: f32 = 20.0;

/// The slider's top and height inside a track of `track` px, for a view `offset` lines back in `history` lines of scrollback
/// with `rows` on screen.
pub fn slider(track: f32, history: usize, offset: usize, rows: usize) -> (f32, f32) {
    let total = (history + rows) as f32;
    let height = (track * rows as f32 / total).max(MIN_SLIDER).min(track);
    let top = if history == 0 { 0.0 } else { (history - offset.min(history)) as f32 / history as f32 * (track - height) };
    (top, height)
}

/// The scrollbar, or None while it is hidden.
pub fn bar(s: &State, t: &Theme) -> Option<El> {
    let live = s.terminal.live.as_ref()?;
    let (history, offset, rows) = live.term.scroll_state();
    if history == 0 || !(s.terminal.hover || s.terminal.bar_held) {
        return None;
    }
    let track = rows as f32 * CELL_H;
    let (top, height) = slider(track, history, offset, rows);
    let (rest, over, held) = (t.text.alpha(0.16), t.text.alpha(0.28), t.text.alpha(0.36));
    Some(
        el().key(BAR).absolute(s.terminal.box_w - PAD[1] - WIDTH, PAD[0]).size(WIDTH, track).child(
            el().key("terminal-slider")
                .absolute(0.0, top)
                .size(WIDTH, height)
                .radius(4.0)
                .fill(if s.terminal.bar_held { held } else { rest })
                .hover(over)
                .cursor(Cursor::Default)
                .on_drag(|s: &mut State, d: &mut rune::Drag, _| s.terminal_bar_drag(d)),
        ),
    )
}
