//! Component 23, piece L: the Board's page. `KanbanBoard.test.ts`' four header cases under their names (the sync one against a
//! fake Fleet that answers a sync as the test's mocked API does), then what this app adds: the toast goes after 5 s and a new
//! sync restarts it, Cmd/Ctrl+Shift+E toggles the mode (not before the board loads, not from a field), the subtitles, the
//! tiles, and the states a first load leaves.
use crate::fleet::board::page::is_text_field;
use crate::fleet::board::store::Mode;
use crate::mock::board::Case;
use crate::tests::{FRAME, app, click};
use crate::tests_board_store::{loaded, refusing_sync, rig};
use crate::{Rail, State};
use rune::{Key, KeyEvent, Mods, Runtime};

fn words(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().iter().filter(|e| !e.exiting).filter_map(|e| e.text.clone()).collect()
}

fn says(rt: &Runtime<State>, what: &str) -> bool {
    words(rt).iter().any(|w| w == what)
}

fn text_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).and_then(|e| e.text)
}

/// A button's words: its label.
fn label_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).and_then(|e| e.label)
}

fn shown(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

/// Whether the button is drawn disabled (half faded, as `disabled:opacity-50` has it).
fn is_disabled(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).is_some_and(|e| e.opacity < 1.0)
}

/// The invented board of the scenes, shown and loaded as `case` says.
fn scene(case: Case) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_board(case, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt
}

fn shortcut(rt: &mut Runtime<State>) {
    let mods = Mods { shift: true, cmd: true, ctrl: !cfg!(target_os = "macos"), ..Mods::default() };
    rt.key(KeyEvent { key: Key::Command('e'), mods });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

// ─── KanbanBoard.test.ts ────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn disables_the_mode_toggle_until_the_board_finishes_loading() {
    let mut rt = scene(Case::Loading);
    assert!(!rt.state.board.is_loaded);
    assert_eq!(label_of(&rt, "board-mode").as_deref(), Some("Edit Board"));
    assert!(is_disabled(&rt, "board-mode"));
    assert!(says(&rt, "Loading board…"));
    click(&mut rt, "board-mode");
    assert_eq!(rt.state.board.mode, Mode::Work, "a press does nothing");
}

#[test]
fn keeps_the_mode_toggle_available_during_mutations() {
    let mut rt = scene(Case::Synced);
    rt.update(|s, _| s.board.pending = 1);
    rt.frame(FRAME);
    assert!(!is_disabled(&rt, "board-mode"));
    click(&mut rt, "board-mode");
    assert_eq!(rt.state.board.mode, Mode::Manage);
    assert_eq!(label_of(&rt, "board-mode").as_deref(), Some("Save"));
    assert!(is_disabled(&rt, "kanban-sync-button"), "Sync now waits for the change");
}

#[test]
fn renders_work_mode_by_default_and_exposes_manage_controls_only_in_manage_mode() {
    let mut rt = scene(Case::Synced);
    assert_eq!(rt.state.board.mode, Mode::Work);
    assert_eq!(label_of(&rt, "board-mode").as_deref(), Some("Edit Board"));
    click(&mut rt, "board-mode");
    assert_eq!(label_of(&rt, "board-mode").as_deref(), Some("Save"));
    click(&mut rt, "board-mode");
    assert_eq!(label_of(&rt, "board-mode").as_deref(), Some("Edit Board"));
    // Rename board, Add lane and the lane controls are piece N's: the header has only the two buttons until then.
}

#[test]
fn syncs_the_board_shows_feedback_and_refreshes_cards() {
    let mut rig = rig(loaded());
    rig.open();
    rig.rt.frame(FRAME);
    assert_eq!(label_of(&rig.rt, "kanban-sync-button").as_deref(), Some("Sync now"));
    let before = rig.rt.state.board.cards.len();

    click(&mut rig.rt, "kanban-sync-button");
    assert_eq!(label_of(&rig.rt, "kanban-sync-button").as_deref(), Some("Syncing…"));
    click(&mut rig.rt, "kanban-sync-button");
    rig.settle("the sync");
    rig.wait("the toast", |s| s.board.page.feedback.is_some() && !s.board.page.syncing);
    rig.rt.frame(FRAME);

    assert_eq!(rig.heard_exactly("POST /api/boards/board-1/sync"), 1, "the second press did nothing");
    assert_eq!(text_of(&rig.rt, "kanban-sync-feedback-title").as_deref(), Some("Board synced"));
    assert_eq!(text_of(&rig.rt, "kanban-sync-feedback-message").as_deref(), Some("1 added, 1 updated, 0 stale"));
    assert_eq!(rig.rt.state.board.cards.len(), before + 1);
    // The lanes' cards are drawn from piece M on; the store has the refreshed one now.
    assert!(rig.rt.state.board.cards.iter().any(|c| c.title == "Sync board store with API (updated)"));
    assert_eq!(label_of(&rig.rt, "kanban-sync-button").as_deref(), Some("Sync now"));
}

// ─── What this app adds ─────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_failed_sync_leaves_its_words_in_the_banner_and_no_toast() {
    let mut rig = rig(refusing_sync("GitHub said no."));
    rig.open();
    click(&mut rig.rt, "kanban-sync-button");
    rig.wait("the answer", |s| !s.board.page.syncing && s.board.error.is_some());
    rig.rt.frame(FRAME);
    assert_eq!(text_of(&rig.rt, "board-error-text").as_deref(), Some("GitHub said no."), "{:?}", words(&rig.rt));
    assert!(!shown(&rig.rt, "kanban-sync-feedback"));
    click(&mut rig.rt, "board-error-dismiss");
    assert!(!shown(&rig.rt, "board-error"));
}

#[test]
fn the_toast_goes_after_five_seconds_and_a_new_sync_restarts_the_timer() {
    let mut rt = scene(Case::Synced);
    rt.update(|s, _| {
        let clock = s.clock;
        let (feedback, _) = s.board.page.feedback.take().unwrap();
        s.board.page.feedback = Some((feedback, clock + 5.0));
    });
    for _ in 0..(4.0 / FRAME) as usize {
        rt.frame(FRAME);
    }
    assert!(shown(&rt, "kanban-sync-feedback"), "still there at 4 s");
    // A sync that answers now starts its own five seconds.
    rt.update(|s, _| {
        let clock = s.clock;
        s.board.page.feedback.as_mut().unwrap().1 = clock + 5.0;
    });
    for _ in 0..(4.0 / FRAME) as usize {
        rt.frame(FRAME);
    }
    assert!(shown(&rt, "kanban-sync-feedback"), "4 s after the second sync");
    for _ in 0..(1.2 / FRAME) as usize {
        rt.frame(FRAME);
    }
    assert!(!shown(&rt, "kanban-sync-feedback"), "gone after 5 s");
}

#[test]
fn dismiss_takes_the_toast_away_at_once() {
    let mut rt = scene(Case::Synced);
    assert!(shown(&rt, "kanban-sync-feedback"));
    click(&mut rt, "board-toast-dismiss");
    assert!(!shown(&rt, "kanban-sync-feedback"));
}

#[test]
fn the_shortcut_toggles_the_mode_on_a_loaded_board() {
    let mut rt = scene(Case::Synced);
    shortcut(&mut rt);
    assert_eq!(rt.state.board.mode, Mode::Manage);
    assert_eq!(label_of(&rt, "board-mode").as_deref(), Some("Save"));
    shortcut(&mut rt);
    assert_eq!(rt.state.board.mode, Mode::Work);
}

#[test]
fn the_shortcut_is_ignored_before_the_board_loads_and_off_the_board() {
    let mut rt = scene(Case::Loading);
    shortcut(&mut rt);
    assert_eq!(rt.state.board.mode, Mode::Work, "not before the board loads");

    let mut rt = scene(Case::Synced);
    rt.update(|s, _| s.rail = Rail::Sessions);
    rt.frame(FRAME);
    shortcut(&mut rt);
    assert_eq!(rt.state.board.mode, Mode::Work, "not while another view is shown");
}

#[test]
fn the_shortcut_is_ignored_in_a_text_field() {
    // The Board's fields (the lane and card composers, the rename field: pieces M and N) are keyed `board-…-input`.
    assert!(is_text_field(Some("board-lane-input")));
    assert!(is_text_field(Some("board-rename-input")));
    assert!(!is_text_field(Some("board-mode")));
    assert!(!is_text_field(Some("composer")));
    assert!(!is_text_field(None));
}

#[test]
fn the_subtitle_says_what_the_board_holds_with_no_singulars() {
    let rt = scene(Case::Synced);
    let lanes = rt.state.board.lanes.len();
    let cards = rt.state.board.active_cards().count();
    assert_eq!(
        text_of(&rt, "board-header-description").as_deref(),
        Some(format!("{cards} active cards across {lanes} lanes. Inbox lane: Inbox.").as_str())
    );
    let mut rt = scene(Case::Synced);
    rt.update(|s, _| {
        for lane in &mut s.board.lanes {
            lane.is_inbox = false;
        }
    });
    rt.frame(FRAME);
    assert_eq!(
        text_of(&rt, "board-header-description").as_deref(),
        Some(format!("{cards} active cards across {lanes} lanes. Choose an inbox lane to capture manual work quickly.").as_str())
    );
    let rt = scene(Case::Loading);
    assert_eq!(text_of(&rt, "board-header-description").as_deref(), Some("Loading your persisted board…"));
    let rt = scene(Case::Empty);
    assert_eq!(
        text_of(&rt, "board-header-description").as_deref(),
        Some("Create your first lane to start a persistent board for manual and API-backed cards.")
    );
}

#[test]
fn the_header_names_the_board_and_the_tiles_count_it() {
    let rt = scene(Case::Synced);
    assert_eq!(text_of(&rt, "board-header-eyebrow").as_deref(), Some("PERSISTENT BOARD"));
    assert_eq!(text_of(&rt, "board-header-title").as_deref(), Some("Kite launch"));
    let b = &rt.state.board;
    let manual = b.active_cards().filter(|c| c.source_type.is_none()).count();
    assert_eq!(text_of(&rt, "board-tile-lanes-value"), Some(b.lanes.len().to_string()));
    assert_eq!(text_of(&rt, "board-tile-cards-value"), Some(b.active_cards().count().to_string()));
    assert_eq!(text_of(&rt, "board-tile-manual-value"), Some(manual.to_string()));
    assert_eq!(text_of(&rt, "board-tile-inbox-value").as_deref(), Some("Inbox"));
    assert!(shown(&rt, "kanban-sync-button"));
}

#[test]
fn a_fleet_with_no_board_says_no_lanes_yet_and_offers_no_sync() {
    let rt = scene(Case::Empty);
    assert_eq!(text_of(&rt, "board-header-title").as_deref(), Some("Kanban Board"));
    assert!(says(&rt, "No lanes yet"));
    assert!(says(&rt, "Click Edit Board to unlock lane management, then add your first board column."));
    assert!(!shown(&rt, "kanban-sync-button"));
    assert!(!is_disabled(&rt, "board-mode"));
    assert_eq!(text_of(&rt, "board-tile-inbox-value").as_deref(), Some("Unset"));
}

#[test]
fn a_failed_first_load_shows_neither_the_loading_nor_the_empty_state() {
    let mut rt = scene(Case::Failed);
    assert!(shown(&rt, "board-error"));
    assert!(!says(&rt, "Loading board…") && !says(&rt, "No lanes yet"));
    assert!(is_disabled(&rt, "board-mode"));
    assert_eq!(
        text_of(&rt, "board-header-description").as_deref(),
        Some("Create your first lane to start a persistent board for manual and API-backed cards.")
    );
    // Try again loads once more (here it fails the same way), and Dismiss clears the banner.
    click(&mut rt, "board-error-dismiss");
    assert!(!shown(&rt, "board-error"));
}

#[test]
fn try_again_reads_the_board_again() {
    let mut rt = scene(Case::Failed);
    assert!(rt.state.board.error.is_some());
    rt.update(|s, _| s.board.mock_answers = crate::fleet::board::store::MockAnswers::Data);
    click(&mut rt, "board-error-retry");
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    assert!(rt.state.board.is_loaded && rt.state.board.error.is_none());
    assert!(!shown(&rt, "board-error"));
    assert!(!is_disabled(&rt, "board-mode"));
}

#[test]
fn a_long_board_name_wraps_instead_of_running_off_the_bar() {
    let mut rt = scene(Case::Synced);
    rt.update(|s, _| {
        s.board.board.as_mut().unwrap().name = "Kite-launch-checklist-for-the-spring-festival-".repeat(4);
    });
    for _ in 0..3 {
        rt.frame(FRAME);
    }
    let title = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("board-header-title")).expect("the title");
    assert!(title.rect[3] > 40.0, "more than one line tall, was {}", title.rect[3]);
    assert!(title.rect[0] + title.rect[2] <= 1280.0, "inside the window");
}
