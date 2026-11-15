//! The open session's interruption: a turn a Fleet restart cut off. Loaded when the session opens and after a
//! reconnect, kept by the `session.interrupted` event (rows.rs), and what Continue and Dismiss do
//! (`use-session-interruption.ts`). The requests run on a thread; the answer comes back here.
use super::http;
use super::retry::Act;
use super::rows::{Interruption, RowPatch};
use crate::State;
use serde_json::Value;

const CONTINUE_FAILED: &str = "Fleet couldn't continue the session";
const DISMISS_FAILED: &str = "Fleet couldn't dismiss this";

impl State {
    /// The open session's interruption as Fleet last said it.
    pub fn interruption(&self) -> Option<&Interruption> {
        self.session().row.interrupted.as_ref()
    }

    /// The card's request state: a request on its way, and why the last one failed.
    pub fn interruption_act(&self) -> Act {
        self.draft_owner().and_then(|id| self.interrupt_acts.get(&id).cloned()).unwrap_or_default()
    }

    /// Loads the session's interruption when it opens and after a reconnect (`load`).
    pub fn load_interruption(&self, session: &str) {
        let Some(link) = &self.link else { return };
        let id = session.to_string();
        let asked = id.clone();
        link.off_thread(move |base| http::interruption_load(base, &asked), move |s, _, result| s.on_interruption_loaded(&id, result));
    }

    pub fn on_interruption_loaded(&mut self, session: &str, result: Result<(u16, Value), String>) {
        let Ok((status, body)) = result else { return };
        if !(200..300).contains(&status) {
            return;
        }
        let interruption = if status == 204 { None } else { Interruption::from_wire(&body) };
        self.on_row_patch(RowPatch::Interrupted { session: session.to_string(), interruption });
    }

    /// Continue, or Ask again: Fleet sends "Continue where you left off." (or asks the lost question again).
    pub fn interruption_continue(&mut self) {
        self.act_on_interruption(true);
    }

    /// Dismiss: forgets the interruption without sending anything.
    pub fn interruption_dismiss(&mut self) {
        self.act_on_interruption(false);
    }

    fn act_on_interruption(&mut self, send: bool) {
        let Some(id) = self.draft_owner() else { return };
        if self.interrupt_acts.get(&id).is_some_and(|a| a.busy) {
            return;
        }
        self.interrupt_acts.insert(id.clone(), Act { busy: true, error: None });
        let Some(link) = &self.link else {
            // No server (a scene or the mock data): the card is simply taken from the row.
            let (p, i) = self.open_at();
            if let Some(session) = self.projects.get_mut(p).and_then(|p| p.sessions.get_mut(i)) {
                session.row.interrupted = None;
            }
            self.interrupt_acts.insert(id, Act::default());
            return;
        };
        let asked = id.clone();
        link.off_thread(
            move |base| if send { http::interruption_continue(base, &asked) } else { http::interruption_dismiss(base, &asked) },
            move |s, _, result| s.on_interruption_acted(&id, send, result),
        );
    }

    /// The answer to Continue or Dismiss (`act`): anything but 2xx or 404 is a failure to say, and the interruption is
    /// loaded again to see what Fleet kept.
    pub fn on_interruption_acted(&mut self, session: &str, send: bool, result: Result<(u16, Value), String>) {
        let failure = if send { CONTINUE_FAILED } else { DISMISS_FAILED };
        let act = match result {
            Err(why) => Act { busy: false, error: Some(why) },
            Ok((status, _)) if !(200..300).contains(&status) && status != 404 => {
                self.load_interruption(session);
                Act { busy: false, error: Some(format!("{failure} (HTTP {status}).")) }
            }
            Ok(_) => {
                // Fleet's event says so too; this covers a socket that's down.
                self.on_row_patch(RowPatch::Interrupted { session: session.to_string(), interruption: None });
                Act::default()
            }
        };
        self.interrupt_acts.insert(session.to_string(), act);
    }
}
