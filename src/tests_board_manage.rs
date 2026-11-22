//! Component 23, piece N: the Board's manage mode. `KanbanColumn.test.ts`' four manage cases and `KanbanBoard.test.ts`'
//! "creates, renames, and assigns an inbox lane through the board UI" and "cancels manage-only forms when switching back to
//! work mode", under their names, against a fake Fleet; then what this app adds (moving lanes, Esc, empty and unchanged names).
use crate::State;
use crate::fleet::actions::{CONFIRM_ACTION, CONFIRM_CANCEL};
use crate::fleet::board::store::Mode;
use crate::tests::{FRAME, click, press_key};
use crate::tests_board_store::{Rig, empty, loaded, rig};
use rune::{Key, Mods, Runtime, TextEdit};

fn words(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().iter().filter(|e| !e.exiting).filter_map(|e| e.text.clone()).collect()
}

fn says(rt: &Runtime<State>, what: &str) -> bool {
    words(rt).iter().any(|w| w == what)
}

fn shown(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn text_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).and_then(|e| e.text)
}

fn is_disabled(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).is_some_and(|e| e.opacity < 1.0)
}

fn type_into(rt: &mut Runtime<State>, key: &str, words: &str) {
    click(rt, key);
    press_key(rt, Key::Text(words.into()), Mods::default());
}

fn enter(rt: &mut Runtime<State>) {
    press_key(rt, Key::Enter, Mods::default());
}

/// The invented board, open on the Board in manage mode.
fn managing(kept: crate::tests_board_store::Kept) -> Rig {
    let mut rig = rig(kept);
    rig.open();
    rig.rt.update(|s, _| s.board.set_mode(Mode::Manage));
    rig.rt.frame(FRAME);
    rig
}

fn lane_names(rig: &Rig) -> Vec<String> {
    rig.rt.state.board.sorted_lanes().iter().map(|l| l.name.clone()).collect()
}

// ─── KanbanColumn.test.ts ───────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn shows_lane_management_controls_in_manage_mode() {
    let rig = managing(loaded());
    let rt = &rig.rt;
    assert!(shown(rt, "board-lane-lane-backlog-actions"));
    assert!(says(rt, "Rename") && says(rt, "Make inbox") && says(rt, "Delete"), "{:?}", words(rt));
    // The inbox lane's own button says so and waits; the ends have no way further.
    assert!(says(rt, "Inbox") && says(rt, "Make inbox"), "{:?}", words(rt));
    assert!(is_disabled(rt, "board-lane-lane-backlog-inbox"));
    assert!(is_disabled(rt, "board-lane-lane-backlog-left") && !is_disabled(rt, "board-lane-lane-backlog-right"));
    assert!(shown(rt, "board-adder-button"), "+ Add another lane follows the last lane");
}

#[test]
fn cancels_lane_rename_state_when_switching_from_manage_mode_back_to_work_mode() {
    let mut rig = managing(loaded());
    click(&mut rig.rt, "board-lane-lane-backlog-rename");
    rig.rt.update(|s, _| drop(s.board.compose.lane_renames.insert("lane-backlog".into(), TextEdit::new("Review"))));
    rig.rt.frame(FRAME);
    click(&mut rig.rt, "board-mode");
    assert!(!shown(&rig.rt, "board-lane-lane-backlog-rename-input"));
    assert_eq!(text_of(&rig.rt, "board-lane-lane-backlog-title").as_deref(), Some("Backlog"));
    click(&mut rig.rt, "board-mode");
    click(&mut rig.rt, "board-lane-lane-backlog-rename");
    assert_eq!(rig.rt.state.board.compose.lane_renames["lane-backlog"].text, "Backlog");
}

#[test]
fn asks_before_deleting_a_lane_says_its_cards_go_too_and_deletes_once_confirmed() {
    let mut kept = loaded();
    kept.cards.retain(|c| c.lane_id != "lane-review");
    let mut rig = managing(kept);
    // A lane with a card: Fleet's words, then the server's refusal (FB2).
    click(&mut rig.rt, "board-lane-lane-backlog-delete");
    let c = rig.rt.state.ui.confirm.clone().expect("a question");
    assert_eq!((c.title.as_str(), c.subject.as_str(), c.confirm_label.as_str()), ("Delete this lane?", "Backlog", "Delete lane"));
    assert!(c.destructive);
    assert_eq!(c.body, "Its cards are removed too, unless the server moves them to another lane.");
    assert_eq!(rig.heard_exactly("DELETE /api/boards/board-1/lanes/lane-backlog"), 0, "nothing is sent before the answer");
    click(&mut rig.rt, CONFIRM_ACTION);
    rig.wait("the refusal", |s| s.board.error.is_some());
    assert_eq!(rig.rt.state.board.error.as_deref(), Some("Cannot delete a lane that still contains cards."));
    assert!(rig.rt.state.board.lane("lane-backlog").is_some());
    // An empty one goes.
    click(&mut rig.rt, "board-lane-lane-review-delete");
    assert_eq!(rig.rt.state.ui.confirm.as_ref().map(|c| c.body.as_str()), Some("The lane is empty, so no cards are lost."));
    click(&mut rig.rt, CONFIRM_ACTION);
    rig.wait("the delete", |s| s.board.lane("lane-review").is_none());
    assert_eq!(rig.heard_exactly("DELETE /api/boards/board-1/lanes/lane-review"), 1);
    assert!(rig.rt.state.ui.confirm.is_none());
}

#[test]
fn keeps_the_lane_when_the_confirm_is_cancelled() {
    let mut rig = managing(loaded());
    click(&mut rig.rt, "board-lane-lane-review-delete");
    click(&mut rig.rt, CONFIRM_CANCEL);
    rig.rt.frame(FRAME);
    assert!(rig.rt.state.ui.confirm.is_none());
    assert_eq!(rig.heard_exactly("DELETE /api/boards/board-1/lanes/lane-doing"), 0);
    assert!(rig.rt.state.board.lane("lane-review").is_some() && shown(&rig.rt, "board-lane-lane-review"));
}

// ─── KanbanBoard.test.ts ────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn creates_renames_and_assigns_an_inbox_lane_through_the_board_ui() {
    let mut rig = rig(empty());
    rig.open();
    click(&mut rig.rt, "board-mode");
    click(&mut rig.rt, "board-add-lane");
    type_into(&mut rig.rt, "board-creator-input", "Ready");
    enter(&mut rig.rt);
    rig.wait("the lane", |s| !s.board.lanes.is_empty() && s.board.compose.creator.is_none());
    assert_eq!(rig.heard_exactly("POST /api/boards"), 1);
    assert_eq!(rig.bodies_of("POST /api/boards")[0]["name"], "My Board");
    assert_eq!(rig.bodies_of("POST /api/boards/board-1/lanes")[0]["name"], "Ready");
    rig.rt.frame(FRAME);
    assert!(says(&rig.rt, "Ready"));
    let lane = rig.rt.state.board.lanes[0].id.clone();

    click(&mut rig.rt, &format!("board-lane-{lane}-rename"));
    rig.rt.update({
        let lane = lane.clone();
        move |s, _| drop(s.board.compose.lane_renames.insert(lane, TextEdit::new("In Review")))
    });
    rig.rt.frame(FRAME);
    click(&mut rig.rt, &format!("board-lane-{lane}-save"));
    rig.wait("the rename", |s| s.board.lanes[0].name == "In Review");
    assert_eq!(rig.bodies_of(&format!("PATCH /api/boards/board-1/lanes/{lane}"))[0], serde_json::json!({ "name": "In Review" }));
    rig.rt.frame(FRAME);
    assert!(says(&rig.rt, "In Review"));

    click(&mut rig.rt, &format!("board-lane-{lane}-inbox"));
    rig.wait("the inbox", |s| s.board.lanes[0].is_inbox);
    assert_eq!(rig.bodies_of(&format!("PATCH /api/boards/board-1/lanes/{lane}"))[1], serde_json::json!({ "isInbox": true }));
    rig.rt.frame(FRAME);
    assert!(says(&rig.rt, "Inbox") && !says(&rig.rt, "Make inbox"));
    assert!(is_disabled(&rig.rt, &format!("board-lane-{lane}-inbox")));
}

#[test]
fn cancels_manage_only_forms_when_switching_back_to_work_mode() {
    let mut rig = managing(loaded());
    click(&mut rig.rt, "board-add-lane");
    type_into(&mut rig.rt, "board-creator-input", "Ready");
    click(&mut rig.rt, "board-rename-board");
    rig.rt.update(|s, _| s.board.compose.board_rename = Some(TextEdit::new("Renamed board draft")));
    rig.rt.frame(FRAME);
    assert!(shown(&rig.rt, "board-rename-form") && shown(&rig.rt, "board-lane-creator"));

    click(&mut rig.rt, "board-mode");
    assert!(!shown(&rig.rt, "board-lane-creator") && !shown(&rig.rt, "board-rename-form"));
    assert_eq!(text_of(&rig.rt, "board-header-title").as_deref(), Some("Platform Roadmap"));

    click(&mut rig.rt, "board-mode");
    click(&mut rig.rt, "board-add-lane");
    assert_eq!(rig.rt.state.board.compose.creator.as_ref().map(|e| e.text.as_str()), Some(""));
    click(&mut rig.rt, "board-rename-board");
    assert_eq!(rig.rt.state.board.compose.board_rename.as_ref().map(|e| e.text.as_str()), Some("Platform Roadmap"));
}

// ─── what this app adds ─────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn work_mode_draws_none_of_manage_mode() {
    let mut rig = rig(loaded());
    rig.open();
    for key in ["board-add-lane", "board-rename-board", "board-adder-button", "board-lane-lane-backlog-actions"] {
        assert!(!shown(&rig.rt, key), "{key}");
    }
    rig.rt.update(|s, _| s.board_open_creator());
    assert!(rig.rt.state.board.compose.creator.is_none(), "the creator opens in manage mode only");
}

#[test]
fn the_arrows_swap_a_lane_with_its_neighbour() {
    let mut rig = managing(loaded());
    click(&mut rig.rt, "board-lane-lane-doing-right");
    rig.wait("the reorder", |s| s.board.pending == 0 && s.board.sorted_lanes()[2].id == "lane-doing");
    assert_eq!(lane_names(&rig), ["Backlog", "Review", "In Progress", "Done"]);
    assert_eq!(
        rig.bodies_of("PATCH /api/boards/board-1/lanes/reorder")[0]["laneIds"],
        serde_json::json!(["lane-backlog", "lane-review", "lane-doing", "lane-done"])
    );
    rig.rt.frame(FRAME);
    click(&mut rig.rt, "board-lane-lane-doing-left");
    rig.wait("the reorder back", |s| s.board.pending == 0 && s.board.sorted_lanes()[1].id == "lane-doing");
    assert_eq!(lane_names(&rig), ["Backlog", "In Progress", "Review", "Done"]);
}

#[test]
fn an_empty_or_unchanged_lane_name_sends_nothing() {
    let mut rig = managing(loaded());
    click(&mut rig.rt, "board-lane-lane-doing-rename");
    click(&mut rig.rt, "board-lane-lane-doing-save");
    assert!(rig.rt.state.board.compose.lane_renames.is_empty(), "an unchanged name closes");
    click(&mut rig.rt, "board-lane-lane-doing-rename");
    rig.rt.update(|s, _| drop(s.board.compose.lane_renames.insert("lane-doing".into(), TextEdit::new("   "))));
    rig.rt.frame(FRAME);
    click(&mut rig.rt, "board-lane-lane-doing-save");
    assert!(rig.rt.state.board.compose.lane_renames.is_empty(), "an empty one too");
    click(&mut rig.rt, "board-lane-lane-doing-rename");
    click(&mut rig.rt, "board-lane-lane-doing-rename-input");
    press_key(&mut rig.rt, Key::Escape, Mods::default());
    assert!(rig.rt.state.board.compose.lane_renames.is_empty(), "Esc cancels");
    assert_eq!(rig.heard_exactly("PATCH /api/boards/board-1/lanes/lane-doing"), 0);
}

#[test]
fn the_creator_needs_a_name_and_escape_does_nothing_in_it() {
    let mut rig = managing(loaded());
    // "+ Add another lane" sits past the last lane, off screen here; it does what "Add lane" does.
    rig.rt.update(|s, _| s.board_open_creator());
    rig.rt.frame(FRAME);
    assert!(shown(&rig.rt, "board-lane-creator"));
    assert!(is_disabled(&rig.rt, "board-creator-save"), "Save lane waits for a name");
    enter(&mut rig.rt);
    assert_eq!(rig.heard_exactly("POST /api/boards/board-1/lanes"), 0);
    click(&mut rig.rt, "board-creator-input");
    press_key(&mut rig.rt, Key::Escape, Mods::default());
    assert!(shown(&rig.rt, "board-lane-creator"), "FB12: Esc leaves it open");
    click(&mut rig.rt, "board-creator-cancel");
    assert!(!shown(&rig.rt, "board-lane-creator"));
}

#[test]
fn a_saved_lane_closes_the_creator() {
    let mut rig = managing(loaded());
    click(&mut rig.rt, "board-add-lane");
    type_into(&mut rig.rt, "board-creator-input", "Ready");
    click(&mut rig.rt, "board-creator-save");
    rig.wait("the lane", |s| s.board.lanes.len() == 5);
    assert!(rig.rt.state.board.compose.creator.is_none(), "closes once the server made the lane");
    assert!(lane_names(&rig).contains(&"Ready".to_string()));
}

#[test]
fn create_first_lane_opens_the_creator_and_rename_board_changes_the_name() {
    let mut rig = managing(empty());
    assert!(says(&rig.rt, "No lanes yet"));
    assert!(!shown(&rig.rt, "board-rename-board"), "no board to rename yet");
    click(&mut rig.rt, "board-empty-create");
    assert!(shown(&rig.rt, "board-lane-creator"));

    let mut rig = managing(loaded());
    click(&mut rig.rt, "board-rename-board");
    assert!(!shown(&rig.rt, "board-rename-board"), "the button leaves while the form is open");
    rig.rt.update(|s, _| s.board.compose.board_rename = Some(TextEdit::new("  Launch plan ")));
    rig.rt.frame(FRAME);
    click(&mut rig.rt, "board-rename-save");
    rig.wait("the rename", |s| s.board.board.as_ref().is_some_and(|b| b.name == "Launch plan"));
    assert!(rig.rt.state.board.compose.board_rename.is_none());
}
