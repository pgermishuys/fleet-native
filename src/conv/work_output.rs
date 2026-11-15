//! What the output under a background row reads and keeps (`components/session/WorkOutput.vue`): the tail of a work
//! item's output, read a page at a time from where the last read stopped. No Rune and no I/O in here: the caller does
//! the requests and hands each answer to `Reader::page`.
//!
//! Offsets are bytes, the cap on what is kept is characters, as in Fleet; the cut is always on a character boundary.

/// How much it keeps: the tail, as a terminal would show it.
pub const TAIL_CHARS: usize = 64 * 1024;
/// Where a long output starts reading, from its end, the first time.
pub const TAIL_BYTES: u64 = 32 * 1024;
/// How often it asks again while the work runs.
pub const REFRESH_MS: i64 = 2_000;
/// Pages read in one go before waiting for the next refresh.
pub const MAX_PAGES: usize = 8;

/// One answer of `GET /api/sessions/{id}/work/{item}/output?offset=N`.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub output: String,
    pub next_offset: u64,
    pub size: u64,
    pub truncated: bool,
}

/// What to do after a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    /// Ask for the page at this offset.
    Fetch(u64),
    /// This read is over; ask again in `REFRESH_MS` while the work runs.
    Done,
}

#[derive(Debug, Default)]
pub struct Reader {
    /// Which opening of the panel this is: an answer to an earlier one (closed, opened again) is dropped.
    pub epoch: u64,
    pub text: String,
    /// The note to show when nothing was read yet (`error`).
    pub error: Option<String>,
    pub loading: bool,
    /// Whether what shows starts after the output's start: the harness dropped it, or it was skipped for the tail.
    pub clipped: bool,
    /// A request is out.
    pub in_flight: bool,
    /// When the next refresh is due, while the work runs.
    pub next_at: Option<i64>,
    /// Whether the work ran at the last look, so its end gets one last read.
    pub was_running: bool,
    offset: u64,
    started: bool,
    pages: usize,
}

impl Reader {
    pub fn new(epoch: u64, running: bool) -> Reader {
        Reader { epoch, loading: true, was_running: running, ..Default::default() }
    }

    /// A reader that has already read `text` (a scene).
    pub fn shown(epoch: u64, text: &str, clipped: bool) -> Reader {
        Reader { epoch, text: text.into(), clipped, ..Default::default() }
    }

    /// Starts a read: the offset to ask for first.
    pub fn begin(&mut self) -> u64 {
        self.pages = 0;
        self.in_flight = true;
        self.next_at = None;
        self.offset
    }

    fn append(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }
        self.text.push_str(chunk);
        let chars = self.text.chars().count();
        if chars > TAIL_CHARS {
            self.clipped = true;
            let cut = self.text.char_indices().nth(chars - TAIL_CHARS).map_or(0, |(at, _)| at);
            self.text.drain(..cut);
        }
    }

    fn end(&mut self) -> Next {
        self.loading = false;
        self.in_flight = false;
        Next::Done
    }

    /// `read`'s loop body for one answer. A failure keeps what was read (work that ended may have taken its output with
    /// it) and is a note only while nothing was.
    pub fn page(&mut self, result: Result<Page, String>) -> Next {
        let page = match result {
            Ok(page) => page,
            Err(why) => {
                self.error = if self.text.is_empty() { Some(why) } else { None };
                return self.end();
            }
        };
        self.error = None;
        if page.truncated {
            self.clipped = true;
        }
        self.pages += 1;
        // A long output the first time: skip to its tail rather than reading it all.
        if !self.started && page.size.saturating_sub(page.next_offset) > TAIL_BYTES {
            self.started = true;
            self.clipped = true;
            self.offset = page.size - TAIL_BYTES;
            return if self.pages < MAX_PAGES { Next::Fetch(self.offset) } else { self.end() };
        }
        if !self.started && self.offset > 0 {
            // Started mid-way: drop the part line the tail starts in. (Never reached after the jump above, which sets
            // `started` first: Fleet's own order, kept so both read the same.)
            let rest = page.output.find('\n').map_or("", |at| &page.output[at + 1..]);
            self.append(rest);
        } else {
            self.append(&page.output);
        }
        self.started = true;
        self.offset = self.offset.max(page.next_offset);
        if page.next_offset >= page.size || page.output.is_empty() || self.pages >= MAX_PAGES {
            return self.end();
        }
        Next::Fetch(self.offset)
    }
}
