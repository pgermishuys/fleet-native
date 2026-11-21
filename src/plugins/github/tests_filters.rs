//! Piece J of component 21: the issue filter bar. Fleet has no test of its own for these components, so every test is native, from rows
//! 9.x of docs/parity/21-github.md, over a fake Fleet that answers `/api/integrations/github/*` (no real GitHub, account or token is
//! reached). Every name is invented.
use super::bar::{EXPR_INPUT, Menu, SORTS, sort_words};
use super::filter::{Direction, SortField, StateChoice};
use super::search::DEBOUNCE;
use super::tests_repo::{page, pull, q, search_answer, searches, words};
use super::tests_stores::{Rig, WORK, rig};
use super::{Where, mock, state, state_ref};
use crate::tests::{FRAME, click, press_key};
use crate::{Rail, State};
use rune::{Key, Mods};
use serde_json::json;

const OPEN: &str = "repo:acme/rocket is:issue is:open sort:updated-desc";
const META: &str = "/api/integrations/github/repos/acme/rocket";

fn bar(s: &State) -> &super::bar::Bar {
    &page(s).bar
}

fn issue(number: u64, title: &str) -> serde_json::Value {
    let mut i = pull(number, title);
    i["kind"] = json!("issue");
    i["url"] = json!(format!("https://github.com/acme/rocket/issues/{number}"));
    i
}

/// The Issues tab of `acme/rocket`, with Fleet answering the first list and the three lists the menus read.
fn on_issues() -> Rig {
    let mut r = rig();
    r.answer("GET", WORK, 200, super::tests_repo::work());
    r.answer("GET", &q(OPEN), 200, search_answer(vec![issue(7, "Seven")], None));
    r.answer("GET", &format!("{META}/labels"), 200, mock::labels());
    r.answer("GET", &format!("{META}/milestones"), 200, mock::milestones());
    r.answer("GET", &format!("{META}/assignees"), 200, mock::assignees());
    r.set(|s| {
        s.rail = Rail::Plugin("github");
        state(&mut s.places).at = Where::Repo { owner: "acme".into(), repo: "rocket".into() };
    });
    r.wait("the metadata", |s| {
        let g = state_ref(&s.places);
        [g.metadata.labels.get("acme", "rocket").map(|e| e.data.len()), g.metadata.assignees.get("acme", "rocket").map(|e| e.data.len())]
            == [Some(5), Some(4)]
            && g.metadata.milestones.get("acme", "rocket").is_some_and(|e| e.data.len() == 2)
    });
    click(&mut r.rt, "github-repo-tab-issues");
    r.wait("the first list of issues", |s| !s.github_repo_list().items.is_empty());
    r
}

fn text_of(r: &Rig, key: &str) -> Option<String> {
    r.rt.elements().iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text.clone())
}

fn has(r: &Rig, key: &str) -> bool {
    r.rt.find(key).is_some()
}

fn focused(r: &mut Rig) -> Option<String> {
    let mut now = None;
    r.rt.update(|_, tx| now = tx.focused().map(str::to_string));
    now
}

/// Presses the button that opens `menu`.
fn open(r: &mut Rig, menu: Menu) {
    click(&mut r.rt, menu.trigger());
    assert_eq!(bar(&r.rt.state).menu, Some(menu));
}

/// A press on the empty part of the window, away from the open menu (the scrim's own middle can be under the menu).
fn press_away(r: &mut Rig) {
    r.rt.pointer_move(1100.0, 640.0);
    r.rt.pointer_down(1100.0, 640.0);
    r.rt.pointer_up(1100.0, 640.0);
    r.rt.frame(FRAME);
}

fn row_titles(r: &Rig) -> Vec<String> {
    r.rt.state.github_bar_rows().into_iter().map(|row| row.title).collect()
}

/// Lets a changed filter's pause go by and waits for the search it makes.
fn settled(r: &mut Rig, query: &str) {
    r.rt.frame(DEBOUNCE + 0.05);
    let want = query.to_string();
    let log = r.fake.clone();
    r.wait("the new search", move |_| {
        log.lock()
            .unwrap()
            .log
            .iter()
            .any(|(_, p, _)| p.starts_with("/api/integrations/github/search") && super::tests_repo::unescape(p).contains(&want))
    });
}

#[test]
fn the_bar_has_a_field_open_and_closed_four_menus_and_a_sort() {
    let r = on_issues();
    for key in [
        EXPR_INPUT,
        "github-issues-open",
        "github-issues-closed",
        "github-issues-label",
        "github-issues-author",
        "github-issues-milestone",
        "github-issues-assignee",
        "github-issues-sort",
    ] {
        assert!(has(&r, key), "{key}");
    }
    assert_eq!(text_of(&r, "github-issues-sort-words").as_deref(), Some("Recently updated"));
    assert!(!has(&r, "github-issues-clear"), "no filter, no Clear");
    assert!(page(&r.rt.state).bar.expr.text.is_empty(), "the field shows nothing for the defaults");
}

#[test]
fn the_field_shows_the_serialised_filter() {
    let mut r = on_issues();
    r.set(|s| {
        let f = &mut state(&mut s.places).repo.issues;
        f.labels = vec!["bug".into(), "needs design".into()];
        f.author = Some("kit-lee".into());
        f.sort = SortField::Created;
        f.direction = Direction::Asc;
    });
    assert_eq!(bar(&r.rt.state).expr.text, "label:bug label:\"needs design\" author:kit-lee sort:created-asc");
    assert!(has(&r, "github-issues-clear"), "something is set, so Clear shows");
}

#[test]
fn enter_commits_what_was_typed_and_the_field_lets_go() {
    let mut r = on_issues();
    click(&mut r.rt, EXPR_INPUT);
    assert_eq!(focused(&mut r).as_deref(), Some(EXPR_INPUT));
    press_key(&mut r.rt, Key::Text("is:closed label:bug author:kit-lee leaks".into()), Mods::default());
    press_key(&mut r.rt, Key::Enter, Mods::default());
    let f = &page(&r.rt.state).issues;
    assert_eq!(
        (f.state, f.labels.clone(), f.author.clone(), f.search.as_str()),
        (StateChoice::Closed, vec!["bug".to_string()], Some("kit-lee".into()), "leaks")
    );
    assert_eq!(focused(&mut r), None, "Enter leaves the field");
    assert_eq!(bar(&r.rt.state).expr.text, "is:closed label:bug author:kit-lee leaks", "and it shows the filter again");
    settled(&mut r, "is:closed label:bug author:kit-lee sort:updated-desc leaks");
}

#[test]
fn escape_drops_the_edit_and_leaves_the_filter_alone() {
    let mut r = on_issues();
    r.set(|s| state(&mut s.places).repo.issues.labels = vec!["bug".into()]);
    click(&mut r.rt, EXPR_INPUT);
    press_key(&mut r.rt, Key::Text(" author:kit-lee".into()), Mods::default());
    assert_eq!(bar(&r.rt.state).expr.text, "label:bug author:kit-lee");
    press_key(&mut r.rt, Key::Escape, Mods::default());
    assert_eq!(page(&r.rt.state).issues.author, None);
    assert_eq!(bar(&r.rt.state).expr.text, "label:bug");
    assert_eq!(focused(&mut r), None);
}

#[test]
fn leaving_the_field_commits_it() {
    let mut r = on_issues();
    click(&mut r.rt, EXPR_INPUT);
    press_key(&mut r.rt, Key::Text("label:bug".into()), Mods::default());
    assert!(page(&r.rt.state).issues.labels.is_empty(), "nothing is committed while typing");
    // A press on the tab's chrome takes the focus away.
    click(&mut r.rt, "github-repo-tab-issues");
    r.rt.frame(FRAME);
    assert_eq!(page(&r.rt.state).issues.labels, ["bug"]);
}

#[test]
fn a_press_on_a_control_while_editing_commits_the_edit_first() {
    let mut r = on_issues();
    click(&mut r.rt, EXPR_INPUT);
    press_key(&mut r.rt, Key::Text("label:bug".into()), Mods::default());
    click(&mut r.rt, "github-issues-closed");
    let f = &page(&r.rt.state).issues;
    assert_eq!((f.state, f.labels.clone()), (StateChoice::Closed, vec!["bug".to_string()]), "the edit was kept and the press applied");
}

#[test]
fn clear_puts_the_defaults_back() {
    let mut r = on_issues();
    r.set(|s| {
        let f = &mut state(&mut s.places).repo.issues;
        f.state = StateChoice::Closed;
        f.milestone = Some("none".into());
        f.search = "leaks".into();
    });
    assert!(has(&r, "github-issues-clear"));
    click(&mut r.rt, "github-issues-clear");
    assert_eq!(page(&r.rt.state).issues, super::filter::IssueFilter::default());
    assert!(bar(&r.rt.state).expr.text.is_empty());
    assert!(!has(&r, "github-issues-clear"));
}

#[test]
fn open_and_closed_choose_the_state() {
    let mut r = on_issues();
    click(&mut r.rt, "github-issues-closed");
    assert_eq!(page(&r.rt.state).issues.state, StateChoice::Closed);
    assert_eq!(bar(&r.rt.state).expr.text, "is:closed");
    settled(&mut r, "is:issue is:closed sort:updated-desc");
    click(&mut r.rt, "github-issues-open");
    assert_eq!(page(&r.rt.state).issues.state, StateChoice::Open);
}

#[test]
fn label_chooses_many_and_the_badge_counts_them() {
    let mut r = on_issues();
    open(&mut r, Menu::Label);
    assert_eq!(row_titles(&r), ["bug", "enhancement", "good first issue", "performance", "documentation"]);
    click(&mut r.rt, "github-issues-row-0");
    click(&mut r.rt, "github-issues-row-2");
    assert_eq!(page(&r.rt.state).issues.labels, ["bug", "good first issue"]);
    assert_eq!(bar(&r.rt.state).menu, Some(Menu::Label), "the menu stays open for the next one");
    assert_eq!(text_of(&r, "github-issues-label-badge-count").as_deref(), Some("2"));
    click(&mut r.rt, "github-issues-row-0");
    assert_eq!(page(&r.rt.state).issues.labels, ["good first issue"], "a chosen label is taken out again");
    assert_eq!(text_of(&r, "github-issues-label-badge-count").as_deref(), Some("1"));
    settled(&mut r, "is:issue is:open label:\"good first issue\" sort:updated-desc");
}

#[test]
fn typing_in_a_menu_keeps_the_rows_that_contain_it_and_enter_picks_the_first() {
    let mut r = on_issues();
    open(&mut r, Menu::Label);
    assert_eq!(focused(&mut r).as_deref(), Some(super::bar::MENU_INPUT));
    press_key(&mut r.rt, Key::Text("PER".into()), Mods::default());
    assert_eq!(row_titles(&r), ["performance"]);
    press_key(&mut r.rt, Key::Enter, Mods::default());
    assert_eq!(page(&r.rt.state).issues.labels, ["performance"]);
    assert_eq!(bar(&r.rt.state).menu, Some(Menu::Label));
}

#[test]
fn down_and_up_walk_the_rows_and_wrap() {
    let mut r = on_issues();
    open(&mut r, Menu::Assignee);
    assert_eq!(bar(&r.rt.state).hl, 0);
    press_key(&mut r.rt, Key::Up, Mods::default());
    assert_eq!(bar(&r.rt.state).hl, 4, "Up from the first goes to the last of five rows");
    press_key(&mut r.rt, Key::Down, Mods::default());
    assert_eq!(bar(&r.rt.state).hl, 0);
}

#[test]
fn author_chooses_one_and_closes_choosing_it_again_clears() {
    let mut r = on_issues();
    open(&mut r, Menu::Author);
    assert_eq!(
        row_titles(&r),
        ["mara-quinn", "devon-park", "kit-lee", "sam-rivera"],
        "the repository's assignees; no Clear while none is chosen"
    );
    click(&mut r.rt, "github-issues-row-2");
    assert_eq!(page(&r.rt.state).issues.author.as_deref(), Some("kit-lee"));
    assert_eq!(bar(&r.rt.state).menu, None, "a choice closes the menu");
    assert_eq!(text_of(&r, "github-issues-author-badge-count").as_deref(), Some("1"));
    open(&mut r, Menu::Author);
    assert_eq!(row_titles(&r)[0], "Clear selection", "Clear selection comes first while one is chosen");
    click(&mut r.rt, "github-issues-row-3");
    assert_eq!(page(&r.rt.state).issues.author, None, "choosing the chosen one clears it");
    assert!(!has(&r, "github-issues-author-badge-count"));
    open(&mut r, Menu::Author);
    click(&mut r.rt, "github-issues-row-1");
    assert_eq!(page(&r.rt.state).issues.author.as_deref(), Some("devon-park"), "with nothing chosen row 1 is the second person");
    open(&mut r, Menu::Author);
    click(&mut r.rt, "github-issues-row-0");
    assert_eq!(page(&r.rt.state).issues.author, None, "Clear selection clears");
    assert_eq!(bar(&r.rt.state).menu, None);
}

#[test]
fn milestone_has_no_milestone_and_each_ones_open_and_closed_counts() {
    let mut r = on_issues();
    open(&mut r, Menu::Milestone);
    assert_eq!(row_titles(&r), ["No milestone", "v1.2 launch window", "Autumn cleanup"]);
    let all = words(&r);
    assert!(all.contains("4 open \u{b7} 9 closed") && all.contains("2 open \u{b7} 5 closed"), "{all}");
    click(&mut r.rt, "github-issues-row-0");
    assert_eq!(page(&r.rt.state).issues.milestone.as_deref(), Some("none"));
    settled(&mut r, "is:issue is:open milestone:none sort:updated-desc");
    open(&mut r, Menu::Milestone);
    click(&mut r.rt, "github-issues-row-2");
    assert_eq!(page(&r.rt.state).issues.milestone.as_deref(), Some("v1.2 launch window"));
    assert_eq!(bar(&r.rt.state).expr.text, "milestone:\"v1.2 launch window\"");
}

#[test]
fn assignee_has_unassigned_and_chooses_one() {
    let mut r = on_issues();
    open(&mut r, Menu::Assignee);
    assert_eq!(row_titles(&r)[0], "Unassigned");
    click(&mut r.rt, "github-issues-row-0");
    assert_eq!(page(&r.rt.state).issues.assignee.as_deref(), Some("none"));
    settled(&mut r, "is:issue is:open no:assignee sort:updated-desc");
    open(&mut r, Menu::Assignee);
    assert_eq!(row_titles(&r)[0], "Clear selection");
    click(&mut r.rt, "github-issues-row-4");
    assert_eq!(page(&r.rt.state).issues.assignee.as_deref(), Some("kit-lee"));
    assert_eq!(text_of(&r, "github-issues-assignee-badge-count").as_deref(), Some("1"));
}

#[test]
fn the_sort_menu_lists_five_and_each_sets_the_sort() {
    let mut r = on_issues();
    open(&mut r, Menu::Sort);
    for i in 0..SORTS.len() {
        assert!(has(&r, &format!("github-issues-sort-{i}")), "option {i}");
    }
    assert_eq!(SORTS.map(|(w, _, _)| w), ["Recently updated", "Least recently updated", "Newest", "Oldest", "Most commented"]);
    click(&mut r.rt, "github-issues-sort-3");
    let f = &page(&r.rt.state).issues;
    assert_eq!((f.sort, f.direction), (SortField::Created, Direction::Asc));
    assert_eq!(sort_words(f), "Oldest");
    assert_eq!(text_of(&r, "github-issues-sort-words").as_deref(), Some("Oldest"));
    assert_eq!(bar(&r.rt.state).menu, None);
    settled(&mut r, "is:issue is:open sort:created-asc");
}

#[test]
fn a_change_re_asks_github_after_the_pause_and_the_first_search_does_not_wait() {
    let mut r = on_issues();
    let issues = |r: &Rig| searches(r).into_iter().filter(|q| q.contains("is:issue")).collect::<Vec<_>>();
    assert_eq!(issues(&r), [OPEN], "the first issue query went at once");
    click(&mut r.rt, "github-issues-closed");
    assert_eq!(issues(&r).len(), 1, "nothing goes inside the pause");
    r.rt.frame(DEBOUNCE - 0.1);
    assert_eq!(issues(&r).len(), 1);
    settled(&mut r, "is:issue is:closed sort:updated-desc");
    assert_eq!(issues(&r).len(), 2);
}

#[test]
fn a_menu_says_it_is_loading_and_what_it_found_none_of() {
    let mut r = on_issues();
    r.set(|s| {
        let g = state(&mut s.places);
        g.metadata.show("acme", "rocket", (&json!([]), &json!([]), &json!([])), true);
    });
    open(&mut r, Menu::Label);
    assert!(has(&r, "github-issues-loading"), "a spinner while nothing has come");
    r.set(|s| {
        let g = state(&mut s.places);
        g.metadata.show("acme", "rocket", (&json!([]), &json!([]), &json!([])), false);
    });
    assert!(!has(&r, "github-issues-loading"));
    assert!(!has(&r, "github-issues-empty"), "nothing typed, nothing said (the command list's empty words wait for a search)");
    press_key(&mut r.rt, Key::Text("x".into()), Mods::default());
    assert_eq!(text_of(&r, "github-issues-empty").as_deref(), Some("No labels found."));
    for (menu, words) in
        [(Menu::Author, "No users found."), (Menu::Milestone, "No milestones found."), (Menu::Assignee, "No assignees found.")]
    {
        assert_eq!(menu.empty_words(), words);
    }
}

#[test]
fn a_press_outside_closes_the_menu_and_a_second_press_on_its_button_does_too() {
    let mut r = on_issues();
    open(&mut r, Menu::Label);
    press_away(&mut r);
    assert_eq!(bar(&r.rt.state).menu, None);
    open(&mut r, Menu::Label);
    press_key(&mut r.rt, Key::Escape, Mods::default());
    assert_eq!(bar(&r.rt.state).menu, None, "Escape closes it");
    open(&mut r, Menu::Sort);
    press_away(&mut r);
    assert_eq!(bar(&r.rt.state).menu, None);
}

#[test]
fn a_menu_opens_with_an_empty_box() {
    let mut r = on_issues();
    open(&mut r, Menu::Label);
    press_key(&mut r.rt, Key::Text("bug".into()), Mods::default());
    press_key(&mut r.rt, Key::Escape, Mods::default());
    open(&mut r, Menu::Label);
    assert!(bar(&r.rt.state).query.text.is_empty());
    assert_eq!(row_titles(&r).len(), 5);
}

#[test]
fn the_metadata_is_read_when_the_page_opens() {
    let r = on_issues();
    for kind in ["labels", "milestones", "assignees"] {
        assert_eq!(r.asked("GET", &format!("{META}/{kind}")), 1, "{kind}");
    }
}

#[test]
fn a_search_in_flight_shows_a_spinner_in_the_field() {
    let mut r = on_issues();
    let go = r.gate(&q("repo:acme/rocket is:issue is:closed sort:updated-desc"));
    r.answer("GET", &q("repo:acme/rocket is:issue is:closed sort:updated-desc"), 200, search_answer(Vec::new(), None));
    click(&mut r.rt, "github-issues-closed");
    r.rt.frame(DEBOUNCE + 0.05);
    assert!(r.rt.state.github_repo_list().loading);
    go.send(()).unwrap();
    r.wait("the answer", |s| !s.github_repo_list().loading);
}
