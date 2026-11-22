//! Component 23, piece M: the Board's lanes and cards in work mode. `KanbanCard.test.ts`' five cases (the Delete pair, the
//! summary, the synced GitHub card and the stale one; "applies drag and manual styling" is piece O's), `KanbanColumn.test.ts`'
//! "hides manage-only lane actions in work mode while still allowing card creation" and `KanbanBoard.test.ts`' "creates a card
//! from the lane composer with mocked API responses", under their names, against a fake Fleet; then what this app adds.
use crate::fleet::actions::{CONFIRM_ACTION, CONFIRM_CANCEL};
use crate::fleet::board::Card;
use crate::tests::{FRAME, click, press_key};
use crate::tests_board_store::{Rig, card_fixture, loaded, rig};
use crate::{Rail, State};
use rune::{Key, Mods, Runtime};
use serde_json::json;

fn words(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().iter().filter(|e| !e.exiting).filter_map(|e| e.text.clone()).collect()
}

fn text_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).and_then(|e| e.text)
}

fn label_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).and_then(|e| e.label)
}

fn shown(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn says(rt: &Runtime<State>, what: &str) -> bool {
    words(rt).iter().any(|w| w == what)
}

fn type_into(rt: &mut Runtime<State>, key: &str, words: &str) {
    click(rt, key);
    press_key(rt, Key::Text(words.into()), Mods::default());
}

fn enter(rt: &mut Runtime<State>) {
    press_key(rt, Key::Enter, Mods::default());
}

/// `createGitHubMetadata`.
fn github_metadata(extra: serde_json::Value) -> String {
    let mut m = json!({
        "number": 42, "state": "open", "labels": ["bug", "urgent"], "assignee": "hubot",
        "html_url": "https://github.com/acme/rocket/issues/42", "updated_at": "2026-02-27T10:03:00Z",
    });
    if let (Some(m), Some(extra)) = (m.as_object_mut(), extra.as_object()) {
        m.extend(extra.clone());
    }
    m.to_string()
}

/// A synced GitHub card in the backlog, as `KanbanCard.test.ts` makes one.
fn github_card(extra: serde_json::Value) -> Card {
    Card {
        metadata: Some(github_metadata(extra)),
        ..card_fixture("card-gh", "lane-backlog", "Ship the Vue migration", Some("github"), "github:acme/rocket#42", 5)
    }
}

/// The board of `loaded()` with `cards` in it, open on the Board.
fn open_with(cards: Vec<Card>) -> Rig {
    let mut kept = loaded();
    kept.cards = cards;
    let mut rig = rig(kept);
    rig.open();
    rig.rt.frame(FRAME);
    rig
}

fn manual_card() -> Card {
    Card {
        metadata: Some("Migrate the board to Vue".into()),
        ..card_fixture("card-1", "lane-backlog", "Ship the Vue migration", None, "manual-task-1", 5)
    }
}

// ─── KanbanCard.test.ts ─────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn delete_asks_first_naming_the_card_and_deletes_once_confirmed() {
    let mut rig = open_with(vec![manual_card()]);
    click(&mut rig.rt, "board-card-card-1-delete");
    let c = rig.rt.state.ui.confirm.as_ref().expect("a question");
    assert_eq!(
        (c.title.as_str(), c.subject.as_str(), c.confirm_label.as_str()),
        ("Delete this card?", "Ship the Vue migration", "Delete card")
    );
    assert!(c.destructive);
    assert_eq!(c.body, "It's removed from the board. Archive it instead to keep it out of the way.");
    assert_eq!(rig.heard_exactly("DELETE /api/boards/board-1/cards/card-1"), 0, "nothing is sent before the answer");
    click(&mut rig.rt, CONFIRM_ACTION);
    rig.wait("the delete", |s| s.board.card("card-1").is_none());
    assert_eq!(rig.heard_exactly("DELETE /api/boards/board-1/cards/card-1"), 1);
    assert!(rig.rt.state.ui.confirm.is_none());
}

#[test]
fn delete_keeps_the_card_when_the_confirm_is_cancelled() {
    let mut rig = open_with(vec![manual_card()]);
    click(&mut rig.rt, "board-card-card-1-delete");
    click(&mut rig.rt, CONFIRM_CANCEL);
    rig.rt.frame(FRAME);
    assert!(rig.rt.state.ui.confirm.is_none());
    assert_eq!(rig.heard_exactly("DELETE /api/boards/board-1/cards/card-1"), 0);
    assert!(rig.rt.state.board.card("card-1").is_some());
    assert!(shown(&rig.rt, "board-card-card-1"));
}

#[test]
fn renders_the_card_summary_and_metadata() {
    let rig = open_with(vec![manual_card()]);
    let rt = &rig.rt;
    assert_eq!(text_of(rt, "board-card-card-1-title").as_deref(), Some("Ship the Vue migration"));
    assert_eq!(text_of(rt, "board-card-card-1-source").as_deref(), Some("MANUAL"));
    assert_eq!(text_of(rt, "board-card-card-1-metadata").as_deref(), Some("Migrate the board to Vue"));
    assert_eq!(text_of(rt, "board-card-card-1-key-value").as_deref(), Some("manual-task-1"));
    assert_eq!(text_of(rt, "board-card-card-1-position-value").as_deref(), Some("1024"));
    assert!(text_of(rt, "board-card-card-1-updated").is_some_and(|t| t.starts_with("Updated ")), "{:?}", words(rt));
    assert!(!shown(rt, "board-card-card-1-github"), "a manual card has no GitHub block");
}

#[test]
fn renders_synced_github_card_treatment_with_outbound_link() {
    let rig = open_with(vec![github_card(json!({}))]);
    let rt = &rig.rt;
    assert_eq!(text_of(rt, "board-card-card-gh-source").as_deref(), Some("GITHUB SYNC"));
    assert_eq!(text_of(rt, "board-card-card-gh-issue").as_deref(), Some("Issue #42"));
    assert_eq!(label_of(rt, "board-card-card-gh-link").as_deref(), Some("Open Issue #42 on GitHub"));
    for (key, want) in [("state", "open"), ("assignee", "hubot")] {
        assert_eq!(text_of(rt, &format!("board-card-card-gh-{key}-value")).as_deref(), Some(want));
    }
    assert!(shown(rt, "board-card-card-gh-github-updated-value"));
    assert!(says(rt, "bug") && says(rt, "urgent"), "{:?}", words(rt));
    assert!(!shown(rt, "board-card-card-gh-stale"));
}

#[test]
fn shows_stale_indicator_for_stale_synced_cards() {
    let rig = open_with(vec![github_card(json!({ "stale": true }))]);
    assert_eq!(text_of(&rig.rt, "board-card-card-gh-stale").as_deref(), Some("Stale"));
    assert_eq!(label_of(&rig.rt, "board-card-card-gh-stale-pill").as_deref(), Some("Sync is stale"));
}

// ─── KanbanColumn.test.ts ───────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn hides_manage_only_lane_actions_in_work_mode_while_still_allowing_card_creation() {
    let rig = open_with(vec![manual_card()]);
    let rt = &rig.rt;
    for lane in ["lane-backlog", "lane-doing"] {
        for action in ["rename", "delete", "left", "right", "inbox"] {
            assert!(!shown(rt, &format!("board-lane-{lane}-{action}")), "{lane} shows {action} in work mode");
        }
        assert!(shown(rt, &format!("board-lane-{lane}-add")), "{lane} lets a card be added");
    }
    assert!(says(rt, "+ Add a card"));
    for manage_only in ["←", "→", "Make inbox", "Rename lane"] {
        assert!(!says(rt, manage_only), "{manage_only:?} is manage only");
    }
}

// ─── KanbanBoard.test.ts ────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn creates_a_card_from_the_lane_composer_with_mocked_api_responses() {
    let mut kept = loaded();
    kept.cards.clear();
    let mut rig = rig(kept);
    rig.open();
    rig.rt.frame(FRAME);
    click(&mut rig.rt, "board-lane-lane-backlog-add");
    type_into(&mut rig.rt, "board-composer-lane-backlog-input", "Write board store tests");
    click(&mut rig.rt, "board-lane-lane-backlog-submit");
    rig.wait("the new card", |s| s.board.cards.len() == 1);
    rig.rt.frame(FRAME);
    assert_eq!(
        rig.bodies_of("POST /api/boards/board-1/cards"),
        vec![json!({ "laneId": "lane-backlog", "title": "Write board store tests" })]
    );
    assert!(says(&rig.rt, "Write board store tests"), "{:?}", words(&rig.rt));
    assert_eq!(text_of(&rig.rt, "board-lane-lane-backlog-count").as_deref(), Some("1 CARD"));
    assert!(!shown(&rig.rt, "board-composer-lane-backlog-input"), "the form closes at once");
}

// ─── What this app adds ─────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_composer_sends_the_trimmed_title_on_enter_and_does_nothing_for_an_empty_one() {
    let mut rig = open_with(vec![manual_card()]);
    click(&mut rig.rt, "board-lane-lane-backlog-add");
    type_into(&mut rig.rt, "board-composer-lane-backlog-input", "   ");
    enter(&mut rig.rt);
    assert!(shown(&rig.rt, "board-composer-lane-backlog-input"), "an empty title leaves the form open");
    assert_eq!(rig.heard_exactly("POST /api/boards/board-1/cards"), 0);
    type_into(&mut rig.rt, "board-composer-lane-backlog-input", "  Pack the kite  ");
    enter(&mut rig.rt);
    rig.wait("the new card", |s| s.board.cards.len() == 2);
    assert_eq!(rig.bodies_of("POST /api/boards/board-1/cards")[0]["title"], "Pack the kite");
}

#[test]
fn escape_cancels_the_composer_and_forgets_the_draft() {
    let mut rig = open_with(vec![manual_card()]);
    click(&mut rig.rt, "board-lane-lane-backlog-add");
    type_into(&mut rig.rt, "board-composer-lane-backlog-input", "Half a thought");
    press_key(&mut rig.rt, Key::Escape, Mods::default());
    assert!(!shown(&rig.rt, "board-composer-lane-backlog-input"));
    assert!(shown(&rig.rt, "board-lane-lane-backlog-add"));
    click(&mut rig.rt, "board-lane-lane-backlog-add");
    assert_eq!(rig.rt.state.board.compose.composers["lane-backlog"].text, "");
}

#[test]
fn renaming_a_card_sends_a_changed_title_only() {
    let mut rig = open_with(vec![manual_card()]);
    click(&mut rig.rt, "board-card-card-1-rename");
    let field = &rig.rt.state.board.compose.renames["card-1"];
    assert_eq!(field.text, "Ship the Vue migration", "the field starts with the title");
    assert_eq!(label_of(&rig.rt, "board-card-card-1-save").as_deref(), Some("Save"));
    // Unchanged: closes, sends nothing.
    click(&mut rig.rt, "board-card-card-1-save");
    assert!(rig.rt.state.board.compose.renames.is_empty());
    assert_eq!(rig.heard_exactly("PATCH /api/boards/board-1/cards/card-1"), 0);
    // Changed: the trimmed title goes.
    click(&mut rig.rt, "board-card-card-1-rename");
    rig.rt.update(|s, _| drop(s.board.compose.renames.insert("card-1".into(), rune::TextEdit::new("  Ship the native port "))));
    rig.rt.frame(FRAME);
    click(&mut rig.rt, "board-card-card-1-save");
    rig.wait("the rename", |s| s.board.card("card-1").is_some_and(|c| c.title == "Ship the native port"));
    assert_eq!(rig.bodies_of("PATCH /api/boards/board-1/cards/card-1"), vec![json!({ "title": "Ship the native port" })]);
}

#[test]
fn an_empty_rename_cancels_and_escape_keeps_the_title() {
    let mut rig = open_with(vec![manual_card()]);
    click(&mut rig.rt, "board-card-card-1-rename");
    rig.rt.update(|s, _| drop(s.board.compose.renames.insert("card-1".into(), rune::TextEdit::new("   "))));
    click(&mut rig.rt, "board-card-card-1-save");
    assert!(rig.rt.state.board.compose.renames.is_empty());
    assert_eq!(rig.heard_exactly("PATCH /api/boards/board-1/cards/card-1"), 0);
    click(&mut rig.rt, "board-card-card-1-rename");
    click(&mut rig.rt, "board-rename-card-1-input");
    press_key(&mut rig.rt, Key::Escape, Mods::default());
    assert!(rig.rt.state.board.compose.renames.is_empty());
    assert_eq!(text_of(&rig.rt, "board-card-card-1-title").as_deref(), Some("Ship the Vue migration"));
}

#[test]
fn archive_asks_nothing_and_the_card_leaves_the_board() {
    let mut rig = open_with(vec![manual_card()]);
    click(&mut rig.rt, "board-card-card-1-archive");
    assert!(rig.rt.state.ui.confirm.is_none());
    rig.wait("the archive", |s| s.board.card("card-1").is_some_and(|c| c.is_archived()));
    rig.rt.frame(FRAME);
    assert_eq!(rig.heard_exactly("POST /api/boards/board-1/cards/card-1/archive"), 1);
    assert!(!shown(&rig.rt, "board-card-card-1"));
    assert_eq!(text_of(&rig.rt, "board-lane-lane-backlog-count").as_deref(), Some("0 CARDS"));
}

#[test]
fn an_empty_lane_says_what_to_do() {
    let mut rig = open_with(Vec::new());
    for (lane, want) in [("lane-backlog", "Inbox is empty."), ("lane-doing", "Drop a card here or add one below.")] {
        assert!(shown(&rig.rt, &format!("board-lane-{lane}-empty")));
        assert!(says(&rig.rt, want), "{lane}: {:?}", words(&rig.rt));
    }
    assert_eq!(rig.rt.state.rail, Rail::Board);
    assert!(text_of(&rig.rt, "board-lane-lane-backlog-inbox-pill").is_some_and(|t| t == "INBOX"));
    rig.rt.frame(FRAME);
}
