//! What the connection is subscribed to, what it last learned from each topic, and the conversations
//! the window keeps while you are away (`use-signalr-socket.ts`: `topicListenersV2`, `lastSnapshotsV2`;
//! `use-session-stream.ts`: `keptStreams`). Pure data: the link thread and the state each own one.
use super::model::StreamState;
use serde_json::Value;
use std::collections::HashMap;

/// How many left conversations are kept (`KEPT_SESSIONS`).
pub const KEPT_SESSIONS: usize = 8;

/// The Vue client's reconnect delays: SignalR's own `withAutomaticReconnect` (1, 2, 5, 10 s) and then
/// `RECONNECT_DELAYS_MS` (2, 5, 10, 30 s). Native has no in-socket retry to hide behind, so one loop
/// takes the first four and stays on the longer tail.
pub const RECONNECT_SECS: [u64; 5] = [1, 2, 5, 10, 30];

pub fn reconnect_delay(attempt: usize) -> std::time::Duration {
    std::time::Duration::from_secs(RECONNECT_SECS[attempt.min(RECONNECT_SECS.len() - 1)])
}

/// The session topics the connection listens to: the open one, which the conversation shows, and the
/// ones a component watches (the Files canvas will, component 11). All of them are asked for again
/// after a reconnect, so each gets a fresh snapshot.
#[derive(Default, Debug)]
pub struct Topics {
    open: Option<String>,
    watched: Vec<String>,
    /// The watched ones that are side conversations (`/btw`): their snapshots and events go to the panel, not the subscribers.
    sides: Vec<String>,
    /// The last snapshot of each topic, and how many events arrived since (`eventCount`).
    last: HashMap<String, (Value, u32)>,
}

impl Topics {
    pub fn open(&self) -> Option<&str> {
        self.open.as_deref()
    }

    /// Makes `id` the open session; returns the one it replaced when nobody else needs it.
    pub fn set_open(&mut self, id: Option<String>) -> Option<String> {
        let old = std::mem::replace(&mut self.open, id);
        old.filter(|old| !self.holds(old)).inspect(|old| {
            self.last.remove(old);
        })
    }

    /// Whether anyone listens to `id`.
    pub fn holds(&self, id: &str) -> bool {
        self.open.as_deref() == Some(id) || self.watched.iter().any(|w| w == id)
    }

    /// Adds a watcher; true when the topic is new to the connection and must be subscribed.
    pub fn watch(&mut self, id: &str) -> bool {
        let new = !self.holds(id);
        if !self.watched.iter().any(|w| w == id) {
            self.watched.push(id.to_string());
        }
        new
    }

    /// Removes a watcher; true when nobody listens any more and the topic must be unsubscribed.
    pub fn unwatch(&mut self, id: &str) -> bool {
        self.watched.retain(|w| w != id);
        let gone = !self.holds(id);
        if gone {
            self.last.remove(id);
        }
        gone
    }

    /// Every topic to subscribe to after a connect, the open one first.
    pub fn all(&self) -> Vec<String> {
        self.open.iter().chain(self.watched.iter().filter(|w| self.open.as_ref() != Some(*w))).cloned().collect()
    }

    /// Marks `id` as a side conversation's topic (or not) before it is watched.
    pub fn mark_side(&mut self, id: &str, side: bool) {
        self.sides.retain(|s| s != id);
        if side {
            self.sides.push(id.to_string());
        }
    }

    pub fn is_side(&self, id: &str) -> bool {
        self.sides.iter().any(|s| s == id)
    }

    pub fn has_watched(&self, id: &str) -> bool {
        self.watched.iter().any(|w| w == id)
    }

    /// A snapshot arrived for `id`.
    pub fn record_snapshot(&mut self, id: &str, snapshot: &Value) {
        self.last.insert(id.to_string(), (snapshot.clone(), 0));
    }

    /// An event arrived on `id`: the snapshot is older than the topic now.
    pub fn note_event(&mut self, id: &str) {
        if let Some((_, n)) = self.last.get_mut(id) {
            *n += 1;
        }
    }

    /// The snapshot to reuse for a second listener on `id`: only while no event has arrived since it
    /// was taken, else the caller asks again.
    pub fn fresh_snapshot(&self, id: &str) -> Option<&Value> {
        self.last.get(id).filter(|(_, events)| *events == 0).map(|(snapshot, _)| snapshot)
    }

    /// The connection dropped: nothing known is current.
    pub fn forget_snapshots(&mut self) {
        self.last.clear();
    }
}

/// The conversations of sessions that were left, the most recently left last, `KEPT_SESSIONS` at most.
#[derive(Default)]
pub struct Kept(Vec<(String, StreamState)>);

impl Kept {
    /// Keeps `state` for `id` (`keepCurrentStream`: only a conversation that has messages), dropping
    /// the one left longest ago when there are too many.
    pub fn keep(&mut self, id: &str, state: StreamState) {
        if state.messages.is_empty() {
            return;
        }
        self.0.retain(|(k, _)| k != id);
        self.0.push((id.to_string(), state));
        if self.0.len() > KEPT_SESSIONS {
            self.0.remove(0);
        }
    }

    /// The kept conversation of `id`. It stays: leaving again before the new snapshot arrived must not lose it.
    pub fn get(&self, id: &str) -> Option<&StreamState> {
        self.0.iter().find(|(k, _)| k == id).map(|(_, state)| state)
    }

    pub fn ids(&self) -> Vec<&str> {
        self.0.iter().map(|(k, _)| k.as_str()).collect()
    }
}
