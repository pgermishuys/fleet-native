//! Component 23, piece P: the Board's sources form. `BoardSourceConfig.test.ts`'s six cases under their names, against a fake
//! Fleet and GitHub's bookmarks, then the plugin's board-source place (`plugin-switch.test.ts` and `plugin-runtime.test.ts`:
//! what a switched-off plugin offers), then what this app adds (the words of each state, what a failure says, and unmounting).
use super::{ID, bookmarks::Bookmark, state};
use crate::State;
use crate::fleet::board::Source;
use crate::fleet::board::store::Mode;
use crate::fleet::places::Places;
use crate::fleet::prefs::Prefs;
use crate::tests::{FRAME, click, press_key};
use crate::tests_board_store::{Kept, Rig, empty, loaded, rig};
use rune::{Key, Mods, Runtime};
use serde_json::json;

const SOURCES: &str = "/api/boards/board-1/sources";

fn source(id: &str, config: &str, last_sync_at: Option<&str>) -> Source {
    Source {
        id: id.into(),
        board_id: "board-1".into(),
        provider_type: "github".into(),
        config: config.into(),
        last_sync_at: last_sync_at.map(str::to_string),
        created_at: "2026-04-24T14:00:00.000Z".into(),
        updated_at: "2026-04-24T14:00:00.000Z".into(),
    }
}

/// `bookmarkState`'s one bookmark.
fn bookmark(names: &[&str]) -> Vec<Bookmark> {
    names.iter().map(|n| Bookmark::of(n).expect("an owner and a name")).collect()
}

fn set_bookmarks(rt: &mut Runtime<State>, names: &[&str]) {
    let list = bookmark(names);
    rt.update(move |s: &mut State, _| {
        let b = &mut state(&mut s.places).bookmarks;
        b.list = list;
        b.has_loaded = true;
    });
}

/// The Board on `kept`'s board in manage mode, with the bookmarks given and the sources loaded.
fn managing(kept: Kept, names: &[&str]) -> Rig {
    let mut rig = rig(kept);
    set_bookmarks(&mut rig.rt, names);
    rig.open();
    rig.rt.update(|s: &mut State, _| s.board.set_mode(Mode::Manage));
    rig.rt.frame(FRAME);
    rig.wait("the sources to load", |s| s.board.source_form.is_mounted() && !s.board.source_form.loading);
    rig.rt.frame(FRAME);
    rig
}

fn shown(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn text_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).and_then(|e| e.text)
}

fn says(rt: &Runtime<State>, what: &str) -> bool {
    rt.elements().iter().any(|e| !e.exiting && e.text.as_deref() == Some(what))
}

fn is_off(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).is_some_and(|e| e.opacity < 1.0)
}

fn pick(rig: &mut Rig, n: usize) {
    click(&mut rig.rt, "board-source-repo-select");
    click(&mut rig.rt, &format!("board-source-repo-list-opt{n}"));
}

fn type_labels(rt: &mut Runtime<State>, words: &str) {
    click(rt, "board-source-label-filter");
    press_key(rt, Key::Text(words.into()), Mods::default());
}

fn submit(rig: &mut Rig) {
    click(&mut rig.rt, "board-source-submit");
    rig.wait("the add to answer", |s| !s.board.source_form.creating);
    rig.rt.frame(FRAME);
}

/// The options the open select lists, in order.
fn options(rt: &Runtime<State>) -> Vec<String> {
    (0..8)
        .map_while(|i| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(&format!("board-source-repo-list-opt{i}"))))
        .filter_map(|e| e.label)
        .collect()
}

// ─── BoardSourceConfig.test.ts ──────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn adds_a_repo_source_lists_it_and_removes_it() {
    let mut rig = managing(loaded(), &["acme/rocket"]);
    pick(&mut rig, 1);
    type_labels(&mut rig.rt, "bug, priority:high");
    submit(&mut rig);

    assert_eq!(
        rig.bodies_of(&format!("POST {SOURCES}")),
        [json!({ "providerType": "github", "config": r#"{"repository":"acme/rocket","labels":"bug, priority:high"}"# })]
    );
    assert!(shown(&rig.rt, "board-source-source-1"), "the new source is listed");
    assert!(says(&rig.rt, "acme/rocket"));
    let meta = text_of(&rig.rt, "board-source-source-1-meta").expect("its line");
    assert!(
        meta.contains("Labels: bug, priority:high") && meta.contains("Any assignee") && meta.contains("Last sync: Not synced yet"),
        "{meta}"
    );

    click(&mut rig.rt, "board-source-source-1-remove");
    rig.wait("the remove to answer", |s| s.board.source_form.removing.is_none() && s.board.source_form.sources.is_empty());
    rig.rt.frame(FRAME);
    assert_eq!(rig.heard_exactly(&format!("DELETE {SOURCES}/source-1")), 1);
    assert!(!shown(&rig.rt, "board-source-source-1"));
    assert!(shown(&rig.rt, "board-source-empty") && says(&rig.rt, "No configured sources yet"));
}

#[test]
fn adds_an_assigned_to_me_source_with_assignee_in_config_json() {
    let mut rig = managing(loaded(), &["acme/rocket"]);
    pick(&mut rig, 1);
    click(&mut rig.rt, "board-source-assigned-to-me");
    submit(&mut rig);

    assert_eq!(
        rig.bodies_of(&format!("POST {SOURCES}")),
        [json!({ "providerType": "github", "config": r#"{"repository":"acme/rocket","assignee":"@me"}"# })]
    );
    assert!(text_of(&rig.rt, "board-source-source-1-meta").is_some_and(|m| m.contains("Assignee: @me")));
}

#[test]
fn offers_no_way_to_add_a_source_when_no_plugin_contributes_a_board_source() {
    let mut kept = loaded();
    kept.sources = vec![source("source-1", r#"{"repository":"acme/rocket","labels":"bug, priority:high"}"#, None)];
    let mut rig = rig(kept);
    rig.rt.update(|s: &mut State, _| s.places = Places::default());
    rig.open();
    rig.rt.update(|s: &mut State, _| s.board.set_mode(Mode::Manage));
    rig.rt.frame(FRAME);
    rig.wait("the sources to load", |s| s.board.source_form.is_mounted() && !s.board.source_form.loading);
    rig.rt.frame(FRAME);

    assert!(!shown(&rig.rt, "board-source-form"));
    assert!(
        !shown(&rig.rt, "board-source-description") && !shown(&rig.rt, "board-source-helper"),
        "no words from a plugin that isn't there"
    );
    // What is already configured still lists, and can be removed.
    assert!(shown(&rig.rt, "board-source-source-1") && shown(&rig.rt, "board-source-source-1-remove"));
    assert!(says(&rig.rt, "Sources") || says(&rig.rt, "SOURCES"));
}

#[test]
fn lists_the_bookmarked_repositories_by_name_in_the_repository_select() {
    let mut rig = managing(loaded(), &["acme/zebra", "acme/anvil"]);
    click(&mut rig.rt, "board-source-repo-select");
    assert_eq!(options(&rig.rt), ["Select a repository", "acme/anvil", "acme/zebra"]);
    assert!(!says(&rig.rt, "Bookmark a repository in the GitHub panel to add it as a board source."));
}

#[test]
fn points_at_the_github_panel_when_there_are_no_bookmarks_and_says_so_while_they_load() {
    let mut rig = managing(loaded(), &[]);
    assert!(says(&rig.rt, "Bookmark a repository in the GitHub panel to add it as a board source."));

    rig.rt.update(|s: &mut State, _| state(&mut s.places).bookmarks.loading = true);
    rig.rt.frame(FRAME);
    assert!(!says(&rig.rt, "Bookmark a repository in the GitHub panel to add it as a board source."));
    assert!(says(&rig.rt, "Loading bookmarks…"), "the select says so");
    assert!(is_off(&rig.rt, "board-source-repo-select"), "and is disabled");
}

#[test]
fn shows_a_bookmark_error_with_a_retry_that_refreshes_the_bookmarks() {
    let mut rig = managing(loaded(), &["acme/rocket"]);
    rig.rt.update(|s: &mut State, _| state(&mut s.places).bookmarks.error = Some("Failed to fetch bookmarks from server.".into()));
    rig.rt.frame(FRAME);
    assert!(rig.rt.find("board-source-banner").is_some());
    assert_eq!(text_of(&rig.rt, "board-source-error").as_deref(), Some("Failed to fetch bookmarks from server."));
    assert!(says(&rig.rt, "Try loading bookmarks again"));

    // Refreshing asks the server again; this fake has no bookmarks route, which is a refusal, and the loading state shows.
    rig.rt.update(|s: &mut State, _| state(&mut s.places).bookmarks.error = None);
    click_retry(&mut rig);
}

fn click_retry(rig: &mut Rig) {
    rig.rt.update(|s: &mut State, _| state(&mut s.places).bookmarks.error = Some("Failed to fetch bookmarks from server.".into()));
    rig.rt.frame(FRAME);
    click(&mut rig.rt, "board-source-retry");
    // The retry asked for the bookmarks again: loading begins (the one request in flight).
    assert!(state(&mut rig.rt.state.places).bookmarks.loading);
}

// ─── the plugin's place (plugin-switch.test.ts, plugin-runtime.test.ts) ────────────────────────────────────────────────

fn off(id: &str) -> Prefs {
    let mut p = Prefs::default();
    p.adopt(&json!({ "plugins.disabled": serde_json::to_string(&[id]).unwrap() }));
    p
}

#[test]
fn removes_everything_a_plugin_adds_when_it_is_off_and_restores_it_when_it_is_on_board_sources() {
    let places = crate::plugins::register(Places::default());
    let sources = places.board_sources(&Prefs::default());
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].id, ID);
    assert!(places.board_sources(&off(ID)).is_empty(), "off, the plugin offers no board source");
    assert!(places.manifests().iter().any(|p| p.id == ID), "still listed, so the Plugins page can switch it back on");
    assert_eq!(places.board_sources(&Prefs::default()).len(), 1);
}

#[test]
fn reads_board_sources_that_plugins_contribute_and_drops_them_with_the_plugin() {
    let places = crate::plugins::register(Places::default());
    let one = places.board_sources(&Prefs::default())[0];
    assert_eq!(one.description, "Add GitHub issue sources from your bookmarks, optionally narrowing sync to matching labels.");
    assert_eq!(one.empty_hint, "Bookmark a repository in the GitHub panel to add it as a board source.");
    // A plugin that has none (or is off) leaves the form with nothing to add from.
    assert!(Places::default().board_sources(&Prefs::default()).is_empty());
}

#[test]
fn the_place_reads_the_plugin_s_bookmarks() {
    let mut r = crate::tests::app();
    set_bookmarks(&mut r, &["acme/zebra", "acme/anvil"]);
    let place = r.state.board_source().expect("GitHub fills the place");
    assert_eq!((place.repositories)(&r.state), ["acme/zebra", "acme/anvil"], "the plugin's own order; the form sorts");
    assert!(!(place.is_loading)(&r.state) && (place.error)(&r.state).is_none());
    r.update(|s: &mut State, _| state(&mut s.places).bookmarks.error = Some("down".into()));
    assert_eq!((place.error)(&r.state).as_deref(), Some("down"));
    r.update(|s: &mut State, _| s.pickers.prefs.adopt(&json!({ "plugins.disabled": json!([ID]).to_string() })));
    assert!(r.state.board_source().is_none(), "off, no source");
}

// ─── what this app adds ─────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_form_says_what_a_source_s_config_holds_and_when_it_last_synced() {
    let mut kept = loaded();
    kept.sources = vec![
        source("source-1", r#"{"repository":"acme/rocket","labels":["bug"," "],"assignee":"@me"}"#, Some("2026-04-24T14:30:00.000Z")),
        source("source-2", r#"{"owner":"acme","repo":"anvil"}"#, Some("not a time")),
        source("source-3", "not json", None),
    ];
    let rig = managing(kept, &["acme/rocket"]);
    let meta = |id: &str| text_of(&rig.rt, &format!("board-source-{id}-meta")).unwrap_or_default();
    assert!(meta("source-1").starts_with("Labels: bug•Assignee: @me•Last sync: Apr 24, "), "{}", meta("source-1"));
    assert_eq!(meta("source-2"), "No label filter•Any assignee•Last sync: Unknown sync time");
    assert_eq!(meta("source-3"), "No label filter•Any assignee•Last sync: Not synced yet");
    assert_eq!(text_of(&rig.rt, "board-source-source-3-repository").as_deref(), Some("Unknown repository"));
    // Sorted by repository: "Unknown repository" after the a's.
    let order: Vec<String> = rig.rt.state.board.source_form.sources.iter().map(|s| s.id.clone()).collect();
    assert_eq!(order, ["source-1", "source-2", "source-3"]);
}

#[test]
fn a_repeated_source_is_refused_with_a_line_and_the_repository_stays_after_an_add() {
    let mut rig = managing(loaded(), &["acme/rocket"]);
    pick(&mut rig, 1);
    submit(&mut rig);
    assert_eq!(rig.rt.state.board.source_form.repository, "acme/rocket", "the repository stays");
    assert!(says(&rig.rt, "This repository and label filter is already configured."));
    assert!(is_off(&rig.rt, "board-source-submit"), "a duplicate can't be added again");
    click(&mut rig.rt, "board-source-submit");
    assert_eq!(rig.bodies_of(&format!("POST {SOURCES}")).len(), 1);
    // Other labels make it another source.
    type_labels(&mut rig.rt, "bug");
    assert!(!says(&rig.rt, "This repository and label filter is already configured."));
}

#[test]
fn a_failed_load_add_or_remove_says_why_in_the_band_and_the_board_s_own_banner_stays_quiet() {
    let mut kept = loaded();
    kept.refuse_listing = Some("The sources are down.".into());
    let rig = managing(kept, &["acme/rocket"]);
    assert_eq!(text_of(&rig.rt, "board-source-error").as_deref(), Some("The sources are down."));
    assert!(rig.rt.state.board.error.is_none(), "the board's banner never hears of it");
    assert!(!shown(&rig.rt, "board-source-retry"), "the retry is for the bookmarks");

    let mut kept = loaded();
    kept.refuse_adding = Some("No more sources.".into());
    let mut rig = managing(kept, &["acme/rocket"]);
    pick(&mut rig, 1);
    submit(&mut rig);
    assert_eq!(text_of(&rig.rt, "board-source-error").as_deref(), Some("No more sources."));
    assert!(rig.rt.state.board.source_form.sources.is_empty());

    let mut kept = loaded();
    kept.sources = vec![source("source-1", r#"{"repository":"acme/rocket"}"#, None)];
    kept.refuse_removing = Some("Not now.".into());
    let mut rig = managing(kept, &["acme/rocket"]);
    click(&mut rig.rt, "board-source-source-1-remove");
    rig.wait("the remove to answer", |s| s.board.source_form.removing.is_none());
    rig.rt.frame(FRAME);
    assert_eq!(text_of(&rig.rt, "board-source-error").as_deref(), Some("Not now."));
    assert!(shown(&rig.rt, "board-source-source-1"), "it stays listed");
}

#[test]
fn with_no_board_the_form_and_list_give_way_to_a_note() {
    let mut rig = rig(empty());
    set_bookmarks(&mut rig.rt, &["acme/rocket"]);
    rig.open();
    rig.rt.update(|s: &mut State, _| s.board.set_mode(Mode::Manage));
    rig.rt.frame(FRAME);
    rig.rt.frame(FRAME);
    assert!(shown(&rig.rt, "board-source-no-board") && says(&rig.rt, "Create a board lane first to enable source syncing"));
    assert!(!shown(&rig.rt, "board-source-form") && !shown(&rig.rt, "board-source-empty"));
    assert_eq!(rig.heard_exactly(&format!("GET {SOURCES}")), 0, "nothing to ask for");
}

#[test]
fn leaving_manage_mode_unmounts_the_form_and_the_next_visit_loads_again() {
    let mut rig = managing(loaded(), &["acme/rocket"]);
    pick(&mut rig, 1);
    type_labels(&mut rig.rt, "bug");
    assert_eq!(rig.heard_exactly(&format!("GET {SOURCES}")), 1);
    rig.rt.update(|s: &mut State, _| s.board.set_mode(Mode::Work));
    rig.rt.frame(FRAME);
    assert!(!shown(&rig.rt, "board-sources"));
    assert!(!rig.rt.state.board.source_form.is_mounted() && rig.rt.state.board.source_form.repository.is_empty());

    rig.rt.update(|s: &mut State, _| s.board.set_mode(Mode::Manage));
    rig.rt.frame(FRAME);
    rig.wait("the sources to load again", |s| s.board.source_form.is_mounted() && !s.board.source_form.loading);
    assert_eq!(rig.heard_exactly(&format!("GET {SOURCES}")), 2);
    assert_eq!(rig.rt.state.board.source_form.labels.text, "", "the form is fresh");
}
