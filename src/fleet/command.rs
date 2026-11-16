//! Running a slash command (`use-send-command.ts`): `/review src` goes to `POST /api/sessions/{id}/command` with the
//! agent and model picked in the composer, not as a message. The session is shown busy at once; a refusal takes that back,
//! says why above the composer, and puts the text back unless something new was typed meanwhile.
use super::http;
use super::queue::error_message;
use crate::State;
use crate::composer::status::Optimistic;
use rune::Tx;
use serde_json::{Value, json};

/// The body of `POST /api/sessions/{id}/command`: the arguments and the pick are left out when there are none.
pub fn command_body(command: &str, args: &str, pick: &http::Pick) -> Value {
    let mut body = json!({ "command": command.trim() });
    if !args.trim().is_empty() {
        body["arguments"] = json!(args.trim());
    }
    if let Some(agent) = &pick.agent {
        body["agent"] = json!(agent);
    }
    if let Some((provider, model)) = &pick.model {
        body["model"] = json!({ "providerID": provider, "modelID": model });
    }
    body
}

impl State {
    /// Sends the draft as the command it is. The caller checked there is a server and a session.
    pub fn send_command(&mut self, tx: &mut Tx, command: String, args: String) {
        let Some(session) = self.open_id() else { return };
        let typed = self.draft.text.clone();
        if let Some(owner) = self.draft_owner() {
            let text = typed.trim().to_string();
            let now = self.clock;
            self.drafts.push_history(&owner, &text, now);
        }
        self.composer_error = None;
        self.optimistic = Some(Optimistic::new(self.clock));
        tx.wake_after(std::time::Duration::from_secs_f64(Optimistic::GIVE_UP + 0.1));
        self.draft.clear();
        let pick = self.prompt_pick(None);
        let body = command_body(&command, &args, &pick);
        let Some(link) = self.link_for(&session) else { return };
        let asked = session.clone();
        link.off_thread(move |base| http::command(base, &asked, &body), move |s, _, result| s.on_command_answer(&session, typed, result));
    }

    /// The server's answer: 202 is all there is to do; anything else is a refusal.
    pub fn on_command_answer(&mut self, session: &str, typed: String, result: Result<(u16, Value), String>) {
        let why = match result {
            Ok((status, _)) if (200..300).contains(&status) => return,
            Ok((status, body)) => error_message(&body, format!("HTTP {status}")),
            Err(why) if why.trim().is_empty() => "Failed to send command".to_string(),
            Err(why) => why,
        };
        if self.draft_owner().as_deref() != Some(session) {
            return;
        }
        // Nothing started, so no status event will come to undo the working it was shown as.
        self.optimistic = None;
        self.composer_error = Some(why);
        if self.draft.text.is_empty() {
            self.draft = rune::TextEdit::new(typed);
        }
    }
}
