//! How often an Input tells its mod about typing (`ModInput.vue`, `onInput`): the first change at once, then at most
//! `INPUT_CHANGES_PER_SECOND` a second. Changes inside the window coalesce into the latest, which goes when the window ends; the
//! last value always goes, and a field that goes away sends what it still holds first. Times are the app's clock in seconds.
use super::types::INPUT_CHANGES_PER_SECOND;

/// The window between two sends (`1000 / inputChangesPerSecond` ms).
pub const WINDOW: f64 = 1.0 / INPUT_CHANGES_PER_SECOND as f64;

/// What a change asks the caller to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    /// Send this value now.
    Send(String),
    /// Held for the window (a later `due` sends it): wake the app after this many seconds, unless it already will.
    Hold { wake_in: Option<f64> },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputRate {
    last_sent: Option<f64>,
    pending: Option<String>,
    /// When the window ends, while a timer is waiting for it (`timer` in the Vue component).
    due_at: Option<f64>,
}

impl InputRate {
    /// The field's text changed to `value` at `now`.
    pub fn changed(&mut self, now: f64, value: &str) -> Change {
        let wait = self.last_sent.map_or(0.0, |at| at + WINDOW - now);
        if wait <= 0.0 && self.due_at.is_none() {
            self.last_sent = Some(now);
            return Change::Send(value.to_string());
        }
        self.pending = Some(value.to_string());
        let wake_in = if self.due_at.is_none() {
            let wait = wait.max(0.0);
            self.due_at = Some(now + wait);
            Some(wait)
        } else {
            None
        };
        Change::Hold { wake_in }
    }

    /// The clock moved: the held value, when its window has ended.
    pub fn due(&mut self, now: f64) -> Option<String> {
        if self.due_at.is_none_or(|at| now < at) {
            return None;
        }
        self.due_at = None;
        let value = self.pending.take()?;
        self.last_sent = Some(now);
        Some(value)
    }

    /// Seconds until the held value is due, while there is one.
    pub fn wait(&self, now: f64) -> Option<f64> {
        self.due_at.map(|at| (at - now).max(0.0))
    }

    /// The field is going away: its timer ends, and what it still held goes (nothing when nothing was held).
    pub fn flush(&mut self) -> Option<String> {
        self.due_at = None;
        self.pending.take()
    }
}
