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

/// How long the bar stays after the pointer leaves or the view last moved, and how long it then takes to fade out
/// (VS Code's scrollbar, which xterm 6 uses: a 500 ms delay, an 800 ms linear fade; it shows at once).
pub const LINGER: f64 = 0.5;
pub const FADE: f64 = 0.8;

/// The bar's opacity: whole while it has a reason to show, then fading linearly once `idle` seconds have passed without one.
pub fn opacity(shown: bool, idle: f64) -> f32 {
    if shown { 1.0 } else { (1.0 - (idle - LINGER) / FADE).clamp(0.0, 1.0) as f32 }
}

/// Where the slider's top goes when the pointer is `y` px down the track: the slider centres on it (a click on the empty track).
pub fn offset_at(y: f32, track: f32, history: usize, rows: usize) -> usize {
    let (_, height) = slider(track, history, 0, rows);
    let room = (track - height).max(f32::EPSILON);
    let top = (y - height / 2.0).clamp(0.0, room);
    (history as f32 * (1.0 - top / room)).round() as usize
}

/// The scrollbar, or None once it has faded away.
pub fn bar(s: &State, t: &Theme) -> Option<El> {
    let live = s.terminal.live.as_ref()?;
    let (history, offset, rows) = live.term.scroll_state();
    let shown = s.terminal.hover || s.terminal.bar_held;
    let fade = opacity(shown, s.terminal.bar_idle(s.clock));
    if history == 0 || fade <= 0.0 {
        return None;
    }
    let track = rows as f32 * CELL_H;
    let (top, height) = slider(track, history, offset, rows);
    let (rest, over, held) = (t.text.alpha(0.16), t.text.alpha(0.28), t.text.alpha(0.36));
    Some(
        el().key(BAR)
            .absolute(s.terminal.box_w - PAD[1] - WIDTH, PAD[0])
            .size(WIDTH, track)
            .opacity(fade)
            .on_drag(|s: &mut State, d: &mut rune::Drag, _| s.terminal_track_drag(d))
            .child(
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
