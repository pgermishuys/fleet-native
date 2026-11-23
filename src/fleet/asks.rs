//! The agent's permission asks waiting on the user (`composables/use-session-permissions.ts`, rows 7.4-7.7, 3.12 and
//! 3.15 of `docs/parity/04-questions-and-permissions.md`). Fleet keeps them: they load when a session opens and
//! after a reconnect, `permission.asked` and `permission.replied` on the session's topic keep them current, and
//! answering is one POST. Requests run on a thread; the answer comes back here.
use super::events::{Event, Kind};
use super::http;
use super::retry::Act;
use super::state::Topic;
use crate::State;
use crate::asks::permission::{PermissionAsk, to_permission_ask};
use rune::Tx;
use serde_json::Value;
use std::collections::HashMap;

/// How the user answers an ask: the word the route takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reply {
    Once,
    Always,
    Reject,
}

impl Reply {
    pub fn word(self) -> &'static str {
        match self {
            Reply::Once => "once",
            Reply::Always => "always",
            Reply::Reject => "reject",
        }
    }
}

/// Each session's waiting asks as Fleet last said, so switching back to a session shows them at once, and the number
/// of the newest load or event per session: a load asked for before an event arrived is older than it (`loadId`).
#[derive(Debug, Default)]
pub struct Asks {
    by_session: HashMap<String, Vec<PermissionAsk>>,
    generation: HashMap<String, u64>,
}

impl Asks {
    pub fn of(&self, session: &str) -> &[PermissionAsk] {
        self.by_session.get(session).map_or(&[], Vec::as_slice)
    }

    pub fn begin_load(&mut self, session: &str) -> u64 {
        let n = self.generation.entry(session.to_string()).or_default();
        *n += 1;
        *n
    }

    /// A load's answer: dropped when an event arrived since it was asked for.
    pub fn loaded(&mut self, session: &str, generation: u64, asks: Vec<PermissionAsk>) {
        if self.generation.get(session) == Some(&generation) {
            self.by_session.insert(session.to_string(), asks);
        }
    }

    /// `permission.asked`: added unless its id is held already.
    pub fn asked(&mut self, session: &str, ask: PermissionAsk) {
        *self.generation.entry(session.to_string()).or_default() += 1;
        let held = self.by_session.entry(session.to_string()).or_default();
        if !held.iter().any(|a| a.id == ask.id) {
            held.push(ask);
        }
    }

    /// `permission.replied`: the ask is gone from this session.
    pub fn replied(&mut self, session: &str, id: &str) {
        *self.generation.entry(session.to_string()).or_default() += 1;
        if let Some(held) = self.by_session.get_mut(session) {
            held.retain(|a| a.id != id);
        }
    }

    /// An answer that went through, or an ask that no longer waits, leaves every session's list.
    pub fn forget(&mut self, id: &str) {
        for held in self.by_session.values_mut() {
            held.retain(|a| a.id != id);
        }
    }

    /// Replaces a session's list outright (a scene or a test with no server).
    pub fn set(&mut self, session: &str, asks: Vec<PermissionAsk>) {
        self.by_session.insert(session.to_string(), asks);
    }
}

/// `toPermissionAsk` over a list: what doesn't read is left out; anything but a list is no asks.
pub fn read_asks(body: &Value) -> Vec<PermissionAsk> {
    body.as_array().map(|items| items.iter().filter_map(to_permission_ask).collect()).unwrap_or_default()
}

/// Fleet's own error text from a failed answer, or a plain fallback (`errorMessage`).
pub fn error_text(body: &Value, status: u16) -> String {
    ["error", "detail", "title"]
        .iter()
        .find_map(|k| body.get(k).and_then(Value::as_str).filter(|s| !s.trim().is_empty()))
        .map_or_else(|| format!("The answer didn't reach the agent (HTTP {status})."), str::to_string)
}

impl State {
    /// The asks waiting on the open session, oldest first.
    pub fn open_asks(&self) -> &[PermissionAsk] {
        self.draft_owner().map_or(&[], |id| self.asks.of(&id))
    }

    /// The card's request state: an answer on its way, and why the last one failed.
    pub fn ask_act(&self, ask: &str) -> Act {
        self.ask_acts.get(ask).cloned().unwrap_or_default()
    }

    /// `permission.asked` and `permission.replied` on a session's topic. An ask that can't be read is ignored.
    pub fn on_ask_event(&mut self, topic: &Topic, event: &Event) {
        let Topic::Session(session) = topic else { return };
        let Event::Other(kind, payload) = event else { return };
        match kind {
            Kind::PermissionAsked => {
                if let Some(ask) = to_permission_ask(payload) {
                    self.asks.asked(session, ask);
                }
            }
            Kind::PermissionReplied => {
                if let Some(id) = payload.get("id").and_then(Value::as_str) {
                    self.asks.replied(session, id);
                }
            }
            _ => {}
        }
    }

    /// Loads the session's asks when it opens and after a reconnect. A failed load is not shown.
    pub fn load_asks(&mut self, session: &str) {
        let Some(link) = self.link_for(session) else { return };
        let generation = self.asks.begin_load(session);
        let id = session.to_string();
        let asked = id.clone();
        link.off_thread(
            move |base| http::permissions(base, &asked),
            move |s, tx, result| {
                if let Ok((status, body)) = result
                    && (200..300).contains(&status)
                {
                    s.asks.loaded(&id, generation, read_asks(&body));
                    s.focus_new_ask(tx);
                }
            },
        );
    }

    /// A card that just appeared takes the keyboard on "Allow once" unless the user is typing elsewhere (`onMounted`).
    pub fn focus_new_ask(&mut self, tx: &mut Tx) {
        let fresh: Vec<String> = self.open_asks().iter().map(|a| a.id.clone()).filter(|id| !self.ask_seen.contains(id)).collect();
        if fresh.is_empty() {
            return;
        }
        self.ask_seen.extend(fresh);
        if crate::asks::focus::takes_focus(tx.focused(), "composer", self.draft.text.len()) {
            tx.focus("permission-allow-once", false);
        }
    }

    /// The side conversation's card that just appeared takes the keyboard the same way (Fleet's card mounts in the panel too), on
    /// its own keys (`-side`), so 1, 2, 3, Esc and the arrows answer it.
    pub fn focus_new_side_ask(&mut self, side: &str, tx: &mut Tx) {
        let fresh: Vec<String> = self.asks.of(side).iter().map(|a| a.id.clone()).filter(|id| !self.ask_seen.contains(id)).collect();
        if fresh.is_empty() {
            return;
        }
        self.ask_seen.extend(fresh);
        if crate::asks::focus::takes_focus(tx.focused(), "composer", self.draft.text.len()) {
            tx.focus("permission-allow-once-side", false);
        }
    }

    /// `answer`: sends `reply`; the card goes when Fleet takes it (or says the ask no longer waits), and stays with
    /// Fleet's reason when it didn't. Ignored while an answer is on its way.
    pub fn answer_ask(&mut self, ask: &PermissionAsk, reply: Reply) {
        if self.ask_act(&ask.id).busy {
            return;
        }
        // Deny's words go with a reject; a hand-off has nobody to tell, so none.
        let words = (reply == Reply::Reject && ask.handoff.is_none()).then(|| self.deny.text.trim().to_string()).filter(|w| !w.is_empty());
        let Some(link) = self.link_for(&ask.session_id) else {
            // No server (a scene or the mock data): the card is simply taken away.
            self.asks.forget(&ask.id);
            return;
        };
        self.ask_acts.insert(ask.id.clone(), Act { busy: true, error: None });
        let (session, id) = (ask.session_id.clone(), ask.id.clone());
        link.off_thread(move |base| http::reply(base, &session, &id, reply.word(), words.as_deref()), {
            let id = ask.id.clone();
            move |s, _, result| s.on_ask_answered(&id, result)
        });
    }

    /// The answer to `answer_ask`: ok or 404 removes the ask everywhere; anything else is the card's error line.
    pub fn on_ask_answered(&mut self, ask: &str, result: Result<(u16, Value), String>) {
        let act = match result {
            Ok((status, _)) if (200..300).contains(&status) || status == 404 => {
                self.asks.forget(ask);
                self.deny = rune::TextEdit::new("");
                Act::default()
            }
            Ok((status, body)) => Act { busy: false, error: Some(error_text(&body, status)) },
            Err(why) => {
                Act { busy: false, error: Some(if why.trim().is_empty() { "The answer didn't reach the agent.".into() } else { why }) }
            }
        };
        self.ask_acts.insert(ask.to_string(), act);
    }
}
