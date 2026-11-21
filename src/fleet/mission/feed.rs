//! One machine's feed as a state machine (component 23, piece C; `lib/machine-feed.ts`): when to read, and what the feed
//! says of itself. It holds no thread and no clock of its own: the caller says what happened (`event`, `hub_connected`,
//! `read_done`) and asks `due` what to do now, with `now` in seconds on the app's clock. What a read fetches and how it
//! goes out is `read.rs`; opening, closing and syncing feeds is `store.rs`.
//!
//! - A read at once (`start`), then the hub: every `sessions` event re-reads `FOLD` later, a burst being one read, and an
//!   event that comes during a read reads again when it ends.
//! - While the hub is down the feed reads every `POLL`; with a live hub it reads every `SAFETY` anyway, so a missed event
//!   doesn't leave the view wrong for long.
//! - `status`: `Connecting` until the first answer, `Live` with the hub, `Polling` without it, `Unreachable` when a read
//!   fails (the last read is kept by the caller, with the reason in `error`).
use super::FeedStatus;

/// A burst of events is folded into one read this long after the first.
pub const FOLD: f64 = 0.3;
/// While the hub is down.
pub const POLL: f64 = 15.0;
/// Even with a live hub (`SAFETY_REFRESH_MS`).
pub const SAFETY: f64 = 60.0;

#[derive(Clone, Debug, Default)]
pub struct Feed {
    pub status: FeedStatus,
    /// When the machine last answered, on the wall clock in ms; none until its first answer.
    pub last_heard_at: Option<i64>,
    /// Why the last read failed, when the machine said (a refusal); none for a machine that didn't answer at all.
    pub error: Option<String>,
    hub_up: bool,
    reading: bool,
    /// An event came during a read: read again once it ends, so the change isn't missed.
    read_again: bool,
    fold_at: Option<f64>,
    poll_at: Option<f64>,
    safety_at: Option<f64>,
    stopped: bool,
}

impl Feed {
    /// A feed that has not started: it reads nothing until `start`.
    pub fn new() -> Feed {
        Feed { stopped: true, ..Feed::default() }
    }

    /// Begins: true means read now. The safety timer runs from here (`start`).
    pub fn start(&mut self, now: f64) -> bool {
        self.stopped = false;
        self.safety_at = Some(now + SAFETY);
        self.begin()
    }

    /// Ends: no timer is left, and nothing the feed is told afterwards makes it read.
    pub fn stop(&mut self) {
        self.stopped = true;
        self.fold_at = None;
        self.poll_at = None;
        self.safety_at = None;
        self.read_again = false;
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    pub fn is_reading(&self) -> bool {
        self.reading
    }

    /// Whether a read is waiting on a timer (for the caller's next wake).
    pub fn next_deadline(&self) -> Option<f64> {
        [self.fold_at, self.poll_at, self.safety_at].into_iter().flatten().reduce(f64::min)
    }

    /// A read is wanted now. Concurrent calls share one read (`refresh`): true only when none is under way.
    fn begin(&mut self) -> bool {
        if self.stopped || self.reading {
            return false;
        }
        self.reading = true;
        true
    }

    /// Reads now, as after an answer or a nudge: the card goes when the machine says the ask is gone (`refresh`).
    pub fn refresh(&mut self) -> bool {
        self.begin()
    }

    /// The hub is connected and subscribed to `sessions`. False when the feed was stopped meanwhile: nothing will stop the
    /// connection later, so the caller lets it go (`connect`).
    pub fn hub_connected(&mut self) -> bool {
        if self.stopped {
            return false;
        }
        self.hub_up = true;
        self.poll_at = None;
        if self.status != FeedStatus::Unreachable {
            self.status = FeedStatus::Live;
        }
        true
    }

    /// The hub won't connect, or closed: poll (`startPolling`).
    pub fn hub_lost(&mut self, now: f64) {
        self.hub_up = false;
        if self.stopped || self.poll_at.is_some() {
            return;
        }
        if self.status != FeedStatus::Unreachable {
            self.status = FeedStatus::Polling;
        }
        self.poll_at = Some(now + POLL);
    }

    /// The `sessions` topic said something changed: read soon, folding a burst into one read (`scheduleRefresh`).
    pub fn event(&mut self, now: f64) {
        if self.fold_at.is_none() && !self.stopped {
            self.fold_at = Some(now + FOLD);
        }
    }

    /// Whether a timer fell due and a read starts now.
    pub fn due(&mut self, now: f64) -> bool {
        if self.stopped {
            return false;
        }
        let mut read = false;
        if self.fold_at.is_some_and(|at| now >= at) {
            self.fold_at = None;
            if self.reading {
                self.read_again = true;
            } else {
                read = true;
            }
        }
        if self.poll_at.is_some_and(|at| now >= at) {
            self.poll_at = Some(now + POLL);
            read = true;
        }
        if self.safety_at.is_some_and(|at| now >= at) {
            self.safety_at = Some(now + SAFETY);
            read = true;
        }
        read && self.begin()
    }

    /// A read ended: `Ok` carries the wall clock of the answer, `Err` why the machine can't be heard (`None` for no answer
    /// at all). True when another read follows at once, for an event that came during this one.
    pub fn read_done(&mut self, result: Result<i64, Option<String>>) -> bool {
        self.reading = false;
        match result {
            Ok(heard) => {
                self.last_heard_at = Some(heard);
                self.error = None;
                self.status = if self.hub_up {
                    FeedStatus::Live
                } else if self.poll_at.is_some() || self.status == FeedStatus::Unreachable {
                    FeedStatus::Polling
                } else {
                    self.status
                };
            }
            Err(why) => {
                self.status = FeedStatus::Unreachable;
                self.error = why;
            }
        }
        if std::mem::take(&mut self.read_again) && !self.stopped {
            return self.begin();
        }
        false
    }
}
