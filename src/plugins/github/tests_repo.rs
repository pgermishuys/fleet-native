//! Piece I of component 21: a repository's page. `GitHubRepoPage.test.ts`'s three tests keep their names (over a fake Fleet that
//! answers `/api/integrations/github/*`; no real GitHub, account or token is reached); the rest are native, from rows 8.x of
//! docs/parity/21-github.md. Every name is invented.
use super::items::ItemSummary;
use super::repo::{FILTER_INPUT, PULL_FILTERS, PullFilter, RepoPage, Tab, fleet_numbers, toggled_text};
use super::search::{DEBOUNCE, path};
use super::tests_stores::{Rig, WORK, rig};
use super::{Where, state, state_ref};
use crate::tests::{FRAME, click};
use crate::{Rail, State};
use serde_json::{Value, json};

const OPEN: &str = "repo:acme/rocket is:pr is:open sort:updated-desc";

/// A pull request, as search and `items?numbers=` send one (the Vue test's `pull`).
fn pull(number: u64, title: &str) -> Value {
    json!({
        "kind": "pull", "owner": "acme", "repo": "rocket", "number": number, "title": title,
        "url": format!("https://github.com/acme/rocket/pull/{number}"), "state": "open", "isDraft": false, "author": "pat",
        "authorAvatarUrl": null, "createdAt": "2026-09-25T00:00:00Z", "updatedAt": "2026-09-25T00:00:00Z", "comments": 0,
        "labels": [{"name": "client", "color": "1d76db"}, {"name": "needs design", "color": "fbca04"}], "headRef": "feat/x", "baseRef": "main",
        "additions": 10, "deletions": 2, "checks": "failure", "reviewDecision": null, "mergeable": "MERGEABLE", "reviewers": [], "assignees": [],
    })
}

fn search_answer(items: Vec<Value>, cursor: Option<&str>) -> Value {
    let total = items.len();
    json!({"items": items, "totalCount": total, "endCursor": cursor, "hasNextPage": cursor.is_some()})
}

fn work() -> Value {
    json!({"login": "pat", "avatarUrl": null, "reviewRequested": [], "authored": [], "assigned": [],
           "repos": [{"fullName": "acme/rocket", "openPullRequests": 5, "openIssues": 6}]})
}

/// A session's link to `acme/rocket#9`.
fn link() -> Value {
    json!({
        "id": "l1", "sessionId": "s1", "url": "https://github.com/acme/rocket/pull/9", "providerId": "github", "resourceType": "pull_request",
        "resourceId": "acme/rocket#9", "title": "acme/rocket #9: Nine", "status": "open", "statusLabel": "Open", "metadataJson": "{}",
        "isDismissed": false, "isTerminal": false, "createdAt": "", "updatedAt": "", "relationship": "own", "enrichmentStatus": "resolved",
        "lastCheckedAt": null,
    })
}

fn q(query: &str) -> String {
    path(query, None)
}

/// The page on `acme/rocket` with Fleet answering what the Vue test's fake did.
fn on_repo() -> Rig {
    let mut r = rig();
    r.answer("GET", WORK, 200, work());
    r.answer("GET", &q(OPEN), 200, search_answer(vec![pull(7, "Seven")], None));
    r.answer("GET", "/api/integrations/github/repos/acme/rocket/items?numbers=9", 200, json!([pull(9, "Nine")]));
    r.set(|s| {
        s.smart_links.header_links_loaded(&[link()]);
        s.rail = Rail::Plugin("github");
        state(&mut s.places).at = Where::Repo { owner: "acme".into(), repo: "rocket".into() };
    });
    r
}

fn page(s: &State) -> &RepoPage {
    &state_ref(&s.places).repo
}

fn listed(s: &State) -> Vec<String> {
    s.github_repo_list().items.iter().map(|i: &ItemSummary| i.title.clone()).collect()
}

/// The searches sent so far: the `q` of each request to the search route.
fn searches(r: &Rig) -> Vec<String> {
    let log = r.fake.lock().unwrap().log.clone();
    log.into_iter()
        .filter(|(_, p, _)| p.starts_with("/api/integrations/github/search"))
        .map(|(_, p, _)| {
            let q = p.split("q=").nth(1).and_then(|r| r.split('&').next()).unwrap_or_default().to_string();
            unescape(&q)
        })
        .collect()
}

/// A percent-encoded query back to its words.
fn unescape(q: &str) -> String {
    let b = q.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() && u8::from_str_radix(&q[i + 1..i + 3], 16).is_ok() => {
                out.push(u8::from_str_radix(&q[i + 1..i + 3], 16).unwrap_or_default());
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn words(r: &Rig) -> String {
    r.rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

// ─── GitHubRepoPage.test.ts ─────────────────────────────────────────────────

#[test]
fn lists_open_pull_requests_first_with_their_checks_and_filters_to_mine() {
    let mut r = on_repo();
    r.wait("the first list", |s| !listed(s).is_empty());
    assert_eq!(searches(&r), [OPEN]);
    assert_eq!(page(&r.rt.state).tab, Tab::Pulls);
    let all = words(&r);
    assert!(all.contains("Pull requests") && all.contains('5'), "the open count of the work is on the tab: {all}");
    assert_eq!(listed(&r.rt.state), ["Seven"]);

    let mine = "repo:acme/rocket is:pr is:open author:@me sort:updated-desc";
    r.answer("GET", &q(mine), 200, search_answer(vec![pull(8, "Eight")], None));
    click(&mut r.rt, "github-pull-filter-mine");
    r.rt.frame(DEBOUNCE + 0.05);
    r.wait("the list of mine", |s| listed(s) == ["Eight"]);
    assert_eq!(searches(&r).last().map(String::as_str), Some(mine));
}

#[test]
fn shows_the_pull_requests_fleet_sessions_are_on_under_in_fleet() {
    let mut r = on_repo();
    r.wait("the first list", |s| !listed(s).is_empty());
    let count = r.rt.elements().iter().find(|e| e.key.as_deref() == Some("github-pull-filter-fleet-count")).and_then(|e| e.text.clone());
    assert_eq!(count.as_deref(), Some("1"));
    click(&mut r.rt, "github-pull-filter-fleet");
    r.wait("the pull request by number", |s| listed(s) == ["Nine"]);
    let by_number = "/api/integrations/github/repos/acme/rocket/items?numbers=9";
    assert_eq!(r.asked("GET", by_number), 1);
    assert_eq!(searches(&r), [OPEN], "In Fleet asks the search for nothing more");
}

#[test]
fn opens_the_pull_requests_page_from_its_row() {
    let mut r = on_repo();
    r.wait("the first list", |s| !listed(s).is_empty());
    click(&mut r.rt, "github-row-acme/rocket#7");
    r.rt.frame(FRAME);
    assert_eq!(state_ref(&r.rt.state.places).at.path(), "/github/acme/rocket/pulls/7");
}

// ─── native ─────────────────────────────────────────────────────────────────

#[test]
fn each_filter_makes_its_query_and_its_empty_words() {
    let mut r = on_repo();
    r.wait("the first list", |s| !listed(s).is_empty());
    let cases = [
        (PullFilter::Review, "is:open review-requested:@me", "Nobody is waiting on your review here."),
        (PullFilter::Mine, "is:open author:@me", "You have no open pull requests here."),
        (PullFilter::Closed, "is:closed", "No closed pull requests match."),
        (PullFilter::Open, "is:open", "No open pull requests match."),
    ];
    for (filter, qualifier, empty) in cases {
        let query = format!("repo:acme/rocket is:pr {qualifier} sort:updated-desc");
        r.answer("GET", &q(&query), 200, search_answer(Vec::new(), None));
        click(&mut r.rt, &format!("github-pull-filter-{}", filter.id()));
        r.rt.frame(DEBOUNCE + 0.05);
        let want = query.clone();
        r.wait("the empty list", move |s| {
            page(s).filter == filter && s.github_repo_list().items.is_empty() && !s.github_repo_list().loading
        });
        assert!(searches(&r).contains(&want), "{want} in {:?}", searches(&r));
        assert!(words(&r).contains(empty), "{empty}: {}", words(&r));
    }
    assert_eq!(PULL_FILTERS.map(PullFilter::id), ["open", "review", "mine", "fleet", "closed"]);
}

#[test]
fn what_is_typed_joins_the_query_with_its_spaces_collapsed() {
    let mut r = on_repo();
    r.wait("the first list", |s| !listed(s).is_empty());
    let want = "repo:acme/rocket is:pr is:open label:bug author:kit-lee sort:updated-desc";
    r.answer("GET", &q(want), 200, search_answer(vec![pull(12, "Twelve")], None));
    r.set(|s| state(&mut s.places).repo.text.replace_text("  label:bug   author:kit-lee "));
    r.rt.frame(DEBOUNCE + 0.05);
    r.wait("the typed filter's list", |s| listed(s) == ["Twelve"]);
    assert_eq!(searches(&r).last().map(String::as_str), Some(want));
    assert!(r.rt.elements().iter().any(|e| e.key.as_deref() == Some(FILTER_INPUT)));
}

#[test]
fn a_label_toggles_in_the_typed_filter_on_pull_requests_and_in_the_filter_on_issues() {
    assert_eq!(toggled_text("", "client"), "label:client");
    assert_eq!(toggled_text("label:client", "client"), "");
    assert_eq!(toggled_text("author:pat label:client", "client"), "author:pat");
    assert_eq!(toggled_text("author:pat", "needs design"), "author:pat label:\"needs design\"");
    assert_eq!(toggled_text("label:\"needs design\"", "needs design"), "");
    let mut r = on_repo();
    r.wait("the first list", |s| !listed(s).is_empty());
    r.set(|s| s.github_repo_toggle_label("client"));
    assert_eq!(page(&r.rt.state).text.text, "label:client");
    r.set(|s| s.github_repo_toggle_label("client"));
    assert_eq!(page(&r.rt.state).text.text, "");
    click(&mut r.rt, "github-row-acme/rocket#7-label-client");
    r.rt.frame(FRAME);
    assert_eq!(page(&r.rt.state).text.text, "label:client", "a press on a label in a row filters by it");
    assert_eq!(state_ref(&r.rt.state.places).at, Where::Repo { owner: "acme".into(), repo: "rocket".into() }, "and doesn't open the row");

    click(&mut r.rt, "github-repo-tab-issues");
    r.set(|s| {
        s.github_repo_toggle_label("bug");
        s.github_repo_toggle_label("client");
    });
    assert_eq!(page(&r.rt.state).issues.labels, ["bug", "client"]);
    r.set(|s| s.github_repo_toggle_label("bug"));
    assert_eq!(page(&r.rt.state).issues.labels, ["client"]);
    assert_eq!(page(&r.rt.state).text.text, "label:client", "the pull requests' text is left alone");
}

#[test]
fn the_issues_tab_asks_for_issues_and_says_so_when_there_are_none() {
    let mut r = on_repo();
    r.wait("the first list", |s| !listed(s).is_empty());
    let issues = "repo:acme/rocket is:issue is:open sort:updated-desc";
    r.answer("GET", &q(issues), 200, search_answer(Vec::new(), None));
    click(&mut r.rt, "github-repo-tab-issues");
    r.rt.frame(DEBOUNCE + 0.05);
    r.wait("the issues", |s| page(s).tab == Tab::Issues && s.github_repo_list().items.is_empty() && !s.github_repo_list().loading);
    assert_eq!(searches(&r).last().map(String::as_str), Some(issues));
    assert!(words(&r).contains("No issues match."));
    assert!(words(&r).contains('6'), "the issues tab's open count");
}

#[test]
fn load_more_adds_the_next_page_and_says_loading_meanwhile() {
    let mut r = rig();
    r.answer("GET", WORK, 200, work());
    r.answer("GET", &q(OPEN), 200, search_answer(vec![pull(7, "Seven")], Some("c1")));
    r.answer("GET", &path(OPEN, Some("c1")), 200, search_answer(vec![pull(6, "Six")], None));
    let go = r.gate(&path(OPEN, Some("c1")));
    r.set(|s| {
        s.rail = Rail::Plugin("github");
        state(&mut s.places).at = Where::Repo { owner: "acme".into(), repo: "rocket".into() };
    });
    r.wait("the first page", |s| s.github_repo_list().has_more);
    click(&mut r.rt, "github-repo-more");
    let fake = r.fake.clone();
    r.wait("the next page's request", move |_| fake.lock().unwrap().log.iter().any(|(_, p, _)| p.contains("after=c1")));
    assert!(words(&r).contains("Loading\u{2026}"), "{}", words(&r));
    go.send(()).unwrap();
    r.wait("the next page", |s| listed(s) == ["Seven", "Six"]);
    assert!(!words(&r).contains("Load more"), "no more pages, no button");
}

#[test]
fn a_failed_search_says_why_and_try_again_asks_again() {
    let mut r = rig();
    r.answer("GET", WORK, 200, work());
    r.answer("GET", &q(OPEN), 502, json!({"error": "GitHub is down."}));
    r.set(|s| {
        s.rail = Rail::Plugin("github");
        state(&mut s.places).at = Where::Repo { owner: "acme".into(), repo: "rocket".into() };
    });
    r.wait("the failure", |s| s.github_repo_list().error.is_some());
    assert!(words(&r).contains("Try again"));
    r.answer("GET", &q(OPEN), 200, search_answer(vec![pull(7, "Seven")], None));
    click(&mut r.rt, "github-repo-retry");
    r.wait("the list", |s| listed(s) == ["Seven"]);
    assert_eq!(r.asked("GET", &q(OPEN)), 2);
}

#[test]
fn another_repository_starts_the_page_over() {
    let mut r = on_repo();
    r.wait("the first list", |s| !listed(s).is_empty());
    click(&mut r.rt, "github-pull-filter-closed");
    r.set(|s| state(&mut s.places).repo.text.replace_text("label:x"));
    r.set(|s| state(&mut s.places).at = Where::Repo { owner: "acme".into(), repo: "billing".into() });
    let p = page(&r.rt.state);
    assert_eq!((p.filter, p.text.text.as_str(), p.tab), (PullFilter::Open, "", Tab::Pulls));
    assert_eq!(p.asked, Some(("acme".into(), "billing".into())));
}

#[test]
fn fleet_numbers_are_this_repositorys_pull_requests_newest_first() {
    let mut r = rig();
    let wire = |id: &str, session: &str, kind: &str, resource: &str| {
        let mut l = link();
        l["id"] = json!(id);
        l["sessionId"] = json!(session);
        l["resourceType"] = json!(kind);
        l["resourceId"] = json!(resource);
        l
    };
    r.set(move |s| {
        s.smart_links.header_links_loaded(&[
            wire("a", "s1", "pull_request", "acme/rocket#9"),
            wire("b", "s2", "pull_request", "ACME/Rocket#12"),
            wire("c", "s3", "pull_request", "acme/rocket#9"),
            wire("d", "s3", "issue", "acme/rocket#40"),
            wire("e", "s4", "pull_request", "acme/billing#99"),
            wire("f", "s4", "pull_request", "acme/rocket-x#5"),
        ])
    });
    assert_eq!(fleet_numbers(&r.rt.state, "acme", "rocket"), [12, 9]);
}

#[test]
fn in_fleet_says_why_it_could_not_and_with_no_sessions_says_there_are_none() {
    let mut r = on_repo();
    r.wait("the first list", |s| !listed(s).is_empty());
    r.answer("GET", "/api/integrations/github/repos/acme/rocket/items?numbers=9", 502, json!({"error": "GitHub is down."}));
    click(&mut r.rt, "github-pull-filter-fleet");
    r.wait("the failure", |s| s.github_repo_list().error.is_some());
    // No session is on a pull request any more: an empty set of numbers reloads and asks nothing.
    r.set(|s| s.smart_links.header_links_loaded(&[]));
    r.wait("the empty list", |s| s.github_repo_list().error.is_none() && !s.github_repo_list().loading);
    assert!(words(&r).contains("No session is working on a pull request here."));
    assert_eq!(r.asked("GET", "/api/integrations/github/repos/acme/rocket/items?numbers=9"), 1);
}
