//! How many columns and rows fit a box (xterm's fit addon, `TerminalView.vue` `fitNow`).

/// One cell, in Fleet's CSS pixels at 1×: JetBrains Mono 12 px has an advance xterm floors to 7, and its line height
/// is the font's own height times 1.2 (`lineHeight: 1.2`). Measured in Fleet.
pub const CELL_W: f32 = 7.0;
pub const CELL_H: f32 = 19.0;
pub const FONT: f32 = 12.0;

/// The scrollbar xterm reserves while there is scrollback.
const SCROLLBAR: f32 = 14.0;

/// The box's padding (`.terminal-view`: `6px 4px 4px 12px`, top right bottom left).
pub const PAD: [f32; 4] = [6.0, 4.0, 4.0, 12.0];

/// `cols = ⌊(width − 14) / cell w⌋`, `rows = ⌊height / cell h⌋`, at least 2 × 1. `width` and `height` are the area xterm
/// is given: the box less its padding.
pub fn fit(width: f32, height: f32) -> (u16, u16) {
    let cols = ((width - SCROLLBAR) / CELL_W).floor().max(2.0);
    let rows = (height / CELL_H).floor().max(1.0);
    (cols as u16, rows as u16)
}

/// The size for a box of `w` × `h` points, padding taken off.
pub fn fit_box(w: f32, h: f32) -> (u16, u16) {
    fit(w - PAD[1] - PAD[3], h - PAD[0] - PAD[2])
}
