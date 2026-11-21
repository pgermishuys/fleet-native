//! Sending a failing check or a review comment to the session's agent when the user asks (`use-send-to-agent.ts`), keyed so
//! each button shows its own "Sending" and "Sent" and works alone. The header's pull request popover has the buttons; the
//! Context tab's smart-link card (piece J) shares them.
//!
//! The request is the plain `POST /api/sessions/{id}/prompt` with `{ text, userMessageId }`: Fleet's own helper doesn't
//! take the composer's model pick along, and it doesn't add a pending message to the conversation (the server's event does).
use super::http;
use super::prompts::new_correlation_id;
use crate::State;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// The key of the popover's footer button, which sends everything at once.
pub const FIX: &str = "fix";

/// The key of one failing check's button.
pub fn check_key(name: &str) -> String {
    format!("check:{name}")
}

/// The key of one review thread's button.
pub fn thread_key(thread_node_id: &str) -> String {
    format!("thread:{thread_node_id}")
}

/// What the line under the buttons says when no answer came.
pub const UNREACHABLE: &str = "Couldn't reach Fleet to send this to the agent.";

/// What it says when Fleet refused.
pub fn refused(status: u16) -> String {
    format!("Couldn't send to the agent (HTTP {status}).")
}

/// The body of the request.
pub fn body(text: &str, user_message_id: &str) -> Value {
    json!({ "text": text, "userMessageId": user_message_id })
}

/// The request itself, on a thread: `Err` holds the sentence the popover shows.
pub fn post(base: &str, session: &str, text: &str) -> Result<(), String> {
    let body = body(text, &new_correlation_id());
    match http::text_with("POST", base, &format!("/api/sessions/{session}/prompt"), Some(&body)) {
        Ok((status, _)) if (200..300).contains(&status) => Ok(()),
        Ok((status, _)) => Err(refused(status)),
        Err(_) => Err(UNREACHABLE.to_string()),
    }
}

/// One session's buttons: the ones sending, the ones that have sent, and the last error.
#[derive(Default)]
struct Keyed {
    sending: HashSet<String>,
    sent: HashSet<String>,
    error: Option<String>,
}

#[derive(Default)]
pub struct SendToAgent {
    by_session: HashMap<String, Keyed>,
}

impl SendToAgent {
    pub fn is_sending(&self, session: &str, key: &str) -> bool {
        self.by_session.get(session).is_some_and(|k| k.sending.contains(key))
    }

    pub fn is_sent(&self, session: &str, key: &str) -> bool {
        self.by_session.get(session).is_some_and(|k| k.sent.contains(key))
    }

    /// A button is off while it sends and once it has sent.
    pub fn is_off(&self, session: &str, key: &str) -> bool {
        self.is_sending(session, key) || self.is_sent(session, key)
    }

    pub fn error(&self, session: &str) -> Option<&str> {
        self.by_session.get(session)?.error.as_deref()
    }

    /// Starts `key`; false when it is already sending (nothing is sent twice at once).
    fn begin(&mut self, session: &str, key: &str) -> bool {
        let keyed = self.by_session.entry(session.to_string()).or_default();
        if !keyed.sending.insert(key.to_string()) {
            return false;
        }
        keyed.error = None;
        true
    }

    fn finish(&mut self, session: &str, key: &str, result: Result<(), String>) {
        let keyed = self.by_session.entry(session.to_string()).or_default();
        keyed.sending.remove(key);
        match result {
            Ok(()) => {
                keyed.sent.insert(key.to_string());
            }
            Err(sentence) => keyed.error = Some(sentence),
        }
    }

    /// For a scene: `key` as it looks mid-send.
    pub fn show_sending(&mut self, session: &str, key: &str) {
        self.begin(session, key);
    }

    /// For a scene: `key` as it looks once sent.
    pub fn show_sent(&mut self, session: &str, key: &str) {
        self.finish(session, key, Ok(()));
    }

    /// For a scene: the error line.
    pub fn show_error(&mut self, session: &str, sentence: &str) {
        self.finish(session, "", Err(sentence.to_string()));
    }
}

impl State {
    /// Sends `text` to the open session's agent under `key`. With no server (the invented list) nothing can answer, which
    /// reads the way a Fleet that can't be reached does.
    pub fn send_to_agent(&mut self, key: &str, text: String) {
        let session = self.session().id.clone();
        self.send_to_session(&session, key, text);
    }

    /// `send_to_agent` for any session, on whichever machine it is (a GitHub page sends to the first session working on its item).
    pub fn send_to_session(&mut self, session: &str, key: &str, text: String) {
        let session = session.to_string();
        // A button that is off sends nothing, whatever reaches it.
        if self.agent_sends.is_sent(&session, key) || !self.agent_sends.begin(&session, key) {
            return;
        }
        let Some(link) = self.link_for(&session) else {
            self.agent_sends.finish(&session, key, Err(UNREACHABLE.to_string()));
            return;
        };
        let (id, key) = (session.clone(), key.to_string());
        link.off_thread(move |base| post(base, &session, &text), move |s, _, result| s.agent_sends.finish(&id, &key, result));
    }
}
