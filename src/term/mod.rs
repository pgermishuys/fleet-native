//! The terminal's state, kept by `alacritty_terminal` (docs/PORT.md, decision 2). Nothing else in the app names its types:
//! the rest asks this wrapper to take bytes, change size, give up what the parser wants to say back, and show rows.
//!
//! A `Terminal` is shared between the connection's thread, which feeds it output, and the UI thread, which reads the rows
//! on screen; each holds the lock for one chunk or one read.
pub mod fit;
pub mod keys;
pub mod palette;
pub mod screen;

use std::sync::{Arc, Mutex, MutexGuard};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config, Term, TermMode};

pub use alacritty_terminal::term::TermMode as Mode;
/// The modes the view asks about.
pub const APP_CURSOR: Mode = TermMode::APP_CURSOR;
pub const BRACKETED_PASTE: Mode = TermMode::BRACKETED_PASTE;
use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};

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

    /// Reads the grid and takes its damage: what changed since the last read that took it (`Screen::read`).
    pub fn read_damage<R>(&self, f: impl FnOnce(&mut Term<Listener>) -> R) -> R {
        f(&mut self.lock().term)
    }
}

impl Inner {
    fn new(cols: u16, rows: u16, answers: &Listener) -> Inner {
        let config = Config { scrolling_history: SCROLLBACK, ..Config::default() };
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
