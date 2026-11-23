//! The session list's filter box and keys (`SessionsPanel.vue`): the box clears with its own button and with Esc (a
//! `type="search"` field's own doing in Chromium) and is lost when the panel unmounts for another rail; a pinned row moved
//! with Alt+Up/Down keeps the keyboard (`handleMovePin`).
use crate::{Rail, State, Tx};
use rune::TextEdit;

/// The filter's field key.
pub const FILTER: &str = "filter";

/// What the list's keys leave for the next frame.
#[derive(Default, Debug)]
pub struct ListKeys {
    /// The session whose row takes the keyboard once the list has drawn it where a pin move put it.
    refocus: Option<String>,
}

impl ListKeys {
    pub fn refocus(&mut self, id: &str) {
        self.refocus = Some(id.to_string());
    }
}

impl State {
    /// Empties the box (its clear button, Esc in it) and keeps the focus in it, as the browser's does.
    pub fn clear_filter(&mut self, tx: &mut Tx) {
        self.filter = TextEdit::new("");
        tx.focus(FILTER, false);
    }

    /// Esc in the box: it clears, and the picked rows let go as they do on any Esc (`handleSelectionKeydown`). With nothing
    /// typed the key is the field's own, which leaves it.
    pub fn filter_escape(&mut self, tx: &mut Tx) -> bool {
        self.selection.clear();
        if self.filter.text.is_empty() {
            return false;
        }
        self.clear_filter(tx);
        true
    }

    /// The panel isn't mounted while another rail shows, so what was typed is lost with it (`shallowRef` in the panel).
    pub(crate) fn keep_filter(&mut self, tx: &mut Tx) -> bool {
        let gone = self.active_rail() != Some(Rail::Sessions) && !self.filter.text.is_empty();
        if gone {
            self.filter = TextEdit::new("");
        }
        // A pinned row that moved is a new element in the Pinned group: the key it had is not where the focus stays.
        let place = self.list_keys.refocus.as_deref().and_then(|id| {
            self.projects.iter().enumerate().find_map(|(p, pr)| pr.sessions.iter().position(|s| s.id == id).map(|i| (p, i)))
        });
        if let Some((p, i)) = place {
            tx.focus(format!("session-{p}-{i}"), true);
            self.list_keys.refocus = None;
        }
        gone || place.is_some()
    }
}
