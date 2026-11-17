//! What the two memory events carry (`lib/agent-memory.ts` `MemorySavedPayload`, `lib/domain-events.ts`), read into the page's
//! own types. `memory.saved` is also what the "Remembered" notice reads (`fleet/memory_notice.rs`, a smaller view of it);
//! the note lists use this one to keep up with notes agents save while the page is open.
use super::memory::{Note, parse_note};
use serde_json::Value;

/// `memory.saved`'s payload (`MemorySavedPayload`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub note: Note,
    pub repository_name: Option<String>,
    /// The note this one replaced, as it was, so Undo can put it back.
    pub previous: Option<Note>,
}

/// A payload without a note id says nothing (`if (!saved?.note?.id) return`).
pub fn parse_saved(payload: &Value) -> Option<Saved> {
    Some(Saved {
        note: parse_note(payload.get("note")?)?,
        repository_name: payload.get("repositoryName").and_then(Value::as_str).map(str::to_string),
        previous: payload.get("previous").and_then(parse_note),
    })
}

/// `memory.dreamed`'s payload is a whole dream (`DreamView`, read by the Dreaming panel's piece); what every reader needs
/// first is its id: a payload without one is ignored (`if (dream?.id)`).
pub fn dreamed_id(payload: &Value) -> Option<String> {
    payload.get("id").and_then(Value::as_str).filter(|id| !id.is_empty()).map(str::to_string)
}
