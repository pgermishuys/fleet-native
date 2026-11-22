//! What the Board's page keeps beside the store (`KanbanBoard.vue`'s own refs): whether Sync now is running, the toast it leaves
//! and when that goes, and the keys: Cmd/Ctrl+Shift+E toggles the mode.
use super::SyncResult;
use crate::fleet::labs::Feature;
use crate::{Rail, State};
use rune::{KeyEvent, Tx};
use std::time::Duration;

/// How long the toast stays (`showSyncFeedback`'s `setTimeout`).
pub const TOAST_SECONDS: f64 = 5.0;

/// `SyncFeedback`.
#[derive(Clone, Debug, PartialEq)]
pub struct Feedback {
    pub title: String,
    pub message: String,
}

impl Feedback {
    /// `showSyncFeedback`'s words.
    pub fn of(result: &SyncResult) -> Feedback {
        Feedback {
            title: "Board synced".into(),
            message: format!("{} added, {} updated, {} stale", result.cards_created, result.cards_updated, result.cards_marked_stale),
        }
    }
}

#[derive(Debug, Default)]
pub struct BoardPage {
    /// `isSyncing`.
    pub syncing: bool,
    /// The toast and the second on the app clock at which it goes.
    pub feedback: Option<(Feedback, f64)>,
}

/// Whether the focused element's key is a text field of the Board's page: those are keyed `board-…-input` (the lane and card
/// composers and the rename field), and Fleet's `useKeyboardShortcut` leaves a key typed in a field alone.
pub fn is_text_field(focused: Option<&str>) -> bool {
    focused.is_some_and(|key| key.starts_with("board-") && key.ends_with("-input"))
}

impl State {
    /// "Edit Board" / "Save": `handleToggleBoardMode`, which does nothing before the board has loaded.
    pub fn board_toggle_pressed(&mut self) {
        if self.board.is_loaded {
            self.board.toggle_mode();
        }
    }

    /// Cmd/Ctrl+Shift+E (`board-toggle-mode`), while the Board's page is shown with its switch on and the board loaded, and not
    /// typed in a text field. Fleet lets a user bind another command to the chord; this app has no key bindings to change yet.
    pub fn board_key(&mut self, ev: &KeyEvent, tx: &Tx) -> bool {
        if !ev.is_command('e') || !ev.mods.shift || ev.mods.alt {
            return false;
        }
        let shown = self.rail == Rail::Board && self.labs_feature(Feature::Board) == Some(true);
        if !shown || !self.board.is_loaded || is_text_field(tx.focused()) {
            return false;
        }
        self.board_toggle_pressed();
        true
    }

    /// Sync now (`handleSyncBoard`): only with a board, and a press while one runs does nothing. The toast comes with the answer
    /// and a failure leaves its words in the banner (the store says it). With no server nothing is sent.
    pub fn board_sync_pressed(&mut self) {
        if !self.board.has_board() || self.board.page.syncing || self.link.is_none() {
            return;
        }
        self.board.page.syncing = true;
        self.board_sync(Box::new(|s, answer| {
            s.board.page.syncing = false;
            if let Ok(result) = answer {
                s.board.page.feedback = Some((Feedback::of(result), s.clock + TOAST_SECONDS));
            }
        }));
    }

    /// The toast's Dismiss.
    pub fn board_dismiss_toast(&mut self) {
        self.board.page.feedback = None;
    }

    /// From `poll`: the toast goes when its five seconds are up.
    pub fn keep_board_toast(&mut self, now: f64, tx: &mut Tx) -> bool {
        match &self.board.page.feedback {
            Some((_, until)) if now >= *until => {
                self.board.page.feedback = None;
                true
            }
            Some(_) => {
                tx.wake_after(Duration::from_millis(100));
                false
            }
            None => false,
        }
    }
}
