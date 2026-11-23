//! Component 23, piece Q: the Board's left panel. Native cases (Fleet has no test for `BoardControlsPanel`): unchecking a
//! status changes the counts and the Summary but never the lanes (FB3), Group by changes the summary's word, the two selects
//! choose, and the agents are a checkbox each, sorted.
use crate::State;
use crate::fleet::board::cards::{SortBy, Status};
use crate::fleet::board::filters::GroupBy;
use crate::mock::board::Case;
use crate::tests::{FRAME, app, click};
use rune::Runtime;

fn scene() -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| s.show_board(Case::Board, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt
}

fn text_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).and_then(|e| e.text)
}

fn shown(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn card_keys(rt: &Runtime<State>) -> Vec<String> {
    let mut keys: Vec<String> =
        rt.elements().iter().filter(|e| !e.exiting).filter_map(|e| e.key.clone()).filter(|k| k.starts_with("board-card-")).collect();
    keys.sort();
    keys
}

#[test]
fn the_panel_has_its_six_sections_and_the_status_rows_in_order() {
    let rt = scene();
    for name in ["project", "status", "agents", "group", "sort", "summary"] {
        assert!(shown(&rt, &format!("board-controls-{name}")), "{name}");
    }
    for st in Status::ALL {
        assert!(shown(&rt, &format!("board-status-{}", st.value())), "{}", st.label());
    }
    assert!(shown(&rt, "board-quick-stats"));
}

#[test]
fn unchecking_a_status_changes_the_counts_not_the_lanes() {
    let mut rt = scene();
    let lanes_before = card_keys(&rt);
    assert!(!lanes_before.is_empty(), "the lanes draw cards");
    let visible_before = rt.state.board.quick_stats().visible;
    assert!(visible_before > 0);
    let off = Status::ALL.into_iter().find(|st| rt.state.board.sessions().iter().any(|c| c.status == *st)).unwrap();

    click(&mut rt, &format!("board-status-{}", off.value()));

    assert!(!rt.state.board.filters.status_on(off));
    let after = rt.state.board.quick_stats();
    assert!(after.visible < visible_before, "the count follows");
    assert_eq!(text_of(&rt, "board-summary"), Some(format!("{} visible • grouped by status", after.visible)));
    assert_eq!(card_keys(&rt), lanes_before, "the lanes keep every card (FB3)");

    click(&mut rt, &format!("board-status-{}", off.value()));
    assert_eq!(rt.state.board.quick_stats().visible, visible_before);
}

#[test]
fn group_by_changes_the_summarys_word() {
    let mut rt = scene();
    for (g, word) in [(GroupBy::Project, "project"), (GroupBy::Agent, "agent"), (GroupBy::Status, "status")] {
        click(&mut rt, &format!("board-group-{}", g.label().to_lowercase()));
        assert_eq!(rt.state.board.filters.group_by, g);
        let said = text_of(&rt, "board-summary").unwrap();
        assert!(said.ends_with(&format!("grouped by {word}")), "{said}");
    }
}

#[test]
fn an_agent_is_a_checkbox_that_takes_its_cards_out_of_the_counts() {
    let mut rt = scene();
    let agents = rt.state.board.available_agents();
    assert!(!agents.is_empty());
    let mut sorted = agents.clone();
    sorted.sort_by(|a, b| crate::fleet::board::filters::locale_cmp(a, b));
    assert_eq!(agents, sorted, "sorted");
    let before = rt.state.board.quick_stats().visible;
    click(&mut rt, &format!("board-agent-{}", agents[0]));
    assert!(!rt.state.board.filters.agent_on(&agents[0]));
    assert!(rt.state.board.quick_stats().visible < before);
}

#[test]
fn the_selects_choose_a_project_and_a_sort() {
    let mut rt = scene();
    click(&mut rt, "board-sort");
    click(&mut rt, "board-sort-list-opt1");
    assert_eq!(rt.state.board.filters.sort_by, SortBy::Oldest);

    click(&mut rt, "board-project");
    click(&mut rt, "board-project-list-opt1");
    assert_eq!(rt.state.board.filters.project, "Kite launch");
    click(&mut rt, "board-project");
    click(&mut rt, "board-project-list-opt0");
    assert_eq!(rt.state.board.filters.project, "all");
}
