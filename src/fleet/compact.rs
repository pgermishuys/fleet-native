//! Compact now: `POST /api/sessions/{id}/compact` (`compactSession` and `useSessionContext` in
//! `use-session-context.ts`). Fleet answers once the harness has taken the request; the compaction's start and end
//! arrive as `context.updated` (the ring reads them from the stream). The call runs on a thread.
use crate::State;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// The reason in an error body: Fleet's `{ error }`, or a problem's `detail` or `title`.
fn error_message(body: &Value, fallback: String) -> String {
    for key in ["error", "detail", "title"] {
        if let Some(text) = body.get(key).and_then(Value::as_str).filter(|t| !t.trim().is_empty()) {
            return text.to_string();
        }
    }
    fallback
}

/// `compactSession`'s verdict from a status and body, or from a request that never got an answer.
pub fn verdict(answer: Result<(u16, Value), String>) -> Result<(), String> {
    match answer {
        Ok((status, _)) if (200..300).contains(&status) => Ok(()),
        Ok((status, body)) => Err(error_message(&body, format!("It couldn't be compacted (HTTP {status})."))),
        Err(network) if network.trim().is_empty() => Err("It couldn't be compacted.".into()),
        Err(network) => Err(network),
    }
}

/// What the ring keeps, per session: the requests Fleet hasn't answered and why the last one was refused.
#[derive(Debug, Default)]
pub struct Compact {
    pub requesting: HashSet<String>,
    pub errors: HashMap<String, String>,
}

impl Compact {
    /// Starts a request unless one is in flight for the session; false when it is (a no-op).
    pub fn begin(&mut self, session: &str) -> bool {
        if !self.requesting.insert(session.to_string()) {
            return false;
        }
        self.errors.remove(session);
        true
    }

    pub fn finish(&mut self, session: &str, result: Result<(), String>) {
        self.requesting.remove(session);
        if let Err(e) = result {
            self.errors.insert(session.to_string(), e);
        }
    }
}

impl State {
    /// The popover's Compact now button. Nothing happens while a request is in flight, or without a server.
    pub fn compact_now(&mut self) {
        let Some(id) = self.open_id() else { return };
        if !self.compact.begin(&id) {
            return;
        }
        let Some(link) = self.link_for(&id) else {
            // A scene or a test with no server: the request is "in flight" until the test finishes it.
            return;
        };
        link.off_thread(
            {
                let id = id.clone();
                move |base| super::http::compact(base, &id)
            },
            move |s, _, answer| s.compact.finish(&id, verdict(answer)),
        );
    }

    /// Whether a Compact now for the open session is waiting for Fleet's answer.
    pub fn compact_requesting(&self) -> bool {
        self.open_id().is_some_and(|id| self.compact.requesting.contains(&id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ContextRing.test.ts: "compacts now, and says why when Fleet refuses"
    #[test]
    fn compacts_now_and_says_why_when_fleet_refuses() {
        assert_eq!(verdict(Ok((202, Value::Null))), Ok(()));
        let refused = json!({ "error": "Wait for the agent to finish its turn, then compact." });
        assert_eq!(verdict(Ok((409, refused))), Err("Wait for the agent to finish its turn, then compact.".into()));
        // A problem body: detail, then title; else the status.
        assert_eq!(verdict(Ok((500, json!({ "title": "Oops", "detail": "It broke." })))), Err("It broke.".into()));
        assert_eq!(verdict(Ok((502, json!({ "error": "  " })))), Err("It couldn't be compacted (HTTP 502).".into()));
        assert_eq!(verdict(Err("connection refused".into())), Err("connection refused".into()));
    }

    // A second press while one is in flight does nothing; a refusal is kept until the next press.
    #[test]
    fn a_press_while_one_is_in_flight_does_nothing_and_a_new_one_forgets_the_old_error() {
        let mut c = Compact::default();
        assert!(c.begin("s1"));
        assert!(!c.begin("s1"));
        c.finish("s1", Err("no".into()));
        assert!(!c.requesting.contains("s1"));
        assert_eq!(c.errors.get("s1").map(String::as_str), Some("no"));
        assert!(c.begin("s1"));
        assert!(c.errors.is_empty());
        assert!(c.begin("s2"));
    }

    /// Against a scratch Fleet: `FLEET_SCRATCH_URL=http://127.0.0.1:<port> FLEET_SCRATCH_SESSION=<id> cargo test -- --ignored compact_against`.
    #[test]
    #[ignore]
    fn compact_against_a_live_scratch_fleet() {
        let (Ok(base), Ok(id)) = (std::env::var("FLEET_SCRATCH_URL"), std::env::var("FLEET_SCRATCH_SESSION")) else { return };
        assert_eq!(verdict(crate::fleet::http::compact(&base, &id)), Ok(()));
        // A session Fleet doesn't know is refused with a reason, not a network error.
        assert!(verdict(crate::fleet::http::compact(&base, "no-such-session")).is_err());
    }
}
