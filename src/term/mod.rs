//! The terminal's state, kept by `alacritty_terminal` (docs/PORT.md, decision 2). Nothing else in the app names its types:
//! the rest asks this wrapper to take bytes, change size, give up what the parser wants to say back, and show rows.
//!
//! A `Terminal` is shared between the connection's thread, which feeds it output, and the UI thread, which reads the rows
//! on screen; each holds the lock for one chunk or one read.
pub mod cells;
pub mod fit;
pub mod keys;
pub mod links;
pub mod palette;
pub mod screen;
pub mod selection;

use std::sync::{Arc, Mutex, MutexGuard};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config, Term, TermMode};

pub use alacritty_terminal::term::TermMode as Mode;
/// The modes the view asks about.
pub const APP_CURSOR: Mode = TermMode::APP_CURSOR;
pub const BRACKETED_PASTE: Mode = TermMode::BRACKETED_PASTE;
use alacritty_terminal::vte::ansi::{CursorShape, CursorStyle, Processor, StdSyncHandler};

/// The scrollback Fleet's xterm keeps (`TerminalView.vue`), as the server's replay does.
pub const SCROLLBACK: usize = 5000;

/// What the parser wants sent to the shell: the answer to a device-attributes or cursor-position query. xterm.js answers
/// those through `onData`, which Fleet writes to the socket.
#[derive(Clone, Default)]
pub struct Listener(Arc<Mutex<Vec<u8>>>);

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event {
            self.0.lock().unwrap().extend_from_slice(text.as_bytes());
        }
    }
}

struct Size {
    cols: usize,
    rows: usize,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

struct Inner {
    term: Term<Listener>,
    parser: Processor<StdSyncHandler>,
}

pub struct Terminal {
    inner: Mutex<Inner>,
    answers: Listener,
}

impl Terminal {
    pub fn new(cols: u16, rows: u16) -> Terminal {
        let answers = Listener::default();
        Terminal { inner: Mutex::new(Inner::new(cols, rows, &answers)), answers }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A reader that panicked left the grid whole; keep drawing it.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Parses output from the shell.
    pub fn feed(&self, bytes: &[u8]) {
        let mut inner = self.lock();
        let Inner { term, parser } = &mut *inner;
        parser.advance(term, bytes);
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        self.lock().term.resize(Size { cols: cols as usize, rows: rows as usize });
    }

    /// A fresh terminal of the same size: every (re)connect starts here, and the scrollback replays into it.
    pub fn reset(&self) {
        let mut inner = self.lock();
        let (cols, rows) = (inner.term.columns() as u16, inner.term.screen_lines() as u16);
        *inner = Inner::new(cols, rows, &self.answers);
        self.answers.0.lock().unwrap().clear();
    }

    /// xterm.js's `clear()` (the drawer's Clear): everything but the cursor's row goes, scrollback included, and that row
    /// moves to the top. Done with the escapes a shell could send: scroll up by the cursor's row, erase the history, go back up.
    pub fn clear(&self) {
        let row = self.read(|t| t.grid().cursor.point.line.0.max(0));
        if row == 0 {
            return self.feed(b"\x1b[3J");
        }
        self.feed(format!("\x1b[{row}S\x1b[3J\x1b[{row}A").as_bytes());
    }

    /// Shift+PageUp / Shift+PageDown (`scrollPages`): the view moves a page through the scrollback and the shell hears nothing.
    pub fn scroll_page(&self, up: bool) {
        use alacritty_terminal::grid::Scroll;
        self.lock().term.scroll_display(if up { Scroll::PageUp } else { Scroll::PageDown });
    }

    /// xterm's `scrollOnUserInput`: typing brings the view back to the live screen.
    pub fn scroll_to_bottom(&self) {
        self.lock().term.scroll_display(alacritty_terminal::grid::Scroll::Bottom);
    }

    /// The wheel and the scrollbar: moves the view `lines` back into the scrollback (negative: forward), stopping at either end.
    pub fn scroll_by(&self, lines: i32) {
        self.lock().term.scroll_display(alacritty_terminal::grid::Scroll::Delta(lines));
    }

    /// (lines of scrollback, lines the view is scrolled back, rows on screen): what the scrollbar draws.
    pub fn scroll_state(&self) -> (usize, usize, usize) {
        self.read(|t| (t.grid().history_size(), t.grid().display_offset(), t.screen_lines()))
    }

    /// The scrollbar's slider moved: the view is `offset` lines back.
    pub fn scroll_to_offset(&self, offset: usize) {
        let now = self.display_offset() as i32;
        self.scroll_by(offset as i32 - now);
    }

    /// How many lines the view is scrolled back from the live screen.
    pub fn display_offset(&self) -> usize {
        self.read(|t| t.grid().display_offset())
    }

    /// What the parser wants sent back to the shell since the last call (13.16).
    pub fn take_answers(&self) -> Vec<u8> {
        std::mem::take(&mut *self.answers.0.lock().unwrap())
    }

    pub fn mode(&self) -> TermMode {
        *self.lock().term.mode()
    }

    pub fn size(&self) -> (u16, u16) {
        let inner = self.lock();
        (inner.term.columns() as u16, inner.term.screen_lines() as u16)
    }

    /// Reads the grid.
    pub fn read<R>(&self, f: impl FnOnce(&Term<Listener>) -> R) -> R {
        f(&self.lock().term)
    }

    /// The link under a cell of the view (13.75, 13.76).
    pub fn link_at(&self, row: usize, col: usize) -> Option<links::Link> {
        self.read(|t| links::link_at(t, row, col))
    }

    /// Reads the grid and takes its damage: what changed since the last read that took it (`Screen::read`).
    pub fn read_damage<R>(&self, f: impl FnOnce(&mut Term<Listener>) -> R) -> R {
        f(&mut self.lock().term)
    }
}

impl Inner {
    fn new(cols: u16, rows: u16, answers: &Listener) -> Inner {
        // xterm.js's word separators (`wordSeparator`), for a double click.
        // `cursorBlink: true`: a cursor the shell hasn't set blinks, and one it asked to be steady doesn't.
        let default_cursor_style = CursorStyle { shape: CursorShape::Block, blinking: true };
        let config = Config {
            scrolling_history: SCROLLBACK,
            default_cursor_style,
            semantic_escape_chars: " ()[]{}',\"`".into(),
            ..Config::default()
        };
        let size = Size { cols: cols.max(1) as usize, rows: rows.max(1) as usize };
        Inner { term: Term::new(config, &size, answers.clone()), parser: Processor::new() }
    }
}

/// The text of the screen's rows with trailing spaces trimmed, for tests (a wide character's second cell is skipped, a
/// combining mark follows its letter).
#[cfg(test)]
pub fn text_of(t: &Terminal) -> Vec<String> {
    use alacritty_terminal::index::{Column, Line};
    use alacritty_terminal::term::cell::Flags;
    t.read(|term| {
        let grid = term.grid();
        (0..grid.screen_lines())
            .map(|l| {
                let mut line = String::new();
                for c in 0..grid.columns() {
                    let cell = &grid[Line(l as i32)][Column(c)];
                    if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                        continue;
                    }
                    line.push(cell.c);
                    line.extend(cell.zerowidth().into_iter().flatten());
                }
                line.trim_end().to_string()
            })
            .collect()
    })
}
