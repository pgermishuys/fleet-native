//! The New session page's reads (piece C): the cases of Fleet's `use-harnesses.test.ts`, `use-harness-catalog.test.ts`,
//! `folder-access.test.ts` and the data parts of `NewSessionComposer.test.ts` that belong to this app, under Fleet's names
//! (test names are the describe path, snake_cased), plus one test per error text. Canned JSON for each call; one test
//! runs the threads against a fake server on a loopback port. The folders are invented.
use super::feed_state::*;
use super::feeds::*;
use super::harness_choice::{HarnessEntry, Load};
use crate::fleet::events::{Event, Kind};
use crate::fleet::settings::{HOME_MACHINE_KEY, Sidebar};
use serde_json::json;

fn answer(status: u16, body: serde_json::Value) -> Answer {
    Ok((status, body.to_string()))
}

fn repo(name: &str, root: &str) -> Repository {
    Repository { name: name.into(), path: format!("{root}/{name}"), parent_root: root.into() }
}

// ─── useRepositories ────────────────────────────────────────────────────────

#[test]
fn group_by_root_groups_by_location_in_the_order_met_each_group_sorted_by_name() {
    let list = [repo("zeta", "/b"), repo("beta", "/a"), repo("Alpha", "/b"), repo("alpha", "/b")];
    let groups = group_by_root(&list);
    assert_eq!(groups.iter().map(|(r, _)| r.as_str()).collect::<Vec<_>>(), ["/b", "/a"]);
    assert_eq!(groups[0].1.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["Alpha", "alpha", "zeta"]);
}

#[test]
fn use_repositories_reads_the_scan_and_keeps_its_time() {
    let body = json!({ "repositories": [{ "name": "rocket", "path": "/src/rocket", "parentRoot": "/src" }, { "name": "no-path" }], "scannedAt": 1700000000000i64 });
    let scan = parse_scan(answer(200, body)).unwrap();
    assert_eq!(scan.repositories, [repo("rocket", "/src")]);
    assert_eq!(scan.scanned_at, Some(1_700_000_000_000));
}

#[test]
fn use_repositories_says_the_servers_words_for_a_refusal_and_keeps_the_list_it_had() {
    assert_eq!(parse_scan(answer(500, json!({ "error": "Scan failed." }))), Err("Scan failed.".to_string()));
    assert_eq!(parse_scan(answer(502, json!({}))), Err("HTTP 502".to_string()));
    assert_eq!(parse_scan(Ok((200, String::new()))), Err("No data returned".to_string()));
    assert_eq!(parse_scan(Err("connection refused".into())), Err("connection refused".to_string()));

    let mut feed = RepoFeed::seeded(Scan { repositories: vec![repo("rocket", "/src")], scanned_at: Some(5) });
    feed.begin();
    assert!(feed.loading);
    assert!(!feed.finish(Err("Scan failed.".into())));
    assert_eq!((feed.list.len(), feed.error.as_deref(), feed.loading), (1, Some("Scan failed."), false));
}

#[test]
fn use_repositories_draws_the_list_saved_on_the_last_visit_at_once_per_machine() {
    let mut sidebar = Sidebar::default();
    let scan = Scan { repositories: vec![repo("rocket", "/src")], scanned_at: Some(7) };
    sidebar.save_repositories(HOME_MACHINE_KEY, &scan);
    sidebar.save_repositories("falcon", &Scan { repositories: vec![repo("nest", "/srv")], scanned_at: None });
    let again = Sidebar::parse(&sidebar.to_json());
    let saved: Vec<(String, Scan)> = again.saved_repositories().map(|(k, s)| (k.clone(), s.clone())).collect();
    assert_eq!(saved.len(), 2);
    let home = RepoFeed::seeded(saved.iter().find(|(k, _)| k == HOME_MACHINE_KEY).unwrap().1.clone());
    assert_eq!((home.list, home.scanned_at), (scan.repositories, Some(7)));
    // Unreadable storage is an empty store, and a value of the wrong kind is dropped on its own.
    assert_eq!(Sidebar::parse("not json").saved_repositories().count(), 0);
}

// ─── useRepositoryDetail, useWorktrees ──────────────────────────────────────

fn detail_body() -> serde_json::Value {
    json!({ "repository": { "name": "rocket", "path": "/src/rocket", "branch": "fix-login", "defaultBase": "main", "defaultBranch": "main",
        "branches": [{ "name": "main", "shortHash": "aaa111", "message": "m", "author": "Ada", "date": "2026-11-01", "isCurrent": false, "isRemote": false },
                     { "name": "fix-login", "shortHash": "bbb222", "message": "m", "author": "Ada", "date": "2026-11-02", "isCurrent": true, "isRemote": false }, { "oops": 1 }],
        "remotes": [{ "name": "origin", "url": "git@example.com:ada/rocket.git", "github": { "owner": "ada", "repo": "rocket" } }, { "name": "other", "url": "u" }] } })
}

#[test]
fn use_repository_detail_reads_the_branches_and_the_remotes_with_their_github_repository() {
    let d = parse_detail(answer(200, detail_body())).unwrap();
    assert_eq!((d.branch.as_str(), d.default_base.as_str()), ("fix-login", "main"));
    assert_eq!(d.branches.len(), 2);
    assert!(d.branches[1].is_current && !d.branches[0].is_current);
    assert_eq!(d.remotes[0].github, Some(("ada".into(), "rocket".into())));
    assert_eq!(d.remotes[1].github, None);
    assert_eq!(parse_detail(answer(404, json!({ "error": "Not a repository." }))), Err("Not a repository.".to_string()));
    assert_eq!(parse_detail(answer(200, json!({}))), Err("No data returned".to_string()));
}

#[test]
fn use_repository_detail_drops_an_answer_for_a_path_the_page_has_left() {
    let mut feeds = Feeds::default();
    let first = feeds.detail.begin(Some("/src/rocket"), false).unwrap();
    assert!(feeds.detail.loading);
    let second = feeds.detail.begin(Some("/src/comet"), false).unwrap();
    // The first one's answer arrives late: it changes nothing.
    assert!(!feeds.detail.finish(first, parse_detail(answer(200, detail_body()))));
    assert!(feeds.detail.value.is_none() && feeds.detail.loading);
    assert!(feeds.detail.finish(second, Err("boom".into())));
    assert_eq!((feeds.detail.error.as_deref(), feeds.detail.loading), (Some("boom"), false));
    // A null path cancels and clears.
    let third = feeds.detail.begin(Some("/src/rocket"), false).unwrap();
    assert_eq!(feeds.detail.begin(None, false), None);
    assert!(!feeds.detail.finish(third, parse_detail(answer(200, detail_body()))));
    assert!(feeds.detail.value.is_none() && !feeds.detail.loading && feeds.detail.path.is_none());
}

#[test]
fn use_worktrees_lists_them_keeps_the_last_while_the_next_loads_and_shows_none_after_an_error() {
    let body = json!({ "worktrees": [{ "path": "/src/rocket-worktrees/a", "branch": "a", "commitHash": "abc" }, { "path": "/src/rocket-worktrees/d", "branch": null, "commitHash": "def" }] });
    let list = parse_worktrees(answer(200, body)).unwrap();
    assert_eq!((list.len(), list[1].branch.clone()), (2, None));
    let mut feeds = Feeds::default();
    let a = feeds.worktrees.begin(Some("/src/rocket"), true).unwrap();
    feeds.worktrees.finish_or(a, Ok(list), true);
    let b = feeds.worktrees.begin(Some("/src/comet"), true).unwrap();
    assert_eq!(feeds.worktrees.value.as_ref().map(Vec::len), Some(2));
    feeds.worktrees.finish_or(b, parse_worktrees(answer(500, json!({ "error": "git failed" }))), true);
    assert_eq!((feeds.worktrees.value.as_ref().map(Vec::len), feeds.worktrees.error.as_deref()), (Some(0), Some("git failed")));
    assert_eq!(parse_worktrees(answer(200, json!({}))), Err("No data returned".to_string()));
}

// ─── folder-access ──────────────────────────────────────────────────────────

#[test]
fn list_workspace_roots_lists_the_roots_that_exist_in_the_servers_shape() {
    let body =
        json!({ "roots": [{ "path": "/work", "exists": true }, { "path": "/gone", "exists": false }, { "path": "/src", "exists": true }] });
    assert_eq!(parse_roots(answer(200, body)), Ok(vec!["/work".to_string(), "/src".to_string()]));
}

#[test]
fn list_workspace_roots_says_couldn_t_load_your_locations_for_any_failure() {
    let words = Err("Couldn't load your locations.".to_string());
    assert_eq!(parse_roots(answer(500, json!({ "error": "x" }))), words);
    assert_eq!(parse_roots(Err("down".into())), words);
    assert_eq!(parse_roots(answer(200, json!({ "nope": 1 }))), words);
}

#[test]
fn new_folder_defaults_fall_back_to_main_and_an_unknown_home() {
    let d = parse_defaults(answer(200, json!({ "firstBranch": "trunk", "home": "/home/ada" })));
    assert_eq!((d.first_branch.as_str(), d.home.as_deref()), ("trunk", Some("/home/ada")));
    assert_eq!(parse_defaults(answer(500, json!({ "error": "x" }))), DirectoryDefaults::default());
    assert_eq!(parse_defaults(Err("down".into())).first_branch, "main");
    assert_eq!(parse_defaults(answer(200, json!({}))), DirectoryDefaults::default());
}

#[test]
fn inspect_folder_reads_the_flags_and_says_couldn_t_check_that_folder_without_a_reason() {
    let i =
        parse_inspection(answer(200, json!({ "path": "/src/rocket", "exists": true, "isGitRepo": true, "isWithinRoots": false }))).unwrap();
    assert_eq!((i.exists, i.is_git_repo, i.is_within_roots), (true, true, false));
    assert_eq!(parse_inspection(answer(400, json!({ "error": "Path must be absolute." }))), Err("Path must be absolute.".to_string()));
    assert_eq!(parse_inspection(answer(500, json!({}))), Err("Couldn't check that folder.".to_string()));
    assert_eq!(parse_inspection(Ok((200, String::new()))), Err("Couldn't check that folder.".to_string()));
}

#[test]
fn add_folder_to_fleet_says_the_servers_reason_or_couldn_t_add_that_folder() {
    assert_eq!(parse_added_root(Ok((200, "ok".into()))), Ok(()));
    assert_eq!(
        parse_added_root(answer(400, json!({ "error": "That folder is inside another location." }))),
        Err("That folder is inside another location.".to_string())
    );
    assert_eq!(parse_added_root(answer(500, json!({}))), Err("Couldn't add that folder.".to_string()));
}

#[test]
fn list_folder_reads_the_folder_as_the_machine_writes_it_and_says_couldn_t_list_that_folder() {
    let body = json!({ "currentPath": "/home/ada/src", "exists": false, "nearestExisting": "/home/ada",
        "entries": [{ "name": "rocket", "path": "/home/ada/src/rocket", "isGitRepo": true }, { "name": "docs", "path": "/home/ada/src/docs" }] });
    let l = parse_listing("~/src", answer(200, body)).unwrap();
    assert_eq!((l.path.as_str(), l.exists, l.nearest_existing.as_deref()), ("/home/ada/src", false, Some("/home/ada")));
    assert_eq!(l.entries.iter().map(|e| e.is_git_repo).collect::<Vec<_>>(), [true, false]);
    assert_eq!(parse_listing("~/src", answer(200, json!({ "exists": true, "entries": [] }))).unwrap().path, "~/src");
    for bad in [answer(500, json!({ "error": "denied" })), Err("down".to_string()), Ok((200, String::new()))] {
        assert_eq!(parse_listing("/x", bad), Err("Couldn't list that folder.".to_string()));
    }
}

#[test]
fn list_folder_drops_a_listing_a_newer_one_has_overtaken() {
    let mut feeds = Feeds::default();
    let old = feeds.listing.begin(Some("/a"), false).unwrap();
    let new = feeds.listing.begin(Some("/a/b"), false).unwrap();
    assert!(!feeds.listing.finish(old, Ok(Listing::default())));
    assert!(feeds.listing.finish(new, Err("Couldn't list that folder.".into())));
    assert_eq!(feeds.listing.error.as_deref(), Some("Couldn't list that folder."));
}

// ─── useHarnesses ───────────────────────────────────────────────────────────

fn harness_body(kinds: &[&str]) -> Answer {
    answer(
        200,
        json!(kinds.iter().map(|k| json!({ "type": k, "displayName": k, "available": true, "userEnabled": true })).collect::<Vec<_>>()),
    )
}

#[test]
fn use_harnesses_asks_once_for_every_list_mounted_together_as_a_page_with_a_row_per_session_does() {
    let mut feed = HarnessFeed::default();
    let first = feed.begin(false);
    assert!(first.is_some());
    assert_eq!(feed.begin(false), None);
    assert_eq!(feed.begin(false), None);
    assert!(feed.finish(first.unwrap(), false, 100, parse_harnesses(harness_body(&["opencode2"]))));
    assert_eq!(feed.harnesses.all.len(), 1);
    assert!(feed.begin(false).is_some(), "the next page asks again");
}

#[test]
fn use_harnesses_keeps_the_newest_answer_when_an_older_slower_one_arrives_after_it() {
    let mut feed = HarnessFeed::default();
    let old = feed.begin(true).unwrap();
    let new = feed.begin(true).unwrap();
    assert!(feed.finish(new, true, 10, parse_harnesses(harness_body(&["new"]))));
    assert!(!feed.finish(old, true, 20, parse_harnesses(harness_body(&["old"]))));
    assert_eq!(feed.harnesses.all[0].kind, "new");
}

#[test]
fn use_harnesses_uses_a_moment_old_answer_without_asking_again_as_rows_mounted_one_after_another_do() {
    let mut feed = HarnessFeed::default();
    assert!(feed.wants_read(0));
    let s = feed.begin(false).unwrap();
    feed.finish(s, false, 1_000, parse_harnesses(harness_body(&["a"])));
    assert!(!feed.wants_read(1_000 + REUSE_FOR_MS - 1));
    assert!(feed.wants_read(1_000 + REUSE_FOR_MS));
}

#[test]
fn use_harnesses_asks_the_machine_to_check_every_harness_again_for_check_again_and_after_a_change() {
    let mut feed = HarnessFeed::default();
    assert!(feed.begin(false).is_some());
    // A fresh read goes ahead even while a plain one is running.
    assert!(feed.begin(true).is_some());
    // The call is `GET /api/harnesses?fresh=true`: the live test below sees it on the wire.
}

#[test]
fn use_harnesses_keeps_each_machine_s_list_apart_and_says_why_a_read_failed() {
    let mut feeds = Feeds::default();
    for (key, kind) in [(HOME_MACHINE_KEY, "opencode2"), ("falcon", "claude-code")] {
        let f = feeds.harnesses.entry(key.to_string()).or_default();
        let s = f.begin(false).unwrap();
        f.finish(s, false, 1, parse_harnesses(harness_body(&[kind])));
    }
    assert_eq!(feeds.harnesses[HOME_MACHINE_KEY].harnesses.all[0].kind, "opencode2");
    assert_eq!(feeds.harnesses["falcon"].harnesses.all[0].kind, "claude-code");

    let f = feeds.harnesses.get_mut("falcon").unwrap();
    let s = f.begin(true).unwrap();
    f.finish(s, true, 2, parse_harnesses(answer(500, json!({ "error": "down for repairs" }))));
    assert_eq!((f.error.as_deref(), f.harnesses.load), (Some("down for repairs"), Load::Failed));
    assert_eq!(f.harnesses.all.len(), 1, "the list it had stays");
    assert_eq!(parse_harnesses(answer(503, json!({}))), Err("HTTP 503".to_string()));
    assert_eq!(parse_harnesses(Err("connection refused".into())), Err("Failed to fetch harnesses".to_string()));
}

#[test]
fn use_harnesses_reads_what_the_choice_logic_needs() {
    let body = json!([{ "type": "opencode2", "displayName": "OpenCode 2", "available": false, "userEnabled": true, "reason": "OpenCode isn't installed.", "isDefault": true, "capabilities": { "supportsProfiles": true } }]);
    let list: Vec<HarnessEntry> = parse_harnesses(answer(200, body)).unwrap();
    assert!(list[0].supports_profiles && list[0].is_default && !list[0].available);
}

// ─── profiles ───────────────────────────────────────────────────────────────

#[test]
fn harness_profiles_store_reads_the_list_and_leaves_it_as_it_was_on_a_failure() {
    let body = json!([{ "id": "work", "name": "Work", "content": "{}", "isDefault": true }, { "id": "play", "name": "Play" }]);
    let list = parse_profiles(answer(200, body)).unwrap();
    assert_eq!((list.len(), list[0].is_default, list[1].is_default), (2, true, false));
    assert_eq!(parse_profiles(answer(500, json!({ "error": "x" }))), None);
    assert_eq!(parse_profiles(answer(200, json!({ "not": "a list" }))), None);
    let mut feeds = Feeds::default();
    feeds.profiles.insert((HOME_MACHINE_KEY.into(), "opencode2".into()), list);
    assert_eq!(feeds.default_profile(HOME_MACHINE_KEY, "opencode2").map(|p| p.id.as_str()), Some("work"));
    assert!(feeds.profiles_of("falcon", "opencode2").is_empty());
}

// ─── preferences ────────────────────────────────────────────────────────────

#[test]
fn starters_stay_away_when_turned_off_and_until_preferences_are_known() {
    let mut prefs = crate::fleet::prefs::Prefs::default();
    assert!(!starters_shown(&prefs));
    prefs.adopt(&json!({}));
    assert!(starters_shown(&prefs));
    prefs.adopt(&json!({ "NewSessionStarters": "false" }));
    assert!(!starters_shown(&prefs));
    assert!(!workflows_enabled(&prefs));
    prefs.adopt(&json!({ "Workflows": "true" }));
    assert!(workflows_enabled(&prefs));
}

// ─── worktree naming ────────────────────────────────────────────────────────

#[test]
fn worktree_naming_fills_what_the_server_leaves_out_with_fleet_s_defaults() {
    let body = json!({ "effective": { "branch": "{prefix}/{slug}", "capture": { "issue": "#(\\d+)" }, "prefix": "ada" },
        "user": { "prefix": "ada" }, "defaults": { "root": "{repoParent}/wt" }, "layers": { "prefix": "user", "branch": "project" } });
    let n = parse_naming(answer(200, body)).unwrap();
    assert_eq!((n.effective.prefix.as_str(), n.effective.root.as_str()), ("ada", "{repoParent}/{repo}-worktrees"));
    assert_eq!(n.effective.capture, [("issue".to_string(), "#(\\d+)".to_string())]);
    assert_eq!((n.defaults.root.as_str(), n.defaults.branch.as_str()), ("{repoParent}/wt", "{prefix}/{slug}"));
    assert_eq!(n.user.prefix.as_deref(), Some("ada"));
    assert!(n.has_project_convention());
}

#[test]
fn worktree_naming_says_the_servers_words_or_couldn_t_load_worktree_naming() {
    assert_eq!(parse_naming(answer(400, json!({ "detail": "Bad directory." }))), Err("Bad directory.".to_string()));
    assert_eq!(parse_naming(answer(400, json!({ "title": "Bad Request" }))), Err("Bad Request".to_string()));
    assert_eq!(parse_naming(answer(500, json!({}))), Err("Couldn't load worktree naming.".to_string()));
    assert_eq!(parse_naming(Err("down".into())), Err("Couldn't load worktree naming.".to_string()));
}

#[test]
fn worktree_naming_keeps_the_last_answer_when_a_read_fails_and_says_where_it_was_read() {
    let mut feed = NamingFeed::default();
    let s = feed.begin();
    feed.finish(
        s,
        Some("/src/rocket"),
        HOME_MACHINE_KEY,
        parse_naming(answer(200, json!({ "effective": { "prefix": "ada" }, "user": {}, "defaults": {}, "layers": {} }))),
    );
    assert_eq!((feed.loaded_for.as_deref(), feed.loaded_on.as_deref()), (Some("/src/rocket"), Some(HOME_MACHINE_KEY)));
    let s = feed.begin();
    feed.finish(s, Some("/src/comet"), HOME_MACHINE_KEY, Err("down".into()));
    assert_eq!(
        (feed.answer.effective.prefix.as_str(), feed.loaded_for.as_deref(), feed.error.as_deref()),
        ("ada", Some("/src/rocket"), Some("down"))
    );
}

// ─── useHarnessCatalog ──────────────────────────────────────────────────────

fn catalog_with(agent: &str) -> Catalog {
    parse_catalog(answer(200, json!({ "supported": true, "agents": [{ "name": agent, "mode": "primary", "hidden": false }], "providers": [], "defaultAgent": agent }))).unwrap()
}

fn key(harness: &str, dir: Option<&str>, profile: Option<&str>) -> CatalogKey {
    CatalogKey { harness: harness.into(), directory: dir.map(Into::into), profile: profile.map(Into::into) }
}

fn change(dir: &str) -> CatalogChange {
    CatalogChange::parse(
        &json!({ "harnessType": "opencode2", "directory": dir, "quickChat": false, "profileIds": ["none"], "sessionIds": [] }),
    )
    .unwrap()
}

#[test]
fn use_harness_catalog_pushed_changes_asks_again_when_the_harness_says_the_folder_s_catalog_changed() {
    let mut feed = CatalogFeed::default();
    let k = key("opencode2", Some("/work/rocket"), None);
    let Ask::Fetch(s) = feed.select(Some(k.clone()), 0) else { panic!("asks") };
    feed.finish(s, 0, Ok(catalog_with("first")));
    assert_eq!(feed.shown.as_ref().unwrap().default_agent.as_deref(), Some("first"));
    assert!(feed.on_change(&change("/work/rocket/")), "the trailing slash is the same folder");
    let s = feed.refetch().unwrap();
    assert!(feed.shown.is_some() && feed.loading, "the old catalog stays up meanwhile");
    feed.finish(s, 1, Ok(catalog_with("second")));
    assert_eq!(feed.shown.as_ref().unwrap().default_agent.as_deref(), Some("second"));
}

#[test]
fn use_harness_catalog_pushed_changes_ignores_a_change_to_another_folder_harness_or_profile() {
    let mut feed = CatalogFeed::default();
    let Ask::Fetch(s) = feed.select(Some(key("opencode2", Some("/work/rocket"), Some("work"))), 0) else { panic!() };
    feed.finish(s, 0, Ok(catalog_with("a")));
    assert!(!feed.on_change(&change("/work/other")));
    let mut other_harness = change("/work/rocket");
    other_harness.harness_type = "claude-code".into();
    assert!(!feed.on_change(&other_harness));
    // The change names profile "none" only: the "work" catalog is not it.
    assert!(!feed.on_change(&change("/work/rocket")));
    assert!(feed.is_cached(&key("opencode2", Some("/work/rocket"), Some("work"))));
    // Without a profile asked for, any profile of the folder counts; a quick chat is told by `quickChat`.
    assert!(change("/work/rocket").is_for("opencode2", Some("/work/rocket"), None));
    let quick = CatalogChange { quick_chat: true, ..change("/tmp/chats") };
    assert!(quick.is_for("opencode2", None, Some("none")) && !quick.is_for("opencode2", Some("/tmp/chats-x"), None));
}

#[test]
fn use_harness_catalog_pushed_changes_keeps_the_catalog_it_has_when_asking_again_fails() {
    let mut feed = CatalogFeed::default();
    let Ask::Fetch(s) = feed.select(Some(key("opencode2", Some("/work/rocket"), None)), 0) else { panic!() };
    feed.finish(s, 0, Ok(catalog_with("kept")));
    let s = feed.refetch().unwrap();
    assert!(feed.finish(s, 1, parse_catalog(answer(502, json!({ "error": "down" })))));
    assert_eq!((feed.error.as_deref(), feed.loading), (Some("down"), false));
    assert_eq!(feed.shown.as_ref().unwrap().default_agent.as_deref(), Some("kept"));
    assert_eq!(parse_catalog(answer(500, json!({}))), Err("HTTP 500".to_string()));
}

#[test]
fn use_harness_catalog_pushed_changes_asks_again_next_time_for_a_cached_catalog_that_changed_while_it_was_not_shown() {
    let mut feed = CatalogFeed::default();
    let rocket = key("opencode2", Some("/work/rocket"), None);
    let Ask::Fetch(s) = feed.select(Some(rocket.clone()), 0) else { panic!() };
    feed.finish(s, 0, Ok(catalog_with("a")));
    assert_eq!(feed.select(None, 1), Ask::Nothing);
    assert!(feed.shown.is_none());
    assert!(!feed.on_change(&change("/work/rocket")), "nothing on screen to ask for now");
    assert!(!feed.is_cached(&rocket));
    assert!(matches!(feed.select(Some(rocket), 2), Ask::Fetch(_)));
}

#[test]
fn use_harness_catalog_draws_the_folder_s_cached_catalog_at_once_and_asks_again_when_it_is_old() {
    let mut feed = CatalogFeed::default();
    let rocket = key("opencode2", Some("/work/rocket"), None);
    let Ask::Fetch(s) = feed.select(Some(rocket.clone()), 1_000) else { panic!() };
    feed.finish(s, 1_000, Ok(catalog_with("a")));
    // Fresh: shown, no call.
    assert_eq!(feed.select(Some(rocket.clone()), 1_000 + CATALOG_FRESH_MS - 1), Ask::Cached);
    assert!(feed.is_current() && !feed.loading);
    // Old: shown at once, and asked behind it.
    assert!(matches!(feed.select(Some(rocket), 1_000 + CATALOG_FRESH_MS), Ask::Fetch(_)));
    assert!(feed.shown.is_some() && feed.loading && feed.is_current());
}

#[test]
fn use_harness_catalog_keeps_the_previous_folder_up_while_another_loads_and_drops_a_late_answer() {
    let mut feed = CatalogFeed::default();
    let Ask::Fetch(a) = feed.select(Some(key("opencode2", Some("/work/a"), None)), 0) else { panic!() };
    feed.finish(a, 0, Ok(catalog_with("a")));
    let Ask::Fetch(b) = feed.select(Some(key("opencode2", Some("/work/b"), None)), 1) else { panic!() };
    assert!(!feed.is_current());
    assert_eq!(feed.shown.as_ref().unwrap().default_agent.as_deref(), Some("a"));
    let Ask::Fetch(c) = feed.select(Some(key("opencode2", Some("/work/c"), None)), 2) else { panic!() };
    assert!(!feed.finish(b, 3, Ok(catalog_with("b"))));
    assert!(feed.finish(c, 3, Ok(catalog_with("c"))) && feed.is_current());
    // A failure for a folder with nothing cached and nothing shown leaves the pickers empty, marked as that folder's.
    let mut empty = CatalogFeed::default();
    let Ask::Fetch(s) = empty.select(Some(key("opencode2", Some("/work/x"), None)), 0) else { panic!() };
    empty.finish(s, 0, Err("down".into()));
    assert!(empty.shown.is_none() && empty.is_current() && !empty.supported());
}

#[test]
fn use_harness_catalog_on_another_machine_keeps_its_catalog_apart() {
    let mut feeds = Feeds::default();
    for (machine, agent) in [(HOME_MACHINE_KEY, "home-agent"), ("falcon", "falcon-agent")] {
        let f = feeds.catalogs.entry(machine.to_string()).or_default();
        let Ask::Fetch(s) = f.select(Some(key("opencode2", Some("/work/rocket"), None)), 0) else { panic!() };
        f.finish(s, 0, Ok(catalog_with(agent)));
    }
    // The change is pushed by the Fleet the app is connected to: only home's catalogs hear it (`on_feeds_event`).
    assert!(feeds.catalogs.get_mut(HOME_MACHINE_KEY).unwrap().on_change(&change("/work/rocket")));
    assert!(feeds.catalogs["falcon"].is_cached(&key("opencode2", Some("/work/rocket"), None)));
    assert_eq!(feeds.catalogs["falcon"].shown.as_ref().unwrap().default_agent.as_deref(), Some("falcon-agent"));
}

#[test]
fn agent_and_model_asks_the_harness_about_the_chosen_folder_and_says_what_default_is() {
    let body = json!({ "supported": true, "defaultAgent": "build", "defaultModel": { "providerID": "anthropic", "modelID": "sonnet" },
        "agents": [{ "name": "build", "description": "Writes code" }, { "name": "hidden-one", "hidden": true }],
        "providers": [{ "id": "anthropic", "name": "Anthropic", "models": [{ "id": "sonnet", "name": "Sonnet" }] }] });
    let c = parse_catalog(answer(200, body)).unwrap();
    assert!(c.supported);
    assert_eq!(c.agents.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["build"]);
    assert_eq!((c.models.len(), c.default_agent.as_deref()), (1, Some("build")));
    assert_eq!(c.default_model.map(|m| m.model_id), Some("sonnet".to_string()));
}

#[test]
fn agent_and_model_offers_no_choice_when_the_harness_can_t_list_its_agents() {
    let c = parse_catalog(answer(200, json!({ "supported": false, "agents": [], "providers": [], "defaultAgent": null }))).unwrap();
    let mut feed = CatalogFeed::default();
    let Ask::Fetch(s) = feed.select(Some(key("codex", Some("/work/rocket"), None)), 0) else { panic!() };
    feed.finish(s, 0, Ok(c));
    assert!(!feed.supported());
}

#[test]
fn agent_and_model_asks_about_an_existing_worktree_as_it_is() {
    let mut feed = CatalogFeed::default();
    let tree = "/work/rocket-worktrees/fix-login";
    assert!(matches!(feed.select(Some(key("opencode2", Some(tree), Some("work"))), 0), Ask::Fetch(_)));
    assert_eq!(feed.wanted, Some(key("opencode2", Some(tree), Some("work"))));
    assert!(
        matches!(feed.select(Some(key("opencode2", Some("/work/rocket"), Some("work"))), 0), Ask::Fetch(_)),
        "another folder is another catalog"
    );
}

#[test]
fn catalog_change_needs_a_harness_and_a_folder() {
    assert!(CatalogChange::parse(&json!({ "harnessType": "opencode2" })).is_none());
    assert!(CatalogChange::parse(&json!({ "directory": "/x" })).is_none());
    assert!(CatalogChange::parse(&serde_json::Value::Null).is_none());
    let mut feed = CatalogFeed::default();
    let Ask::Fetch(s) = feed.select(Some(key("opencode2", Some("/work/rocket"), None)), 0) else { panic!() };
    feed.finish(s, 0, Ok(catalog_with("a")));
    feed.forget_harness("opencode2");
    assert!(!feed.is_cached(&key("opencode2", Some("/work/rocket"), None)));
}

// ─── the threads, against a fake server ─────────────────────────────────────

mod live {
    use super::*;
    use crate::State;
    use crate::fleet::link::{Job, Link, Post};
    use crate::tests::app;
    use std::io::Write;
    use std::sync::mpsc::channel;
    use std::sync::{Arc, Mutex};

    fn serve(route: impl Fn(&str, &str) -> (u16, String) + Send + Sync + 'static) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = crate::test_http::bind().unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen: Arc<Mutex<Vec<String>>> = Arc::default();
        let (log, route) = (seen.clone(), Arc::new(route));
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let (log, route) = (log.clone(), route.clone());
                std::thread::spawn(move || {
                    let text = crate::test_http::read(&mut stream).raw();
                    let mut first = text.lines().next().unwrap_or("").split(' ');
                    let (method, target) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
                    log.lock().unwrap().push(format!("{method} {target}"));
                    let (status, body) = route(&method, &target);
                    let reply = format!(
                        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(reply.as_bytes());
                });
            }
        });
        (base, seen)
    }

    fn connected(base: &str) -> (rune::Runtime<State>, std::sync::mpsc::Receiver<Job>) {
        let (jobs, queue) = channel::<Job>();
        let jobs = Mutex::new(jobs);
        let post = Post::new(move |job| {
            let _ = jobs.lock().unwrap().send(job);
        });
        let mut rt = app();
        let link = Link::start(base.to_string(), post);
        rt.update(move |s, _| {
            s.link = Some(link);
            s.feeds = Feeds::default();
        });
        (rt, queue)
    }

    fn wait(rt: &mut rune::Runtime<State>, queue: &std::sync::mpsc::Receiver<Job>, until: impl Fn(&State) -> bool, what: &str) {
        crate::test_wait::drive(rt, queue, what, until, crate::test_wait::app_state);
    }

    #[test]
    fn each_read_runs_on_a_thread_fills_its_feed_and_asks_the_machine_by_its_own_path() {
        let (base, seen) = serve(|method, target| {
            let path = target.split('?').next().unwrap();
            match (method, path) {
                ("GET", "/api/repositories") => (
                    200,
                    json!({ "repositories": [{ "name": "rocket", "path": "/src/rocket", "parentRoot": "/src" }], "scannedAt": 5 })
                        .to_string(),
                ),
                ("POST", "/api/repositories/refresh") => (200, json!({ "repositories": [], "scannedAt": 6 }).to_string()),
                ("GET", "/api/repositories/detail") => (200, detail_body().to_string()),
                ("GET", "/api/repositories/worktrees") => (500, json!({ "error": "git failed" }).to_string()),
                ("GET", "/api/workspace-roots") => (200, json!({ "roots": [{ "path": "/src", "exists": true }] }).to_string()),
                ("GET", "/api/directories/defaults") => (200, json!({ "firstBranch": "trunk", "home": "/home/ada" }).to_string()),
                ("GET", "/api/directories") => (200, json!({ "currentPath": "/src", "exists": true, "entries": [] }).to_string()),
                ("GET", "/api/harnesses") => {
                    (200, json!([{ "type": "opencode2", "available": true, "userEnabled": true, "isDefault": true }]).to_string())
                }
                ("GET", "/api/harnesses/opencode2/profiles") => {
                    (200, json!([{ "id": "work", "name": "Work", "isDefault": true }]).to_string())
                }
                ("GET", "/api/harnesses/opencode2/catalog") => (
                    200,
                    json!({ "supported": true, "agents": [{ "name": "build" }], "providers": [], "defaultAgent": "build" }).to_string(),
                ),
                ("GET", "/api/worktrees/naming") => {
                    (200, json!({ "effective": { "prefix": "ada" }, "user": {}, "defaults": {}, "layers": {} }).to_string())
                }
                ("GET", "/api/directories/inspect") => {
                    (200, json!({ "path": "/src/rocket", "exists": true, "isGitRepo": true, "isWithinRoots": true }).to_string())
                }
                _ => (404, "{}".into()),
            }
        });
        let (mut rt, queue) = connected(&base);
        let home = HOME_MACHINE_KEY;
        rt.update(move |s, _| {
            s.read_repositories(home, false);
            s.read_detail(home, Some("/src/rocket"));
            s.read_worktrees(home, Some("/src/rocket"));
            s.read_roots(home);
            s.read_directory_defaults(home);
            s.list_folder(home, Some("~/src"));
            s.read_harnesses(home, false);
            s.read_harnesses(home, true);
            s.read_profiles(home, "opencode2");
            s.read_naming(home, Some("/src/rocket"));
            s.select_catalog(home, Some(key("opencode2", Some("/src/rocket"), Some("work"))));
        });
        wait(
            &mut rt,
            &queue,
            |s| {
                let f = &s.feeds;
                !f.repos(HOME_MACHINE_KEY).list.is_empty()
                    && f.detail.value.is_some()
                    && f.worktrees.error.is_some()
                    && !f.roots.is_empty()
                    && f.defaults.is_some()
                    && f.listing.value.is_some()
                    && f.harnesses.get(HOME_MACHINE_KEY).is_some_and(|h| h.harnesses.load == Load::Done)
                    && !f.profiles_of(HOME_MACHINE_KEY, "opencode2").is_empty()
                    && f.naming.loaded_for.is_some()
                    && f.catalogs.get(HOME_MACHINE_KEY).is_some_and(CatalogFeed::is_current)
            },
            "every read lands",
        );
        let s = &rt.state;
        assert_eq!(s.feeds.first_branch(), "trunk");
        assert_eq!(s.feeds.home_folder(), Some("/home/ada"));
        assert_eq!(s.feeds.worktrees.value.as_ref().map(Vec::len), Some(0));
        assert_eq!(s.feeds.listing.value.as_ref().unwrap().path, "/src");
        assert_eq!(s.harness_choice(HOME_MACHINE_KEY).default_type(), "opencode2");
        assert_eq!(s.feeds.catalogs[HOME_MACHINE_KEY].shown.as_ref().unwrap().default_agent.as_deref(), Some("build"));
        // The scan was kept for the next visit.
        assert_eq!(s.sidebar.saved_repositories().count(), 1);
        let asked = seen.lock().unwrap().clone();
        assert!(asked.contains(&"GET /api/repositories/detail?path=%2Fsrc%2Frocket".to_string()), "{asked:?}");
        assert!(asked.contains(&"GET /api/directories?path=~%2Fsrc&unconstrained=true".to_string()), "{asked:?}");
        assert!(asked.contains(&"GET /api/harnesses?fresh=true".to_string()), "{asked:?}");
        assert!(asked.contains(&"GET /api/harnesses/opencode2/catalog?directory=%2Fsrc%2Frocket&profile=work".to_string()), "{asked:?}");

        // The scan again after a folder was added: the refresh call.
        rt.update(move |s, _| s.read_repositories(home, true));
        wait(&mut rt, &queue, |s| s.feeds.repos(HOME_MACHINE_KEY).scanned_at == Some(6), "the refreshed scan");
        assert!(seen.lock().unwrap().iter().any(|a| a == "POST /api/repositories/refresh"));

        // One-shot calls hand their answer to whoever asked.
        let (tx, rx) = channel();
        rt.update(move |s, _| {
            s.inspect_folder(home, "/src/rocket", Box::new(move |_, _, r| tx.send(r).unwrap()));
        });
        let mut inspected = None;
        crate::test_wait::drive(
            &mut rt,
            &queue,
            "the inspection",
            |_| {
                inspected = inspected.take().or_else(|| rx.try_recv().ok());
                inspected.is_some()
            },
            crate::test_wait::app_state,
        );
        let inspected = inspected.unwrap();
        assert!(inspected.unwrap().is_git_repo);
    }

    #[test]
    fn a_catalog_event_forgets_the_folder_s_catalog_and_asks_again_with_the_old_one_up() {
        let calls = Arc::new(Mutex::new(0));
        let counted = calls.clone();
        let (base, _) = serve(move |_, target| {
            if !target.starts_with("/api/harnesses/opencode2/catalog") {
                return (404, "{}".into());
            }
            let mut n = counted.lock().unwrap();
            *n += 1;
            let agent = if *n == 1 { "first" } else { "second" };
            (200, json!({ "supported": true, "agents": [{ "name": agent }], "providers": [], "defaultAgent": agent }).to_string())
        });
        let (mut rt, queue) = connected(&base);
        let k = key("opencode2", Some("/work/rocket"), None);
        let ask = k.clone();
        rt.update(move |s, _| s.select_catalog(HOME_MACHINE_KEY, Some(ask)));
        let agent = |s: &State| s.feeds.catalogs.get(HOME_MACHINE_KEY).and_then(|c| c.shown.as_ref()).and_then(|c| c.default_agent.clone());
        wait(&mut rt, &queue, |s| agent(s).as_deref() == Some("first"), "the first catalog");
        let event = Event::Other(
            Kind::HarnessCatalogChanged,
            json!({ "harnessType": "opencode2", "directory": "/work/rocket", "profileIds": ["none"], "sessionIds": [] }),
        );
        rt.update(move |s, _| s.on_feeds_event(&event));
        assert_eq!(rt.state.feeds.catalogs[HOME_MACHINE_KEY].shown.as_ref().unwrap().default_agent.as_deref(), Some("first"));
        wait(&mut rt, &queue, |s| agent(s).as_deref() == Some("second"), "the second catalog");
        // A change to another folder asks nothing.
        let other = Event::Other(
            Kind::HarnessCatalogChanged,
            json!({ "harnessType": "opencode2", "directory": "/work/other", "profileIds": ["none"] }),
        );
        rt.update(move |s, _| s.on_feeds_event(&other));
        assert_eq!(*calls.lock().unwrap(), 2);
    }

    #[test]
    fn with_no_server_nothing_is_asked_and_the_invented_data_stays() {
        let mut rt = app();
        let before = rt.state.feeds.repos(HOME_MACHINE_KEY).list.len();
        assert!(before > 0);
        rt.update(|s, _| {
            s.read_repositories(HOME_MACHINE_KEY, true);
            s.read_harnesses(HOME_MACHINE_KEY, true);
            s.read_detail(HOME_MACHINE_KEY, Some("/x"));
        });
        assert_eq!(rt.state.feeds.repos(HOME_MACHINE_KEY).list.len(), before);
        assert!(!rt.state.feeds.repos(HOME_MACHINE_KEY).loading);
        assert_eq!(rt.state.harness_choice(HOME_MACHINE_KEY).load, Load::Done);
    }

    /// Reads every call of the piece from a scratch Fleet at `FLEET_URL` (a repository at `FLEET_REPO`) and prints what filled
    /// the feeds; `FLEET_HIDE` is a prefix to print as `<scratch>`.
    #[test]
    #[ignore = "needs a scratch Fleet at FLEET_URL"]
    fn live_feeds_from_a_scratch_fleet() {
        let (url, repo) = (std::env::var("FLEET_URL").expect("FLEET_URL"), std::env::var("FLEET_REPO").expect("FLEET_REPO"));
        let hide = std::env::var("FLEET_HIDE").unwrap_or_default();
        let (mut rt, queue) = connected(&url);
        let home = HOME_MACHINE_KEY;
        let asked = repo.clone();
        rt.update(move |s, _| {
            s.read_repositories(home, false);
            s.read_detail(home, Some(&asked));
            s.read_worktrees(home, Some(&asked));
            s.read_roots(home);
            s.read_directory_defaults(home);
            s.list_folder(home, Some("~"));
            s.read_harnesses(home, false);
            s.read_profiles(home, "opencode");
            s.read_naming(home, Some(&asked));
            s.select_catalog(home, Some(key("opencode", Some(&asked), None)));
        });
        wait(
            &mut rt,
            &queue,
            |s| {
                let f = &s.feeds;
                !f.repos(HOME_MACHINE_KEY).loading
                    && !f.detail.loading
                    && !f.worktrees.loading
                    && !f.roots.is_empty()
                    && f.defaults.is_some()
                    && !f.listing.loading
                    && f.harnesses.get(HOME_MACHINE_KEY).is_some_and(|h| h.harnesses.load != Load::Loading)
                    && !f.naming.loading
                    && f.catalogs.get(HOME_MACHINE_KEY).is_some_and(|c| !c.loading)
            },
            "every read answers",
        );
        let (f, show) = (&rt.state.feeds, |t: &str| t.replace(&hide, "<scratch>"));
        let repos = f.repos(HOME_MACHINE_KEY);
        println!(
            "repositories ({}): {}",
            repos.list.len(),
            repos.list.iter().map(|r| format!("{} under {}", r.name, show(&r.parent_root))).collect::<Vec<_>>().join("; ")
        );
        println!("grouped by root: {:?}", group_by_root(&repos.list).iter().map(|(r, g)| (show(r), g.len())).collect::<Vec<_>>());
        let d = f.detail.value.clone().unwrap_or_default();
        println!(
            "detail: branch={} default_base={:?} branches={:?} remotes={}",
            d.branch,
            d.default_base,
            d.branches.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(),
            d.remotes.len()
        );
        println!("worktrees: {:?}", f.worktrees.value.iter().flatten().map(|w| (show(&w.path), w.branch.clone())).collect::<Vec<_>>());
        println!(
            "roots: {:?}  first_branch={} home={:?}",
            f.roots.iter().map(|r| show(r)).collect::<Vec<_>>(),
            f.first_branch(),
            f.home_folder().map(show)
        );
        println!(
            "listing of ~: {:?} entries={}",
            f.listing.value.as_ref().map(|l| show(&l.path)),
            f.listing.value.as_ref().map_or(0, |l| l.entries.len())
        );
        let h = rt.state.harness_choice(home);
        println!(
            "harnesses: {:?} default={:?} picker={} reason={:?}",
            h.all.iter().map(|e| (e.kind.as_str(), e.available)).collect::<Vec<_>>(),
            h.default_type(),
            h.shows_picker(),
            h.no_harness_reason()
        );
        println!("profiles(opencode): {}", f.profiles_of(home, "opencode").len());
        println!(
            "naming: prefix={} branch={} project_convention={}",
            f.naming.answer.effective.prefix,
            f.naming.answer.effective.branch,
            f.naming.answer.has_project_convention()
        );
        let c = f.catalogs.get(home).and_then(|c| c.shown.as_ref());
        println!(
            "catalog: supported={:?} agents={:?} default_agent={:?} models={:?}",
            c.map(|c| c.supported),
            c.map(|c| c.agents.iter().map(|a| a.name.as_str()).collect::<Vec<_>>()),
            c.and_then(|c| c.default_agent.clone()),
            c.map(|c| c.models.len())
        );
        println!(
            "errors: repos={:?} detail={:?} worktrees={:?} naming={:?} catalog={:?}",
            repos.error,
            f.detail.error,
            f.worktrees.error,
            f.naming.error,
            f.catalogs.get(home).and_then(|c| c.error.clone())
        );
    }

    #[test]
    fn the_projects_for_the_field_are_the_session_list_s_without_scratch() {
        let mut rt = app();
        rt.update(|s, _| {
            s.project_load = crate::fleet::groups::ProjectsLoad::new(true);
        });
        let names: Vec<String> = rt.state.new_session_projects(HOME_MACHINE_KEY).into_iter().map(|p| p.name).collect();
        assert!(names.iter().all(|n| n != "Ungrouped"), "{names:?}");
    }
}
