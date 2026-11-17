//! What the Turns tab keeps (`TurnsCanvas.vue`'s script): the filter, which turns the user opened or closed, the one open
//! file, and the turns derived from the open session's conversation. Deriving is plain logic in `fleet::turns`; this
//! decides when to redo it (only while the tab is shown, and only when something it reads changed) so a frame never
//! parses a long session again.
use super::canvases::CanvasKind;
use super::turns::{EditCache, SessionTurn, TurnOptions, TurnTotals, derive_turns_cached, turn_totals};
use crate::State;
use crate::composer::choice::ModelOption;
use crate::conv::fold::FileStats;
use crate::conv::message_list::{FileStat, FileStatus};
use crate::fleet::diffs::FileStatus as DiffStatus;
use rune::Tx;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// At 300 px of canvas or less the facts and the "model changed" pill go (`@container (max-width: 300px)`).
pub const NARROW_W: f32 = 300.0;

#[derive(Debug, Default)]
pub struct TurnsTab {
    /// "Only turns that changed files". Kept across tab switches and sessions: one `turns` instance (row 4.7).
    pub changed_only: bool,
    /// What the user chose per turn id; the newest turn is open until they choose (`openOverrides`).
    overrides: HashMap<String, bool>,
    /// `turnId|path` of the one file whose diff is open, across every turn.
    pub open_file: Option<String>,
    turns: Vec<SessionTurn>,
    /// What `turns` was derived from, so the same inputs aren't derived twice.
    signature: Option<u64>,
    cache: EditCache,
    /// The canvas's width as last laid out (a frame late), for the 300 px rule.
    pub width: f32,
}

impl TurnsTab {
    /// Newest first, as derived.
    pub fn turns(&self) -> &[SessionTurn] {
        &self.turns
    }

    pub fn totals(&self) -> TurnTotals {
        turn_totals(&self.turns)
    }

    /// The turns the filter leaves (`visibleTurns`): with it on, those that wrote a file.
    pub fn visible(&self) -> Vec<&SessionTurn> {
        self.turns.iter().filter(|t| !self.changed_only || !t.read_only).collect()
    }

    /// `isOpen`: what the user chose, else only the newest turn of all (not of the filtered list).
    pub fn is_open(&self, turn: &SessionTurn) -> bool {
        self.overrides.get(&turn.id).copied().unwrap_or_else(|| self.turns.first().is_some_and(|first| first.id == turn.id))
    }

    pub fn toggle_turn(&mut self, id: &str) {
        let now = self.turns.iter().find(|t| t.id == id).is_some_and(|t| self.is_open(t));
        self.overrides.insert(id.to_string(), !now);
    }

    /// A scene or a test opens a turn as a click would.
    pub fn set_open(&mut self, id: &str, open: bool) {
        self.overrides.insert(id.to_string(), open);
    }

    pub fn file_key(turn: &SessionTurn, path: &str) -> String {
        format!("{}|{path}", turn.id)
    }

    /// A second click on the open file closes it; another file's click replaces it.
    pub fn toggle_file(&mut self, key: &str) {
        self.open_file = if self.open_file.as_deref() == Some(key) { None } else { Some(key.to_string()) };
    }

    pub fn narrow(&self) -> bool {
        self.width > 0.0 && self.width <= NARROW_W
    }

    /// Replace what is derived (a scene, or a test of the words).
    pub fn set_turns(&mut self, turns: Vec<SessionTurn>) {
        self.turns = turns;
    }

    pub fn cached_calls(&self) -> usize {
        self.cache.len()
    }
}

/// The session diff's counts per file, for a file a call wrote whole (`useFileStats`). `None` while the list isn't the open
/// session's or is unavailable: those files then show no counts.
fn file_stats(s: &State, session: &str) -> Option<FileStats> {
    let diffs = &s.diffs;
    if diffs.session() != session || !diffs.available {
        return None;
    }
    Some(
        diffs
            .files()
            .iter()
            .map(|d| {
                let status = match d.status {
                    DiffStatus::Added => FileStatus::Added,
                    DiffStatus::Deleted => FileStatus::Deleted,
                    DiffStatus::Modified => FileStatus::Modified,
                };
                (d.file.clone(), FileStat { additions: d.additions as u32, deletions: d.deletions as u32, status: Some(status) })
            })
            .collect(),
    )
}

impl State {
    /// The model names the open session's catalog gives (`useModels`).
    pub fn turn_models(&self) -> &[ModelOption] {
        self.pickers.catalog_of(&self.canvas_session()).map_or(&[][..], |c| c.models.as_slice())
    }

    /// Derives the turns again when what they read changed, and says whether it did. Called every frame; does nothing unless the Turns tab is on
    /// screen, so a session's turns cost nothing while another tab is open. The cheap check is a hash of the message
    /// count, the last two messages (a reply streams into the last) and the delegations and diff counts.
    pub fn keep_turns_tab(&mut self) -> bool {
        if !self.panel_body_shown(CanvasKind::Turns) {
            return false;
        }
        let session = self.canvas_session();
        let directory = self.session().row.directory.clone();
        let stats = file_stats(self, &session);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        session.hash(&mut hasher);
        directory.hash(&mut hasher);
        if let Some(stream) = self.stream.as_ref().filter(|_| self.stream_of.as_deref().is_none_or(|of| of == session)) {
            stream.messages.len().hash(&mut hasher);
            for m in stream.messages.iter().rev().take(2) {
                format!("{:?}", (&m.id, &m.parts, m.completed_at, m.cost, m.tokens, &m.model_id)).hash(&mut hasher);
            }
            for d in &stream.delegations {
                (&d.delegation_id, d.status.wire(), &d.title).hash(&mut hasher);
            }
        }
        if let Some(stats) = &stats {
            let mut counts: Vec<_> = stats.iter().map(|(p, f)| (p.clone(), f.additions, f.deletions, f.status)).collect();
            counts.sort_by(|a, b| a.0.cmp(&b.0));
            format!("{counts:?}").hash(&mut hasher);
        }
        let signature = hasher.finish();
        if self.turns_tab.signature == Some(signature) {
            return false;
        }
        let tab = &mut self.turns_tab;
        let derived = match self.stream.as_ref() {
            Some(stream) => derive_turns_cached(
                &stream.messages,
                &stream.delegations,
                TurnOptions { file_stats: stats.as_ref(), directory: (!directory.is_empty()).then_some(directory.as_str()) },
                &mut tab.cache,
            ),
            None => Vec::new(),
        };
        tab.turns = derived;
        tab.signature = Some(signature);
        true
    }

    /// "Show this turn in the chat ↑": the panel steps aside (`hideRightPanel`, which on the desktop collapses the column
    /// too), then the conversation scrolls to the round's first message and rings it.
    pub fn show_turn_in_chat(&mut self, id: &str, tx: &mut Tx) {
        self.set_content_open(false);
        self.show_message(Some(id), None, tx);
    }
}
