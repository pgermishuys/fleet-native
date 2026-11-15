//! A session's queue: what the user sent while its agent worked (`use-session-queue.ts`). The queue is Fleet's, not
//! ours: the server keeps it, sends the first item when the turn ends and says so with `session.queue` on the session's
//! topic. So it is loaded when a session opens and after a reconnect, replaced whole by every event, and the requests
//! here (queue, take back, send now) run on a thread and answer into this store.
use super::events::{Event, Kind};
use super::http::{self, Pick};
use crate::State;
use serde_json::{Value, json};
use std::collections::HashMap;

/// How a queued item goes out: a message, a slash command, or a shell command (`!git status`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Prompt,
    Command,
    Shell,
}

impl ItemKind {
    fn wire(self) -> &'static str {
        match self {
            ItemKind::Prompt => "prompt",
            ItemKind::Command => "command",
            ItemKind::Shell => "shell",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    /// What was typed: the block Fleet adds for the agent's `@` sessions is taken off.
    pub text: String,
    pub kind: ItemKind,
    /// The mod that queued it, when the user didn't.
    pub from_mod: Option<String>,
}

/// What goes with a queued item besides its text (`QueueOptions`): the composer's pick travels with it so it goes out as
/// it would have gone now.
#[derive(Clone, Debug)]
pub struct Options {
    pub kind: ItemKind,
    /// For a slash command: its name and arguments.
    pub command: Option<(String, String)>,
    pub pick: Pick,
}

/// `stripSessionReferences`: a message's text without the `<fleet-session-references>` block Fleet writes after it.
pub fn strip_session_references(body: &str) -> &str {
    const OPEN: &str = "<fleet-session-references>\n";
    const CLOSE: &str = "</fleet-session-references>";
    let Some(at) = body.find(OPEN) else { return body };
    // The block ends the message (`\s*$`); anything after the closing tag but whitespace means it isn't ours.
    match body[at..].find(CLOSE) {
        Some(end) if body[at + end + CLOSE.len()..].trim().is_empty() => body[..at].trim_end(),
        _ => body,
    }
}

/// `toQueuedMessage`: an item that has no id or text doesn't read; an unknown kind reads as a prompt.
pub fn read_item(value: &Value) -> Option<Item> {
    let id = value.get("id")?.as_str()?.to_string();
    let text = strip_session_references(value.get("text")?.as_str()?).to_string();
    let kind = match value.get("kind").and_then(Value::as_str) {
        Some("command") => ItemKind::Command,
        Some("shell") => ItemKind::Shell,
        _ => ItemKind::Prompt,
    };
    let from_mod = value.get("mod").and_then(Value::as_str).filter(|m| !m.is_empty()).map(str::to_string);
    Some(Item { id, text, kind, from_mod })
}

/// `toQueue`: the items that read, in order; anything but a list is an empty queue.
pub fn read_queue(value: &Value) -> Vec<Item> {
    value.as_array().map(|items| items.iter().filter_map(read_item).collect()).unwrap_or_default()
}

/// The reason in an error body: Fleet's `{ error }`, or a problem's `detail` or `title`, else `fallback`.
pub fn error_message(body: &Value, fallback: String) -> String {
    for key in ["error", "detail", "title"] {
        if let Some(text) = body.get(key).and_then(Value::as_str).filter(|t| !t.trim().is_empty()) {
            return text.to_string();
        }
    }
    fallback
}

/// The body of `POST /api/sessions/{id}/queue`.
pub fn enqueue_body(text: &str, options: &Options) -> Value {
    let (command, arguments) = match &options.command {
        Some((name, args)) => (Some(name.as_str()), Some(args.as_str()).filter(|a| !a.is_empty())),
        None => (None, None),
    };
    json!({
        "text": text,
        "kind": options.kind.wire(),
        "command": command,
        "arguments": arguments,
        "agent": options.pick.agent,
        "model": options.pick.model.as_ref().map(|(p, m)| json!({ "providerID": p, "modelID": m })),
        "effort": options.pick.effort,
    })
}

/// Each session's queue as Fleet last said it, so coming back to a session shows it at once, and the number of the
/// newest load or event per session: a load that was asked for before an event arrived is older than it (`loadId`).
#[derive(Debug, Default)]
pub struct Queues {
    items: HashMap<String, Vec<Item>>,
    generation: HashMap<String, u64>,
    /// Ids for items made without a server (a scene, the mock data).
    local: u64,
}

impl Queues {
    pub fn of(&self, session: &str) -> &[Item] {
        self.items.get(session).map_or(&[], Vec::as_slice)
    }

    /// Starts a load: its number, to hand back with the answer.
    pub fn begin_load(&mut self, session: &str) -> u64 {
        let n = self.generation.entry(session.to_string()).or_default();
        *n += 1;
        *n
    }

    /// A load's answer: dropped when something newer arrived since it was asked for.
    pub fn loaded(&mut self, session: &str, generation: u64, items: Vec<Item>) {
        if self.generation.get(session) == Some(&generation) {
            self.items.insert(session.to_string(), items);
        }
    }

    /// An event carries the whole queue and is newer than any load in flight.
    pub fn replace(&mut self, session: &str, items: Vec<Item>) {
        *self.generation.entry(session.to_string()).or_default() += 1;
        self.items.insert(session.to_string(), items);
    }

    /// Adds an item unless the event got there first.
    pub fn add(&mut self, session: &str, item: Item) {
        let items = self.items.entry(session.to_string()).or_default();
        if !items.iter().any(|i| i.id == item.id) {
            items.push(item);
        }
    }

    pub fn remove(&mut self, session: &str, id: &str) {
        if let Some(items) = self.items.get_mut(session) {
            items.retain(|i| i.id != id);
        }
    }

    fn local_id(&mut self) -> String {
        self.local += 1;
        format!("queued-local-{}", self.local)
    }
}

impl State {
    /// The open session's queue.
    pub fn queued(&self) -> &[Item] {
        self.draft_owner().map_or(&[], |id| self.queue.of(&id))
    }

    /// `session.queue` `{sessionId, items}`: another session's event is ignored.
    pub fn on_queue_event(&mut self, event: &Event) {
        let Event::Other(Kind::SessionQueue, props) = event else { return };
        let Some(session) = props.get("sessionId").and_then(Value::as_str) else { return };
        let items = read_queue(props.get("items").unwrap_or(&Value::Null));
        self.queue.replace(session, items);
    }

    /// Loads the session's queue when it opens and after a reconnect.
    pub fn load_queue(&mut self, session: &str) {
        let Some(link) = &self.link else { return };
        let generation = self.queue.begin_load(session);
        let id = session.to_string();
        let asked = id.clone();
        link.off_thread(
            move |base| http::queue_load(base, &asked),
            move |s, _, result| {
                if let Ok((status, body)) = result
                    && (200..300).contains(&status)
                {
                    s.queue.loaded(&id, generation, read_queue(&body));
                }
            },
        );
    }

    /// Queues `text`. The caller has cleared the draft; `typed` is what comes back to it if Fleet refuses, unless
    /// something new was typed meanwhile (`queueDraft`).
    pub fn enqueue(&mut self, session: &str, typed: String, text: String, options: Options) {
        self.composer_error = None;
        let Some(link) = &self.link else {
            // No server (a scene or the mock data): the item is kept here, as Fleet's event would have it.
            let id = self.queue.local_id();
            self.queue.add(session, Item { id, text, kind: options.kind, from_mod: None });
            return;
        };
        let body = enqueue_body(&text, &options);
        let id = session.to_string();
        let asked = id.clone();
        link.off_thread(move |base| http::queue_add(base, &asked, &body), move |s, _, result| s.on_enqueued(&id, typed, result));
    }

    /// The answer to a queue request: the item joins the list (the event usually said so first); a refusal says why
    /// and the typed text comes back.
    pub fn on_enqueued(&mut self, session: &str, typed: String, result: Result<(u16, Value), String>) {
        let refused = match result {
            Ok((status, body)) if (200..300).contains(&status) => {
                if let Some(item) = read_item(&body) {
                    self.queue.add(session, item);
                }
                return;
            }
            Ok((status, body)) => error_message(&body, format!("The message couldn't be queued (HTTP {status}).")),
            Err(why) if why.trim().is_empty() => "The message couldn't be queued.".to_string(),
            Err(why) => why,
        };
        self.composer_error = Some(refused);
        if self.draft.text.is_empty() && self.draft_owner().as_deref() == Some(session) {
            self.draft = rune::TextEdit::new(typed);
        }
    }

    /// Whether Send now is offered for an item: when the session is idle any item can go (say, one left after a
    /// restart); while it works only a message can, and only where the harness takes one mid-turn.
    pub fn can_send_queued_now(&self, item: &Item) -> bool {
        use crate::composer::status::ComposerStatus;
        match self.composer_status() {
            ComposerStatus::Idle => true,
            ComposerStatus::Busy => self.caps().can_steer && item.kind == ItemKind::Prompt,
            ComposerStatus::WaitingInput => false,
        }
    }

    /// Send now on a row: the item goes into the running turn, or as usual when idle; the draft is left alone.
    pub fn send_queued_now(&mut self, index: usize) {
        let Some(session) = self.draft_owner() else { return };
        let Some(item) = self.queue.of(&session).get(index).cloned() else { return };
        if self.composer_disabled() || !self.can_send_queued_now(&item) {
            return;
        }
        self.composer_error = None;
        let Some(link) = &self.link else {
            self.queue.remove(&session, &item.id);
            return;
        };
        let (asked, id) = (session.clone(), item.id.clone());
        link.off_thread(move |base| http::queue_send(base, &asked, &id), move |s, _, result| s.on_queue_sent(&session, &item.id, result));
    }

    /// The answer to Send now: a refusal says why and the queue is loaded again; on success the item leaves here (the
    /// event follows).
    pub fn on_queue_sent(&mut self, session: &str, item: &str, result: Result<(u16, Value), String>) {
        match result {
            Ok((status, _)) if (200..300).contains(&status) => self.queue.remove(session, item),
            other => {
                self.composer_error = Some(match other {
                    Ok((status, body)) => error_message(&body, format!("The message couldn't be sent (HTTP {status}).")),
                    Err(why) if why.trim().is_empty() => "The message couldn't be sent.".to_string(),
                    Err(why) => why,
                });
                self.load_queue(session);
            }
        }
    }

    /// Takes an item back: it leaves at once, and Fleet is told.
    pub fn remove_queued(&mut self, index: usize) {
        let Some(session) = self.draft_owner() else { return };
        let Some(item) = self.queue.of(&session).get(index).cloned() else { return };
        self.queue.remove(&session, &item.id);
        let Some(link) = &self.link else { return };
        let asked = session.clone();
        link.off_thread(
            move |base| http::queue_remove(base, &asked, &item.id),
            move |s, _, result| {
                // A 404 is as good: it was gone already.
                if !matches!(result, Ok((status, _)) if (200..300).contains(&status) || status == 404) {
                    s.load_queue(&session);
                }
            },
        );
    }
}
