//! The rows on screen, read from the terminal's grid into Rune's `GridRow`s. A row whose cells
//! didn't change keeps its `Rc`, so Rune keeps its shaped line and last drawing.
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::{Term, TermMode};
use alacritty_terminal::vte::ansi::{Color as Ansi, CursorShape, NamedColor};
use rune::tok::TextStyle;
use rune::{Color, GridRow, Span};

/// Fleet's terminal at its default size: 12 px JetBrains Mono, line height 1.2 (`TerminalView.vue`).
pub const FONT: f32 = 12.0;
pub const LINE: f32 = 14.4;

pub fn style() -> TextStyle {
    TextStyle::new(FONT, 400.0).mono().lh(LINE)
}

/// A cell's width: the mono face's advance, measured once.
pub fn cell_w() -> f32 {
    thread_local!(static W: f32 = rune::text::advance(&"0".repeat(100), style()) / 100.0);
    W.with(|w| *w)
}

/// Whether `c` must be pinned to its column: drawn by another face, or wider than a cell.
fn pinned(c: char) -> bool {
    thread_local!(static SEEN: RefCell<HashMap<char, bool>> = RefCell::new(HashMap::new()));
    c > '\u{7e}'
        && SEEN.with(|m| *m.borrow_mut().entry(c).or_insert_with(|| (rune::text::advance(&c.to_string(), style()) - cell_w()).abs() > 0.05))
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    let d = |v: u8| rune::color::srgb_decode(v as f32 / 255.0);
    Color { r: d(r), g: d(g), b: d(b), a: 1.0 }
}

/// CSS's `color-mix(in srgb, a, b t)`: mixed in sRGB, not in the linear light `Color` holds.
fn mix(a: Color, b: Color, t: f32) -> Color {
    use rune::color::{srgb_decode as dec, srgb_encode as enc};
    let m = |x: f32, y: f32| dec(enc(x) + (enc(y) - enc(x)) * t);
    Color { r: m(a.r, b.r), g: m(a.g, b.g), b: m(a.b, b.b), a: 1.0 }
}

fn hex(s: &str) -> Color {
    rune::color::hex(s).expect("a colour literal")
}

/// Fleet's `terminalTheme()` (`client/src/lib/terminal-theme.ts`) for its dark and light themes:
/// red, green, yellow, blue and magenta from the status colours, the rest from its ANSI palette.
pub struct Palette {
    pub fg: Color,
    /// `--terminal-bg`: `--panel-bg` with 4% of `--text` (`TerminalDrawer.vue`).
    pub bg: Color,
    pub selection: Color,
    /// The spike's status line, a step off the background.
    pub bar: Color,
    ansi: [Color; 16],
}

impl Palette {
    pub fn new(dark: bool) -> Palette {
        let (ansi, text, panel, accent) = if dark {
            (
                [
                    "#3a3a44", "#ef4444", "#22c55e", "#f59e0b", "#3b82f6", "#8b5cf6", "#67e8f9", "#d6d6dd", "#6b6b76", "#fca5a5",
                    "#86efac", "#fde68a", "#c7d2fe", "#ddd6fe", "#a5f3fc", "#f5f5f7",
                ],
                "#e8e8ec",
                "#141418",
                "#6366f1",
            )
        } else {
            (
                [
                    "#1a1918", "#dc2626", "#16a34a", "#d97706", "#2563eb", "#7c3aed", "#0e7490", "#8a857f", "#6e6a65", "#dc2626",
                    "#16a34a", "#b45309", "#2563eb", "#7c3aed", "#0891b2", "#34322f",
                ],
                "#1A1918",
                "#FFFFFF",
                "#5B6EC7",
            )
        };
        let (text, panel) = (hex(text), hex(panel));
        Palette {
            fg: text,
            bg: mix(panel, text, 0.04),
            selection: mix(panel, hex(accent), 0.28),
            bar: mix(panel, text, 0.1),
            ansi: ansi.map(hex),
        }
    }

    fn indexed(&self, i: u8) -> Color {
        match i {
            0..16 => self.ansi[i as usize],
            16..232 => {
                let v = |n: u8| if n == 0 { 0 } else { 55 + n * 40 };
                let n = i - 16;
                rgb(v(n / 36), v(n / 6 % 6), v(n % 6))
            }
            _ => {
                let g = 8 + (i - 232) * 10;
                rgb(g, g, g)
            }
        }
    }

    /// A cell colour: what the program set, or what Fleet's theme says for a named one.
    fn resolve(&self, c: Ansi, colors: &Colors, bold: bool) -> Color {
        let from = |i: usize| colors[i].map(|c| rgb(c.r, c.g, c.b));
        match c {
            Ansi::Spec(c) => rgb(c.r, c.g, c.b),
            Ansi::Indexed(i) => from(i as usize).unwrap_or_else(|| self.indexed(i)),
            Ansi::Named(n) => {
                let i = n as usize;
                // xterm.js draws bold text in the bright colours by default.
                let i = if bold && i < 8 { i + 8 } else { i };
                from(i).unwrap_or(match n {
                    NamedColor::Foreground | NamedColor::BrightForeground | NamedColor::Cursor => self.fg,
                    NamedColor::Background => self.bg,
                    NamedColor::DimForeground => self.fg.alpha(0.5),
                    _ if i < 16 => self.ansi[i],
                    _ => self.ansi[(i - NamedColor::DimBlack as usize) % 8].alpha(0.5),
                })
            }
        }
    }
}

pub struct Cursor {
    pub col: usize,
    pub line: usize,
    pub shape: CursorShape,
    pub ch: char,
    pub wide: bool,
}

/// What the view draws: the rows on screen and where the cursor is.
#[derive(Default)]
pub struct Screen {
    pub rows: Vec<Rc<GridRow>>,
    keys: Vec<u64>,
    pub cursor: Option<Cursor>,
    pub mode: TermMode,
    pub offset: usize,
    pub history: usize,
    /// Rows built (not reused) by the last read, and how long the read took.
    pub built: usize,
    pub read_us: u128,
}

impl Screen {
    /// Reads the rows on screen from `term`, building only rows that changed.
    pub fn read<T: alacritty_terminal::event::EventListener>(&mut self, term: &Term<T>, pal: &Palette, dark: bool) {
        let start = std::time::Instant::now();
        let content = term.renderable_content();
        let (offset, sel) = (content.display_offset, content.selection);
        let grid = term.grid();
        let (lines, cols) = (grid.screen_lines(), grid.columns());
        self.rows.resize_with(lines, || Rc::new(GridRow::new("", Vec::new(), style())));
        self.keys.resize(lines, 0);
        self.built = 0;
        for v in 0..lines {
            let line = Line(v as i32 - offset as i32);
            let row = &grid[line];
            let selected = |c: usize| sel.is_some_and(|s| s.contains(Point::new(line, Column(c))));
            let mut h = std::hash::DefaultHasher::new();
            (dark, cols).hash(&mut h);
            for c in 0..cols {
                let cell = &row[Column(c)];
                (cell.c, cell.flags.bits(), code(cell.fg), code(cell.bg), cell.zerowidth(), selected(c)).hash(&mut h);
            }
            let key = h.finish();
            if self.keys[v] != key {
                self.keys[v] = key;
                self.rows[v] = Rc::new(build_row((0..cols).map(|c| &row[Column(c)]), term.colors(), pal, &selected));
                self.built += 1;
            }
        }
        let cur = content.cursor;
        self.cursor = (cur.shape != CursorShape::Hidden && offset == 0 && cur.point.line.0 >= 0).then(|| {
            let cell = &grid[cur.point];
            Cursor {
                col: cur.point.column.0,
                line: cur.point.line.0 as usize,
                shape: cur.shape,
                ch: cell.c,
                wide: cell.flags.contains(Flags::WIDE_CHAR),
            }
        });
        self.mode = content.mode;
        self.offset = offset;
        self.history = grid.history_size();
        self.read_us = start.elapsed().as_micros();
    }
}

/// A colour as a number, for the row's hash (vte's colours don't implement `Hash`).
fn code(c: Ansi) -> u32 {
    match c {
        Ansi::Named(n) => n as u32,
        Ansi::Indexed(i) => 0x1000 | i as u32,
        Ansi::Spec(c) => 0x100_0000 | (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32,
    }
}

/// One row: narrow cells as one line of text with colour spans, wide and other-face cells pinned
/// to their columns, and background runs as fills.
fn build_row<'a>(cells: impl Iterator<Item = &'a Cell>, colors: &Colors, pal: &Palette, selected: &dyn Fn(usize) -> bool) -> GridRow {
    let (mut text, mut spans, mut fills, mut pins) = (String::new(), Vec::<Span>::new(), Vec::new(), Vec::new());
    let mut fill: Option<(u16, u16, Color)> = None;
    let mut wraps = false;
    for (col, cell) in cells.enumerate() {
        let flags = cell.flags;
        wraps = flags.contains(Flags::WRAPLINE);
        let bold = flags.intersects(Flags::BOLD);
        let (mut fg, mut bg) = (pal.resolve(cell.fg, colors, bold), pal.resolve(cell.bg, colors, false));
        if flags.contains(Flags::INVERSE) {
            std::mem::swap(&mut fg, &mut bg);
        }
        if flags.contains(Flags::DIM) {
            fg = fg.alpha(0.5);
        }
        let bg = if selected(col) { Some(pal.selection) } else { (bg != pal.bg).then_some(bg) };
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
    for (col, width, glyph, st, fg) in pins {
        row = row.cell(col, width, glyph, st, fg);
    }
    row
}
