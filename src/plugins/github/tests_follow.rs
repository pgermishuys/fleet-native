//! Piece E of component 21: Follow a repository (`AddRepositoryDialog.vue`). Fleet has no test for it, so these are native: the
//! dialog is opened from the panel's row, over the fake Fleet of the store tests where it asks the server. Every name is invented.
use super::follow::listed;
use super::repos::Repo;
use super::tests_stores::{MARKS, WORK, rig};
use super::{mock, state, state_ref};
use crate::tests::{FRAME, click, press_key};
use crate::{Rail, State};
use rune::{Key, Mods, Runtime};

const REPOS: &str = "/api/integrations/github/repos?page=1&per_page=100&sort=updated";

fn names(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().iter().filter_map(|e| e.key.as_deref()?.strip_prefix("github-follow-row-").map(str::to_string)).collect()
}

fn says(rt: &Runtime<State>, words: &str) -> bool {
    rt.elements().iter().any(|e| e.text.as_deref() == Some(words))
}

fn repos() -> Vec<Repo> {
    mock::repos().as_array().unwrap().iter().filter_map(super::repos::read_repo).collect()
}

/// The panel, connected, following `acme/rocket`, with the repository cache already filled so nothing asks for it.
fn on_the_panel(r: &mut super::tests_stores::Rig, cached: bool) {
    r.set(move |s| {
        s.rail = Rail::Plugin("github");
        let clock = s.clock;
        let g = state(&mut s.places);
        g.panel_asked = true;
        g.home_asked = true;
        g.auth.asked = true;
        g.auth.connected = true;
        g.bookmarks.has_loaded = true;
        g.bookmarks.list = vec![super::bookmarks::Bookmark::of("acme/rocket").unwrap()];
        if cached {
            let mut c = g.repos.get();
            c.repos = repos();
            c.updated = Some(clock);
        }
    });
}

#[test]
fn the_follow_row_opens_the_dialog_with_the_search_ready() {
    let mut r = rig();
    on_the_panel(&mut r, true);
    assert!(r.rt.find("github-follow-dialog").is_none());
    click(&mut r.rt, "github-panel-add");
    assert!(r.rt.find("github-follow-dialog").is_some());
    assert!(says(&r.rt, "Follow a repository"));
    assert_eq!(r.rt.state.ui.open, None, "the plugin's dialog is not one of core's overlays");
    typed_in(&mut r.rt, "cre");
    assert_eq!(names(&r.rt), ["acme/crew-screen"]);
}

fn typed_in(rt: &mut Runtime<State>, text: &str) {
    press_key(rt, Key::Text(text.into()), Mods::default());
}

#[test]
fn opening_it_reads_a_stale_cache_and_keeps_a_fresh_one() {
    let mut r = rig();
    on_the_panel(&mut r, false);
    r.answer("GET", REPOS, 200, mock::repos());
    click(&mut r.rt, "github-panel-add");
    r.wait("the repositories", |s| !state_ref(&s.places).repos.get().repos.is_empty());
    assert_eq!(r.asked("GET", REPOS), 1);
    assert!(names(&r.rt).contains(&"acme/launchpad".to_string()));

    // Fresh: shutting it and opening it again asks nothing.
    r.rt.key(rune::KeyEvent { key: Key::Escape, mods: Mods::default() });
    r.rt.frame(FRAME);
    click(&mut r.rt, "github-panel-add");
    assert_eq!(r.asked("GET", REPOS), 1, "a cache under 15 minutes old is kept");
    r.rt.frame(15.0 * 60.0 + 1.0);
    r.rt.key(rune::KeyEvent { key: Key::Escape, mods: Mods::default() });
    r.rt.frame(FRAME);
    click(&mut r.rt, "github-panel-add");
    let fake = r.fake.clone();
    r.wait("the stale cache to be read again", move |_| fake.lock().unwrap().log.len() >= 2);
    assert_eq!(r.asked("GET", REPOS), 2);
}

#[test]
fn it_leaves_out_the_repositories_you_follow_already() {
    let mut r = rig();
    on_the_panel(&mut r, true);
    click(&mut r.rt, "github-panel-add");
    let all = names(&r.rt);
    assert_eq!(all.len(), 8, "nine cached, one followed: {all:?}");
    assert!(!all.contains(&"acme/rocket".to_string()));
}

#[test]
fn typing_keeps_the_names_that_contain_it_in_the_caches_order() {
    let repos = repos();
    let hits = |q: &str| listed(&repos, |_| false, q).into_iter().map(|r| r.full_name).collect::<Vec<_>>();
    assert_eq!(hits("").len(), 9, "nothing typed lists them all");
    assert_eq!(hits("")[0], "acme/rocket");
    assert_eq!(hits("lumen"), ["lumen-labs/atlas", "lumen-labs/beacon", "lumen-labs/lens", "lumen-labs/observatory"]);
    assert_eq!(hits("LENS"), ["lumen-labs/lens"], "case is ignored");
    assert_eq!(hits("at"), ["lumen-labs/atlas", "lumen-labs/observatory"], "a substring anywhere, no ranking");
    assert_eq!(hits("lens-"), Vec::<String>::new(), "letters in order are not enough");
    assert!(listed(&repos, |n| n == "lumen-labs/lens", "lens").is_empty(), "followed ones are left out");
}

#[test]
fn the_dialog_filters_as_you_type() {
    let mut r = rig();
    on_the_panel(&mut r, true);
    click(&mut r.rt, "github-panel-add");
    typed_in(&mut r.rt, "cre");
    assert_eq!(names(&r.rt), ["acme/crew-screen"]);
    assert_eq!(state_ref(&r.rt.state.places).follow.active, Some(0), "typing lights the first row");
}

#[test]
fn a_row_shows_its_lock_language_and_stars() {
    let mut r = rig();
    on_the_panel(&mut r, true);
    click(&mut r.rt, "github-panel-add");
    let under = |rt: &Runtime<State>, key: &str| -> Vec<String> {
        let all = rt.elements();
        let e = all.iter().find(|e| e.key.as_deref() == Some(key)).expect("the row");
        all.iter().filter(|x| x.path.starts_with(&e.path)).filter_map(|x| x.text.clone()).collect()
    };
    assert_eq!(under(&r.rt, "github-follow-row-acme/crew-screen"), ["acme/crew-screen", "TypeScript", "48"]);
    assert_eq!(under(&r.rt, "github-follow-row-lumen-labs/observatory"), ["lumen-labs/observatory", "5"], "no language, no pill");
    assert_eq!(under(&r.rt, "github-follow-row-acme/billing"), ["acme/billing", "Go"], "no stars, none shown");
}

#[test]
fn nothing_to_show_says_loading_while_it_loads_else_none_found() {
    let mut r = rig();
    on_the_panel(&mut r, false);
    r.set(|s| {
        state(&mut s.places).follow.open = true;
        state(&mut s.places).repos.get().loading = true;
    });
    assert!(!says(&r.rt, "Loading repositories\u{2026}"), "Fleet's list says nothing until something is typed");
    click(&mut r.rt, "github-follow-input");
    typed_in(&mut r.rt, "la");
    assert!(says(&r.rt, "Loading repositories\u{2026}"));
    assert!(!says(&r.rt, "No repositories found."));
    r.set(|s| state(&mut s.places).repos.get().loading = false);
    assert!(says(&r.rt, "No repositories found."));
    assert!(!says(&r.rt, "Loading repositories\u{2026}"));
}

#[test]
fn enter_follows_the_highlighted_one_closes_and_reads_your_work_again() {
    let mut r = rig();
    on_the_panel(&mut r, true);
    r.answer("GET", WORK, 200, mock::work());
    click(&mut r.rt, "github-panel-add");
    typed_in(&mut r.rt, "lens");
    press_key(&mut r.rt, Key::Enter, Mods::default());
    assert!(r.rt.find("github-follow-dialog").is_none(), "it closes");
    r.wait("the bookmark added", |s| state_ref(&s.places).bookmarks.has("lumen-labs/lens"));
    assert_eq!(r.asked("POST", MARKS), 1);
    assert_eq!(r.bodies("POST", MARKS), [serde_json::json!({"repo": "lumen-labs/lens"})]);
    r.wait("your work read again", |s| state_ref(&s.places).work.work.is_some());
    assert_eq!(r.asked("GET", WORK), 1, "`added` reloads the work");
}

#[test]
fn a_click_on_a_row_follows_it() {
    let mut r = rig();
    on_the_panel(&mut r, true);
    click(&mut r.rt, "github-panel-add");
    click(&mut r.rt, "github-follow-row-acme/launchpad");
    assert!(r.rt.find("github-follow-dialog").is_none());
    r.wait("the bookmark added", |s| state_ref(&s.places).bookmarks.has("acme/launchpad"));
}

#[test]
fn the_arrows_move_the_highlight_and_wrap() {
    let mut r = rig();
    on_the_panel(&mut r, true);
    click(&mut r.rt, "github-panel-add");
    let active = |r: &super::tests_stores::Rig| state_ref(&r.rt.state.places).follow.active;
    assert_eq!(active(&r), None, "it opens with no row lit, as Fleet's list does");
    press_key(&mut r.rt, Key::Enter, Mods::default());
    assert!(r.rt.find("github-follow-dialog").is_some(), "Enter with no row lit follows nothing");
    press_key(&mut r.rt, Key::Down, Mods::default());
    assert_eq!(active(&r), Some(0), "down from nothing lights the first");
    press_key(&mut r.rt, Key::Up, Mods::default());
    assert_eq!(active(&r), Some(7), "up from the first wraps to the last of the eight");
    press_key(&mut r.rt, Key::Down, Mods::default());
    assert_eq!(active(&r), Some(0));
    press_key(&mut r.rt, Key::Down, Mods::default());
    assert_eq!(active(&r), Some(1));
}

#[test]
fn escape_and_the_close_button_shut_it_and_follow_nothing() {
    let mut r = rig();
    on_the_panel(&mut r, true);
    click(&mut r.rt, "github-panel-add");
    press_key(&mut r.rt, Key::Escape, Mods::default());
    assert!(r.rt.find("github-follow-dialog").is_none());
    click(&mut r.rt, "github-panel-add");
    click(&mut r.rt, "github-follow-close");
    assert!(r.rt.find("github-follow-dialog").is_none());
    assert_eq!(r.asked("POST", MARKS), 0);
    // Opened again, it starts empty.
    click(&mut r.rt, "github-panel-add");
    assert_eq!(state_ref(&r.rt.state.places).follow.query.text, "");
}

#[test]
fn a_refused_follow_says_why_and_reads_nothing_again() {
    let mut r = rig();
    on_the_panel(&mut r, true);
    r.answer("POST", MARKS, 500, serde_json::json!({"error": "Nope."}));
    click(&mut r.rt, "github-panel-add");
    click(&mut r.rt, "github-follow-row-acme/launchpad");
    r.wait("the refusal", |s| state_ref(&s.places).bookmarks.action_error.as_deref() == Some("Nope."));
    assert!(!state_ref(&r.rt.state.places).bookmarks.has("acme/launchpad"));
    assert_eq!(r.asked("GET", WORK), 0);
}

#[test]
fn a_plugin_that_is_off_draws_no_dialog() {
    let mut r = rig();
    on_the_panel(&mut r, true);
    click(&mut r.rt, "github-panel-add");
    r.set(|s| s.pickers.prefs.adopt(&serde_json::json!({ "plugins.disabled": "[\"github\"]" })));
    assert!(r.rt.find("github-follow-dialog").is_none());
}
