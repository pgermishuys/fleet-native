//! What the mouse does in the terminal's box (component 13, piece F): select by dragging, by word and by line, copy, add the
//! selected lines to the message, the wheel, and the scrollbar's slider. The cells and lines are `term::selection`; the bubble
//! is `view::terminal_pop`, the slider `view::terminal_bar`.
use rune::{Drag, Phase, PointerButton, Tx, WheelEvent};

use crate::State;
use crate::composer::attached::TerminalDraft;
use crate::term::fit::{CELL_H, CELL_W, PAD};
use crate::term::selection::Pick;

/// How far the pointer may travel and still be a click (Rune's own double-click slop).
/// The most lines one tick of a drag's own scrolling moves.
const DRAG_SCROLL_MAX: i32 = 5;

const CLICK_SLOP: f32 = 4.0;

/// Lines one notch of a wheel scrolls (xterm: three).
pub const NOTCH_LINES: f32 = 3.0;

/// The cell under a point in the box (padding included), and whether the point is in the cell's right half.
fn cell_at(x: f32, y: f32) -> (usize, usize, bool) {
    let (cx, cy) = ((x - PAD[3]) / CELL_W, (y - PAD[0]) / CELL_H);
    (cx.floor().max(0.0) as usize, cy.floor().max(0.0) as usize, cx - cx.floor() >= 0.5)
}

impl State {
    /// The box's drag: a press selects (a word on a double click, a line on a triple), moving extends. A press past the top or
    /// bottom edge scrolls the view a line with each move, so a selection can reach what is off screen.
    pub fn terminal_drag(&mut self, d: &Drag, tx: &mut Tx) {
        if self.report_drag(d, tx) {
            return;
        }
        // Without a program asking for the mouse, only the primary button selects.
        if tx.button() != PointerButton::Primary {
            return;
        }
        let Some(live) = self.terminal.live.as_ref() else { return };
        let (col, row, right) = cell_at(d.x, d.y);
        match d.phase {
            Phase::Down => {
                let pick = match d.clicks {
                    0 | 1 => Pick::Chars,
                    2 => Pick::Word,
                    _ => Pick::Line,
                };
                live.term.select_start(pick, col, row, right);
            }
            Phase::Move => {
                // Held past an edge the view scrolls on a timer (`keep_drag_scroll`), faster the further out the pointer is.
                let bottom = PAD[0] + live.term.size().1 as f32 * CELL_H;
                self.terminal.drag_scroll = if d.y < PAD[0] {
                    1 + ((PAD[0] - d.y) / CELL_H) as i32
                } else if d.y > bottom {
                    -1 - ((d.y - bottom) / CELL_H) as i32
                } else {
                    0
                }
                .clamp(-DRAG_SCROLL_MAX, DRAG_SCROLL_MAX);
                self.terminal.drag_cell = (col, row, right);
                live.term.select_to(col, row, right);
            }
            Phase::Up => {
                self.terminal.drag_scroll = 0;
                // A click that never moved on a link opens it; a drag that began on one selected instead.
                if d.clicks <= 1 && d.dx.abs() < CLICK_SLOP && d.dy.abs() < CLICK_SLOP && !d.cancelled() {
                    self.open_terminal_link(tx);
                }
            }
        }
    }

    /// The wheel: the view moves through the scrollback and the shell hears nothing. True when the wheel was ours.
    pub fn terminal_wheel(&mut self, e: &WheelEvent) -> bool {
        if self.report_wheel(e) {
            return true;
        }
        let Some(live) = self.terminal.live.as_ref() else { return false };
        // A notch is three lines; a trackpad's steps are points, kept until they add up to a line.
        let lines = if e.precise { e.dy / CELL_H } else { e.dy.signum() * NOTCH_LINES };
        self.terminal.bar_seen = self.clock;
        self.terminal.wheel_rem += lines;
        let whole = self.terminal.wheel_rem.trunc();
        self.terminal.wheel_rem -= whole;
        if whole != 0.0 {
            live.term.scroll_by(whole as i32);
        }
        true
    }

    /// Copy (Ctrl Shift C, ⌘ C): the selected text goes on the clipboard and the selection stays.
    pub fn copy_terminal_selection(&mut self, tx: &mut Tx) {
        if let Some(text) = self.terminal.live.as_ref().and_then(|l| l.term.selected_text()) {
            tx.copy(text);
        }
    }

    /// The bubble's Copy (`copySelection`): copies, then clears the selection.
    pub fn copy_and_clear_terminal_selection(&mut self, tx: &mut Tx) {
        self.copy_terminal_selection(tx);
        if let Some(live) = self.terminal.live.as_ref() {
            live.term.clear_selection();
        }
    }

    /// The bubble's Add (`onAttach` in `TerminalDrawer.vue`): the lines join the open session's draft as a chip labelled with
    /// the tab's title, the selection clears and the composer takes the keyboard.
    pub fn attach_terminal_lines(&mut self, tx: &mut Tx) {
        let Some(live) = self.terminal.live.as_ref() else { return };
        // The setup row listens to nothing: the bubble's Add only lets go of the selection (13.80).
        if self.terminal.setup.is_some() {
            live.term.clear_selection();
            return;
        }
        let Some(lines) = live.term.selected_lines() else { return };
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
        let context = TerminalDraft {
            id: format!("{}:{}-{}:{ms}", live.id, lines.from, lines.to),
            terminal_id: live.id.clone(),
            label: live.title.clone(),
            from: lines.from,
            to: lines.to,
            text: lines.text,
        };
        live.term.clear_selection();
        self.add_terminal_lines(context);
        tx.focus("composer", false);
    }

    /// The slider is pressed or dragged: `start` is the view's offset when it was pressed, `dy` how far the pointer has gone
    /// down since, and the slider moves with the pointer, so the offset moves by that share of the scrollback.
    pub fn terminal_bar_drag(&mut self, d: &mut Drag) {
        let Some(live) = self.terminal.live.as_ref() else { return };
        let (history, offset, rows) = live.term.scroll_state();
        match d.phase {
            Phase::Down => {
                d.data = offset as f32;
                self.terminal.bar_held = true;
            }
            Phase::Move => {
                let track = rows as f32 * CELL_H;
                let (_, slider) = crate::view::terminal_bar::slider(track, history, offset, rows);
                let per_line = ((track - slider) / history.max(1) as f32).max(f32::EPSILON);
                let back = d.data - d.dy / per_line;
                live.term.scroll_to_offset(back.round().clamp(0.0, history as f32) as usize);
            }
            Phase::Up => {
                self.terminal.bar_held = false;
                self.terminal.bar_seen = self.clock;
            }
        }
    }

    /// A press on the empty track (or a drag that began there): the slider centres on the pointer and follows it.
    pub fn terminal_track_drag(&mut self, d: &mut Drag) {
        let Some(live) = self.terminal.live.as_ref() else { return };
        let (history, _, rows) = live.term.scroll_state();
        self.terminal.bar_held = d.phase != Phase::Up;
        self.terminal.bar_seen = self.clock;
        if d.phase != Phase::Up {
            live.term.scroll_to_offset(crate::view::terminal_bar::offset_at(d.y, rows as f32 * CELL_H, history, rows));
        }
    }

    /// The pointer came over the box or left it.
    pub fn terminal_hover(&mut self, inside: bool) {
        self.terminal.bar_seen = self.clock;
        self.terminal.hover = inside;
        if !inside {
            self.terminal.link = None;
        }
    }

    /// The pointer moved over the box: the link under it, if any, is underlined and clickable (13.75, 13.76).
    pub fn terminal_pointer(&mut self, x: f32, y: f32) {
        let Some(live) = self.terminal.live.as_ref() else { return };
        // A program that asked for the mouse owns the pointer: no link under it, and plain motion goes to the program.
        if self.mouse_taken() {
            self.terminal.link = None;
            self.report_motion(x, y);
            return;
        }
        let (cols, rows) = live.term.size();
        let (cx, cy) = ((x - PAD[3]) / CELL_W, (y - PAD[0]) / CELL_H);
        let link =
            (cx >= 0.0 && cy >= 0.0 && cx < cols as f32 && cy < rows as f32).then(|| live.term.link_at(cy as usize, cx as usize)).flatten();
        if link != self.terminal.link {
            self.terminal.link = link;
        }
    }

    /// A click on a link opens it in the system browser (`window.open(uri, "_blank", "noopener,noreferrer")`).
    pub fn open_terminal_link(&mut self, tx: &mut Tx) {
        if let Some(link) = self.terminal.link.as_ref() {
            let _ = tx.open_url(&link.url);
        }
    }
}
