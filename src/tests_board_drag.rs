//! Component 23, piece O: dragging a card. `KanbanBoard.test.ts`' two drag cases and `KanbanCard.test.ts`' "applies drag and
//! manual styling" under their names, against a fake Fleet; then what this app adds (the same place sends nothing, a refused
//! move puts the card back, Escape, no drag while a change runs) and the drop index's rule.
use crate::State;
use crate::fleet::board::drag::{Target, drop_index, target_of};
use crate::fleet::board::store::Mode;
use crate::tests::FRAME;
use crate::tests_board_store::{Rig, loaded, rig};
use rune::{Key, KeyEvent, Mods, Runtime};

const MOVE: &str = "POST /api/boards/board-1/cards/card-backlog/move";

fn at(rt: &Runtime<State>, key: &str) -> (f32, f32) {
    rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"))
}

/// Press on a card, move off it (the drag begins), then to `to` and stay there with the mouse down.
fn grab(rt: &mut Runtime<State>, card: &str, to: &str) -> (f32, f32) {
    // The card's own key is its middle, which may be a button's: press on its title, which no button covers.
    let (x, y) = at(rt, &format!("board-card-{card}-title"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_move(x + 12.0, y + 4.0);
    rt.frame(FRAME);
    rt.frame(FRAME);
    let end = at(rt, to);
    rt.pointer_move(end.0, end.1);
    rt.frame(FRAME);
    rt.frame(FRAME);
    end
}

fn release(rig: &mut Rig, end: (f32, f32)) {
    rig.rt.pointer_up(end.0, end.1);
    rig.rt.frame(FRAME);
    rig.settle("the move");
}

fn drag_to(rig: &mut Rig, card: &str, to: &str) {
    let end = grab(&mut rig.rt, card, to);
    release(rig, end);
}

/// Fleet's fixture for the two drag cases: the card in the first lane, the second lane with none.
fn two_lanes() -> crate::tests_board_store::Kept {
    let mut kept = loaded();
    kept.cards.retain(|c| c.id == "card-backlog");
    kept
}

fn over(rig: &Rig) -> Option<(String, usize)> {
    rig.rt.state.board.compose.drag.as_ref().and_then(|d| d.over.clone())
}

// ─── KanbanBoard.test.ts ────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn keeps_card_drag_and_drop_working_in_work_mode() {
    let mut rig = rig(two_lanes());
    rig.open();
    assert_eq!(rig.rt.state.board.mode, Mode::Work);
    // Fleet drops on the second lane's cards area, with no slot under it: the end, which is index 0 in an empty lane.
    drag_to(&mut rig, "card-backlog", "board-lane-lane-doing-empty");
    assert_eq!(rig.bodies_of(MOVE), [serde_json::json!({ "laneId": "lane-doing", "position": 0 })]);
    assert_eq!(rig.rt.state.board.card("card-backlog").map(|c| c.lane_id.as_str()), Some("lane-doing"));
    assert!(rig.rt.state.board.compose.drag.is_none(), "the drag ends");
}

#[test]
fn shows_structural_controls_and_keeps_card_drag_and_drop_working_in_manage_mode() {
    let mut rig = rig(two_lanes());
    rig.open();
    rig.rt.update(|s, _| s.board.set_mode(Mode::Manage));
    rig.rt.frame(FRAME);
    assert!(rig.rt.find("board-lane-lane-backlog-actions").is_some() && rig.rt.find("board-lane-lane-doing-actions").is_some());
    drag_to(&mut rig, "card-backlog", "board-lane-lane-doing-empty");
    assert_eq!(rig.bodies_of(MOVE), [serde_json::json!({ "laneId": "lane-doing", "position": 0 })]);
    assert_eq!(rig.rt.state.board.card("card-backlog").map(|c| c.lane_id.as_str()), Some("lane-doing"));
}

// ─── KanbanCard.test.ts ─────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn applies_drag_and_manual_styling() {
    let mut rig = rig(loaded());
    rig.open();
    let opacity =
        |rig: &Rig| rig.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("board-card-card-backlog")).map(|e| e.opacity);
    assert_eq!(opacity(&rig), Some(1.0), "not dragging: whole");
    let to = grab(&mut rig.rt, "card-backlog", "board-slot-lane-review-1");
    assert!(rig.rt.state.board.compose.drag.is_some(), "the press-and-move begins a drag: the card is draggable");
    assert!(opacity(&rig).is_some_and(|o| (o - 0.45).abs() < 0.01), "k-card--dragging: opacity .45, got {:?}", opacity(&rig));
    // A manual card has no source, so it has its own edge and fill (`k-card--manual`), which the drag leaves alone.
    assert!(rig.rt.state.board.card("card-backlog").is_some_and(|c| c.source_type.is_none()));
    release(&mut rig, to);
}

// ─── this app ───────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_slot_under_the_pointer_takes_the_drop_and_grows() {
    let mut rig = rig(loaded());
    rig.open();
    let small = rig.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("board-slot-lane-review-1")).map(|e| e.rect);
    let end = grab(&mut rig.rt, "card-backlog", "board-slot-lane-review-0");
    assert_eq!(over(&rig), Some(("lane-review".to_string(), 0)));
    let tall = |rig: &Rig, key: &str| rig.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).map(|e| e.rect[3]);
    assert!(tall(&rig, "board-slot-lane-review-0").is_some_and(|h| h > 17.0), "the active slot is 18 high");
    assert!(small.is_some_and(|r| r[3] < 9.0), "the others are 8");
    release(&mut rig, end);
    assert_eq!(rig.bodies_of(MOVE), [serde_json::json!({ "laneId": "lane-review", "position": 0 })]);
    assert_eq!(rig.card_ids("lane-review"), ["card-backlog", "card-review"]);
}

#[test]
fn a_drop_in_the_same_place_sends_nothing() {
    let mut rig = rig(loaded());
    rig.open();
    // The card is the only one in its lane: the slot before it and the one after it are both its own place.
    for slot in ["board-slot-lane-backlog-0", "board-slot-lane-backlog-1"] {
        drag_to(&mut rig, "card-backlog", slot);
        assert_eq!(rig.heard_exactly("POST /api/boards/board-1/cards/card-backlog/move"), 0, "{slot}");
        assert!(rig.rt.state.board.compose.drag.is_none());
        assert_eq!(rig.card_ids("lane-backlog"), ["card-backlog"]);
    }
}

#[test]
fn a_refused_move_puts_the_card_back_and_shows_the_banner() {
    let mut rig = rig(loaded());
    rig.open();
    rig.kept().fail_move = Some("Card move failed.".into());
    drag_to(&mut rig, "card-backlog", "board-slot-lane-review-1");
    assert_eq!(rig.heard_exactly("POST /api/boards/board-1/cards/card-backlog/move"), 1);
    assert_eq!(rig.card_ids("lane-backlog"), ["card-backlog"], "back in its lane");
    assert_eq!(rig.card_ids("lane-review"), ["card-review"]);
    assert_eq!(rig.rt.state.board.error.as_deref(), Some("Card move failed."));
    assert!(rig.rt.find("board-error-text").is_some(), "the banner");
}

#[test]
fn escape_puts_the_card_down_without_a_move() {
    let mut rig = rig(loaded());
    rig.open();
    let end = grab(&mut rig.rt, "card-backlog", "board-slot-lane-review-1");
    rig.rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rig.rt.frame(FRAME);
    release(&mut rig, end);
    assert_eq!(rig.heard_exactly("POST /api/boards/board-1/cards/card-backlog/move"), 0);
    assert!(rig.rt.state.board.compose.drag.is_none());
}

#[test]
fn a_card_does_not_drag_while_a_change_runs() {
    let mut rig = rig(loaded());
    rig.open();
    rig.rt.update(|s, _| s.board.pending = 1);
    rig.rt.frame(FRAME);
    let end = grab(&mut rig.rt, "card-backlog", "board-slot-lane-review-1");
    assert!(rig.rt.state.board.compose.drag.is_none(), "draggable is off while mutating");
    rig.rt.pointer_up(end.0, end.1);
    assert_eq!(rig.heard_exactly("POST /api/boards/board-1/cards/card-backlog/move"), 0);
}

#[test]
fn over_a_lanes_cards_with_no_slot_the_drop_is_at_the_end_and_the_lane_is_tinted() {
    let mut rig = rig(loaded());
    rig.open();
    let end = grab(&mut rig.rt, "card-backlog", "board-card-card-review");
    assert_eq!(over(&rig), Some(("lane-review".to_string(), 1)), "no slot: the end of the lane's cards");
    release(&mut rig, end);
    assert_eq!(rig.card_ids("lane-review"), ["card-review", "card-backlog"]);
}

#[test]
fn a_pointer_off_every_lane_has_no_place_and_drops_nothing() {
    let mut rig = rig(loaded());
    rig.open();
    let end = grab(&mut rig.rt, "card-backlog", "board-mode");
    release(&mut rig, end);
    assert_eq!(rig.heard_exactly("POST /api/boards/board-1/cards/card-backlog/move"), 0);
}

// ─── the rules, by themselves ───────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_drop_index_counts_one_less_below_its_own_place_in_its_own_lane() {
    // Card at 1 of a lane: slots 1 and 2 are its own place, 0 and 3 move it.
    assert_eq!(drop_index("a", Some(1), "a", 0), Some(0));
    assert_eq!(drop_index("a", Some(1), "a", 1), None);
    assert_eq!(drop_index("a", Some(1), "a", 2), None);
    assert_eq!(drop_index("a", Some(1), "a", 3), Some(2));
    // Another lane takes the index as it is.
    assert_eq!(drop_index("a", Some(1), "b", 2), Some(2));
    assert_eq!(drop_index("a", Some(1), "b", 0), Some(0));
}

#[test]
fn slots_and_cards_areas_are_read_from_keys_with_hyphens_in_the_lane_id() {
    let lanes = ["lane", "lane-doing", "lane-doing-2"];
    let read = |p: &str| target_of(p, lanes.iter().copied());
    assert_eq!(read("a/board-slot-lane-doing-2-7/#1"), Some(Target::Slot("lane-doing-2".into(), 7)));
    assert_eq!(read("a/board-lane-lane-doing-cards/#3/board-card-x"), Some(Target::Cards("lane-doing".into())));
    assert_eq!(read("a/board-lane-lane-doing/board-lane-lane-doing-title"), None);
}
