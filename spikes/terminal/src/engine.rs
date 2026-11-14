//! The terminal's state (alacritty_terminal's `Term`) and where its bytes come from: a
//! recorded stream, a flood, or a local shell. Bytes are parsed on a thread of their own; the UI
//! thread only locks the grid to read the rows on screen (`screen.rs`).
//!
//! Fleet's client would feed the bytes from its terminal socket the way `spawn_feeder` does:
//! the pty is the server's, so the app needs none (the local shell is for the spike only).
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};

/// What the parsing thread tells the UI thread.
#[derive(Default)]
pub struct Shared {
    /// The grid changed since the UI last read it.
    pub dirty: AtomicBool,
    /// Bytes parsed so far, for the rate in the status line.
    pub bytes: AtomicU64,
    pub exited: AtomicBool,
    pub title: Mutex<String>,
    waker: Mutex<Option<rune::Waker>>,
    /// Where the terminal's own answers go (cursor position, device attributes): the shell.
    answer: Mutex<Option<EventLoopSender>>,
}

impl Shared {
    /// One wake per change the UI hasn't read yet, so a flood costs one redraw a frame.
    pub fn wake(&self) {
        if !self.dirty.swap(true, Ordering::AcqRel)
            && let Some(w) = &*self.waker.lock().unwrap()
        {
            w();
        }
    }
}

#[derive(Clone)]
pub struct Listener(pub Arc<Shared>);

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        match event {
            Event::Wakeup => self.0.wake(),
            Event::PtyWrite(text) => {
                if let Some(tx) = &*self.0.answer.lock().unwrap() {
                    let _ = tx.send(Msg::Input(Cow::Owned(text.into_bytes())));
                }
            }
            Event::Title(t) => {
                *self.0.title.lock().unwrap() = t;
                self.0.wake();
            }
            Event::ChildExit(_) | Event::Exit => {
                self.0.exited.store(true, Ordering::Release);
                self.0.wake();
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy)]
pub struct Size {
    pub cols: usize,
    pub lines: usize,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.lines
    }
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

pub struct Engine {
    pub term: Arc<FairMutex<Term<Listener>>>,
    pub shared: Arc<Shared>,
    pub size: Size,
    shell: Option<EventLoopSender>,
}

impl Engine {
    pub fn new(size: Size) -> Engine {
        let shared = Arc::new(Shared::default());
        // Fleet's xterm keeps 5,000 lines (`TerminalView.vue`), as the server's replay does.
        let config = Config { scrolling_history: 5000, ..Config::default() };
        let term = Term::new(config, &size, Listener(shared.clone()));
        Engine { term: Arc::new(FairMutex::new(term)), shared, size, shell: None }
    }

    pub fn set_waker(&self, waker: rune::Waker) {
        *self.shared.waker.lock().unwrap() = Some(waker);
    }

    /// Parses `bytes` now, on this thread: for scenes and tests.
    pub fn feed_now(&self, bytes: &[u8]) {
        let mut parser: Processor<StdSyncHandler> = Processor::new();
        parser.advance(&mut *self.term.lock(), bytes);
        self.shared.bytes.fetch_add(bytes.len() as u64, Ordering::Relaxed);
        self.shared.wake();
    }

    /// Parses chunks from `source` on a thread of its own, as the socket's reader would. The
    /// lock is held for one chunk at a time, so the UI can read the grid between chunks.
    pub fn spawn_feeder(&self, source: impl Iterator<Item = Vec<u8>> + Send + 'static) {
        let (term, shared) = (self.term.clone(), self.shared.clone());
        std::thread::spawn(move || {
            let mut parser: Processor<StdSyncHandler> = Processor::new();
            for chunk in source {
                parser.advance(&mut *term.lock(), &chunk);
                shared.bytes.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                shared.wake();
            }
        });
    }

    /// A local shell in a pty, read on alacritty's own thread. The spike's stand-in for Fleet's
    /// socket; Fleet's server sets the same variables for its shells (`TerminalEnvironment.cs`).
    pub fn spawn_shell(&mut self, cell: (f32, f32)) -> std::io::Result<()> {
        let env = HashMap::from(
            [("TERM", "xterm-256color"), ("COLORTERM", "truecolor"), ("TERM_PROGRAM", "WeaveFleet")].map(|(k, v)| (k.into(), v.into())),
        );
        let options = tty::Options { env, ..tty::Options::default() };
        let pty = tty::new(&options, self.window_size(cell), 0)?;
        let event_loop = EventLoop::new(self.term.clone(), Listener(self.shared.clone()), pty, false, false)?;
        let tx = event_loop.channel();
        *self.shared.answer.lock().unwrap() = Some(tx.clone());
        self.shell = Some(tx);
        event_loop.spawn();
        Ok(())
    }

    pub fn write(&self, bytes: Vec<u8>) {
        if let Some(tx) = &self.shell {
            let _ = tx.send(Msg::Input(Cow::Owned(bytes)));
        }
    }

    pub fn resize(&mut self, size: Size, cell: (f32, f32)) {
        self.size = size;
        self.term.lock().resize(size);
        if let Some(tx) = &self.shell {
            let _ = tx.send(Msg::Resize(self.window_size(cell)));
        }
    }

    fn window_size(&self, (w, h): (f32, f32)) -> WindowSize {
        WindowSize { num_lines: self.size.lines as u16, num_cols: self.size.cols as u16, cell_width: w as u16, cell_height: h as u16 }
    }
}

/// Synthetic output as fast as the parser takes it: the recorded streams and plain lines, in
/// 64 KiB chunks, until `total` bytes.
pub fn flood(total: u64) -> impl Iterator<Item = Vec<u8>> + Send {
    let mut pattern = Vec::new();
    for s in crate::STREAMS {
        pattern.extend_from_slice(s.1);
        pattern.extend_from_slice(b"\x1b[0m\r\n");
    }
    for i in 0..400 {
        pattern
            .extend_from_slice(format!("\x1b[32m{i:>5}\x1b[0m y y y y y y y y y y \x1b[1;34mlantern\x1b[0m wick oil glow\r\n").as_bytes());
    }
    let mut sent = 0u64;
    let mut at = 0usize;
    std::iter::from_fn(move || {
        if sent >= total {
            return None;
        }
        let mut chunk = Vec::with_capacity(65536);
        while chunk.len() < 65536 {
            let take = (65536 - chunk.len()).min(pattern.len() - at);
            chunk.extend_from_slice(&pattern[at..at + take]);
            at = (at + take) % pattern.len();
        }
        sent += chunk.len() as u64;
        Some(chunk)
    })
}
