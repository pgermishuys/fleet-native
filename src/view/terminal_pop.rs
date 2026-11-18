//! The bubble over a terminal selection (component 13, piece F; `.terminal-view__pop` in `TerminalView.vue`): "Add line 9 to
//! message" and Copy, above the selection's first row or, near the top, under its last.
use rune::{Cursor, El, Shadow, Tx, icon, row, text};

use crate::State;
use crate::composer::attached::terminal_line_range;
use crate::icons::{COPY, MESSAGE_SQUARE_PLUS};
use crate::term::fit::{CELL_H, CELL_W, PAD};
use crate::term::selection::{SelectedLines, Span};
use crate::theme::{RADIUS_BTN, Theme, sans};

pub const POP: &str = "terminal-pop";
pub const ADD: &str = "terminal-pop-add";
pub const COPY_KEY: &str = "terminal-pop-copy";

/// Where the bubble goes, in the view's own points: its top-left corner. `rows` is the screen's height in rows, `client_w` the
/// width of the host (`clientWidth`), `size` the bubble's own. xterm centres it on the selection's end column, 130 px from either
/// side at the nearest, and puts it above the first row unless that is within 34 px of the top.
pub fn place(span: Span, rows: usize, client_w: f32, size: (f32, f32)) -> (f32, f32, bool) {
    let below = (span.first_row as f32) * CELL_H < 34.0;
    let top = if below { (span.last_row + 1).min(rows as i32) as f32 * CELL_H + 6.0 } else { span.first_row as f32 * CELL_H + 6.0 };
    let centre = (span.end_x as f32 * CELL_W).max(130.0).min(client_w - 130.0);
    // `translate(-50%, calc(-100% - 4px))` above, `translate(-50%, 0)` below.
    (centre - size.0 / 2.0, if below { top } else { top - size.1 - 4.0 }, below)
}

fn label(lines: &SelectedLines) -> String {
    format!("Add {} to message", terminal_line_range(lines.from, lines.to))
}

/// The two buttons' widths, for placing the bubble by its centre: 26 px tall, padding 0 9, gap 6, an icon of 13.
fn size(lines: &SelectedLines) -> (f32, f32) {
    let word = |s: &str| rune::text::advance(s, sans(12.0, 500.0, 1.4));
    let button = |s: &str| 9.0 * 2.0 + 13.0 + 6.0 + word(s);
    (2.0 * (1.0 + 3.0) + button(&label(lines)) + 2.0 + button("Copy"), 26.0 + 2.0 * (1.0 + 3.0))
}

/// The bubble, or None while nothing worth adding is selected.
pub fn pop(s: &State, t: &Theme) -> Option<El> {
    let live = s.terminal.live.as_ref()?;
    let lines = live.term.selected_lines()?;
    let span = live.term.selection_span()?;
    let (_, rows) = live.term.size();
    let client_w = s.terminal.box_w - PAD[1] - PAD[3];
    let (w, h) = size(&lines);
    let (x, y, _) = place(span, rows as usize, client_w, (w, h));
    // `mousedown.prevent`: pressing a button must not take the keyboard from the terminal.
    let button = |key: &str, ico: &'static rune::Icon, ink: rune::Color, words: String| {
        row()
            .key(key)
            .h(26.0)
            .px(9.0)
            .gap(6.0)
            .items_center()
            .radius(RADIUS_BTN - 2.0)
            .hover(t.wash(6.0))
            .cursor(Cursor::Pointer)
            .label(words.clone())
            .child(icon(ico, 13.0).color(ink))
            .child(text(words).font(sans(12.0, 500.0, 1.4)).color(ink))
    };
    let add = button(ADD, &MESSAGE_SQUARE_PLUS, t.accent_text, label(&lines))
        .on_press_tx(|s: &mut State, tx: &mut Tx| s.attach_terminal_lines(tx))
        .focusable(false);
    let copy = button(COPY_KEY, &COPY, t.text, "Copy".into())
        .on_press_tx(|s: &mut State, tx: &mut Tx| s.copy_and_clear_terminal_selection(tx))
        .focusable(false);
    Some(
        row()
            .key(POP)
            .absolute(x, y)
            .w(w)
            .h(h)
            .px(3.0)
            .py(3.0)
            .gap(2.0)
            .radius(RADIUS_BTN + 1.0)
            .border(1.0, t.border)
            .border_layout()
            .fill(t.card_bg)
            .box_shadow(Shadow { x: 0.0, y: 10.0, blur: 30.0, spread: -12.0, color: rune::Color::BLACK.alpha(0.35), inset: false })
            .box_shadow(Shadow { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: rune::Color::BLACK.alpha(0.08), inset: false })
            .child(add)
            .child(copy),
    )
}
