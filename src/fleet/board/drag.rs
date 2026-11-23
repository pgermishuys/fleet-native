//! Dragging a card (`KanbanBoard.vue` 343-383, `KanbanColumn.vue` 89-164 and 347-379, `KanbanCard.vue` 179-197).
//! Fleet uses the browser's drag-and-drop; Rune has none with targets, so the card's pointer drag does the whole of it, as
//! `row_drag.rs` does for session rows: each move asks the runtime which keyed element is under the pointer (`Tx::element_at`)
//! and the path names the slot (`board-slot-<lane>-<i>`) or the lane's cards (`board-lane-<id>-cards`). The release drops on the
//! last place found. A drag stays inside the Board: no browser ghost and no auto-scroll of the lanes row (25.3).
use super::Card;
use crate::{State, Tx};
use rune::Picked;

/// The card being dragged (`draggedCard`) and the slot it is over (`activeDropIndex` of the lane under the pointer).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardDrag {
    pub card: String,
    pub lane: String,
    /// The lane the pointer is over and the slot in it that grows; `None` while it is over no lane's cards.
    pub over: Option<(String, usize)>,
}

/// `.kanban-col__drop-slot` of a lane, before the card at `index` (`index` = the card count: the last one).
pub fn slot_key(lane: &str, index: usize) -> String {
    format!("board-slot-{lane}-{index}")
}

/// `.kanban-col__cards`, which takes the drop when no slot is under the pointer.
pub fn cards_key(lane: &str) -> String {
    format!("board-lane-{lane}-cards")
}

/// What a pick's path names: a slot of a lane, or the cards of a lane with no slot under the pointer.
#[derive(Debug, PartialEq, Eq)]
pub enum Target {
    Slot(String, usize),
    Cards(String),
}

/// The innermost slot or cards area on the path. Lane ids hold hyphens, so each known lane's id is tried in turn.
pub fn target_of<'a>(path: &str, lanes: impl Iterator<Item = &'a str> + Clone) -> Option<Target> {
    for seg in path.rsplit('/') {
        for lane in lanes.clone() {
            if let Some(i) = seg.strip_prefix(format!("board-slot-{lane}-").as_str()).and_then(|i| i.parse().ok()) {
                return Some(Target::Slot(lane.to_string(), i));
            }
            if seg == cards_key(lane) {
                return Some(Target::Cards(lane.to_string()));
            }
        }
    }
    None
}

/// `handleDropCard`: where the card goes in its target lane, or `None` when that is the place it already has. In its own lane a
/// slot below its own place counts one less, because the card leaves its place first.
pub fn drop_index(from_lane: &str, current: Option<usize>, to_lane: &str, index: usize) -> Option<usize> {
    let mut next = index;
    if from_lane == to_lane {
        if current.is_some_and(|at| at < index) {
            next = index - 1;
        }
        if current == Some(next) {
            return None;
        }
    }
    Some(next)
}

impl State {
    /// `draggable` is `!isMutating`: a card can't be picked up while a change runs.
    pub fn board_card_drag_move(&mut self, card: &str, at: [f32; 2], tx: &mut Tx) {
        if self.board.compose.drag.is_none() {
            let Some(lane) = self.board.card(card).map(|c| c.lane_id.clone()).filter(|_| !self.board.is_mutating()) else { return };
            self.board.compose.drag = Some(CardDrag { card: card.to_string(), lane, over: None });
        }
        tx.element_at(at[0], at[1], |s: &mut State, picked: Option<Picked>, tx: &mut Tx| {
            let target = picked.and_then(|p| target_of(p.path(), s.board.lanes.iter().map(|l| l.id.as_str())));
            s.board_drag_over(target);
            // The slot grows on a 200 ms ease (`transition: height .2s`); a drag's own commits land at once otherwise.
            tx.motion_curve(rune::Curve::tween(0.2, rune::Easing::cubic(0.25, 0.1, 0.25, 1.0)));
        });
    }

    /// The pointer moved to `target`. A slot is the place; over a lane's cards with no slot it is the place it had in that lane
    /// (`handleCardsDragOver` only sets one when there is none), else the end; anywhere else, none.
    fn board_drag_over(&mut self, target: Option<Target>) {
        let Some(d) = self.board.compose.drag.as_ref() else { return };
        let over = match target {
            Some(Target::Slot(lane, i)) => Some((lane, i)),
            Some(Target::Cards(lane)) => match &d.over {
                Some((was, i)) if *was == lane => Some((lane, *i)),
                _ => {
                    let end = self.board.cards_for_lane(&lane).len();
                    Some((lane, end))
                }
            },
            None => None,
        };
        if let Some(d) = self.board.compose.drag.as_mut() {
            d.over = over;
        }
    }

    /// The release (`handleDropCard`). `cancelled` (Escape, a lost pointer) drops nothing. The same place sends nothing; any other
    /// goes through `moveCard`, which shows the card there at once and puts it back, with the banner, if the server refuses.
    pub fn board_card_drag_end(&mut self, cancelled: bool) {
        let Some(CardDrag { card, lane, over }) = self.board.compose.drag.take() else { return };
        let Some((to, index)) = over.filter(|_| !cancelled) else { return };
        let current = self.board.cards_for_lane(&lane).iter().position(|c| c.id == card);
        let Some(next) = drop_index(&lane, current, &to, index) else { return };
        self.board_move_card(&card, &to, next, super::store::ignore());
    }

    /// The card is the one being dragged (`isDragging`).
    pub fn board_is_dragging(&self, card: &Card) -> bool {
        self.board.compose.drag.as_ref().is_some_and(|d| d.card == card.id)
    }
}
