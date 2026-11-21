//! Piece K of component 21: an item's page, its header, actions, description and conversation. `GitHubWorkItemDetailPage.test.ts`'s
//! tests keep their names (over a fake Fleet that answers `/api/integrations/github/*` and the session's prompt; no real GitHub,
//! account or token is reached); the rest are native, from rows 10.x and 11.x of docs/parity/21-github.md. Every name is invented.
use super::item_page::{Dot, Run, by_words, files_words, fix_label, primary, reference_place, timeline_tone, timeline_words};
use super::items::{ItemDetail, ItemSummary, TimelineEntry};
use super::tests_stores::{Rig, rig};
use super::view::rows_scene;
use super::{Kind, Where, mock, state, state_ref};
use crate::tests::{FRAME, click};
use crate::{Rail, State};
use serde_json::{Value, json};

const PULL: &str = "/api/integrations/github/repos/acme/rocket/items/218";
const ISSUE: &str = "/api/integrations/github/repos/acme/rocket/items/7";
const PROMPT: &str = "/api/sessions/sess-working/prompt";

fn at(s: &mut State, kind: Kind, number: u64) {
    s.rail = Rail::Plugin("github");
    state(&mut s.places).at = Where::Item { owner: "acme".into(), repo: "rocket".into(), kind, number };
}

/// The page on `acme/rocket#218` (or the issue) with Fleet answering the detail, and the invented sessions linked.
fn on(kind: Kind) -> Rig {
    let mut r = rig();
    r.answer("GET", PULL, 200, mock::pull_detail());
    r.answer("GET", ISSUE, 200, mock::issue_detail());
    r.set(rows_scene::link);
    let number = if kind == Kind::Pull { 218 } else { 7 };
    r.set(move |s| at(s, kind, number));
    r.wait("the item", |s| state_ref(&s.places).item.detail.is_some() || state_ref(&s.places).item.error.is_some());
    r
}

fn words(r: &Rig) -> String {
    r.rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

fn text_of(r: &Rig, key: &str) -> Option<String> {
    r.rt.elements().iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text.clone())
}

fn summary(extra: Value) -> ItemSummary {
    let mut v = mock::pull_detail()["summary"].clone();
    v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    ItemSummary::read(&v)
}

fn entry(v: Value) -> TimelineEntry {
    ItemDetail::read(&json!({"summary": {}, "timeline": [v]})).timeline.remove(0)
}

// ─── GitHubWorkItemDetailPage.test.ts ───────────────────────────────────────

#[test]
fn shows_the_conversation_labels_and_assignees_of_an_issue() {
    // The conversation half is here; the labels and assignees are the rail's (piece L).
    let r = on(Kind::Issue);
    let all = words(&r);
    for want in [
        "Launch countdown skips a second",
        "Steps",
        "devon-park",
        "commented",
        "mentioned this in",
        "#218 Queue retries with backoff",
        "#12 Count in whole seconds",
        "closed this",
        "Probably the tick handler",
    ] {
        assert!(all.contains(want), "{want} in {all}");
    }
    assert!(!all.contains("bot: release notes"));
    assert_eq!(text_of(&r, "github-item-state").as_deref(), None, "the pill's words are its child's");
    assert!(all.contains("Open"), "an open issue says Open: {all}");
    assert!(all.contains("kit-lee"), "the author and the people in the conversation are named");
}

#[test]
fn sends_a_pull_requests_failing_checks_to_the_session_working_on_it() {
    let mut r = on(Kind::Pull);
    r.set(|s| {
        super::view::item_scene::failing_link_for_tests(s);
    });
    assert!(words(&r).contains("Fix checks and address review"), "the session has a failing check and an open thread: {}", words(&r));
    click(&mut r.rt, "github-item-primary");
    r.rt.frame(FRAME);
    r.wait("the prompt", |s| s.agent_sends.is_sent("sess-working", crate::fleet::send_to_agent::FIX));
    let sent = r.bodies("POST", PROMPT);
    assert_eq!(sent.len(), 1, "one prompt, to the first session: {sent:?}");
    let text = sent[0]["text"].as_str().unwrap_or_default();
    assert!(text.contains("test") && text.contains("Please add a test."), "the failing check and the thread are in it: {text}");
    assert!(words(&r).contains("Sent to the session"));
    click(&mut r.rt, "github-item-primary");
    r.rt.frame(FRAME);
    assert_eq!(r.bodies("POST", PROMPT).len(), 1, "a button that has sent sends nothing more");
}

#[test]
fn says_why_it_couldnt_load() {
    let mut r = rig();
    r.answer("GET", PULL, 401, "{}");
    r.set(|s| at(s, Kind::Pull, 218));
    r.wait("the failure", |s| state_ref(&s.places).item.error.is_some());
    assert_eq!(text_of(&r, "github-item-error-words").as_deref(), Some("Connect GitHub in Settings to see this."));
    r.answer("GET", PULL, 404, json!({"error": "Not Found"}));
    click(&mut r.rt, "github-item-retry");
    r.rt.frame(FRAME);
    r.wait("the second failure", |s| state_ref(&s.places).item.error.as_deref() == Some("Not Found"));
    assert_eq!(text_of(&r, "github-item-error-words").as_deref(), Some("Not Found"));
    r.answer("GET", PULL, 502, "<html>");
    click(&mut r.rt, "github-item-retry");
    r.wait("the third", |s| state_ref(&s.places).item.error.as_deref() == Some("GitHub didn't answer (HTTP 502)."));
    r.answer("GET", PULL, 200, mock::pull_detail());
    click(&mut r.rt, "github-item-retry");
    r.wait("the item", |s| state_ref(&s.places).item.detail.is_some());
    assert!(r.rt.find("github-item-error").is_none(), "Try again loads it");
    assert_eq!(r.asked("GET", PULL), 4);
}

// ─── native ─────────────────────────────────────────────────────────────────

#[test]
fn a_link_that_isnt_a_number_says_so_and_asks_nothing() {
    let mut r = rig();
    r.set(|s| at(s, Kind::Issue, 0));
    assert_eq!(text_of(&r, "github-item-error-words").as_deref(), Some("That isn't a GitHub issue link."));
    let asked_items = r.fake.lock().unwrap().log.iter().filter(|(_, p, _)| p.contains("/items/")).count();
    assert_eq!(asked_items, 0, "a bad number asks GitHub for nothing");
}

#[test]
fn a_new_number_loads_again_and_a_stale_answer_is_dropped() {
    let mut r = rig();
    r.answer("GET", PULL, 200, mock::pull_detail());
    r.answer("GET", ISSUE, 200, mock::issue_detail());
    let go = r.gate(PULL);
    r.set(|s| at(s, Kind::Pull, 218));
    r.wait("the first ask", |s| state_ref(&s.places).item.loading);
    r.set(|s| at(s, Kind::Issue, 7));
    r.wait("the issue", |s| state_ref(&s.places).item.detail.as_ref().is_some_and(|d| d.summary.number == 7));
    let _ = go.send(());
    // Wait for the first request to be the one that was answered late, then look again: still the issue.
    r.wait("both asked", |_| true);
    assert_eq!(r.asked("GET", PULL), 1);
    for _ in 0..30 {
        r.rt.frame(0.05);
    }
    let shown = state_ref(&r.rt.state.places).item.detail.as_ref().map(|d| d.summary.number);
    assert_eq!(shown, Some(7), "the answer for the old item doesn't replace the new one");
}

#[test]
fn refresh_keeps_the_page_and_asks_again() {
    let mut r = on(Kind::Pull);
    let go = r.gate(PULL);
    click(&mut r.rt, "github-item-refresh");
    r.rt.frame(FRAME);
    assert!(state_ref(&r.rt.state.places).item.refreshing);
    assert!(r.rt.find("github-item-title").is_some(), "the page stays while it refreshes");
    let _ = go.send(());
    r.wait("the refresh", |s| !state_ref(&s.places).item.refreshing);
    assert_eq!(r.asked("GET", PULL), 2);
}

#[test]
fn the_primarys_labels_follow_the_session_and_what_there_is_to_fix() {
    let open = summary(json!({}));
    let closed = summary(json!({"state": "closed"}));
    let issue = ItemSummary::read(&mock::issue_detail()["summary"]);
    assert_eq!(fix_label(&open, 1, 0), Some("Fix failing checks"));
    assert_eq!(fix_label(&open, 0, 2), Some("Address review"));
    assert_eq!(fix_label(&open, 1, 1), Some("Fix checks and address review"));
    assert_eq!(fix_label(&open, 0, 0), None);
    assert_eq!(fix_label(&closed, 1, 1), None, "only an open pull request");
    assert_eq!(fix_label(&issue, 1, 1), None, "only a pull request");

    let p = primary(&open, true, Some("Fix failing checks"), false);
    assert_eq!((p.label.as_str(), p.run), ("Fix failing checks", Run::Fix));
    assert_eq!(primary(&open, true, Some("Fix failing checks"), true).label, "Sent to the session");
    let p = primary(&open, true, None, false);
    assert_eq!((p.label.as_str(), p.run), ("Open session", Run::Open));
    let p = primary(&open, false, None, false);
    assert_eq!((p.label.as_str(), p.run), ("Start a session on this branch", Run::Start));
    assert_eq!(primary(&closed, false, None, false).label, "Start a session");
    assert_eq!(primary(&issue, false, None, false).label, "Start a session");
}

#[test]
fn the_primary_says_what_the_page_would_do() {
    // With no session: start. With one and nothing to fix: open. The scene's session #218 has nothing to fix until its link carries it.
    let mut r = rig();
    r.answer("GET", PULL, 200, mock::pull_detail());
    r.set(|s| at(s, Kind::Pull, 218));
    r.wait("the item", |s| state_ref(&s.places).item.detail.is_some());
    assert!(words(&r).contains("Start a session on this branch"));
    r.set(rows_scene::link);
    assert!(words(&r).contains("Open session"), "{}", words(&r));
}

#[test]
fn the_menu_lists_the_sessions_then_starts_another() {
    let mut r = on(Kind::Pull);
    click(&mut r.rt, "github-item-more");
    r.rt.frame(FRAME);
    let all = words(&r);
    assert!(
        all.to_uppercase().contains("IN FLEET") && all.contains("Fix the retry backoff") && all.contains("Start another session"),
        "{all}"
    );
    click(&mut r.rt, "github-item-menu-start");
    r.rt.frame(FRAME);
    let s = &r.rt.state;
    assert_eq!(s.rail, Rail::Sessions, "starting goes to the New session page");
    let preset = s.new_draft.draft().and_then(|d| d.github_preset.clone()).expect("the New session page took the item");
    assert_eq!((preset.number, preset.repo_full_name.as_str()), (218, "acme/rocket"));
    assert_eq!(preset.body.as_deref(), Some("Retries a failed launch.\n<!-- bot: release notes -->\nCloses #7."));

    // With no session the menu has one row, the primary's words.
    let mut r = rig();
    r.answer("GET", PULL, 200, mock::pull_detail());
    r.set(|s| at(s, Kind::Pull, 218));
    r.wait("the item", |s| state_ref(&s.places).item.detail.is_some());
    click(&mut r.rt, "github-item-more");
    r.rt.frame(FRAME);
    assert!(r.rt.find("github-item-menu-start").is_some() && r.rt.find("github-item-menu-session-0").is_none());
    assert!(words(&r).matches("Start a session on this branch").count() >= 2, "the button and the menu row say it");
}

#[test]
fn a_mention_in_this_repository_opens_fleets_page_and_another_opens_github() {
    let mut r = on(Kind::Issue);
    // The first mention is `acme/rocket#218`.
    click(&mut r.rt, "github-item-entry-1-ref");
    r.rt.frame(FRAME);
    assert_eq!(state_ref(&r.rt.state.places).at.path(), "/github/acme/rocket/pulls/218");
    let here = entry(json!({"kind": "referenced", "createdAt": "", "state": "open", "url": "https://github.com/ACME/Rocket/pull/218",
        "reference": {"kind": "pull", "repository": "ACME/Rocket", "number": 218}}));
    assert_eq!(
        reference_place(&here, "acme", "rocket").map(|w| w.path()).as_deref(),
        Some("/github/acme/rocket/pulls/218"),
        "ignoring case"
    );
    let there = entry(
        json!({"kind": "referenced", "createdAt": "", "reference": {"kind": "pull", "repository": "lumen-labs/atlas", "number": 12}}),
    );
    assert_eq!(reference_place(&there, "acme", "rocket"), None);
}

#[test]
fn the_by_line_says_who_did_what_to_which_branches() {
    let open = summary(json!({}));
    assert_eq!(by_words(&open), "devon-park wants to merge");
    assert_eq!(by_words(&summary(json!({"state": "merged"}))), "devon-park merged");
    assert_eq!(by_words(&summary(json!({"headRef": null}))), "devon-park opened this");
    assert_eq!(by_words(&ItemSummary::read(&mock::issue_detail()["summary"])), "kit-lee opened this");
    let detail = |n: Value| ItemDetail::read(&json!({"summary": {}, "changedFiles": n}));
    assert_eq!(files_words(Some(&detail(json!(5)))).as_deref(), Some("in 5 files"));
    assert_eq!(files_words(Some(&detail(json!(1)))).as_deref(), Some("in 1 file"));
    assert_eq!(files_words(Some(&detail(json!(0)))), None);
    assert_eq!(files_words(Some(&detail(Value::Null))), None);
}

#[test]
fn the_page_draws_the_by_line_the_title_and_the_trail() {
    let r = on(Kind::Pull);
    let all = words(&r);
    assert!(all.contains("Queue retries with backoff #218"), "{all}");
    assert!(all.contains("devon-park wants to merge") && all.contains("feat/pr-218") && all.contains("main"), "{all}");
    assert!(all.contains("+42") && all.contains("in 5 files"), "{all}");
    for key in ["github-item-trail-home", "github-item-trail-repo", "github-item-refresh", "github-item-link", "github-item-primary"] {
        assert!(r.rt.find(key).is_some(), "{key}");
    }
    let mut r = r;
    click(&mut r.rt, "github-item-trail-repo");
    r.rt.frame(FRAME);
    assert_eq!(state_ref(&r.rt.state.places).at.path(), "/github/acme/rocket");
}

#[test]
fn every_entry_has_its_words_and_its_dot() {
    let words_of = |kind: &str, state: Value| timeline_words(&entry(json!({"kind": kind, "createdAt": "", "state": state})));
    assert_eq!(words_of("comment", Value::Null), "commented");
    assert_eq!(words_of("review", json!("APPROVED")), "approved");
    assert_eq!(words_of("review", json!("CHANGES_REQUESTED")), "requested changes");
    assert_eq!(words_of("review", json!("DISMISSED")), "had a review dismissed");
    assert_eq!(words_of("review", json!("COMMENTED")), "reviewed");
    assert_eq!(words_of("merged", Value::Null), "merged this");
    assert_eq!(words_of("closed", Value::Null), "closed this");
    assert_eq!(words_of("reopened", Value::Null), "reopened this");
    assert_eq!(words_of("ready", Value::Null), "marked this ready for review");
    assert_eq!(words_of("draft", Value::Null), "marked this as a draft");
    assert_eq!(words_of("referenced", Value::Null), "mentioned this in");
    let tone = |kind: &str, state: Value| timeline_tone(&entry(json!({"kind": kind, "createdAt": "", "state": state})));
    assert_eq!(tone("review", json!("APPROVED")), Dot::Approved);
    assert_eq!(tone("review", json!("CHANGES_REQUESTED")), Dot::Changes);
    assert_eq!(tone("review", json!("COMMENTED")), Dot::Plain);
    assert_eq!(tone("merged", Value::Null), Dot::Merged);
    assert_eq!(tone("closed", Value::Null), Dot::Closed);
    assert_eq!(tone("comment", Value::Null), Dot::Plain);
}

#[test]
fn an_entry_with_no_author_says_someone_and_no_description_says_so() {
    let mut r = rig();
    let mut detail = mock::pull_detail();
    detail["body"] = json!("<!-- only a note -->");
    detail["timeline"] = json!([{"kind": "closed", "author": null, "createdAt": "2026-11-20T09:00:00Z", "body": null, "state": null, "url": null, "reference": null}]);
    r.answer("GET", PULL, 200, detail);
    r.set(|s| at(s, Kind::Pull, 218));
    r.wait("the item", |s| state_ref(&s.places).item.detail.is_some());
    assert_eq!(text_of(&r, "github-item-empty").as_deref(), Some("No description."));
    let all = words(&r);
    assert!(all.contains("Someone") && all.contains("closed this"), "{all}");
}

#[test]
fn a_failed_send_shows_under_the_header() {
    let mut r = on(Kind::Pull);
    r.set(|s| {
        super::view::item_scene::failing_link_for_tests(s);
        s.agent_sends.show_error("sess-working", "Couldn't send to the agent (HTTP 502).");
    });
    assert_eq!(text_of(&r, "github-item-send-error").as_deref(), Some("Couldn't send to the agent (HTTP 502)."));
}
