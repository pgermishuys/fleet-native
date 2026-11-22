//! What the lanes and cards keep while they are drawn (`KanbanColumn.vue`'s and `KanbanCard.vue`'s own refs): which lanes have
//! their composer open and what is typed in it, which cards are being renamed, the card the pointer is over, and what each
//! button does. A change goes through the store (`board_create_card`, `board_rename_card`, …), which counts it and says why it
//! failed; with no server nothing is sent.
use super::store::ignore;
use crate::State;
use crate::fleet::actions::Call;
use crate::ui::confirm::Confirm;
use rune::{TextEdit, Tx};
use std::collections::BTreeMap;

/// `maxlength` of the lane composer's and the card rename's inputs.
pub const CARD_TITLE_MAX: usize = 240;

#[derive(Debug, Default)]
pub struct Compose {
    /// Lanes with their composer open (`isCreatingCard`), by lane id, and the draft in each.
    pub composers: BTreeMap<String, TextEdit>,
    /// Cards being renamed (`isEditing`), by card id, and the draft in each.
    pub renames: BTreeMap<String, TextEdit>,
    /// The card under the pointer, which lifts.
    pub hover: Option<String>,
}

/// An input's `maxlength`: a longer text is cut, as the browser would not have let it in.
fn limit(edit: &mut TextEdit, max: usize) {
    if edit.text.chars().count() > max {
        *edit = TextEdit::new(edit.text.chars().take(max).collect::<String>());
    }
}

impl State {
    /// "+ Add a card".
    pub fn board_begin_card(&mut self, lane: &str) {
        self.board.compose.composers.entry(lane.to_string()).or_default();
    }

    /// Cancel, and Esc in the field (`cancelCreateCard`).
    pub fn board_cancel_card(&mut self, lane: &str) {
        self.board.compose.composers.remove(lane);
    }

    /// The composer's text changed: `maxlength`.
    pub fn board_card_typed(&mut self, lane: &str) {
        if let Some(edit) = self.board.compose.composers.get_mut(lane) {
            limit(edit, CARD_TITLE_MAX);
        }
    }

    /// Add card / Enter (`submitCreateCard`): the trimmed title is sent and the composer closes at once; an empty one does
    /// nothing, and so does a press while a change runs (the field and the button are disabled then).
    pub fn board_submit_card(&mut self, lane: &str) {
        if self.board.is_mutating() {
            return;
        }
        let Some(title) = self.board.compose.composers.get(lane).map(|e| e.text.trim().to_string()).filter(|t| !t.is_empty()) else {
            return;
        };
        self.board_create_card(lane, &title, None, ignore());
        self.board_cancel_card(lane);
    }

    /// "Rename" (`beginRename`): the field starts with the title.
    pub fn board_begin_rename(&mut self, card: &str) {
        if let Some(c) = self.board.card(card) {
            let title = c.title.clone();
            self.board.compose.renames.insert(card.to_string(), TextEdit::new(title));
        }
    }

    pub fn board_cancel_rename(&mut self, card: &str) {
        self.board.compose.renames.remove(card);
    }

    pub fn board_rename_typed(&mut self, card: &str) {
        if let Some(edit) = self.board.compose.renames.get_mut(card) {
            limit(edit, CARD_TITLE_MAX);
        }
    }

    /// Save / Enter (`submitRename`): empty cancels, an unchanged title sends nothing.
    pub fn board_submit_rename(&mut self, card: &str) {
        if self.board.is_mutating() {
            return;
        }
        let Some(edit) = self.board.compose.renames.get(card) else { return };
        let title = edit.text.trim().to_string();
        let unchanged = self.board.card(card).is_none_or(|c| c.title == title);
        if !title.is_empty() && !unchanged {
            self.board_rename_card(card, &title, ignore());
        }
        self.board_cancel_rename(card);
    }

    /// "Archive": no question, the card leaves the board.
    pub fn board_archive_pressed(&mut self, card: &str) {
        if !self.board.is_mutating() {
            self.board_archive_card(card, ignore());
        }
    }

    /// "Delete" (`handleDelete`): asks first, naming the card.
    pub fn board_delete_ask(&mut self, card: &str, tx: &mut Tx) {
        if self.board.is_mutating() {
            return;
        }
        let Some(c) = self.board.card(card) else { return };
        let ask = Confirm::delete_card(c.title.clone(), Call::DeleteCard { id: card.to_string() });
        self.ask(ask, tx);
    }

    /// Delete card, once confirmed.
    pub fn board_delete_confirmed(&mut self, card: &str) {
        self.board.compose.renames.remove(card);
        self.board_delete_card(card, ignore());
    }

    /// The pointer came over, or left, a card.
    pub fn board_hover_card(&mut self, card: &str, over: bool) {
        let hover = &mut self.board.compose.hover;
        if over {
            *hover = Some(card.to_string());
        } else if hover.as_deref() == Some(card) {
            *hover = None;
        }
    }
}
