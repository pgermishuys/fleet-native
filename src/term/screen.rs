//! The rows on screen, read from the terminal's grid into Rune's `GridRow`s. A row whose cells didn't change keeps its `Rc`,
//! so Rune keeps its shaped line and its last drawing (13.17).
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::selection::SelectionRange;
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::{Term, TermDamage, TermMode};
use alacritty_terminal::vte::ansi::{Color as Ansi, CursorShape, NamedColor};
use rune::tok::TextStyle;
use rune::{Color, GridRow, Span};

use super::fit::{CELL_H, CELL_W, FONT};
use super::palette::Palette;

/// JetBrains Mono at 12 px with the advance xterm floors it to: the face's own advance is a little over 7, so each
/// glyph is tracked back to 7 and a column is `CELL_W` wide, as in Fleet.
pub fn style() -> TextStyle {
    let mut s = TextStyle::new(FONT, 400.0).mono().lh(CELL_H);
    s.tracking = CELL_W - natural_advance();
    s
}

/// The face's own advance at `FONT`, measured once.
fn natural_advance() -> f32 {
    thread_local!(static W: f32 = rune::text::advance(&"0".repeat(100), TextStyle::new(FONT, 400.0).mono()) / 100.0);
    W.with(|w| *w)
}

/// Whether `c` must be pinned to its column: drawn by another face, or wider than a cell.
fn pinned(c: char) -> bool {
    thread_local!(static SEEN: RefCell<HashMap<char, bool>> = RefCell::new(HashMap::new()));
    c > '\u{7e}'
        && SEEN.with(|m| *m.borrow_mut().entry(c).or_insert_with(|| (rune::text::advance(&c.to_string(), style()) - CELL_W).abs() > 0.05))
}

/// Where the cursor is and what is under it.
pub struct Cursor {
    pub col: usize,
    pub line: usize,
    pub shape: CursorShape,
    pub ch: String,
    pub wide: bool,
}

/// What the view draws: the rows on screen and where the cursor is.
#[derive(Default)]
pub struct Screen {
    pub rows: Vec<Rc<GridRow>>,
    keys: Vec<u64>,
    pub cols: usize,
    pub cursor: Option<Cursor>,
    pub mode: TermMode,
    /// What the selection is painted with (13.8: the active or the inactive colour), or None to paint none. Set before each read.
    pub selection: Option<Color>,
    /// The columns each row had selected when it was last read.
    spans: Vec<Option<(u16, u16, u64)>>,
    /// Rows built (not reused) by the last read.
    pub built: usize,
    /// Rows whose cells were read and hashed by the last read: the rows alacritty said had changed, or all of them.
    pub scanned: usize,
    look: u64,
}

impl Screen {
    /// Reads the rows on screen from `term`, building only rows that changed. `look` says which palette the rows were
    /// built with: another one rebuilds them all. Rows alacritty's damage says are untouched aren't even looked at (13.17);
    /// reading takes the damage, so one `Screen` reads one terminal.
    pub fn read<T: EventListener>(&mut self, term: &mut Term<T>, pal: &Palette, bg: Color, look: u64) {
        let (lines, cols) = (term.screen_lines(), term.columns());
        let offset = term.grid().display_offset();
        // Which cells are selected, a row at a time. The selection isn't damage, so a row whose span changed is read again.
        let range = self.selection.and_then(|_| term.selection.as_ref().and_then(|s| s.to_range(term)));
        // The colour is part of what a selected row looks like: the other rows don't care which colour it is.
        let paint =
            self.selection.map_or(0, |c| [c.r, c.g, c.b].map(f32::to_bits).into_iter().fold(0u64, |h, b| h.rotate_left(7) ^ b as u64));
        let spans: Vec<Option<(u16, u16, u64)>> = (0..lines)
            .map(|v| range.as_ref().and_then(|r| span_of(r, v as i32 - offset as i32, cols)).map(|(s, e)| (s, e, paint)))
            .collect();
        let damaged = {
            let mut hit = vec![false; lines];
            let all = match term.damage() {
                TermDamage::Full => true,
                TermDamage::Partial(it) => {
                    for d in it {
                        if let Some(h) = hit.get_mut(d.line) {
                            *h = true;
                        }
                    }
                    false
                }
            };
            term.reset_damage();
            // Scrolled back, another palette or another size: every row may differ.
            let all = all || offset != 0 || look != self.look || cols != self.cols || self.rows.len() != lines;
            (!all).then_some(hit)
        };
        let content = term.renderable_content();
        let grid = term.grid();
        if self.cols != cols {
            self.keys.clear();
        }
        self.look = look;
        self.cols = cols;
        self.rows.resize_with(lines, || Rc::new(GridRow::new("", Vec::new(), style())));
        self.keys.resize(lines, 0);
        self.spans.resize(lines, None);
        self.built = 0;
        self.scanned = 0;
        for v in 0..lines {
            if self.keys[v] != 0 && self.spans[v] == spans[v] && damaged.as_ref().is_some_and(|d| !d[v]) {
                continue;
            }
            self.scanned += 1;
            let row = &grid[Line(v as i32 - offset as i32)];
            let mut h = std::hash::DefaultHasher::new();
            (look, cols, spans[v]).hash(&mut h);
            for c in 0..cols {
                let cell = &row[Column(c)];
                (cell.c, cell.flags.bits(), code(cell.fg), code(cell.bg), cell.zerowidth()).hash(&mut h);
            }
            let key = h.finish() | 1;
            self.spans[v] = spans[v];
            if self.keys[v] != key {
                self.keys[v] = key;
                let sel = spans[v].map(|(s, e, _)| (s, e)).zip(self.selection);
                self.rows[v] = Rc::new(build_row((0..cols).map(|c| &row[Column(c)]), term.colors(), pal, bg, sel));
                self.built += 1;
            }
        }
        let cur = content.cursor;
        self.cursor = (cur.shape != CursorShape::Hidden && offset == 0 && cur.point.line.0 >= 0).then(|| {
            let cell = &grid[cur.point];
            let mut ch = String::from(cell.c);
            ch.extend(cell.zerowidth().into_iter().flatten());
            Cursor {
                col: cur.point.column.0,
                line: cur.point.line.0 as usize,
                shape: cur.shape,
                ch,
                wide: cell.flags.contains(Flags::WIDE_CHAR),
            }
        });
        self.mode = content.mode;
    }
}

/// The columns of the selection on one line (grid line numbers: negative is scrollback), or None when it doesn't touch it.
fn span_of(r: &SelectionRange, line: i32, cols: usize) -> Option<(u16, u16)> {
    if line < r.start.line.0 || line > r.end.line.0 {
        return None;
    }
    let (s, e) = if r.is_block {
        (r.start.column.0.min(r.end.column.0), r.start.column.0.max(r.end.column.0) + 1)
    } else {
        (if line == r.start.line.0 { r.start.column.0 } else { 0 }, if line == r.end.line.0 { r.end.column.0 + 1 } else { cols })
    };
    (e > s).then_some((s as u16, e.min(cols) as u16))
}

/// A colour as a number, for the row's hash (vte's colours don't implement `Hash`).
fn code(c: Ansi) -> u32 {
    match c {
        Ansi::Named(n) => n as u32,
        Ansi::Indexed(i) => 0x1000 | i as u32,
        Ansi::Spec(c) => 0x100_0000 | (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32,
    }
}

/// A cell colour: what the program set, or what Fleet's theme says for a named one. `None` is the default background.
fn resolve(pal: &Palette, c: Ansi, colors: &Colors, bold: bool, fg_default: Color) -> Option<Color> {
    let from = |i: usize| colors[i].map(|c| pal.truecolor(c.r, c.g, c.b));
    match c {
        Ansi::Spec(c) => Some(pal.truecolor(c.r, c.g, c.b)),
        Ansi::Indexed(i) => Some(from(i as usize).unwrap_or_else(|| pal.indexed(i))),
        Ansi::Named(n) => {
            let i = n as usize;
            // xterm.js draws bold text in the bright colours by default (`drawBoldTextInBrightColors`).
            let i = if bold && i < 8 { i + 8 } else { i };
            match n {
                NamedColor::Background => None,
                NamedColor::Foreground | NamedColor::BrightForeground | NamedColor::Cursor => Some(fg_default),
                NamedColor::DimForeground => Some(fg_default.alpha(0.5)),
                _ if i < 16 => Some(from(i).unwrap_or_else(|| pal.ansi(i))),
                _ => Some(pal.ansi((i - NamedColor::DimBlack as usize) % 8).alpha(0.5)),
            }
        }
    }
}

/// One row: narrow cells as one line of text with colour spans, wide and other-face cells pinned to their columns, and
/// background runs as fills.
fn build_row<'a>(
    cells: impl Iterator<Item = &'a Cell>,
    colors: &Colors,
    pal: &Palette,
    bg_default: Color,
    selected: Option<((u16, u16), Color)>,
) -> GridRow {
    let (mut text, mut spans, mut fills, mut pins) = (String::new(), Vec::<Span>::new(), Vec::new(), Vec::new());
    let mut fill: Option<(u16, u16, Color)> = None;
    let mut wraps = false;
    for (col, cell) in cells.enumerate() {
        let flags = cell.flags;
        wraps = flags.contains(Flags::WRAPLINE);
        let bold = flags.intersects(Flags::BOLD);
        let mut fg = resolve(pal, cell.fg, colors, bold, pal.fg).unwrap_or(pal.fg);
        let mut bg = resolve(pal, cell.bg, colors, false, pal.fg);
        if flags.contains(Flags::INVERSE) {
            let swapped = bg.unwrap_or(bg_default);
            bg = Some(fg);
            fg = swapped;
        }
        if flags.contains(Flags::DIM) {
            fg = fg.alpha(0.5);
        }
        // Background runs: extend the open one while the colour holds.
        let c = col as u16;
        fill = match (fill, bg) {
            (Some((s, e, k)), Some(b)) if k == b && e == c => Some((s, c + 1, k)),
            (open, b) => {
                fills.extend(open);
                b.map(|b| (c, c + 1, b))
            }
        };
        if flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
            text.push(' ');
            continue;
        }
        let mut glyph = String::from(if flags.contains(Flags::HIDDEN) { ' ' } else { cell.c });
        glyph.extend(cell.zerowidth().into_iter().flatten());
        let mut st = style();
        if bold {
            st.weight = 600.0;
        }
        if flags.contains(Flags::ITALIC) {
            st = st.italic();
        }
        let wide = flags.contains(Flags::WIDE_CHAR);
        if wide || pinned(cell.c) || glyph.len() > cell.c.len_utf8() {
            pins.push((c, if wide { 2 } else { 1 }, glyph, st, fg));
            text.push(' ');
            continue;
        }
        let at = text.len();
        text.push_str(&glyph);
        let underline = flags.intersects(Flags::ALL_UNDERLINES);
        // Spans: extend the last one while the style holds.
        match spans.last_mut() {
            Some(s)
                if s.end as usize == at
                    && s.color == Some(fg)
                    && s.weight == Some(st.weight)
                    && s.italic == st.italic
                    && s.underline.is_some() == underline =>
            {
                s.end = text.len() as u32;
            }
            _ => {
                let mut s = Span::new(at, text.len()).color(fg).weight(st.weight);
                s.italic = st.italic;
                if underline {
                    s = s.underline(fg);
                }
                spans.push(s);
            }
        }
    }
    fills.extend(fill);
    let mut row = GridRow::new(text, spans, style()).wraps(wraps);
    for (s, e, k) in fills {
        row = row.fill(s, e, k);
    }
    // The selection is painted last: over the cell backgrounds, under the text.
    if let Some(((s, e), k)) = selected {
        row = row.fill(s, e, k);
    }
    for (col, width, glyph, st, fg) in pins {
        row = row.cell(col, width, glyph, st, fg);
    }
    row
}
