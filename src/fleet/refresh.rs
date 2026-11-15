//! When the session list is fetched again (`use-sessions.ts`, `lib/machine-feed.ts`). Fleet's client polls every 15 s
//! while the page is visible and fetches again when it becomes visible. Create, archive, rename and delete have no
//! event handler there; this app also fetches 400 ms after any `sessions` event that no row patch explains, which
//! is quicker than the poll and the same truth. Pure: the link thread asks it what is due and when.
use std::time::{Duration, Instant};

/// `DEFAULT_POLL_INTERVAL_MS`.
pub const POLL: Duration = Duration::from_secs(15);
/// How long events are let to pile up before one fetch (the machine feed's `scheduleRefresh` waits 300 ms).
pub const DEBOUNCE: Duration = Duration::from_millis(400);

pub struct Refresh {
    /// The first unexplained event since the last fetch.
    dirty_since: Option<Instant>,
    /// When the poll is next due.
    poll_at: Instant,
    visible: bool,
    /// Fetch at the next `due`, whatever else: the window came back.
    now: bool,
}

impl Refresh {
    pub fn new(now: Instant) -> Refresh {
        Refresh { dirty_since: None, poll_at: now + POLL, visible: true, now: false }
    }

    /// A `sessions` event the list can't patch itself (a session made, renamed, archived, removed).
    pub fn event(&mut self, now: Instant) {
        self.dirty_since.get_or_insert(now);
    }

    /// The window came back on screen or got the keyboard: `visibilitychange`.
    pub fn window_back(&mut self) {
        self.now = true;
    }

    pub fn set_visible(&mut self, visible: bool, now: Instant) {
        if visible && !self.visible {
            self.now = true;
        }
        if visible != self.visible {
            self.poll_at = now + POLL;
        }
        self.visible = visible;
    }

    /// Whether to fetch the list now. Answering yes starts the next wait.
    pub fn due(&mut self, now: Instant) -> bool {
        let debounced = self.dirty_since.is_some_and(|since| now.duration_since(since) >= DEBOUNCE);
        let polled = self.visible && now >= self.poll_at;
        if !(debounced || polled || self.now) {
            return false;
        }
        self.dirty_since = None;
        self.now = false;
        self.poll_at = now + POLL;
        true
    }
}
