//! The peek (component 23, piece E; `PeekPanel.vue`, and the nudge of `useMissionControl`): what the picked tile's session did
//! last, the helpers it has running, and a field to send it a word. The reads and the nudge go to the tile's own machine on a
//! thread and come back through `Post::update`; nothing here draws.
use super::peek::{PEEK_COUNT, PeekEvent, peek_events};
use super::read::Reach;
use super::requests::{self, AnswerOutcome};
use super::wall::{DIDNT_GO_THROUGH, Wall};
use super::{Tile, TileState};
use crate::State;
use crate::conv::export_history::{convert_fleet_message, encode_component};
use crate::fleet::labs::FleetView;
use crate::fleet::work::{WorkStatus, to_items};
use rune::TextEdit;
use serde_json::Value;

/// A helper the session has running (`peek-helpers`): its title and what it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Helper {
    pub id: String,
    pub title: String,
    pub label: Option<String>,
}

/// What the line under the field says after a nudge went through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sent {
    /// It went into the turn, or was a plain prompt: "Sent."
    Sent,
    /// It waits in Fleet's queue for the turn's end.
    Queued,
}

/// The peek of the tile shown now; reset whenever another tile (or none) is shown.
pub struct Peek {
    /// The tile it belongs to.
    pub key: Option<String>,
    pub events: Vec<PeekEvent>,
    pub helpers: Vec<Helper>,
    pub nudge: TextEdit,
    pub sending: bool,
    pub sent: Option<Sent>,
    pub error: Option<String>,
    /// What the tile said when it was last read (its key, now line and helper count): a change reads again.
    loaded: Option<(String, String, u32)>,
    /// Counts the tiles shown: an answer from an earlier one is dropped.
    epoch: u64,
    /// Counts the reads of this tile: only the newest one is kept.
    reads: u64,
}

impl Default for Peek {
    fn default() -> Self {
        Peek {
            key: None,
            events: Vec::new(),
            helpers: Vec::new(),
            nudge: TextEdit::new(""),
            sending: false,
            sent: None,
            error: None,
            loaded: None,
            epoch: 0,
            reads: 0,
        }
    }
}

impl Peek {
    /// Starts over for another tile (or none); what is still on its way for the last one is dropped.
    fn reset(&mut self, key: Option<String>) {
        *self = Peek { key, epoch: self.epoch + 1, ..Peek::default() };
    }

    /// A scene's peek, with what a read would have brought.
    pub fn shown(key: &str, events: Vec<PeekEvent>, helpers: Vec<Helper>) -> Peek {
        Peek { key: Some(key.to_string()), events, helpers, ..Peek::default() }
    }
}

/// The tile the open machine's page shows beside the list, if the pick is one of its tiles (`selected`).
pub fn selected<'a>(s: &State, wall: &'a Wall) -> Option<&'a Tile> {
    let FleetView::Machine(machine) = &s.labs.fleet_view else { return None };
    let picked = s.mission.picked.as_deref()?;
    wall.tiles.iter().find(|t| t.key == picked && t.machine_id == *machine)
}

/// What a read of the tile's machine brought for the peek: `messages?limit=6` and the session's work. A read that failed is
/// nothing (no events, no helpers).
fn read_peek(reach: &Reach, session: &str) -> (Vec<PeekEvent>, Vec<Helper>) {
    let id = encode_component(session);
    let body = |path: String| -> Option<Value> {
        let (status, text) = reach.get(&path).ok()?;
        if !(200..300).contains(&status) {
            return None;
        }
        serde_json::from_str(&text).ok()
    };
    let (messages, work) = std::thread::scope(|scope| {
        let messages = scope.spawn(|| body(format!("/api/sessions/{id}/messages?limit={PEEK_COUNT}")));
        let work = scope.spawn(|| body(format!("/api/sessions/{id}/work")));
        (messages.join().ok().flatten(), work.join().ok().flatten())
    });
    let wire = messages.as_ref().and_then(|m| m.get("messages")).and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
    let converted: Vec<_> = wire.iter().filter_map(|m| convert_fleet_message(m, "")).collect();
    let helpers = to_items(work.as_ref())
        .into_iter()
        .filter(|item| item.status == WorkStatus::Running)
        .map(|item| Helper { id: item.id, title: item.title, label: item.label })
        .collect();
    (peek_events(&converted, PEEK_COUNT), helpers)
}

impl State {
    /// Pressing a tile picks it (`selectedKey`).
    pub fn mission_pick(&mut self, key: &str) {
        self.mission.picked = Some(key.to_string());
    }

    /// Keeps the peek in step with the page: it belongs to the tile shown, starts over for another, reads again when the tile's
    /// now line or helper count changes, and lets the pick go when its tile does. True when something changed.
    pub(crate) fn keep_peek(&mut self) -> bool {
        if self.mission.picked.is_none() && self.mission.peek.key.is_none() {
            return false;
        }
        let wall = self.mission_wall();
        if self.mission.picked.as_deref().is_some_and(|key| !wall.tiles.iter().any(|t| t.key == key)) {
            self.mission.picked = None;
        }
        let Some(tile) = selected(self, &wall).cloned() else {
            if self.mission.peek.key.is_none() {
                return false;
            }
            self.mission.peek.reset(None);
            return true;
        };
        let mut changed = false;
        if self.mission.peek.key.as_deref() != Some(tile.key.as_str()) {
            self.mission.peek.reset(Some(tile.key.clone()));
            changed = true;
        }
        let said = (tile.key.clone(), tile.now.clone(), tile.helpers);
        if self.mission.peek.loaded.as_ref() != Some(&said) {
            self.mission.peek.loaded = Some(said);
            self.peek_read(&tile);
            changed = true;
        }
        changed
    }

    fn peek_read(&mut self, tile: &Tile) {
        let (Some(post), Some(reach)) = (self.link.as_ref().map(|l| l.post()), self.mission_reach(&tile.machine_id)) else { return };
        let peek = &mut self.mission.peek;
        peek.reads += 1;
        let (epoch, read, session) = (peek.epoch, peek.reads, tile.session_id.clone());
        std::thread::spawn(move || {
            let (events, helpers) = read_peek(&reach, &session);
            post.update(move |s, tx| {
                let peek = &mut s.mission.peek;
                if peek.epoch == epoch && peek.reads == read {
                    peek.events = events;
                    peek.helpers = helpers;
                    tx.wake_after(std::time::Duration::from_millis(20));
                }
            });
        });
    }

    /// Enter in the nudge field: the trimmed words to the session, into its turn where its harness steers, queued where it
    /// doesn't (`nudge`). Not while one is on its way, and not blank.
    pub fn peek_nudge(&mut self) {
        let peek = &self.mission.peek;
        let text = peek.nudge.text.trim().to_string();
        let Some(key) = peek.key.clone().filter(|_| !peek.sending && !text.is_empty()) else { return };
        let wall = self.mission_wall();
        let Some(tile) = wall.tiles.into_iter().find(|t| t.key == key) else { return };
        let peek = &mut self.mission.peek;
        (peek.sending, peek.sent, peek.error) = (true, None, None);
        let epoch = peek.epoch;
        let machine = match requests::machine_key(&self.machines, &tile) {
            Ok(machine) => machine,
            Err(words) => return self.peek_nudged(epoch, AnswerOutcome { ok: false, gone: false, error: Some(words.into()) }, false),
        };
        let working = tile.state == TileState::Working;
        let send = {
            let machine = machine.clone();
            let tile = tile.clone();
            move |s: &mut State, steers: bool| {
                let request = requests::nudge_request(&tile.session_id, &text, working, steers, &crate::fleet::prompts::uuid());
                s.mission_send(&machine, request, move |s, outcome| s.peek_nudged(epoch, outcome, working && !steers));
            }
        };
        if working && self.link.is_some() {
            self.mission_steers(&machine, &tile.harness_type, send);
        } else {
            send(self, false);
        }
    }

    fn peek_nudged(&mut self, epoch: u64, outcome: AnswerOutcome, queued: bool) {
        let peek = &mut self.mission.peek;
        if peek.epoch != epoch {
            return;
        }
        peek.sending = false;
        if outcome.ok {
            peek.nudge = TextEdit::new("");
            peek.sent = Some(if queued { Sent::Queued } else { Sent::Sent });
        } else {
            peek.error = Some(outcome.error.unwrap_or_else(|| DIDNT_GO_THROUGH.into()));
        }
    }
}
