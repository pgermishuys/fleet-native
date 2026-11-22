//! Manage mode's forms and buttons (`KanbanColumn.vue` and `KanbanBoard.vue`'s own refs): the lane actions (move, rename, make
//! inbox, delete), the lane creator and the board's rename. Each form keeps its draft in `Compose`, and leaving manage mode
//! drops them all (`Compose::leave_manage`). A change goes through the store, which counts it and says why it failed.
use super::compose::limit;
use super::store::ignore;
use crate::State;
use crate::fleet::actions::Call;
use crate::ui::confirm::Confirm;
use rune::{TextEdit, Tx};

/// `maxlength` of the lane's, the creator's and the board's inputs.
pub const NAME_MAX: usize = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    Left,
    Right,
}

impl State {
    // ─── a lane's rename ─────────────────────────────────────────────────────────────────────────────────────────────

    /// "Rename" (`beginLaneRename`): the field starts with the name.
    pub fn board_begin_lane_rename(&mut self, lane: &str) {
        if let Some(l) = self.board.lane(lane) {
            let name = l.name.clone();
            self.board.compose.lane_renames.insert(lane.to_string(), TextEdit::new(name));
        }
    }

    pub fn board_cancel_lane_rename(&mut self, lane: &str) {
        self.board.compose.lane_renames.remove(lane);
    }

    pub fn board_lane_rename_typed(&mut self, lane: &str) {
        if let Some(edit) = self.board.compose.lane_renames.get_mut(lane) {
            limit(edit, NAME_MAX);
        }
    }

    /// Save / Enter (`submitLaneRename`): empty or unchanged closes without a request.
    pub fn board_submit_lane_rename(&mut self, lane: &str) {
        if self.board.is_mutating() {
            return;
        }
        let Some(edit) = self.board.compose.lane_renames.get(lane) else { return };
        let name = edit.text.trim().to_string();
        let unchanged = self.board.lane(lane).is_none_or(|l| l.name == name);
        if !name.is_empty() && !unchanged {
            self.board_rename_lane(lane, &name, ignore());
        }
        self.board_cancel_lane_rename(lane);
    }

    // ─── a lane's other actions ──────────────────────────────────────────────────────────────────────────────────────

    /// "←" / "→" (`handleMoveLane`): swaps with the neighbour; at an end nothing happens.
    pub fn board_move_lane(&mut self, lane: &str, way: Way) {
        if self.board.is_mutating() {
            return;
        }
        let mut ids: Vec<String> = self.board.lanes_with_cards().into_iter().map(|(l, _)| l.id).collect();
        let Some(at) = ids.iter().position(|i| i == lane) else { return };
        let to = match way {
            Way::Left => at.checked_sub(1),
            Way::Right => Some(at + 1).filter(|n| *n < ids.len()),
        };
        let Some(to) = to else { return };
        ids.swap(at, to);
        self.board_reorder_lanes(&ids, ignore());
    }

    /// "Make inbox": nothing for the lane that already is.
    pub fn board_make_inbox(&mut self, lane: &str) {
        if self.board.is_mutating() || self.board.lane(lane).is_none_or(|l| l.is_inbox) {
            return;
        }
        self.board_set_inbox_lane(lane, ignore());
    }

    /// "Delete" (`handleDeleteLane`): asks first, naming the lane and saying whether it holds cards.
    pub fn board_delete_lane_ask(&mut self, lane: &str, tx: &mut Tx) {
        if self.board.is_mutating() {
            return;
        }
        let Some(l) = self.board.lane(lane) else { return };
        let has_cards = !self.board.cards_for_lane(lane).is_empty();
        let ask = Confirm::delete_lane(l.name.clone(), has_cards, Call::DeleteLane { id: lane.to_string() });
        self.ask(ask, tx);
    }

    /// Delete lane, once confirmed.
    pub fn board_delete_lane_confirmed(&mut self, lane: &str) {
        self.board.compose.lane_renames.remove(lane);
        self.board_delete_lane(lane, ignore());
    }

    // ─── the lane creator ────────────────────────────────────────────────────────────────────────────────────────────

    /// "Add lane", "+ Add another lane", "Create first lane" (`openAddLaneForm`): manage mode only.
    pub fn board_open_creator(&mut self) {
        if self.board.is_manage_mode() {
            self.board.compose.creator.get_or_insert_with(TextEdit::default);
        }
    }

    pub fn board_cancel_creator(&mut self) {
        self.board.compose.creator = None;
    }

    pub fn board_creator_typed(&mut self) {
        if let Some(edit) = self.board.compose.creator.as_mut() {
            limit(edit, NAME_MAX);
        }
    }

    /// Save lane / Enter (`handleCreateLane`): an empty name does nothing; the form closes once the server has made the lane,
    /// and stays, draft and all, when it refuses.
    pub fn board_submit_creator(&mut self) {
        if self.board.is_mutating() {
            return;
        }
        let Some(name) = self.board.compose.creator.as_ref().map(|e| e.text.trim().to_string()).filter(|n| !n.is_empty()) else {
            return;
        };
        self.board_create_lane(
            &name,
            None,
            Box::new(|s, answer| {
                if answer.is_ok() {
                    s.board_cancel_creator();
                }
            }),
        );
    }

    // ─── the board's rename ──────────────────────────────────────────────────────────────────────────────────────────

    /// "Rename board" (`beginBoardRename`).
    pub fn board_begin_board_rename(&mut self) {
        if let Some(b) = self.board.board.as_ref() {
            self.board.compose.board_rename = Some(TextEdit::new(b.name.clone()));
        }
    }

    pub fn board_cancel_board_rename(&mut self) {
        self.board.compose.board_rename = None;
    }

    pub fn board_name_typed(&mut self) {
        if let Some(edit) = self.board.compose.board_rename.as_mut() {
            limit(edit, NAME_MAX);
        }
    }

    /// Save board / Enter (`handleRenameBoard`): empty or the same name closes without a request; the form closes once the
    /// server has answered well.
    pub fn board_submit_board_rename(&mut self) {
        if self.board.is_mutating() {
            return;
        }
        let Some(name) = self.board.compose.board_rename.as_ref().map(|e| e.text.trim().to_string()) else { return };
        if name.is_empty() || self.board.board.as_ref().is_none_or(|b| b.name == name) {
            self.board_cancel_board_rename();
            return;
        }
        self.board_rename(
            &name,
            Box::new(|s, answer| {
                if answer.is_ok() {
                    s.board_cancel_board_rename();
                }
            }),
        );
    }
}
