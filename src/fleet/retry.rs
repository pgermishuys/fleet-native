//! The open session's scheduled retry: loaded when it opens, kept by the `session.retry` event (rows.rs), and what the
//! bar's Try now and Don't retry do (`use-session-retry.ts`). The requests run on a thread; the answer comes back here.
use super::http;
use super::rows::{RowPatch, ScheduledRetry};
use crate::State;
use crate::conv::retry::next_tick_ms;
use rune::Tx;
use serde_json::Value;

/// What the bar shows besides the retry: a request on its way, and why the last one failed (`busy`, `error`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Act {
    pub busy: bool,
    pub error: Option<String>,
}

const SEND_FAILED: &str = "Fleet couldn't try again now";
const CANCEL_FAILED: &str = "Fleet couldn't stop the retry";

impl State {
    /// The open session's retry as Fleet last said it.
    pub fn retry(&self) -> Option<&ScheduledRetry> {
        self.session().row.scheduled_retry.as_ref()
    }

    /// The bar's text changes when this does: every second in the last minute before it is due, else every 15 s
    /// (`tick`). Mixed into the failure row's `memo` key and compared in `poll`, so the countdown redraws by itself.
    pub fn retry_bucket(&self) -> i64 {
        let Some(retry) = self.retry() else { return 0 };
        let now = crate::view::message::now_ms(self);
        now / next_tick_ms(retry, now)
    }

    /// Everything about the world around a failure card that changes how it draws, as one number for the row's `memo`: it
    /// is the newest message, its details are open, the retry, the countdown's second, the buttons' state.
    pub fn failure_frame(&self, msg: &crate::view::conversation::Message, newest: bool) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let act = self.retry_act();
        let shown = newest && msg.meta.failure.as_ref().is_some_and(|f| f.kind.is_some());
        (newest, self.failure_open.contains(&msg.id), !self.composer_disabled(), act.busy, &act.error).hash(&mut h);
        if shown && let Some(r) = self.retry() {
            (&r.due_at, r.attempt, &r.kind, r.provider_said, self.retry_bucket()).hash(&mut h);
        } else {
            (self.retry().is_some(), newest).hash(&mut h);
        }
        h.finish()
    }

    /// Wakes the countdown (`tick`); true when its text changed and the view must redraw.
    pub fn keep_retry_clock(&mut self, tx: &mut Tx) -> bool {
        let shown = self.session().messages.last().and_then(|m| m.meta.failure.as_ref()).is_some_and(|f| f.kind.is_some());
        let Some(retry) = self.retry().filter(|_| shown) else { return false };
        let now = crate::view::message::now_ms(self);
        tx.wake_after(std::time::Duration::from_millis(next_tick_ms(retry, now) as u64));
        let bucket = self.retry_bucket();
        std::mem::replace(&mut self.retry_seen, bucket) != bucket
    }

    /// The open session's retry request state.
    pub fn retry_act(&self) -> Act {
        self.draft_owner().and_then(|id| self.retry_acts.get(&id).cloned()).unwrap_or_default()
    }

    /// Loads the session's retry when it opens and after a reconnect (`load`).
    pub fn load_retry(&self, session: &str) {
        let Some(link) = &self.link else { return };
        let id = session.to_string();
        let asked = id.clone();
        link.off_thread(move |base| http::retry_load(base, &asked), move |s, _, result| s.on_retry_loaded(&id, result));
    }

    pub fn on_retry_loaded(&mut self, session: &str, result: Result<(u16, Value), String>) {
        let Ok((status, body)) = result else { return };
        if !(200..300).contains(&status) {
            return;
        }
        let retry = if status == 204 { None } else { ScheduledRetry::from_wire(&body) };
        self.on_row_patch(RowPatch::ScheduledRetry { session: session.to_string(), retry });
    }

    /// Try now: sends "Continue where you left off." at once.
    pub fn retry_now(&mut self) {
        self.act_on_retry(true);
    }

    /// Don't retry: Fleet doesn't try again; what the user queued goes out now.
    pub fn retry_cancel(&mut self) {
        self.act_on_retry(false);
    }

    fn act_on_retry(&mut self, send: bool) {
        let Some(id) = self.draft_owner() else { return };
        if self.retry_acts.get(&id).is_some_and(|a| a.busy) {
            return;
        }
        self.retry_acts.insert(id.clone(), Act { busy: true, error: None });
        let Some(link) = &self.link else {
            // No server (a scene or the mock data): the retry is simply taken from the row.
            let (p, i) = self.open_at();
            if let Some(session) = self.projects.get_mut(p).and_then(|p| p.sessions.get_mut(i)) {
                session.row.scheduled_retry = None;
            }
            self.retry_acts.insert(id, Act::default());
            return;
        };
        let asked = id.clone();
        link.off_thread(
            move |base| if send { http::retry_send(base, &asked) } else { http::retry_cancel(base, &asked) },
            move |s, _, result| s.on_retry_acted(&id, send, result),
        );
    }

    /// The answer to Try now or Don't retry (`act`): anything but 2xx or 404 is a failure to say; the retry is
    /// loaded again to see what Fleet kept.
    pub fn on_retry_acted(&mut self, session: &str, send: bool, result: Result<(u16, Value), String>) {
        let failure = if send { SEND_FAILED } else { CANCEL_FAILED };
        let act = match result {
            Err(why) => Act { busy: false, error: Some(why) },
            Ok((status, _)) if !(200..300).contains(&status) && status != 404 => {
                self.load_retry(session);
                Act { busy: false, error: Some(format!("{failure} (HTTP {status}).")) }
            }
            Ok(_) => {
                // Fleet's event says so too; this covers a socket that's down.
                self.on_row_patch(RowPatch::ScheduledRetry { session: session.to_string(), retry: None });
                Act::default()
            }
        };
        self.retry_acts.insert(session.to_string(), act);
    }
}
