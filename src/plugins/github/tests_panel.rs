//! Piece D of component 21: the rail's panel (`GitHubPanel.test.ts`, under its test names) and what loads it. Fleet's tests mount the
//! panel over mocked stores; these set the plugin's stores directly, and the one that needs a server (Stop following) uses the fake Fleet
//! of the store tests. Every name is invented.
use super::bookmarks::Bookmark;
use super::items::Work;
use super::tests_stores::{MARKS, WORK, rig};
use super::view::panel::{counts_for, current_repo};
use super::{Where, mock, state, state_ref};
use crate::tests::{FRAME, app, click};
use crate::{Rail, State};
use rune::Runtime;
use serde_json::json;

fn mark(full: &str) -> Bookmark {
    Bookmark::of(full).expect("an owner and a name")
}

fn words(rt: &Runtime<State>) -> String {
    rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

fn texts_of(rt: &Runtime<State>, key: &str) -> Vec<String> {
    let all = rt.elements();
    let Some(el) = all.iter().find(|e| e.key.as_deref() == Some(key)) else { return Vec::new() };
    all.iter().filter(|e| e.path.starts_with(&el.path)).filter_map(|e| e.text.clone()).collect()
}

/// The app on the GitHub rail with the panel's questions already asked, so nothing reaches for a server.
fn on_the_rail() -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| {
        s.rail = Rail::Plugin("github");
        let g = state(&mut s.places);
        g.panel_asked = true;
        g.auth.connected = true;
        g.bookmarks.has_loaded = true;
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn set(rt: &mut Runtime<State>, f: impl FnOnce(&mut super::GitHub) + 'static) {
    rt.update(move |s, _| f(state(&mut s.places)));
    rt.frame(FRAME);
    rt.frame(FRAME);
}

#[test]
fn says_the_repositories_you_follow_are_loading_not_to_follow_one_until_they_have_loaded() {
    let mut rt = on_the_rail();
    set(&mut rt, |g| {
        g.bookmarks.list.clear();
        g.bookmarks.loading = true;
    });
    assert!(!words(&rt).contains("Follow a repository to see"));
    assert!(rt.find("github-bookmarks-loading").is_some());

    set(&mut rt, |g| g.bookmarks.loading = false);
    assert!(rt.find("github-bookmarks-loading").is_none());
    assert!(words(&rt).contains("Follow a repository to see"));
}

#[test]
fn says_a_failed_load_failed_not_to_follow_a_repository() {
    let mut rt = on_the_rail();
    set(&mut rt, |g| g.bookmarks.error = Some("Failed to fetch bookmarks from server.".into()));
    assert!(!words(&rt).contains("Follow a repository to see"));
    let note = texts_of(&rt, "github-bookmarks-error");
    assert!(note.iter().any(|t| t.contains("Couldn't load the repositories you follow.")), "{note:?}");
    assert!(note.iter().any(|t| t.contains("Failed to fetch bookmarks from server.")), "the reason is under it: {note:?}");
}

#[test]
fn marks_only_the_repository_on_screen_not_one_whose_name_it_starts_with() {
    let (rock, rocket) = (mark("acme/rock"), mark("acme/rocket"));
    let on = Where::Item { owner: "acme".into(), repo: "rocket".into(), kind: super::Kind::Pull, number: 7 };
    let rows: Vec<bool> =
        [&rock, &rocket].iter().map(|m| current_repo(&Where::Repo { owner: "acme".into(), repo: "rocket".into() }, m)).collect();
    assert_eq!(rows, [false, true]);
    assert!(current_repo(&on, &rocket), "and under it, a pull request's page");
    assert!(!current_repo(&on, &rock));
    assert!(current_repo(&Where::Repo { owner: "ACME".into(), repo: "Rocket".into() }, &rocket), "ignoring case");
    assert!(!current_repo(&Where::Home, &rocket), "Your work marks no repository");
}

#[test]
fn shows_bookmarked_repos_and_navigates_on_click() {
    let mut rt = on_the_rail();
    set(&mut rt, |g| g.bookmarks.list = vec![mark("acme/rocket")]);
    assert!(texts_of(&rt, "github-bookmark-acme/rocket").concat().contains("acme/rocket"));
    click(&mut rt, "github-bookmark-acme/rocket");
    assert_eq!(state_ref(&rt.state.places).at, Where::Repo { owner: "acme".into(), repo: "rocket".into() });
    assert_eq!(state_ref(&rt.state.places).at.path(), "/github/acme/rocket");
    click(&mut rt, "github-panel-home");
    assert_eq!(state_ref(&rt.state.places).at, Where::Home);
}

#[test]
fn removes_a_bookmark_on_x_click() {
    let mut r = rig();
    r.answer("GET", WORK, 200, mock::work());
    r.answer("GET", MARKS, 200, json!([{"fullName": "acme/rocket", "owner": "acme", "name": "rocket"}]));
    r.set(|s| {
        s.rail = Rail::Plugin("github");
        let g = state(&mut s.places);
        g.panel_asked = true;
        g.auth.connected = true;
        g.bookmarks.has_loaded = true;
        g.bookmarks.list = vec![mark("acme/rocket")];
    });
    click(&mut r.rt, "github-bookmark-remove-acme/rocket");
    r.wait("the bookmark removed", |s| state_ref(&s.places).bookmarks.list.is_empty());
    assert_eq!(r.asked("DELETE", "/api/integrations/github/bookmarks/acme/rocket"), 1);
    r.wait("your work read again", |s| state_ref(&s.places).work.work.is_some());
    assert_eq!(r.asked("GET", WORK), 1, "once the server removed it, the counts are read again");
}

#[test]
fn a_refused_removal_keeps_the_bookmark_and_reads_nothing_again() {
    let mut r = rig();
    r.answer("DELETE", "/api/integrations/github/bookmarks/acme/rocket", 500, json!({"error": "Nope."}));
    r.set(|s| {
        s.rail = Rail::Plugin("github");
        let g = state(&mut s.places);
        g.panel_asked = true;
        g.auth.connected = true;
        g.bookmarks.list = vec![mark("acme/rocket")];
    });
    click(&mut r.rt, "github-bookmark-remove-acme/rocket");
    r.wait("the refusal", |s| state_ref(&s.places).bookmarks.action_error.as_deref() == Some("Nope."));
    assert_eq!(state_ref(&r.rt.state.places).bookmarks.list.len(), 1);
    assert_eq!(r.asked("GET", WORK), 0);
}

#[test]
fn shows_empty_state_when_no_bookmarks() {
    let mut rt = on_the_rail();
    set(&mut rt, |g| g.bookmarks.list.clear());
    assert!(words(&rt).contains("Follow a repository to see its pull requests and issues here"));
}

#[test]
fn shows_the_account_what_needs_you_and_each_repositorys_open_counts() {
    let mut rt = on_the_rail();
    set(&mut rt, |g| {
        g.work.work = Some(Work::read(&mock::work()));
        g.bookmarks.list = vec![mark("acme/rocket"), mark("lumen-labs/atlas"), mark("acme/anvil")];
    });
    let all = words(&rt);
    assert!(all.contains("mara-quinn") && all.contains("Connected"), "{all}");
    // One review asked and two of the user's own blocked (a failing check, requested changes): three need them.
    assert!(texts_of(&rt, "github-panel-home").iter().any(|t| t == "3"), "{:?}", texts_of(&rt, "github-panel-home"));
    assert!(texts_of(&rt, "github-bookmark-acme/rocket").iter().any(|t| t == "4 · 9"));
    assert!(texts_of(&rt, "github-bookmark-lumen-labs/atlas").iter().any(|t| t == "0 · 2"));
    assert!(
        texts_of(&rt, "github-bookmark-acme/anvil").iter().all(|t| !t.contains('·')),
        "a repository the work does not list has no counts"
    );
    let work = Work::read(&mock::work());
    assert_eq!(counts_for(Some(&work), "ACME/Rocket").as_deref(), Some("4 · 9"), "matched ignoring case");
}

// ─── native: the parts Fleet's test does not reach ─────────────────────────────────────────────────────────────────

#[test]
fn says_not_connected_and_shows_nothing_of_the_repositories_until_connected() {
    let mut rt = on_the_rail();
    set(&mut rt, |g| {
        g.auth.connected = false;
        g.bookmarks.list = vec![mark("acme/rocket")];
    });
    let all = words(&rt);
    assert!(all.contains("GitHub") && all.contains("Not connected"), "{all}");
    assert!(rt.find("github-panel-home").is_some(), "Your work is there");
    assert!(rt.find("github-panel-heading").is_none() && rt.find("github-panel-add").is_none());
    assert!(rt.find("github-bookmark-acme/rocket").is_none());
}

#[test]
fn hides_the_count_on_your_work_when_nothing_needs_you() {
    let mut rt = on_the_rail();
    set(&mut rt, |g| g.work.work = Some(Work::default()));
    assert!(texts_of(&rt, "github-panel-home").iter().all(|t| t != "0"));
}

#[test]
fn the_gear_opens_the_connection_page_in_settings() {
    let mut rt = on_the_rail();
    click(&mut rt, "github-panel-settings");
    assert_eq!(rt.state.rail, Rail::Settings);
    assert_eq!(rt.state.settings.page, crate::fleet::settings_nav::Page::Plugin("github".into()));
}

#[test]
fn asks_for_what_it_shows_once_per_visit_and_reads_your_work_when_connected() {
    let mut r = rig();
    r.answer("GET", "/api/integrations/github/auth/status", 200, json!({"connected": true}));
    r.answer("GET", WORK, 200, mock::work());
    r.answer("GET", MARKS, 200, json!([{"fullName": "acme/rocket", "owner": "acme", "name": "rocket"}]));
    r.set(|s| s.rail = Rail::Plugin("github"));
    r.wait("the panel's reads", |s| {
        let g = state_ref(&s.places);
        g.work.work.is_some() && g.bookmarks.has_loaded && g.auth.connected
    });
    assert_eq!(r.asked("GET", MARKS), 1);
    assert_eq!(r.asked("GET", WORK), 1);
    // Going away and coming back is a new visit; staying is not.
    r.set(|s| s.rail = Rail::Sessions);
    r.set(|s| s.rail = Rail::Plugin("github"));
    r.wait("the second visit's status", |s| state_ref(&s.places).panel_asked);
    assert_eq!(r.asked("GET", MARKS), 1, "the list of repositories is loaded once");
}

#[test]
fn ctrl_b_hides_the_panel_and_the_page_takes_the_context() {
    let mut rt = on_the_rail();
    assert!(rt.find("github-panel").is_some() && rt.find("sidebar-gutter").is_some());
    rt.update(|s, _| s.sidebar_open = false);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("github-panel").is_none() && rt.find("github-page").is_some());
}
