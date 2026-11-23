//! Component 23, piece R: the Board's right panel. `BoardSummaryPanel`, `BoardActivityPanel` and `BoardRightPanel` have no unit test in
//! Fleet, so these say what the components do: the shares and spend label from the mock board, the tabs, the fold kept as
//! `right_collapsed(None)` with the gutter staying (FB8), and Fleet's invented feed (FB4, decision D1).
use crate::fleet::board::cards::Status;
use crate::fleet::board::panel::{self, INITIAL_COUNT, Tab};
use crate::mock::board::Case;
use crate::tests::{FRAME, app, click};
use crate::{Rail, State};
use rune::{Key, KeyEvent, Mods, Runtime};

fn scene() -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| s.show_board(Case::Board, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt
}

fn shown(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn words(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().iter().filter(|e| !e.exiting).filter_map(|e| e.text.clone()).collect()
}

fn fold_key(rt: &mut Runtime<State>) {
    let mods = Mods { shift: true, cmd: true, ctrl: !cfg!(target_os = "macos"), ..Mods::default() };
    rt.key(KeyEvent { key: Key::Command('b'), mods });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

#[test]
fn the_shares_come_from_the_mock_boards_visible_cards() {
    let rt = scene();
    let visible = rt.state.board.filtered_sessions();
    assert_eq!(visible.len(), 7, "every card counts, archived ones too");
    let stats = panel::status_stats(&visible);
    assert_eq!(stats.len(), 5);
    assert_eq!(stats.iter().map(|s| s.count).sum::<usize>(), 7);
    for s in &stats {
        assert!((s.share - s.count as f64 / 7.0).abs() < 1e-9);
        // Any count shows at least 6% of its track.
        assert!(s.count == 0 && s.width == 0.0 || s.width >= 6.0);
    }
    assert_eq!(stats[0].status, Status::Active);
    // Cards carry no cost (FB5): the bar is by session share, and every money row reads $0.00.
    let agents = panel::agent_stats(&visible);
    assert_eq!(agents[0].agent, "manual", "most sessions first");
    let shares: f64 = panel::spend_segments(&visible).iter().map(|(_, w)| w).sum();
    assert!((shares - 100.0).abs() < 1e-6);
    assert!(panel::spend_label(&visible).starts_with("Share of sessions by agent: manual "));
    let text = words(&rt);
    assert!(text.iter().any(|w| w == "Status breakdown"));
    assert!(text.iter().any(|w| w == "$0.00"));
    assert!(text.iter().any(|w| w == "7"), "the total");
}

#[test]
fn says_no_spend_by_agent_yet_when_nothing_is_visible() {
    let mut rt = scene();
    rt.update(|s, _| {
        for status in Status::ALL {
            s.board.filters.set_status(status, false);
        }
    });
    rt.frame(FRAME);
    assert!(rt.state.board.filtered_sessions().is_empty());
    assert_eq!(panel::spend_label(&[]), "No spend by agent yet");
    assert!(panel::spend_segments(&[]).is_empty());
    let bar = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("board-spend-bar")).expect("the bar");
    assert_eq!(bar.label.as_deref(), Some("No spend by agent yet"));
    let (cost, tokens, average) = panel::totals(&[]);
    assert_eq!((cost, tokens, average), (0.0, 0.0, 0.0), "no average over no sessions");
}

#[test]
fn the_footnote_follows_the_fleet_summary() {
    let mut rt = scene();
    assert!(shown(&rt, "board-summary-footnote"));
    assert!(words(&rt).iter().any(|w| w == "3 active • 5 idle from fleet summary"));
    assert!(words(&rt).iter().any(|w| w == "Queued tasks"), "the row comes once the summary has answered");
    // A failed read keeps what was held; a stale answer is dropped.
    rt.update(|s, _| {
        s.board.right.answer(0, Err("old".into()));
        assert!(s.board.right.error.is_none(), "a stale answer is dropped");
    });
}

#[test]
fn summary_opens_first_and_pressing_the_open_tab_changes_nothing() {
    let mut rt = scene();
    assert_eq!(rt.state.board.right.tab, Tab::Summary);
    assert!(shown(&rt, "board-summary"));
    click(&mut rt, "board-tab-summary");
    assert!(shown(&rt, "board-summary"));
    click(&mut rt, "board-tab-activity");
    assert_eq!(rt.state.board.right.tab, Tab::Activity);
    assert!(shown(&rt, "board-activity") && !shown(&rt, "board-summary"));
    // Leaving the Board unmounts the panel: the next time it opens on Summary.
    rt.update(|s, _| s.rail = Rail::Sessions);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.board.right.tab, Tab::Summary);
}

#[test]
fn folding_keeps_the_gutter_and_the_last_choice_not_a_sessions() {
    let mut rt = scene();
    assert!(shown(&rt, "board-right-panel") && shown(&rt, "right-gutter"));
    assert!(!rt.state.sidebar.right_collapsed(None));
    click(&mut rt, "board-right-collapse");
    rt.frame(FRAME);
    assert!(rt.state.sidebar.right_collapsed(None), "kept as right_collapsed(None)");
    assert!(!shown(&rt, "board-right-panel"));
    assert!(shown(&rt, "right-gutter"), "FB8: the gutter stays, with no rail");
    assert!(!shown(&rt, "content-rail"));
    assert!(!rt.state.content_open, "a session's panel is not touched");
    fold_key(&mut rt);
    assert!(!rt.state.sidebar.right_collapsed(None));
    assert!(shown(&rt, "board-right-panel"));
    fold_key(&mut rt);
    assert!(rt.state.sidebar.right_collapsed(None));
}

#[test]
fn the_gutter_is_the_sessions_one() {
    let mut rt = scene();
    let before = rt.state.right.width;
    let (x, y) = rt.find("right-gutter").expect("the gutter");
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_move(x - 30.0, y);
    rt.pointer_move(x - 60.0, y);
    rt.pointer_up(x - 60.0, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.state.right.width > before, "dragging left widened the one shared width: {}", rt.state.right.width);
    assert!(rt.state.right.user_sized);
}

// ─── the feed ───────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_feed_starts_with_the_four_newest_and_gets_one_more_every_two_and_a_half_seconds() {
    let mut rt = scene();
    rt.update(|s, _| s.board.right.tab = Tab::Activity);
    rt.frame(FRAME);
    rt.frame(FRAME);
    let feed = rt.state.board.right.feed.clone().expect("mounted");
    assert_eq!(feed.ordered().len(), INITIAL_COUNT);
    assert_eq!(feed.pending(), 4);
    assert_eq!(feed.ordered()[0].action, "session updated with handoff notes", "newest first");
    rt.frame(2.6);
    assert_eq!(rt.state.board.right.feed.as_ref().map(|f| f.ordered().len()), Some(5));
    rt.frame(2.6);
    rt.frame(2.6);
    rt.frame(2.6);
    let feed = rt.state.board.right.feed.clone().unwrap();
    assert_eq!(feed.ordered().len(), 8);
    assert_eq!(feed.pending(), 0);
    let at: Vec<i64> = feed.ordered().iter().map(|e| e.occurred_at).collect();
    assert!(at.windows(2).all(|w| w[0] > w[1]), "sorted newest first, the late ones below");
    assert!(words(&rt).iter().any(|w| w == "JWT Auth Module"));
}
