//! Piece F of component 21: an item's row, its session chip and Fleet's side (`GitHubWorkItemDetailPage.test.ts`'s "waits for the
//! sessions panel switch before setting the preset and navigating", through `start_session`; the rest are native, from the sheet's rows 6.x
//! and 7.x). The rows are the `github-rows` scene's: invented items and sessions, no server.
use super::items::ItemSummary;
use super::sessions::{sessions_for, start_session};
use super::view::row::description;
use super::view::rows_scene;
use super::{Where, mock, state, state_ref};
use crate::fleet::row_status::Tone;
use crate::fleet::store::Route;
use crate::tests::{FRAME, app, click, press_key};
use crate::{Rail, State};
use rune::{Key, Mods, Runtime};

fn rows() -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| rows_scene::scene(s, true));
    for _ in 0..3 {
        rt.frame(FRAME);
    }
    rt
}

fn item(n: usize) -> ItemSummary {
    ItemSummary::read(&mock::rows()[n]["item"])
}

fn opacity_of(rt: &Runtime<State>, key: &str) -> f32 {
    rt.elements().iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).map(|e| e.opacity).unwrap_or_else(|| panic!("no {key}"))
}

fn texts_of(rt: &Runtime<State>, key: &str) -> Vec<String> {
    let all = rt.elements();
    let Some(el) = all.iter().find(|e| e.key.as_deref() == Some(key)) else { return Vec::new() };
    all.iter().filter(|e| e.path.starts_with(&el.path)).filter_map(|e| e.text.clone()).collect()
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..8 {
        rt.frame(FRAME);
    }
}

#[test]
fn waits_for_the_sessions_panel_switch_before_setting_the_preset_and_navigating() {
    let mut rt = rows();
    rt.update(|s, _| {
        s.sidebar_open = false;
        let item = item(1);
        start_session(s, &item, Some("Round to the nearest tenth."));
        // All of it has happened by the time the page is the one on screen: the panel is open, the rail is Sessions, and the item is
        // the page's initial source.
        assert!(s.sidebar_open, "the sessions panel opens");
        assert_eq!(s.rail, Rail::Sessions);
        assert!(matches!(s.route, Route::NewSession { project_id: None }));
        let preset = s.new_draft.initial_source().expect("the item is the initial source").clone();
        assert_eq!((preset.repo_full_name.as_str(), preset.number), ("acme/rocket", 220));
        assert_eq!(preset.body.as_deref(), Some("Round to the nearest tenth."));
    });
    settle(&mut rt);
    let preset = rt.state.new_draft.draft().and_then(|d| d.github_preset.clone()).expect("the page took the item");
    assert_eq!(preset.number, 220);
    assert!(rt.state.new_draft.initial_source().is_none(), "taken once");
}

#[test]
fn a_row_says_what_it_is_in_words() {
    assert_eq!(
        description(&item(0)),
        "Pull request #218: Queue retries with backoff · 1 failing · 2 comments".replace("1 failing", &blocked_words())
    );
    // A merged pull request says no state words; an issue says its comments.
    assert_eq!(description(&item(4)), "Pull request #215: Cache the crew roster · 5 comments");
    assert_eq!(description(&item(5)), "Pull request #214: Drop the old countdown · 1 comment");
    assert_eq!(description(&item(6)), "Issue #7: Launch countdown skips a second · 4 comments");
    // No comments, no tail.
    assert_eq!(description(&item(3)), "Pull request #222: Trim the manifest · Draft");
}

fn blocked_words() -> String {
    crate::fleet::pr_state::pr_words(&super::items::item_pr_facts(&item(0), None))
}

#[test]
fn a_row_shows_its_parts_and_at_most_three_labels_and_reviewers() {
    let rt = rows();
    let line = texts_of(&rt, "github-row-0");
    assert!(line.iter().any(|t| t == "Queue retries with backoff"));
    assert!(line.iter().any(|t| t == "acme/rocket"), "show-repo");
    assert!(line.iter().any(|t| t == "#218"));
    assert!(line.iter().any(|t| t == "devon-park"));
    assert!(line.iter().any(|t| t == "feat/pr-218"));
    assert!(line.iter().any(|t| t.starts_with("+412")) || rt.find("github-row-0-diff").is_some());
    assert!(rt.find("github-row-0-label-needs-tests").is_some());
    assert!(rt.find("github-row-0-label-extra").is_none(), "the fourth label is left out");
    // Those who approved or asked for changes; a comment is not a verdict.
    assert!(rt.find("github-row-0-reviewer-kit-lee").is_some());
    assert!(rt.find("github-row-0-reviewer-sam-rivera").is_some());
    assert!(rt.find("github-row-0-reviewer-ana-brook").is_none());
    // An issue has no checks, diff or reviewers.
    assert!(rt.find("github-row-6-diff").is_none());
    assert!(rt.find("github-row-6-reviewer-kit-lee").is_none());
    assert_eq!(texts_of(&rt, "github-row-1-reason"), ["Review requested"]);
    assert!(rt.find("github-row-0-reason").is_none());
}

#[test]
fn a_press_opens_the_items_page_and_enter_does_too() {
    let mut rt = rows();
    click(&mut rt, "github-row-0-title");
    assert_eq!(
        state_ref(&rt.state.places).at,
        Where::Item { owner: "acme".into(), repo: "rocket".into(), kind: super::Kind::Pull, number: 218 }
    );

    rt.update(|s, tx| {
        state(&mut s.places).at = Where::Home;
        tx.focus("github-row-6", true);
    });
    settle(&mut rt);
    press_key(&mut rt, Key::Enter, Mods::default());
    assert_eq!(
        state_ref(&rt.state.places).at,
        Where::Item { owner: "acme".into(), repo: "rocket".into(), kind: super::Kind::Issue, number: 7 }
    );
}

#[test]
fn a_label_press_doesnt_open_the_row() {
    let mut rt = rows();
    click(&mut rt, "github-row-0-label-bug");
    assert_eq!(state_ref(&rt.state.places).demo_label.as_deref(), Some("bug"));
    assert_eq!(state_ref(&rt.state.places).at, Where::Home);
}

#[test]
fn the_actions_replace_the_signals_while_the_row_has_the_pointer() {
    let mut rt = rows();
    assert!(opacity_of(&rt, "github-row-0-signals") > 0.99);
    assert!(opacity_of(&rt, "github-row-0-actions") < 0.01);
    let (x, y) = rt.find("github-row-0-title").unwrap();
    rt.pointer_move(x, y);
    settle(&mut rt);
    assert!(opacity_of(&rt, "github-row-0-signals") < 0.01, "hovered: the actions take the right side");
    assert!(opacity_of(&rt, "github-row-0-actions") > 0.99);
    rt.pointer_move(1.0, 1.0);
    settle(&mut rt);
    assert!(opacity_of(&rt, "github-row-0-signals") > 0.99);
}

#[test]
fn start_session_is_on_an_item_nothing_is_on_and_open_session_on_one_with_a_session() {
    let mut rt = rows();
    // #218 has a session: Open session, no Start.
    assert!(rt.find("github-row-0-open-session").is_some());
    assert!(rt.find("github-row-0-start").is_none());
    // Issue #7 has none.
    assert!(rt.find("github-row-6-start").is_some());
    assert!(rt.find("github-row-6-open-session").is_none());
    click(&mut rt, "github-row-6-start");
    assert_eq!(rt.state.rail, Rail::Sessions);
    assert!(matches!(rt.state.route, Route::NewSession { .. }));
    settle(&mut rt);
    assert_eq!(rt.state.new_draft.draft().and_then(|d| d.github_preset.as_ref()).map(|p| p.number), Some(7));
}

#[test]
fn open_session_and_the_chip_open_the_first_session() {
    let mut rt = rows();
    click(&mut rt, "github-row-1-chip");
    assert_eq!(rt.state.rail, Rail::Sessions);
    assert_eq!(rt.state.open_id().as_deref(), Some("sess-waiting"));
    let mut rt = rows();
    click(&mut rt, "github-row-0-open-session");
    assert_eq!(rt.state.open_id().as_deref(), Some("sess-working"));
}

#[test]
fn the_chip_says_needs_you_what_the_session_is_doing_or_idle_and_how_many_more() {
    let rt = rows();
    assert_eq!(texts_of(&rt, "github-row-0-chip"), ["Working"]);
    assert_eq!(texts_of(&rt, "github-row-1-chip"), ["Needs you", "+1"]);
    assert_eq!(texts_of(&rt, "github-row-2-chip"), ["Idle"]);
    assert!(rt.find("github-row-3-chip").is_none(), "an archived session has no chip");
    let tone = |id: &str| sessions_for(&rt.state, id).first().map(|s| (s.tone, s.waiting, s.live));
    assert_eq!(tone("acme/rocket#218"), Some((Tone::Working, false, true)));
    assert_eq!(tone("acme/rocket#220"), Some((Tone::Attention, true, true)));
    assert_eq!(tone("acme/rocket#221"), Some((Tone::Quiet, false, false)));
}

#[test]
fn the_chips_title_names_the_sessions_and_says_untitled_for_none() {
    let mut rt = rows();
    let named = sessions_for(&rt.state, "acme/rocket#220");
    assert_eq!(super::view::session_chip::chip_title(&named), "In Fleet: Round the fuel gauge, Review gauge tests");
    rt.update(|s, _| s.projects[0].sessions[1].title = "  ".into());
    let named = sessions_for(&rt.state, "acme/rocket#220");
    assert_eq!(super::view::session_chip::chip_title(&named), "In Fleet: Untitled session, Review gauge tests");
}

#[test]
fn sessions_are_the_ones_linked_ignoring_case_most_recently_active_first_and_not_archived() {
    let mut rt = rows();
    let ids = |s: &State, key: &str| sessions_for(s, key).into_iter().map(|x| x.id).collect::<Vec<_>>();
    assert_eq!(ids(&rt.state, "ACME/Rocket#220"), ["sess-waiting", "sess-waiting-2"]);
    assert!(ids(&rt.state, "acme/rocket#222").is_empty(), "archived is left out");
    assert!(ids(&rt.state, "acme/rocket#999").is_empty());
    // The one used last comes first.
    rt.update(|s, _| s.projects[0].sessions[2].row.updated = s.now_ms.unwrap_or_default());
    assert_eq!(ids(&rt.state, "acme/rocket#220"), ["sess-waiting-2", "sess-waiting"]);
}
