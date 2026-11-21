//! Piece L of component 21: an item's side rail (rows 12.x of docs/parity/21-github.md). `shows the conversation, labels and
//! assignees of an issue` (its labels and assignees half) keeps Fleet's name; the rest are native. Every name is invented.
use super::items::ItemDetail;
use super::tests_stores::{Rig, rig};
use super::view::item_rail::{self, sections};
use super::view::rows_scene;
use super::{Kind, Where, mock, state, state_ref};
use crate::tests::{FRAME, click};
use crate::{Rail, State};
use serde_json::json;

const PULL: &str = "/api/integrations/github/repos/acme/rocket/items/218";
const ISSUE: &str = "/api/integrations/github/repos/acme/rocket/items/7";

fn on(kind: Kind) -> Rig {
    let mut r = rig();
    r.answer("GET", PULL, 200, mock::pull_detail());
    r.answer("GET", ISSUE, 200, mock::issue_detail());
    r.set(rows_scene::link);
    let number = if kind == Kind::Pull { 218 } else { 7 };
    r.set(move |s: &mut State| {
        s.rail = Rail::Plugin("github");
        state(&mut s.places).at = Where::Item { owner: "acme".into(), repo: "rocket".into(), kind, number };
    });
    r.wait("the item", |s| state_ref(&s.places).item.detail.is_some() || state_ref(&s.places).item.error.is_some());
    r.rt.frame(FRAME);
    r
}

fn has(r: &Rig, key: &str) -> bool {
    r.rt.elements().iter().any(|e| e.key.as_deref() == Some(key))
}

fn words(r: &Rig) -> String {
    r.rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

fn y_of(r: &Rig, key: &str) -> f32 {
    r.rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}")).1
}

#[test]
fn shows_the_conversation_labels_and_assignees_of_an_issue() {
    let r = on(Kind::Issue);
    let all = words(&r);
    assert!(all.contains("LABELS") && all.contains("bug"), "the label: {all}");
    assert!(all.contains("ASSIGNEES") && all.contains("mara-quinn"), "the assignee: {all}");
    assert!(all.contains("No session is working on this yet."));
    assert!(has(&r, "github-rail-label-0") && has(&r, "github-rail-assignee-mara-quinn"));
}

#[test]
fn each_section_shows_only_when_it_has_something() {
    let pull = ItemDetail::read(&mock::pull_detail());
    assert_eq!(
        sections(&pull),
        super::view::item_rail::Sections { checks: true, reviewers: true, linked: false, labels: true, assignees: true, branch: true }
    );
    let issue = ItemDetail::read(&mock::issue_detail());
    let s = sections(&issue);
    assert!(!s.checks && !s.reviewers && !s.branch, "an issue has no checks, reviewers or branch: {s:?}");
    assert!(s.linked && s.labels && s.assignees);
    let bare = ItemDetail::read(&json!({"summary": {"kind": "pull"}}));
    assert_eq!(sections(&bare), Default::default(), "nothing to show, nothing drawn");
    let threads = ItemDetail::read(&json!({"summary": {"kind": "pull"}, "unresolvedThreads": 2}));
    assert!(sections(&threads).reviewers, "open threads alone show the section");
    let no_head = ItemDetail::read(&json!({"summary": {"kind": "pull", "headRef": null}}));
    assert!(!sections(&no_head).branch);
}

#[test]
fn the_pull_requests_rail_lists_checks_reviewers_labels_assignees_and_branch() {
    let r = on(Kind::Pull);
    let all = words(&r);
    for want in [
        "IN FLEET",
        "CHECKS",
        "build",
        "lint",
        "test",
        "REVIEWERS",
        "kit-lee",
        "Changes requested",
        "mara-quinn",
        "Approved",
        "1 unresolved thread",
        "LABELS",
        "needs-tests",
        "ASSIGNEES",
        "devon-park",
        "BRANCH",
        "feat/pr-218",
    ] {
        assert!(all.contains(want), "{want} in {all}");
    }
    assert!(!all.contains("LINKED PULL REQUESTS"), "nothing mentions this pull request by another");
    // Only the checks that have an address link to it.
    assert!(has(&r, "github-rail-check-0-link") && !has(&r, "github-rail-check-1-link") && has(&r, "github-rail-check-2-link"));
}

#[test]
fn threads_say_thread_for_one_and_threads_for_more() {
    let mut r = on(Kind::Pull);
    assert!(words(&r).contains("1 unresolved thread") && !words(&r).contains("1 unresolved threads"));
    r.set(|s| {
        if let Some(d) = state(&mut s.places).item.detail.as_mut() {
            d.unresolved_threads = 3;
        }
    });
    r.rt.frame(FRAME);
    assert!(words(&r).contains("3 unresolved threads"));
}

#[test]
fn an_issue_lists_the_pull_requests_that_mention_it() {
    let r = on(Kind::Issue);
    let all = words(&r);
    assert!(all.contains("LINKED PULL REQUESTS"), "{all}");
    assert!(has(&r, "github-rail-linked-0") && has(&r, "github-rail-linked-1"));
    assert!(!all.contains("CHECKS") && !all.contains("REVIEWERS") && !all.contains("BRANCH"));
}

#[test]
fn a_linked_pull_request_in_this_repository_opens_fleets_page() {
    let mut r = on(Kind::Issue);
    click(&mut r.rt, "github-rail-linked-0");
    assert_eq!(state_ref(&r.rt.state.places).at.path(), "/github/acme/rocket/pulls/218");
}

#[test]
fn a_session_row_names_the_session_and_opens_it() {
    let mut r = on(Kind::Pull);
    assert!(has(&r, "github-item-session-0"), "the invented session works on #218");
    assert!(!words(&r).contains("No session is working on this yet."));
    click(&mut r.rt, "github-item-session-0");
    assert_eq!(r.rt.state.rail, Rail::Sessions, "it opens the session");
}

#[test]
fn narrower_than_820_the_rail_comes_first_and_beside_it_comes_after() {
    let mut r = on(Kind::Pull);
    // Not measured yet counts as wide; the real window is wider than 820 here.
    assert!(r.rt.find("github-item-rail").unwrap().0 > r.rt.find("github-item-main").unwrap().0, "beside the conversation");
    r.set(|s| state(&mut s.places).item.pinned_width = Some(700.0));
    r.rt.frame(FRAME);
    assert!(y_of(&r, "github-item-rail") < y_of(&r, "github-item-main"), "the rail first");
    assert!(item_rail::one_column(&r.rt.state) && !item_rail::tight(&r.rt.state));
    r.set(|s| state(&mut s.places).item.pinned_width = Some(500.0));
    assert!(item_rail::tight(&r.rt.state), "under 520 the main column tightens");
    r.set(|s| state(&mut s.places).item.pinned_width = Some(821.0));
    assert!(!item_rail::one_column(&r.rt.state), "821 stays beside");
}
