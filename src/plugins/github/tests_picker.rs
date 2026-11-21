//! Piece M of component 21: the `#` picker (`use-github-picker.test.ts`, `github-reference.test.ts`, `NewSessionComposer.test.ts`'s
//! `# picker`), over the fake Fleet of `tests_stores` that answers the GitHub routes and a repository's detail. Every name is invented.
use super::item::by_number_path;
use super::picker::{parse_github_remote, search_path};
use super::{mock, state_ref};
use crate::State;
use crate::fleet::new_session::places::find_hash_trigger;
use crate::fleet::new_session::request::Workspace;
use crate::tests::{FRAME, click};
use crate::tests_new_send::{ROCKET, Rig, Shared, rig_with};
use rune::{Key, KeyEvent, Mods};
use serde_json::{Value, json};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

const SCOPE: &str = "repo:acme/rocket is:open";

fn detail(remotes: Value) -> Value {
    json!({ "repository": { "name": "rocket", "path": ROCKET, "branch": "main", "defaultBase": "main", "defaultBranch": "main",
        "branches": [], "remotes": remotes } })
}

fn github_remote() -> Value {
    json!([{ "name": "origin", "url": "https://github.com/acme/rocket.git", "github": { "owner": "acme", "repo": "rocket" } }])
}

/// Items 218 (pull), 7 (issue), 220 (pull), 12 (issue), 221 (draft pull) of the scenes, as a search answer.
fn items(of: &[usize]) -> Value {
    let all = mock::picker_items();
    let list: Vec<Value> = of.iter().map(|i| all[*i].clone()).collect();
    json!({ "items": list, "totalCount": list.len(), "endCursor": null, "hasNextPage": false })
}

fn assigned_path() -> String {
    search_path(&format!("{SCOPE} assignee:@me sort:updated-desc"), 5)
}

fn recent_path() -> String {
    search_path(&format!("{SCOPE} sort:updated-desc"), 8)
}

fn matches_path(typed: &str, first: usize) -> String {
    search_path(&format!("{SCOPE} {typed} sort:updated-desc"), first)
}

fn picker(s: &State) -> &super::picker::Picker {
    &state_ref(&s.places).picker
}

fn numbers(s: &State) -> Vec<u64> {
    picker(s).items().iter().map(|i| i.number).collect()
}

fn labels(s: &State) -> Vec<&'static str> {
    picker(s).groups.iter().map(|g| g.label).collect()
}

impl Rig {
    /// The New session page on `rocket`, whose remotes are `remotes`, once Fleet has told the page about them.
    fn start(remotes: Value) -> Rig {
        let fake: Shared = Arc::default();
        fake.lock().unwrap().detail = Some(detail(remotes));
        let mut r = rig_with(fake);
        r.wait("the repository's detail", |s| s.repository_detail().is_some());
        r
    }

    fn answer(&self, path: &str, status: u16, body: Value) {
        self.fake.lock().unwrap().github.insert(path.to_string(), (status, body));
    }

    fn gate(&self, path: &str) -> std::sync::mpsc::Sender<()> {
        let (go, wait) = channel();
        self.fake.lock().unwrap().github_gates.insert(path.to_string(), Arc::new(Mutex::new(wait)));
        go
    }

    /// How many requests reached `path` (query included).
    fn asked(&self, path: &str) -> usize {
        self.fake.lock().unwrap().github_log.iter().filter(|p| *p == path).count()
    }

    fn requests(&self) -> Vec<String> {
        self.fake.lock().unwrap().github_log.clone()
    }

    fn type_in(&mut self, words: &str) {
        click(&mut self.rt, "new-session-message");
        for c in words.chars() {
            self.rt.key(KeyEvent { key: Key::Text(c.to_string()), mods: Mods::default() });
            self.rt.frame(FRAME);
        }
        // The page's keep looks at the text once a frame, before the key: one more frame lets it see the last character.
        self.rt.frame(FRAME);
        assert!(
            self.rt.state.new_box.edit.text.ends_with(words),
            "typing {words:?} left {:?} (route {:?}, folder {:?})",
            self.rt.state.new_box.edit.text,
            self.rt.state.route,
            self.rt.state.new_draft.draft().map(|d| d.folder.clone())
        );
    }

    fn hit(&mut self, key: Key, shift: bool) {
        self.rt.key(KeyEvent { key, mods: Mods { shift, ..Mods::default() } });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    fn listed(&mut self) {
        self.wait("the list", |s| !picker(s).loading);
    }

    /// What is in the box.
    fn typed(&self) -> String {
        self.rt.state.new_box.edit.text.clone()
    }

    fn open(&self) -> bool {
        self.rt.find("github-item-picker").is_some()
    }
}

/// A rig on `rocket` with the GitHub remote and the usual answers: items 218 and 7 assigned, 7, 220, 12 and 221 recent.
fn ready() -> Rig {
    let r = Rig::start(github_remote());
    r.answer(&assigned_path(), 200, items(&[0, 1]));
    r.answer(&recent_path(), 200, items(&[1, 2, 3, 4]));
    r
}

// ─── use-github-picker.test.ts ──────────────────────────────────────────────────

#[test]
fn offers_items_assigned_to_the_user_then_recent_ones_when_nothing_is_typed() {
    let mut r = ready();
    r.type_in("Fix #");
    r.listed();
    assert_eq!(labels(&r.rt.state), ["Assigned to you", "Recently updated"]);
    let groups = &picker(&r.rt.state).groups;
    assert_eq!(groups[0].items.iter().map(|i| i.number).collect::<Vec<_>>(), [218, 7]);
    assert_eq!(groups[1].items.iter().map(|i| i.number).collect::<Vec<_>>(), [220, 12, 221], "7 is already under Assigned to you");
    assert_eq!((r.asked(&assigned_path()), r.asked(&recent_path())), (1, 1), "opening asks at once, once each");
    r.rt.frame(FRAME);
    assert!(r.open(), "the popup is drawn");
}

#[test]
fn puts_the_item_with_a_typed_number_first_and_waits_for_typing_to_pause() {
    let mut r = ready();
    r.type_in("Fix #");
    r.listed();
    let exact = by_number_path("acme", "rocket", &[218]).expect("a path");
    r.answer(&exact, 200, json!([mock::picker_items()[0].clone()]));
    r.answer(&matches_path("218", 8), 200, items(&[1, 0, 2]));
    let early: Vec<String> = ["2", "21"].iter().map(|t| matches_path(t, 8)).collect();
    r.type_in("218");
    assert!(early.iter().all(|p| r.asked(p) == 0), "typing didn't ask for each key");
    assert!(picker(&r.rt.state).loading, "the list says it is asking again");
    r.wait("the list for 218", |s| !picker(s).loading && numbers(s).first() == Some(&218));
    assert_eq!(labels(&r.rt.state), ["Matches"]);
    assert_eq!(numbers(&r.rt.state), [218, 7, 220], "the typed number first, then the search without it");
    assert_eq!((r.asked(&exact), r.asked(&matches_path("218", 8))), (1, 1));
}

#[test]
fn searches_for_words_with_ten_results_and_a_newer_question_drops_the_older_answer() {
    let mut r = ready();
    r.type_in("Fix #");
    r.listed();
    let slow = matches_path("ret", 10);
    let go = r.gate(&slow);
    r.answer(&slow, 200, items(&[0]));
    r.answer(&matches_path("retry", 10), 200, items(&[2]));
    r.type_in("ret");
    let fake = r.fake.clone();
    let asked = slow.clone();
    r.wait("the first question to reach Fleet after the pause", move |_| fake.lock().unwrap().github_log.contains(&asked));
    r.type_in("ry");
    r.rt.frame(0.5);
    go.send(()).expect("release the old answer");
    r.wait("the newer list", |s| !picker(s).loading && numbers(s) == [220]);
    assert_eq!(labels(&r.rt.state), ["Matches"], "the older answer didn't replace it");
}

#[test]
fn asks_nothing_without_a_repository() {
    let mut r = Rig::start(json!([{ "name": "origin", "url": "git@example.com:ada/rocket.git" }]));
    r.type_in("Fix #");
    r.rt.frame(0.5);
    assert!(!picker(&r.rt.state).loading);
    assert!(r.requests().is_empty(), "nothing went to GitHub: {:?}", r.requests());
    let note = r.rt.elements().iter().filter_map(|e| e.text.clone()).any(|t| t.starts_with("Pick a folder whose repository is on GitHub"));
    assert!(note, "the popup says why it has nothing");
}

#[test]
fn says_why_it_couldnt_answer() {
    let mut r = Rig::start(github_remote());
    r.answer(&assigned_path(), 502, json!({ "error": "GitHub is down" }));
    r.answer(&recent_path(), 200, items(&[0]));
    r.type_in("Fix #");
    r.listed();
    let why = picker(&r.rt.state).error.clone().expect("an error");
    assert!(!why.is_empty());
    assert!(picker(&r.rt.state).groups.is_empty());
    r.rt.frame(FRAME);
    assert!(r.rt.elements().iter().any(|e| e.text.as_deref() == Some(why.as_str())), "the popup shows it");
}

// ─── github-reference.test.ts ───────────────────────────────────────────────────

#[test]
fn reads_the_repository_from_a_github_remote() {
    let some = |o: &str, r: &str| Some((o.to_string(), r.to_string()));
    assert_eq!(parse_github_remote("git@github.com:acme/rocket.git"), some("acme", "rocket"));
    assert_eq!(parse_github_remote("https://github.com/acme/rocket"), some("acme", "rocket"));
    assert_eq!(parse_github_remote("https://github.com/acme/rocket.git"), some("acme", "rocket"));
    assert_eq!(parse_github_remote("  https://GitHub.com/acme/rocket/ "), some("acme", "rocket"));
    assert_eq!(parse_github_remote("ssh://git@github.com/acme/my.repo_2.git"), some("acme", "my.repo_2"));
    assert_eq!(parse_github_remote("git@example.com:acme/rocket.git"), None);
    assert_eq!(parse_github_remote("https://github.com/acme"), None);
    assert_eq!(parse_github_remote("https://github.com/acme/rocket/pull/3"), None);
    assert_eq!(parse_github_remote(""), None);
}

#[test]
fn the_remote_origin_names_the_repository_before_the_others_and_the_servers_reading_before_the_url() {
    let r = Rig::start(json!([
        { "name": "upstream", "url": "https://github.com/other/rocket.git" },
        { "name": "origin", "url": "https://github.com/acme/rocket-fork.git" },
    ]));
    assert_eq!(super::picker::repository(&r.rt.state), Some(("acme".into(), "rocket-fork".into())), "origin first, read from its URL");
    let r = Rig::start(json!([{ "name": "origin", "url": "git@example.com:x/y.git", "github": { "owner": "acme", "repo": "rocket" } }]));
    assert_eq!(super::picker::repository(&r.rt.state), Some(("acme".into(), "rocket".into())), "the server's reading");
}

#[test]
fn ignores_hash_inside_a_word_after_a_space_was_typed_or_away_from_the_caret() {
    assert!(find_hash_trigger("C#", 2).is_none(), "inside a word");
    assert!(find_hash_trigger("Fix #3 ", 7).is_none(), "after a space was typed");
    assert!(find_hash_trigger("Fix #3 now", 4).is_none(), "the caret is before the #");
    assert!(find_hash_trigger("Fix #3 now", 10).is_none(), "the caret is after a space");
    assert_eq!(find_hash_trigger("Fix #3 now", 6).map(|t| t.query), Some("3".into()));
}

// ─── NewSessionComposer.test.ts, `# picker` ─────────────────────────────────────

#[test]
fn opens_on_hash_and_asks_the_folders_repository() {
    let mut r = ready();
    assert!(!r.open());
    r.type_in("Fix #");
    r.listed();
    r.rt.frame(FRAME);
    assert!(r.open());
    assert_eq!(r.asked(&assigned_path()), 1, "it asked for acme/rocket");
    let words: Vec<String> = r.rt.elements().iter().filter_map(|e| e.text.clone()).collect();
    assert!(words.iter().any(|w| w == "acme/rocket"), "the head names the repository: {words:?}");
}

#[test]
fn enter_attaches_the_item_drops_the_query_and_starts_in_a_new_worktree() {
    let mut r = ready();
    r.type_in("Fix #");
    r.listed();
    r.rt.update(|s, _| s.new_draft.draft_mut().expect("a draft").workspace = Workspace::Current);
    r.hit(Key::Enter, false);
    assert_eq!(r.typed(), "Fix ", "the #query left the message");
    let d = r.rt.state.new_draft.draft().expect("the draft");
    let preset = d.github_preset.as_ref().expect("attached");
    assert_eq!((preset.number, preset.source_type.as_str()), (218, "github-pull-request"));
    assert_eq!(preset.suggested_branch.as_deref(), Some("feat/pr-218"));
    assert_eq!(d.workspace, Workspace::New);
    assert!(!r.rt.state.new_session_starting(), "nothing was created");
    assert!(r.rt.find("new-session-github-attachment").is_some(), "the chip shows it");
}

#[test]
fn esc_closes_it_and_leaves_the_message_alone() {
    let mut r = ready();
    r.type_in("Fix #");
    r.listed();
    r.hit(Key::Escape, false);
    assert!(!r.open());
    assert_eq!(r.typed(), "Fix #");
    assert!(matches!(r.rt.state.route, crate::fleet::store::Route::NewSession { .. }), "Esc didn't leave the page");
    r.type_in("3");
    r.rt.frame(FRAME);
    assert!(r.open(), "typing opens it again");
}

#[test]
fn enter_waits_while_github_is_still_answering() {
    let mut r = Rig::start(github_remote());
    let go = r.gate(&assigned_path());
    r.answer(&assigned_path(), 200, items(&[0]));
    r.answer(&recent_path(), 200, items(&[1]));
    r.type_in("Fix #");
    r.hit(Key::Enter, false);
    assert_eq!(r.typed(), "Fix #", "the message wasn't sent or changed");
    assert!(!r.rt.state.new_session_starting());
    go.send(()).expect("release the answer");
    r.listed();
    r.hit(Key::Enter, false);
    assert_eq!(r.typed(), "Fix ", "once it answered, Enter picks");
}

#[test]
fn enter_sends_the_message_when_theres_nothing_to_pick() {
    let mut r = Rig::start(github_remote());
    r.answer(&assigned_path(), 200, items(&[]));
    r.answer(&recent_path(), 200, items(&[]));
    r.type_in("Fix #");
    r.listed();
    let mut took = true;
    r.rt.update(|s, tx| took = s.new_autocomplete_key(&KeyEvent { key: Key::Enter, mods: Mods::default() }, tx));
    assert!(!took, "Enter was left for the box, which sends");
    r.rt.frame(FRAME);
    assert!(r.rt.find("github-item-picker").is_some());
    let words: Vec<String> = r.rt.elements().iter().filter_map(|e| e.text.clone()).collect();
    assert!(words.iter().any(|w| w == "No open pull requests or issues match."), "{words:?}");
}

#[test]
fn stays_shut_for_hash_inside_a_word() {
    let mut r = ready();
    r.type_in("Port it to C#");
    r.rt.frame(0.5);
    assert!(!r.open());
    assert!(r.requests().is_empty(), "nothing was asked");
}

// ─── the keys and the rows (A6.6, A6.7, B10.18) ─────────────────────────────────

#[test]
fn the_arrows_wrap_and_a_new_query_goes_back_to_the_first_row() {
    let mut r = ready();
    r.type_in("Fix #");
    r.listed();
    assert_eq!(picker(&r.rt.state).index, 0);
    r.hit(Key::Up, false);
    assert_eq!(picker(&r.rt.state).index, 4, "up from the first goes to the last of five");
    r.hit(Key::Down, false);
    assert_eq!(picker(&r.rt.state).index, 0, "down from the last goes to the first");
    r.hit(Key::Down, false);
    r.hit(Key::Down, false);
    assert_eq!(picker(&r.rt.state).index, 2);
    r.type_in("x");
    assert_eq!(picker(&r.rt.state).index, 0, "a new query resets the row");
    assert_eq!(r.typed(), "Fix #x", "the arrows didn't move the caret out of the query");
}

#[test]
fn tab_picks_the_selected_row_and_a_click_picks_the_one_pressed() {
    let mut r = ready();
    r.type_in("Fix #");
    r.listed();
    r.hit(Key::Down, false);
    r.hit(Key::Tab, false);
    assert_eq!(r.rt.state.new_draft.draft().and_then(|d| d.github_preset.as_ref()).map(|p| p.number), Some(7));
    assert_eq!(
        r.rt.state.new_draft.draft().and_then(|d| d.github_preset.as_ref()).map(|p| p.source_type.clone()).as_deref(),
        Some("github-issue")
    );
    let mut r = ready();
    r.type_in("Fix #");
    r.listed();
    r.rt.frame(FRAME);
    click(&mut r.rt, "github-item-picker-row-3");
    r.rt.frame(FRAME);
    assert_eq!(r.rt.state.new_draft.draft().and_then(|d| d.github_preset.as_ref()).map(|p| p.number), Some(12));
    assert_eq!(r.typed(), "Fix ");
}

#[test]
fn shift_enter_opens_the_items_session_when_it_has_one_and_starts_from_it_when_not() {
    let mut r = ready();
    r.type_in("Fix #");
    r.listed();
    r.hit(Key::Enter, true);
    assert_eq!(
        r.rt.state.new_draft.draft().and_then(|d| d.github_preset.as_ref()).map(|p| p.number),
        Some(218),
        "no session: it starts from it"
    );
    // With a session linked to #218 the same key opens that session instead and leaves the message alone.
    let mut r = ready();
    crate::plugins::github::view::rows_scene::link(&mut r.rt.state);
    r.type_in("Fix #");
    r.listed();
    r.rt.frame(FRAME);
    assert!(r.rt.state.picker_has_session(picker(&r.rt.state).items()[0]), "#218 has a session");
    r.hit(Key::Enter, true);
    assert!(r.rt.state.new_draft.draft().is_none_or(|d| d.github_preset.is_none()), "nothing attached");
    assert!(!r.rt.state.on_new_session_page(), "the session opened");
}
