//! Piece G of component 21: Your work and its connect section. `GitHubBrowserPage.test.ts`'s three tests keep their names (over a fake
//! Fleet that answers `/api/integrations/github/*`; no real GitHub, account or token is reached); the rest are native, from rows 4.x and
//! 5.x of docs/parity/21-github.md. Every name is invented.
use super::auth::Device;
use super::home::{buckets, expires_in_minutes, failure_words, header_line, scope_words, waiting_words};
use super::tests_stores::{MARKS, WORK, rig};
use super::{state, state_ref};
use crate::fleet::store::Route;
use crate::tests::{FRAME, app, click};
use crate::{Rail, State};
use rune::Runtime;
use serde_json::{Value, json};

const STATUS: &str = "/api/integrations/github/auth/status";
const DEVICE: &str = "/api/integrations/github/auth/device-code";

/// A pull request, as `GET .../work` sends one (the Vue test's `item`).
fn item(number: u64, extra: Value) -> Value {
    let mut v = json!({
        "kind": "pull", "owner": "acme", "repo": "rocket", "number": number, "title": format!("Item {number}"),
        "url": format!("https://github.com/acme/rocket/pull/{number}"), "state": "open", "isDraft": false, "author": "pat",
        "authorAvatarUrl": null, "createdAt": "2026-11-18T00:00:00Z", "updatedAt": "2026-11-18T00:00:00Z", "comments": 0, "labels": [],
        "headRef": "x", "baseRef": "main", "additions": 1, "deletions": 1, "checks": "success", "reviewDecision": null,
        "mergeable": "MERGEABLE", "reviewers": [], "assignees": [],
    });
    v.as_object_mut().expect("an object").extend(extra.as_object().expect("an object").clone());
    v
}

fn the_work() -> Value {
    json!({
        "login": "pat", "avatarUrl": null,
        "reviewRequested": [item(1, json!({}))],
        "authored": [item(2, json!({"checks": "failure"})), item(3, json!({}))],
        "assigned": [item(4, json!({"kind": "issue", "checks": null, "headRef": null, "additions": null, "deletions": null}))],
        "repos": [],
    })
}

fn words(rt: &Runtime<State>) -> String {
    rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

fn texts_of(rt: &Runtime<State>, key: &str) -> Vec<String> {
    let all = rt.elements();
    let Some(el) = all.iter().find(|e| e.key.as_deref() == Some(key)) else { return Vec::new() };
    all.iter().filter(|e| e.path.starts_with(&el.path)).filter_map(|e| e.text.clone()).collect()
}

/// The app on a scene of the page, with no server.
fn scene(name: &'static str) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| crate::plugins::scene(s, name, true));
    for _ in 0..3 {
        rt.frame(FRAME);
    }
    rt
}

// ─── GitHubBrowserPage.test.ts ──────────────────────────────────────────────

#[test]
fn puts_review_requests_and_blocked_pull_requests_under_needs_you_each_with_why() {
    let mut r = rig();
    r.answer("GET", STATUS, 200, json!({"connected": true}));
    r.answer("GET", WORK, 200, the_work());
    r.answer("GET", MARKS, 200, json!([{"fullName": "acme/rocket", "owner": "acme", "name": "rocket"}]));
    r.set(|s| s.rail = Rail::Plugin("github"));
    r.wait("your work and the repositories you follow", |s| {
        let g = state_ref(&s.places);
        g.work.work.is_some() && g.bookmarks.has_loaded && g.auth.connected
    });
    let needs = texts_of(&r.rt, "github-needs-you").join("\n");
    assert!(needs.contains("Review requested"), "{needs}");
    assert!(needs.contains("Checks failing"), "{needs}");
    assert!(needs.contains("Item 1") && needs.contains("Item 2") && !needs.contains("Item 3"), "{needs}");
    let all = words(&r.rt);
    assert!(all.contains("Your pull requests") && all.contains("Item 3"), "{all}");
    assert!(all.contains("Assigned to you") && all.contains("Item 4"), "{all}");
    assert_eq!(r.asked("GET", WORK), 1, "opening the page reads your work once, with the panel's");
}

#[test]
fn starts_a_session_from_a_row() {
    let mut r = rig();
    r.answer("GET", STATUS, 200, json!({"connected": true}));
    r.answer("GET", WORK, 200, the_work());
    r.set(|s| s.rail = Rail::Plugin("github"));
    r.wait("your work", |s| state_ref(&s.places).work.work.is_some());
    crate::tests::click(&mut r.rt, "github-pulls-acme/rocket#3-start");
    r.rt.frame(FRAME);
    assert_eq!(r.rt.state.rail, Rail::Sessions);
    assert!(matches!(r.rt.state.route, Route::NewSession { project_id: None }));
    let preset = r.rt.state.new_draft.draft().and_then(|d| d.github_preset.clone()).expect("the item is the page's source");
    assert_eq!(preset.number, 3);
}

#[test]
fn explains_what_connecting_gets_you_and_connects_from_the_page() {
    let mut r = rig();
    r.answer("GET", STATUS, 200, json!({"connected": false}));
    r.answer("POST", DEVICE, 200, json!({"userCode": "ABCD-1234", "verificationUri": "https://github.com/login/device", "deviceCode": "d", "expiresIn": 900, "interval": 5}));
    r.set(|s| s.rail = Rail::Plugin("github"));
    r.wait("the status", |s| {
        let a = &state_ref(&s.places).auth;
        a.asked && !a.checking
    });
    r.rt.frame(FRAME);
    let section = texts_of(&r.rt, "github-connect").join("\n");
    assert!(section.contains("Connect GitHub") && section.contains("See which pull requests need you"), "{section}");
    click(&mut r.rt, "github-connect-start");
    r.wait("the device code request", |s| matches!(state_ref(&s.places).auth.device, Device::Awaiting { .. }));
    assert_eq!(r.asked("POST", DEVICE), 1);
    assert_eq!(r.asked("GET", WORK), 0, "nothing is read from GitHub until it is connected");
    assert!(words(&r.rt).contains("ABCD-1234"));
}

// ─── native ─────────────────────────────────────────────────────────────────

#[test]
fn in_fleet_leaves_out_what_needs_you_lists_and_links_that_are_over() {
    let mut rt = scene("github-home");
    let id = |list: &[super::items::ItemSummary]| list.iter().map(|i| i.number).collect::<Vec<_>>();
    let work = state_ref(&rt.state.places).work.work.clone().expect("work");
    let b = buckets(&rt.state, &work);
    assert_eq!(b.needs.iter().map(|n| n.item.number).collect::<Vec<_>>(), [220, 221, 222]);
    // The session on #221 sits on its Needs you row, not under In Fleet; #215 is on no list but In Fleet.
    assert_eq!(id(&b.in_fleet), [215]);
    assert_eq!(id(&b.pulls), [223]);
    assert_eq!(id(&b.assigned), [7]);
    assert!(!b.is_empty());

    for (what, change) in [
        ("a link that is over", (|l: &mut crate::fleet::smart_links::SmartLink| l.is_terminal = true) as fn(&mut _)),
        ("a link not yet resolved", |l| l.enrichment = crate::fleet::smart_links::Enrichment::Pending),
    ] {
        let mut s = scene("github-home").state;
        s.smart_links.header_by_session.values_mut().flatten().filter(|l| l.resource_id == "acme/rocket#215").for_each(change);
        assert!(buckets(&s, &work).in_fleet.is_empty(), "{what} is not In Fleet");
    }
    // No live session on it, nothing to show.
    rt.update(|s, _| s.projects[0].sessions[1].row.archived = true);
    assert!(buckets(&rt.state, &work).in_fleet.is_empty(), "an archived session is not In Fleet");
}

#[test]
fn in_fleet_lists_the_newest_first_and_each_item_once() {
    let mut s = scene("github-home").state;
    let wire = |id: &str, n: u64, updated: &str| {
        json!({"id": format!("l-{id}"), "sessionId": id, "url": format!("https://github.com/acme/rocket/pull/{n}"), "providerId": "github",
            "resourceType": "pull_request", "resourceId": format!("acme/rocket#{n}"), "title": format!("acme/rocket #{n}: Item {n}"), "status": "open",
            "statusLabel": "Open", "metadataJson": json!({"owner": "acme", "repo": "rocket", "updatedAt": updated}).to_string(), "isDismissed": false,
            "isTerminal": false, "createdAt": "2026-11-01T00:00:00Z", "updatedAt": updated, "relationship": "own", "enrichmentStatus": "resolved"})
    };
    // Sessions 0 and 1 of the scene, both on #300 (listed once), and a session on #301 that updated later.
    s.smart_links.header_links_loaded(&[
        wire("sess-retry", 300, "2026-11-10T00:00:00Z"),
        wire("sess-audit", 300, "2026-11-10T00:00:00Z"),
        wire("sess-x", 301, "2026-11-12T00:00:00Z"),
    ]);
    s.projects[0].sessions[2].id = "sess-x".into();
    let work = state_ref(&s.places).work.work.clone().expect("work");
    let numbers: Vec<_> = buckets(&s, &work).in_fleet.iter().map(|i| i.number).collect();
    assert_eq!(numbers, [301, 300]);
}

#[test]
fn the_line_under_the_title_says_the_scope_and_when_it_loaded() {
    assert_eq!(scope_words(0), "Everything you're involved in");
    assert_eq!(scope_words(1), "1 repository");
    assert_eq!(scope_words(3), "3 repositories");
    assert_eq!(header_line(2, None), "2 repositories");
    assert_eq!(header_line(0, Some(10.0)), "Everything you're involved in · updated just now");
    assert_eq!(header_line(2, Some(65.0)), "2 repositories · updated 1 minute ago");
    assert_eq!(header_line(2, Some(3.0 * 3600.0)), "2 repositories · updated 3 hours ago");
    let rt = scene("github-home");
    assert!(words(&rt).contains("2 repositories · updated just now"), "{}", words(&rt));
}

#[test]
fn the_waiting_words_round_the_minutes_and_leave_the_bracket_out_at_none() {
    assert_eq!(expires_in_minutes(900.0, 0.0), 15);
    assert_eq!(expires_in_minutes(900.0, 20.0), 15);
    assert_eq!(expires_in_minutes(900.0, 40.0), 14);
    assert_eq!(expires_in_minutes(10.0, 5.0), 0);
    assert_eq!(expires_in_minutes(10.0, 50.0), 0, "never below none");
    assert_eq!(waiting_words(14), "Waiting for you to approve it (the code works for 14 more minutes)…");
    assert_eq!(waiting_words(0), "Waiting for you to approve it…");
    assert_eq!(failure_words(&Device::Expired).as_deref(), Some("The code expired. Start again for a new one."));
    assert_eq!(failure_words(&Device::Denied).as_deref(), Some("GitHub says the request was denied."));
    assert_eq!(failure_words(&Device::Error("It broke.".into())).as_deref(), Some("It broke."));
    assert_eq!(failure_words(&Device::Idle), None);
}

#[test]
fn the_connect_section_shows_the_code_and_the_way_back_to_the_button() {
    let mut rt = scene("github-connect-waiting");
    let box_ = texts_of(&rt, "github-connect-code").join("\n");
    assert!(box_.contains("Enter this code at") && box_.contains("github.com/login/device") && !box_.contains("https://"), "{box_}");
    assert!(box_.contains("WDJB-MJHT") && box_.contains("Copy code") && box_.contains("Open GitHub"), "{box_}");
    assert!(box_.contains("the code works for 14 more minutes"), "{box_}");
    assert!(rt.find("github-connect-start").is_none());
    click(&mut rt, "github-connect-cancel");
    assert_eq!(state_ref(&rt.state.places).auth.device, Device::Idle);
    assert!(rt.find("github-connect-start").is_some() && rt.find("github-connect-code").is_none());
}

#[test]
fn a_failed_try_says_why_above_the_button() {
    let rt = scene("github-connect-expired");
    assert_eq!(texts_of(&rt, "github-connect-error"), ["The code expired. Start again for a new one."]);
    assert!(rt.find("github-connect-start").is_some());
    let mut rt = scene("github-connect");
    assert!(rt.find("github-connect-error").is_none());
    rt.update(|s, _| state(&mut s.places).auth.device = Device::Denied);
    rt.frame(FRAME);
    assert_eq!(texts_of(&rt, "github-connect-error"), ["GitHub says the request was denied."]);
    rt.update(|s, _| state(&mut s.places).auth.device = Device::Error("Failed to start GitHub authorization.".into()));
    rt.frame(FRAME);
    assert_eq!(texts_of(&rt, "github-connect-error"), ["Failed to start GitHub authorization."]);
}

#[test]
fn the_token_link_opens_the_connection_page_in_settings() {
    let mut rt = scene("github-connect");
    click(&mut rt, "github-connect-token");
    assert_eq!(rt.state.rail, Rail::Settings);
    assert_eq!(rt.state.settings.page, crate::fleet::settings_nav::Page::Plugin("github".into()));
}

#[test]
fn leaving_the_page_stops_the_device_polling() {
    let mut r = rig();
    r.answer("GET", STATUS, 200, json!({"connected": false}));
    r.answer("POST", DEVICE, 200, json!({"userCode": "ABCD-1234", "verificationUri": "https://github.com/login/device", "deviceCode": "d", "expiresIn": 900, "interval": 5}));
    r.answer("POST", "/api/integrations/github/auth/poll", 200, json!({"status": "pending"}));
    r.set(|s| s.rail = Rail::Plugin("github"));
    r.wait("the status", |s| {
        let a = &state_ref(&s.places).auth;
        a.asked && !a.checking
    });
    r.set(|s| s.github_connect_device());
    r.wait("a code", |s| matches!(state_ref(&s.places).auth.device, Device::Awaiting { .. }));
    // Away: the code goes back to the button, and a poll that was due is never sent.
    r.set(|s| s.rail = Rail::Sessions);
    r.rt.frame(10.0);
    r.set(|_| {});
    assert_eq!(state_ref(&r.rt.state.places).auth.device, Device::Idle);
    assert!(!state_ref(&r.rt.state.places).auth.polling);
    assert_eq!(r.asked("POST", "/api/integrations/github/auth/poll"), 0);
}

#[test]
fn a_connection_that_arrives_reads_your_work_at_once() {
    let mut r = rig();
    r.answer("GET", STATUS, 200, json!({"connected": false}));
    r.answer("GET", WORK, 200, the_work());
    r.set(|s| s.rail = Rail::Plugin("github"));
    r.wait("the status", |s| {
        let a = &state_ref(&s.places).auth;
        a.asked && !a.checking
    });
    assert_eq!(r.asked("GET", WORK), 0);
    r.answer("GET", STATUS, 200, json!({"connected": true}));
    r.set(|s| s.load_github_status());
    r.wait("your work", |s| state_ref(&s.places).work.work.is_some());
    assert_eq!(r.asked("GET", WORK), 1);
}

#[test]
fn says_what_it_is_doing_and_what_went_wrong() {
    let rt = scene("github-home-loading");
    assert!(words(&rt).contains("Asking GitHub what needs you…"));
    assert!(rt.find("github-needs-you").is_none());
    let rt = scene("github-home-error");
    assert_eq!(texts_of(&rt, "github-home-error"), ["GitHub rate limit exceeded. Try again in a few minutes."]);
    assert!(!words(&rt).contains("Asking GitHub"));
}

#[test]
fn nothing_in_any_bucket_says_so_and_says_where_it_looked() {
    let mut rt = scene("github-home-empty");
    let all = words(&rt);
    assert!(all.contains("Nothing needs you"), "{all}");
    assert!(all.contains("No review requests, open pull requests or assigned issues in the repositories you follow."), "{all}");
    rt.update(|s, _| state(&mut s.places).bookmarks.list.clear());
    rt.frame(FRAME);
    assert!(words(&rt).contains("No review requests, open pull requests or assigned issues."), "{}", words(&rt));
    for key in ["github-needs-you", "github-in-fleet", "github-your-pulls", "github-assigned"] {
        assert!(rt.find(key).is_none(), "{key}");
    }
}

#[test]
fn each_bucket_counts_its_rows() {
    let rt = scene("github-home");
    for (key, n) in [("github-needs-you", "3"), ("github-in-fleet", "1"), ("github-your-pulls", "1"), ("github-assigned", "1")] {
        assert_eq!(texts_of(&rt, &format!("{key}-count")), [n], "{key}");
    }
}

#[test]
fn refresh_reads_again_and_follow_opens_the_dialog() {
    let mut r = rig();
    r.answer("GET", STATUS, 200, json!({"connected": true}));
    r.answer("GET", WORK, 200, the_work());
    r.set(|s| s.rail = Rail::Plugin("github"));
    r.wait("your work", |s| state_ref(&s.places).work.work.is_some() && !state_ref(&s.places).work.loading);
    click(&mut r.rt, "github-refresh");
    let fake = r.fake.clone();
    r.wait("the second read", move |_| fake.lock().unwrap().log.iter().filter(|(m, p, _)| m == "GET" && p == WORK).count() == 2);
    r.wait("it to finish", |s| !state_ref(&s.places).work.loading);
    click(&mut r.rt, "github-follow");
    assert!(state_ref(&r.rt.state.places).follow.open);
}
