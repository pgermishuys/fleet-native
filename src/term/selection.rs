//! Selecting in the terminal (component 13, piece F): what xterm.js does with the mouse, kept in alacritty's own `Selection`.
//! Points come in as the view's cells (column, row on screen) and go out as text, as the line numbers Fleet's "Add lines to
//! message" chip carries, and as where the bubble goes.
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::viewport_to_point;

use super::Terminal;

/// How far a press reaches: one character on a drag, a word on a double click, a line on a triple (xterm).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Chars,
    Word,
    Line,
}

/// `selectedLines` in `TerminalView.vue`: whole lines, numbered from the top of the scrollback starting at 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedLines {
    pub from: usize,
    pub to: usize,
    pub text: String,
}

/// Where the selection is on screen, for the bubble. Rows count from the top of the screen and can be off it (negative above,
/// past the last row below) while the view is scrolled; `end_x` is xterm's exclusive end column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub first_row: i32,
    pub last_row: i32,
    pub end_x: usize,
}

impl Terminal {
    /// A press at a cell. `right` says the pointer is in the cell's right half, which is what decides whether a drag that
    /// ends on a cell includes it.
    pub fn select_start(&self, pick: Pick, col: usize, row: usize, right: bool) {
        let mut inner = self.lock();
        let at = at_cell(&inner.term, col, row);
        let ty = match pick {
            Pick::Chars => SelectionType::Simple,
            Pick::Word => SelectionType::Semantic,
            Pick::Line => SelectionType::Lines,
        };
        inner.term.selection = Some(Selection::new(ty, at, side(right)));
    }

    /// The pointer moved, button down.
    pub fn select_to(&self, col: usize, row: usize, right: bool) {
        let mut inner = self.lock();
        let at = at_cell(&inner.term, col, row);
        if let Some(s) = inner.term.selection.as_mut() {
            s.update(at, side(right));
        }
    }

    pub fn clear_selection(&self) {
        self.lock().term.selection = None;
    }

    /// Something is selected (a click that never moved selects nothing).
    pub fn has_selection(&self) -> bool {
        self.read(|t| t.selection.as_ref().and_then(|s| s.to_range(t)).is_some())
    }

    /// `getSelection()`: the selected text, rows joined by a line break unless one wraps onto the next.
    pub fn selected_text(&self) -> Option<String> {
        // A line selection ends in a line break (alacritty's); xterm's `getSelection` doesn't.
        self.read(|t| t.selection_to_string()).map(|s| s.trim_end_matches('\n').to_string()).filter(|s| !s.is_empty())
    }

    /// Selects `from`..=`to` as the drag from a start cell to an end cell would, for tests and for "select all" later.
    #[cfg(test)]
    pub fn select(&self, from: (usize, usize), to: (usize, usize)) {
        self.select_start(Pick::Chars, from.0, from.1, false);
        self.select_to(to.0, to.1, true);
    }

    /// `selectedLines`: the rows the selection touches as whole lines, and where they are in the scrollback. A row that
    /// continues a wrapped line is joined onto it; a selection that ends at the very start of a row leaves that row out (the
    /// range alacritty gives already ends on the row before); trailing blanks go, and nothing but blanks gives none.
    pub fn selected_lines(&self) -> Option<SelectedLines> {
        self.read(|t| {
            let range = t.selection.as_ref().and_then(|s| s.to_range(t))?;
            let grid = t.grid();
            let (first, last) = (range.start.line.0, range.end.line.0);
            let cols = grid.columns();
            let mut text = String::new();
            for l in first..=last {
                let row = &grid[Line(l)];
                if l > first && !grid[Line(l - 1)][Column(cols - 1)].flags.contains(Flags::WRAPLINE) {
                    text.push('\n');
                }
                let continues = row[Column(cols - 1)].flags.contains(Flags::WRAPLINE);
                let mut line = String::new();
                for c in 0..cols {
                    let cell = &row[Column(c)];
                    if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                        continue;
                    }
                    line.push(cell.c);
                    line.extend(cell.zerowidth().into_iter().flatten());
                }
                // `translateToString(true)`: a row that wraps is full to its last cell, so it is kept whole.
                text.push_str(if continues { &line } else { line.trim_end() });
            }
            let text = text.trim_end().to_string();
            let top = grid.history_size() as i32;
            (!text.trim().is_empty()).then(|| SelectedLines { from: (first + top) as usize + 1, to: (last + top) as usize + 1, text })
        })
    }

    /// Where the selection is in the view, for placing the bubble.
    pub fn selection_span(&self) -> Option<Span> {
        self.read(|t| {
            let range = t.selection.as_ref().and_then(|s| s.to_range(t))?;
            let offset = t.grid().display_offset() as i32;
            Some(Span { first_row: range.start.line.0 + offset, last_row: range.end.line.0 + offset, end_x: range.end.column.0 + 1 })
        })
    }
}

fn side(right: bool) -> Side {
    if right { Side::Right } else { Side::Left }
}

/// The grid point under a cell of the view, which may be scrolled back.
fn at_cell<T>(term: &alacritty_terminal::term::Term<T>, col: usize, row: usize) -> Point {
    let row = row.min(term.screen_lines().saturating_sub(1));
    let col = col.min(term.columns().saturating_sub(1));
    viewport_to_point(term.grid().display_offset(), Point::new(row, Column(col)))
}
