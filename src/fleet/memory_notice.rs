//! The "Remembered" notice (`composables/use-memory-notices.ts`, `lib/agent-memory.ts`): when an agent saves a note, a
//! small card says where, with Undo for six seconds. The note is saved already, so Undo takes it back through
//! `/api/memory`: a new note is forgotten, a note that replaced another puts the old text back.
//!
//! Plain Rust but for the thread that sends the calls: [`undo_calls`] says what Undo asks the server, [`run_undo`] sends it
//! with whatever sends calls (a test hands in a fake), and [`State::on_memory_event`] and [`State::keep_memory_undo`]
//! are the two ends that touch the app.
use super::events::{Event, Kind};
use super::notices::{Action, ActionTone, IconRef, Notice};
use super::state::Topic;
use crate::State;
use crate::icons::LIGHTBULB;
use serde_json::{Value, json};
use std::collections::HashMap;

/// How long Undo is offered (`MEMORY_UNDO_MS`).
pub const UNDO_SECS: f64 = 6.0;
/// A notice nobody saw within this long goes away; the note stays in Settings → Memory.
pub const EXPIRES_SECS: f64 = 120.0;
/// What the Undo button tells the notice's source, followed by the notice's id.
pub const UNDO_PREFIX: &str = "memory-undo:";
/// Said when the server gave no reason of its own.
const UNDO_FALLBACK: &str = "Forget the note in Settings → Memory instead.";

/// A saved note, as the event carries it (`MemoryNote`, the parts a notice uses).
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub id: String,
    /// `machine` or `repository`.
    pub list: String,
    pub text: String,
    pub repository: Option<String>,
    pub updated: String,
}

/// `memory.saved`'s payload (`MemorySavedPayload`).
#[derive(Debug, Clone, PartialEq)]
pub struct Saved {
    pub note: Note,
    pub repository_name: Option<String>,
    /// The note this one replaced, when the agent corrected one.
    pub previous: Option<Note>,
}

fn note(v: &Value) -> Option<Note> {
    let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    Some(Note {
        id: text("id").filter(|id| !id.is_empty())?,
        list: text("list").unwrap_or_else(|| "machine".into()),
        text: text("text").unwrap_or_default(),
        repository: text("repository"),
        updated: text("updated").unwrap_or_default(),
    })
}

/// A payload without a note id is ignored (`if (!saved?.note?.id) return`).
pub fn parse(payload: &Value) -> Option<Saved> {
    Some(Saved {
        note: note(payload.get("note")?)?,
        repository_name: payload.get("repositoryName").and_then(Value::as_str).map(str::to_string),
        previous: payload.get("previous").and_then(note),
    })
}

/// Where a note is kept, as the notice says it (`memoryPlace`).
pub fn place(note: &Note, repository_name: Option<&str>) -> String {
    if note.list == "machine" { "this machine".into() } else { repository_name.unwrap_or("this repository").into() }
}

pub fn notice_id(saved: &Saved) -> String {
    format!("memory-saved-{}-{}", saved.note.id, saved.note.updated)
}

/// The card: no chip, so it goes away when it settles; the time left shows as a bar (`show`).
pub fn notice(saved: &Saved) -> Notice {
    let id = notice_id(saved);
    Notice {
        body: Some(saved.note.text.clone()),
        icon: Some(IconRef(&LIGHTBULB)),
        hold_secs: Some(UNDO_SECS),
        countdown: true,
        expires_secs: Some(EXPIRES_SECS),
        actions: vec![Action { label: "Undo".into(), icon: None, tone: ActionTone::Quiet, run: format!("{UNDO_PREFIX}{id}") }],
        ..Notice::new(id, format!("Remembered for {}", place(&saved.note, saved.repository_name.as_deref())))
    }
}

/// One call Undo makes: a method, a path under the server, and a body when it has one.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub method: &'static str,
    pub path: String,
    pub body: Option<Value>,
}

fn note_path(id: &str) -> String {
    format!("/api/memory/notes/{}", super::http::escape(id))
}

/// What Undo asks of the server (`undoMemorySave`): the same note put back to its old text, or the new one forgotten and
/// the old one added again.
pub fn undo_calls(saved: &Saved) -> Vec<Call> {
    let (new, old) = (&saved.note, saved.previous.as_ref());
    if let Some(old) = old.filter(|p| p.id == new.id) {
        return vec![Call { method: "PUT", path: note_path(&new.id), body: Some(json!({ "text": old.text })) }];
    }
    let mut calls = vec![Call { method: "DELETE", path: note_path(&new.id), body: None }];
    if let Some(old) = old {
        let body = json!({ "list": old.list, "repository": old.repository, "text": old.text });
        calls.push(Call { method: "POST", path: "/api/memory/notes".into(), body: Some(body) });
    }
    calls
}

/// Sends Undo's calls one after another through `send` (method, path, body → status and body, or why nothing answered),
/// stopping at the first that fails with the server's own words.
pub fn run_undo(saved: &Saved, mut send: impl FnMut(&Call) -> Result<(u16, String), String>) -> Result<(), String> {
    for call in undo_calls(saved) {
        let (status, body) = send(&call)?;
        if !(200..300).contains(&status) {
            return Err(super::machines::read_error(&body, UNDO_FALLBACK.into()));
        }
    }
    Ok(())
}

/// What the app keeps between the event and Undo: the payloads of the notices still on screen, by notice id.
#[derive(Debug, Default)]
pub struct MemoryNotices {
    saved: HashMap<String, Saved>,
}

impl State {
    /// `memory.saved` on the `sessions` topic posts the card. The id carries the note's `updated`, so a note corrected
    /// again is a new card, and the same event twice is one.
    pub fn on_memory_event(&mut self, topic: &Topic, event: &Event) {
        let Event::Other(Kind::MemorySaved, payload) = event else { return };
        if *topic != Topic::Sessions {
            return;
        }
        let Some(saved) = parse(payload) else { return };
        let notice = notice(&saved);
        self.memory.saved.insert(notice.id.clone(), saved);
        self.notices.post(notice, self.clock);
    }

    /// Runs the Undo the card's button told about: on a thread, then the card closes, or says why it couldn't.
    pub fn keep_memory_undo(&mut self) {
        for run in self.notices.take_heard_with(UNDO_PREFIX) {
            let id = run[UNDO_PREFIX.len()..].to_string();
            let Some(saved) = self.memory.saved.get(&id).cloned() else { continue };
            let Some(link) = self.link.as_ref() else {
                self.memory_undone(&id, Err("Fleet isn't reachable.".into()));
                continue;
            };
            link.off_thread(
                move |base| run_undo(&saved, |c| super::http::text_with(c.method, base, &c.path, c.body.as_ref())),
                move |s, _, result| s.memory_undone(&id, result),
            );
        }
    }

    /// Undo's answer: the notice closes, or says "Couldn't undo it" and why, and loses its button.
    pub fn memory_undone(&mut self, id: &str, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.memory.saved.remove(id);
                self.notices.remove(id);
            }
            Err(why) => self.notices.update(id, |n| {
                n.title = "Couldn't undo it".into();
                n.body = Some(why);
                n.actions.clear();
            }),
        }
    }
}
