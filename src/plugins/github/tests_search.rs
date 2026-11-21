//! Piece H of component 21: search, items by number, one item, and a repository's labels, milestones and assignees. Fleet has no
//! tests for `use-github-search.ts`, `use-github-metadata.ts` or `fetchGitHubItem(sByNumber)`, so these are native, over a fake
//! Fleet (`tests_stores::rig`) that answers `/api/integrations/github/*` and logs what it was asked. Invented names throughout.
use super::item::{Asked, by_number_path, item_path};
use super::search::{DEBOUNCE, path};
use super::tests_stores::{Rig, rig};
use super::{metadata, state_ref};
use crate::State;
use crate::tests::FRAME;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// What a callback was given, once it has run.
type Got<T> = Arc<Mutex<Option<Result<T, String>>>>;

const ID: &str = "repo-page";

fn search_state(s: &State) -> &super::search::Search {
    state_ref(&s.places).search.get(ID).expect("the search")
}

fn page(titles: &[&str], cursor: Option<&str>, total: u64) -> Value {
    let items: Vec<Value> = titles
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut item = super::mock::search_page()["items"][0].clone();
            item["title"] = json!(t);
            item["number"] = json!(100 + i);
            item
        })
        .collect();
    json!({"items": items, "totalCount": total, "endCursor": cursor, "hasNextPage": cursor.is_some()})
}

fn titles(r: &Rig) -> Vec<String> {
    search_state(&r.rt.state).items.iter().map(|i| i.title.clone()).collect()
}

fn idle(r: &mut Rig, what: &str) {
    r.wait(what, |s| {
        let s = search_state(s);
        !s.is_loading && !s.is_loading_more
    });
}

fn query(r: &mut Rig, q: Option<&'static str>) {
    r.set(move |s| s.github_search_set(ID, q));
}

fn seen(r: &Rig, n: usize) {
    let fake = r.fake.clone();
    crate::test_wait::until_true("the requests to arrive", move || fake.lock().unwrap().log.len() >= n);
}

fn total(r: &Rig) -> usize {
    r.fake.lock().unwrap().log.len()
}

// ─── search ─────────────────────────────────────────────────────────────────

#[test]
fn the_first_query_goes_at_once_and_later_ones_after_250_ms() {
    let mut r = rig();
    r.answer("GET", &path("is:open retries", None), 200, page(&["one"], None, 1));
    r.answer("GET", &path("is:open retry", None), 200, page(&["two"], None, 1));
    query(&mut r, Some("is:open retries"));
    seen(&r, 1);
    idle(&mut r, "the first page");
    assert_eq!(titles(&r), ["one"]);
    assert_eq!(r.asked("GET", &path("is:open retries", None)), 1, "no pause before the first");

    query(&mut r, Some("is:open retry"));
    assert!(search_state(&r.rt.state).is_loading, "waiting for typing to pause counts as loading");
    r.rt.frame(DEBOUNCE - 0.1);
    assert_eq!(total(&r), 1, "nothing is sent while the pause lasts");
    r.rt.frame(0.2);
    seen(&r, 2);
    idle(&mut r, "the second page");
    assert_eq!(titles(&r), ["two"]);
    assert_eq!(total(&r), 2);
}

#[test]
fn typing_again_inside_the_pause_sends_only_the_last() {
    let mut r = rig();
    query(&mut r, Some("a"));
    seen(&r, 1);
    idle(&mut r, "the first");
    for q in ["ab", "abc"] {
        query(&mut r, Some(q));
        r.rt.frame(0.1);
    }
    r.rt.frame(DEBOUNCE);
    seen(&r, 2);
    idle(&mut r, "the last");
    assert_eq!(r.asked("GET", &path("abc", None)), 1);
    assert_eq!(r.asked("GET", &path("ab", None)), 0, "a query typed over is never sent");
    // The same query again changes nothing.
    query(&mut r, Some("abc"));
    r.rt.frame(1.0);
    assert_eq!(total(&r), 2);
}

#[test]
fn a_newer_query_drops_the_older_answer() {
    let mut r = rig();
    r.answer("GET", &path("old", None), 200, page(&["from the old query"], None, 1));
    r.answer("GET", &path("new", None), 200, page(&["from the new query"], None, 1));
    let go = r.gate(&path("old", None));
    query(&mut r, Some("old"));
    seen(&r, 1);
    query(&mut r, Some("new"));
    r.rt.frame(DEBOUNCE + 0.05);
    seen(&r, 2);
    idle(&mut r, "the new answer");
    assert_eq!(titles(&r), ["from the new query"]);
    go.send(()).unwrap();
    // The old answer lands after the new one; wait for it to have been read by the fake and handled.
    for _ in 0..30 {
        r.rt.frame(FRAME);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(titles(&r), ["from the new query"], "the older answer is dropped");
    assert!(!search_state(&r.rt.state).is_loading);
}

#[test]
fn load_more_adds_the_next_page_after_the_cursor() {
    let mut r = rig();
    r.answer("GET", &path("is:open", None), 200, page(&["a", "b"], Some("c2"), 3));
    r.answer("GET", &path("is:open", Some("c2")), 200, page(&["c"], None, 3));
    query(&mut r, Some("is:open"));
    seen(&r, 1);
    idle(&mut r, "page one");
    let s = search_state(&r.rt.state);
    assert!(s.has_more);
    assert_eq!(s.total_count, 3);
    r.set(|s| s.github_search_more(ID));
    assert!(search_state(&r.rt.state).is_loading_more);
    seen(&r, 2);
    idle(&mut r, "page two");
    assert_eq!(titles(&r), ["a", "b", "c"]);
    assert!(!search_state(&r.rt.state).has_more);
    assert_eq!(r.asked("GET", &path("is:open", Some("c2"))), 1);
    // With no next page, loading more asks nothing.
    r.set(|s| s.github_search_more(ID));
    r.rt.frame(0.5);
    assert_eq!(total(&r), 2);
}

#[test]
fn refresh_reads_the_first_page_again() {
    let mut r = rig();
    r.answer("GET", &path("is:open", None), 200, page(&["a"], Some("c1"), 2));
    query(&mut r, Some("is:open"));
    seen(&r, 1);
    idle(&mut r, "page one");
    r.set(|s| s.github_search_refresh(ID));
    seen(&r, 2);
    idle(&mut r, "page one again");
    assert_eq!(titles(&r), ["a"], "the list is replaced, not added to");
    assert_eq!(r.asked("GET", &path("is:open", None)), 2);
}

#[test]
fn no_query_clears_the_list_and_asks_nothing() {
    let mut r = rig();
    r.answer("GET", &path("is:open", None), 200, page(&["a"], Some("c1"), 9));
    query(&mut r, Some("is:open"));
    seen(&r, 1);
    idle(&mut r, "page one");
    query(&mut r, None);
    let s = search_state(&r.rt.state);
    assert!((s.items.is_empty(), s.total_count, s.has_more, s.is_loading) == (true, 0, false, false));
    r.set(|s| s.github_search_more(ID));
    r.set(|s| s.github_search_refresh(ID));
    r.rt.frame(1.0);
    assert_eq!(total(&r), 1, "nothing was asked for no query");
    // An empty query is no query; a first query of nothing asks nothing either.
    query(&mut r, Some(""));
    r.rt.frame(1.0);
    assert_eq!(total(&r), 1);
    // And a query after no query waits, as every later one does.
    query(&mut r, Some("is:open"));
    r.rt.frame(DEBOUNCE - 0.1);
    assert_eq!(total(&r), 1);
}

#[test]
fn a_cleared_query_drops_the_answer_on_its_way() {
    let mut r = rig();
    r.answer("GET", &path("slow", None), 200, page(&["late"], None, 1));
    let go = r.gate(&path("slow", None));
    query(&mut r, Some("slow"));
    seen(&r, 1);
    query(&mut r, None);
    go.send(()).unwrap();
    for _ in 0..30 {
        r.rt.frame(FRAME);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(search_state(&r.rt.state).items.is_empty());
}

#[test]
fn search_says_why_it_failed() {
    let mut r = rig();
    for (status, body, words) in [
        (401, "{}", "Connect GitHub in Settings to see this."),
        (400, r#"{"error":"q is required."}"#, "q is required."),
        (502, "", "GitHub didn't answer (HTTP 502)."),
    ] {
        r.answer("GET", &path(&format!("q{status}"), None), status, body);
        let q: &'static str = Box::leak(format!("q{status}").into_boxed_str());
        // A failed first page leaves no list; the words are the server's.
        query(&mut r, Some(q));
        let want = words.to_string();
        r.wait("the failure", move |s| search_state(s).error.as_deref() == Some(want.as_str()));
        assert!(search_state(&r.rt.state).items.is_empty());
    }
}

#[test]
fn a_failed_next_page_keeps_the_list() {
    let mut r = rig();
    r.answer("GET", &path("is:open", None), 200, page(&["a"], Some("c1"), 2));
    r.answer("GET", &path("is:open", Some("c1")), 500, r#"{"error":"Rate limited."}"#);
    query(&mut r, Some("is:open"));
    seen(&r, 1);
    idle(&mut r, "page one");
    r.set(|s| s.github_search_more(ID));
    r.wait("the failure", |s| search_state(s).error.is_some());
    assert_eq!(search_state(&r.rt.state).error.as_deref(), Some("Rate limited."));
    assert_eq!(titles(&r), ["a"]);
}

#[test]
fn search_with_no_server_says_so() {
    let mut r = rig();
    r.set(|s| {
        s.link = None;
        s.github_search_set(ID, Some("x"));
    });
    let s = search_state(&r.rt.state);
    assert_eq!(s.error.as_deref(), Some("GitHub search failed."));
    assert!(!s.is_loading);
}

#[test]
fn the_query_is_sent_encoded() {
    assert_eq!(
        path("is:pr label:\"good first\" a&b", None),
        "/api/integrations/github/search?q=is%3Apr%20label%3A%22good%20first%22%20a%26b&first=30"
    );
    assert!(path("x", Some("abc==")).ends_with("&after=abc%3D%3D"));
}

// ─── by number, one item ────────────────────────────────────────────────────

#[test]
fn by_number_with_no_numbers_asks_nothing() {
    let mut r = rig();
    let got: Arc<Mutex<Option<Asked>>> = Arc::default();
    let into = got.clone();
    r.set(move |s| *into.lock().unwrap() = Some(s.github_items_by_number("acme", "rocket", &[], |_, _, _| panic!("no answer to none"))));
    r.rt.frame(0.5);
    assert_eq!(*got.lock().unwrap(), Some(Asked::Nothing));
    assert_eq!(total(&r), 0);
    assert_eq!(by_number_path("acme", "rocket", &[]), None);
}

#[test]
fn by_number_asks_for_the_numbers_and_reads_the_rows() {
    let mut r = rig();
    let at = by_number_path("acme", "rocket", &[218, 7]).expect("a path");
    assert_eq!(at, "/api/integrations/github/repos/acme/rocket/items?numbers=218,7");
    r.answer("GET", &at, 200, super::mock::search_page()["items"].clone());
    let out: Got<Vec<String>> = Arc::default();
    let into = out.clone();
    r.set(move |s| {
        let asked = s.github_items_by_number("acme", "rocket", &[218, 7], move |_, _, a| {
            *into.lock().unwrap() = Some(a.map(|v| v.into_iter().map(|i| i.title).collect()));
        });
        assert_eq!(asked, Asked::Sent);
    });
    let out2 = out.clone();
    r.wait("the rows", move |_| out2.lock().unwrap().is_some());
    assert_eq!(
        out.lock().unwrap().take(),
        Some(Ok(vec!["Queue retries with backoff".to_string(), "Launch countdown skips a second".to_string()]))
    );
    assert_eq!(r.asked("GET", &at), 1);
}

#[test]
fn one_item_reads_its_page_and_says_why_it_could_not() {
    let mut r = rig();
    let at = item_path("acme", "rocket", 218);
    assert_eq!(at, "/api/integrations/github/repos/acme/rocket/items/218");
    r.answer("GET", &at, 200, super::mock::pull_detail());
    let out: Got<(String, usize)> = Arc::default();
    let into = out.clone();
    r.set(move |s| {
        s.github_item("acme", "rocket", 218, move |_, _, a| *into.lock().unwrap() = Some(a.map(|d| (d.summary.title, d.checks.len()))));
    });
    let out2 = out.clone();
    r.wait("the item", move |_| out2.lock().unwrap().is_some());
    assert_eq!(out.lock().unwrap().take(), Some(Ok(("Queue retries with backoff".to_string(), 3))));

    r.answer("GET", &at, 404, r#"{"error":"No such pull request."}"#);
    let into = out.clone();
    r.set(move |s| {
        s.github_item("acme", "rocket", 218, move |_, _, a| *into.lock().unwrap() = Some(a.map(|d| (d.summary.title, 0))));
    });
    let out2 = out.clone();
    r.wait("the refusal", move |_| out2.lock().unwrap().is_some());
    assert_eq!(out.lock().unwrap().take(), Some(Err("No such pull request.".to_string())));
}

// ─── labels, milestones, assignees ──────────────────────────────────────────

const LABELS: &str = "/api/integrations/github/repos/acme/rocket/labels";
const MILES: &str = "/api/integrations/github/repos/acme/rocket/milestones";
const PEOPLE: &str = "/api/integrations/github/repos/acme/rocket/assignees";

fn labels(r: &Rig, repo: &str) -> Option<usize> {
    state_ref(&r.rt.state.places).metadata.labels.get("acme", repo).map(|e| e.data.len())
}

fn label_entry<T>(r: &Rig, f: impl Fn(&metadata::Metadata) -> Option<&metadata::Entry<T>>) -> bool {
    f(&state_ref(&r.rt.state.places).metadata).is_some_and(|e| !e.loading)
}

#[test]
fn metadata_is_shared_per_repository_and_kept_five_minutes() {
    let mut r = rig();
    r.answer("GET", LABELS, 200, json!([{"name": "bug", "color": "d73a4a", "description": "Something is broken"}, {"name": "docs", "color": "0075ca", "description": null}]));
    r.answer("GET", "/api/integrations/github/repos/acme/billing/labels", 200, json!([{"name": "ledger", "color": "ffffff"}]));
    r.set(|s| {
        s.github_labels("acme", "rocket", false);
        s.github_labels("acme", "rocket", false);
        s.github_labels("acme", "billing", false);
    });
    r.wait("both repositories", |s| {
        let m = &state_ref(&s.places).metadata.labels;
        ["rocket", "billing"].iter().all(|n| m.get("acme", n).is_some_and(|e| e.updated.is_some()))
    });
    assert_eq!(r.asked("GET", LABELS), 1, "two callers, one request");
    assert_eq!(labels(&r, "rocket"), Some(2));
    assert_eq!(labels(&r, "billing"), Some(1), "each repository has its own copy");
    let rocket = state_ref(&r.rt.state.places).metadata.labels.get("acme", "rocket").unwrap().data.clone();
    assert_eq!(rocket[0], metadata::Label { name: "bug".into(), color: "d73a4a".into(), description: Some("Something is broken".into()) });
    assert_eq!(rocket[1].description, None);

    r.set(|s| s.github_labels("acme", "rocket", false));
    r.rt.frame(299.0);
    r.set(|s| s.github_labels("acme", "rocket", false));
    assert_eq!(total(&r), 2, "a copy under five minutes old is used");
    r.rt.frame(2.0);
    r.set(|s| s.github_labels("acme", "rocket", false));
    seen(&r, 3);
    r.wait("a stale copy read again", |s| !state_ref(&s.places).metadata.labels.get("acme", "rocket").unwrap().loading);
    assert_eq!(r.asked("GET", LABELS), 2);
    // A forced read asks whatever the age.
    r.set(|s| s.github_labels("acme", "rocket", true));
    seen(&r, 4);
    r.wait("the forced read", |s| !state_ref(&s.places).metadata.labels.get("acme", "rocket").unwrap().loading);
    assert_eq!(r.asked("GET", LABELS), 3);
}

#[test]
fn a_refresh_while_one_is_out_joins_it() {
    let mut r = rig();
    r.answer("GET", MILES, 200, json!([{"number": 3, "title": "v2", "state": "open", "open_issues": 4, "closed_issues": 9}, {"number": 2, "title": "v1", "state": "closed", "open_issues": 0, "closed_issues": 12}]));
    let go = r.gate(MILES);
    r.set(|s| {
        s.github_milestones("acme", "rocket", false);
        s.github_milestones("acme", "rocket", true);
    });
    seen(&r, 1);
    go.send(()).unwrap();
    r.wait("the milestones", |s| state_ref(&s.places).metadata.milestones.get("acme", "rocket").is_some_and(|e| e.updated.is_some()));
    assert_eq!(r.asked("GET", MILES), 1);
    let m = state_ref(&r.rt.state.places).metadata.milestones.get("acme", "rocket").unwrap().data.clone();
    assert_eq!(m.len(), 2);
    assert_eq!((m[0].number, m[0].open, m[0].open_issues, m[0].closed_issues), (3, true, 4, 9));
    assert!(!m[1].open);
}

#[test]
fn assignees_read_their_login_and_picture() {
    let mut r = rig();
    r.answer("GET", PEOPLE, 200, json!([{"login": "mara-quinn", "avatar_url": "https://avatars.example/mara"}, {"avatar_url": "x"}]));
    r.set(|s| s.github_assignees("acme", "rocket", false));
    r.wait("the people", |s| state_ref(&s.places).metadata.assignees.get("acme", "rocket").is_some_and(|e| e.updated.is_some()));
    let a = &state_ref(&r.rt.state.places).metadata.assignees.get("acme", "rocket").unwrap().data;
    assert_eq!(
        a,
        &[metadata::Assignee { login: "mara-quinn".into(), avatar_url: "https://avatars.example/mara".into() }],
        "an entry with no login is left out"
    );
    assert!(label_entry(&r, |m| m.assignees.get("acme", "rocket")));
}

#[test]
fn metadata_says_why_it_failed_and_keeps_what_it_had() {
    let mut r = rig();
    r.answer("GET", LABELS, 200, json!([{"name": "bug", "color": "d73a4a"}]));
    r.set(|s| s.github_labels("acme", "rocket", false));
    r.wait("the labels", |s| state_ref(&s.places).metadata.labels.get("acme", "rocket").is_some_and(|e| e.updated.is_some()));
    for (status, body, words) in [
        (500, r#"{"error":"The token store is locked."}"#, "The token store is locked."),
        (403, r#"{"message":"Forbidden here."}"#, "Forbidden here."),
        (502, "", "Failed to fetch labels."),
    ] {
        r.answer("GET", LABELS, status, body);
        r.set(|s| s.github_labels("acme", "rocket", true));
        let want = words.to_string();
        r.wait("the failure", move |s| {
            let e = state_ref(&s.places).metadata.labels.get("acme", "rocket").unwrap();
            !e.loading && e.error.as_deref() == Some(want.as_str())
        });
        assert_eq!(labels(&r, "rocket"), Some(1), "the labels it had stay beside the reason");
    }
    // A good answer clears the reason.
    r.answer("GET", LABELS, 200, json!([]));
    r.set(|s| s.github_labels("acme", "rocket", true));
    r.wait("the good answer", |s| state_ref(&s.places).metadata.labels.get("acme", "rocket").unwrap().error.is_none());
}

#[test]
fn metadata_with_no_server_says_so() {
    let mut r = rig();
    r.set(|s| {
        s.link = None;
        s.github_labels("acme", "rocket", false);
    });
    let e = state_ref(&r.rt.state.places).metadata.labels.get("acme", "rocket").unwrap();
    assert_eq!(e.error.as_deref(), Some("Failed to load labels."));
    assert!(!e.loading);
}
