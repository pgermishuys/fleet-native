//! Fleet's `github-items.test.ts` under its names, and the reading of this file's invented answers (`mock`).
use super::items::*;
use super::mock;
use super::where_::{Kind, Where};
use crate::fleet::pr_state::{Checks, Lifecycle, ReviewDecision};
use crate::fleet::smart_links::parse_wire_link;
use serde_json::{Value, json};

fn pull(number: u64, extra: Value) -> ItemSummary {
    let mut v = json!({
        "kind": "pull", "owner": "acme", "repo": "rocket", "number": number, "title": format!("PR {number}"),
        "url": format!("https://github.com/acme/rocket/pull/{number}"), "state": "open", "isDraft": false, "author": "pat",
        "headRef": "feat/x", "baseRef": "main", "additions": 1, "deletions": 1, "checks": "success", "mergeable": "MERGEABLE",
    });
    v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    ItemSummary::read(&v)
}

#[test]
fn reads_a_rows_pull_request_facts() {
    let facts = item_pr_facts(&pull(1, json!({"checks": "failure", "mergeable": "CONFLICTING", "reviewDecision": "APPROVED"})), None);
    assert_eq!(facts.state, Lifecycle::Open);
    assert_eq!(facts.checks, Checks::Failing);
    assert!(facts.conflict);
    assert_eq!(facts.review, Some(ReviewDecision::Approved));
    assert_eq!(item_pr_facts(&pull(1, json!({"state": "merged"})), None).state, Lifecycle::Merged);
}

#[test]
fn lists_review_requests_then_the_users_blocked_pull_requests_as_what_needs_them() {
    let work = Work {
        login: Some("pat".into()),
        review_requested: vec![pull(1, json!({}))],
        authored: vec![pull(2, json!({"checks": "failure"})), pull(3, json!({"reviewDecision": "CHANGES_REQUESTED"})), pull(4, json!({}))],
        ..Work::default()
    };
    let needs: Vec<(u64, String)> = needs_you(&work).into_iter().map(|n| (n.item.number, n.reason)).collect();
    assert_eq!(needs, [(1, "Review requested".to_string()), (2, "Checks failing".into()), (3, "Changes requested".into())]);
    assert_eq!(your_pull_requests(&work).iter().map(|i| i.number).collect::<Vec<_>>(), [4]);
}

#[test]
fn builds_a_row_from_a_sessions_link() {
    let link = parse_wire_link(&json!({
        "id": "l1", "sessionId": "s1", "url": "https://github.com/acme/rocket/pull/7", "providerId": "github", "resourceType": "pull_request",
        "resourceId": "acme/rocket#7", "title": "acme/rocket #7: Queue", "status": "draft", "statusLabel": "Draft",
        "metadataJson": json!({"owner": "acme", "repo": "rocket", "number": 7, "headRef": "feat/q", "additions": 3, "deletions": 1,
                               "mergeable": false, "author": "pat"}).to_string(),
        "isDismissed": false, "isTerminal": false, "createdAt": "", "updatedAt": "", "relationship": "own",
        "enrichmentStatus": "resolved", "lastCheckedAt": null,
    }));
    let row = summary_from_link(&link).unwrap();
    assert_eq!((row.kind, row.owner.as_str(), row.repo.as_str(), row.number), (Kind::Pull, "acme", "rocket", 7));
    assert_eq!((row.title.as_str(), row.state.as_str(), row.is_draft), ("Queue", "open", true));
    assert_eq!(row.head_ref.as_deref(), Some("feat/q"));
    assert_eq!((row.additions, row.deletions), (Some(3), Some(1)));
    assert_eq!((row.mergeable.as_deref(), row.author.as_deref(), row.checks.as_deref()), (Some("CONFLICTING"), Some("pat"), Some("none")));
    assert_eq!(item_resource_id(&row), "acme/rocket#7");
}

#[test]
fn a_link_without_an_owner_makes_no_row() {
    let link = parse_wire_link(&json!({"id": "l", "url": "https://example.com/x", "resourceType": "pull_request", "metadataJson": "{}"}));
    assert_eq!(summary_from_link(&link), None);
}

#[test]
fn hides_html_comments_as_github_does() {
    assert_eq!(visible_body(Some("Fixes it.\n<!-- This is an auto-generated comment: release notes -->\n<!--\nmulti\n-->")), "Fixes it.");
    assert_eq!(visible_body(None), "");
    assert_eq!(visible_body(Some("a <!-- never closed")), "a <!-- never closed");
    assert_eq!(visible_body(Some("a<!--x-->b<!--y-->c")), "abc");
}

#[test]
fn routes_to_fleets_page_for_the_item() {
    assert_eq!(Where::of_item(&pull(7, json!({}))).path(), "/github/acme/rocket/pulls/7");
    let issue = ItemSummary::read(&json!({"kind": "issue", "owner": "acme", "repo": "rocket", "number": 8}));
    assert_eq!(Where::of_item(&issue).path(), "/github/acme/rocket/issues/8");
}

#[test]
fn an_error_says_to_connect_or_what_the_server_said_or_the_status() {
    assert_eq!(error_words(401, r#"{"error":"bad token"}"#), "Connect GitHub in Settings to see this.");
    assert_eq!(error_words(400, r#"{"error":"q is required."}"#), "q is required.");
    assert_eq!(error_words(502, "<html>gateway</html>"), "GitHub didn't answer (HTTP 502).");
    assert_eq!(error_words(500, "{}"), "GitHub didn't answer (HTTP 500).");
}

#[test]
fn counts_skipped_and_neutral_checks_as_passed_and_prefers_a_pages_checks() {
    let detail = ItemDetail::read(&mock::pull_detail());
    let counts = check_counts(&detail.checks);
    assert_eq!((counts.passed, counts.failing, counts.pending), (2, 1, 0));
    // The row says "success"; the page's checks say one fails, and the page wins.
    let mut row = detail.summary.clone();
    row.checks = Some("success".into());
    assert_eq!(item_pr_facts(&row, None).checks, Checks::Passing);
    let facts = item_pr_facts(&row, Some(&detail));
    assert_eq!((facts.checks, facts.unresolved_threads, facts.check_counts), (Checks::Failing, 1, Some(counts)));
    // With no checks on the page, the row's rollup stands.
    let bare = ItemDetail { checks: Vec::new(), ..detail };
    assert_eq!(item_pr_facts(&row, Some(&bare)).checks, Checks::Passing);
}

#[test]
fn a_session_preset_carries_the_branch_of_a_pull_request_only() {
    let preset = item_session_preset(&pull(5, json!({})), Some("body"));
    assert_eq!(preset.source_type, "github-pull-request");
    assert_eq!(
        (preset.repo_full_name.as_str(), preset.suggested_branch.as_deref(), preset.body.as_deref()),
        ("acme/rocket", Some("feat/x"), Some("body"))
    );
    let issue = ItemSummary::read(
        &json!({"kind": "issue", "owner": "acme", "repo": "rocket", "number": 8, "title": "T", "url": "u", "headRef": "x"}),
    );
    let preset = item_session_preset(&issue, None);
    assert_eq!((preset.source_type.as_str(), preset.suggested_branch, preset.body), ("github-issue", None, None));
}

#[test]
fn reads_a_search_page_of_the_invented_github() {
    let page = ItemPage::read(&mock::search_page());
    assert_eq!((page.items.len(), page.total_count, page.has_next_page), (2, 31, true));
    assert_eq!(page.end_cursor.as_deref(), Some("Y3Vyc29yOjI5"));
    assert_eq!((page.items[0].kind, page.items[0].number, page.items[1].kind), (Kind::Pull, 218, Kind::Issue));
    assert_eq!(page.items[1].assignees, ["mara-quinn"]);
    assert_eq!(page.items[0].reviewers[0].login, "kit-lee");
    assert_eq!(ItemPage::read(&json!({})), ItemPage::default(), "missing fields are the empty value");
}

#[test]
fn reads_a_detail_of_the_invented_github() {
    let detail = ItemDetail::read(&mock::pull_detail());
    assert_eq!(visible_body(detail.body.as_deref()), "Retries a failed launch.\n\nCloses #7.");
    assert_eq!((detail.changed_files, detail.unresolved_threads, detail.checks.len()), (Some(5), 1, 3));
    assert_eq!(detail.checks[1].state, CheckResult::Skipped);
    assert_eq!(detail.checks[0].check_run_id, Some(11));
    // The entry of a kind this app doesn't know is left out.
    assert_eq!(detail.timeline.len(), 3);
    let reference = detail.timeline[2].reference.as_ref().unwrap();
    assert_eq!(
        (detail.timeline[2].kind, reference.kind, reference.repository.as_str(), reference.number),
        (TimelineKind::Referenced, Kind::Issue, "acme/billing", 3)
    );
    assert_eq!(detail.summary.reviewers[0].avatar_url.as_deref(), Some("https://avatars.example/kit-lee"));
}

#[test]
fn reads_the_work_of_the_invented_github() {
    let work = Work::read(&mock::work());
    assert_eq!(work.login.as_deref(), Some("mara-quinn"));
    assert_eq!((work.review_requested.len(), work.authored.len(), work.assigned.len()), (1, 3, 1));
    assert_eq!(work.repos[0], RepoCounts { full_name: "acme/rocket".into(), open_pull_requests: 4, open_issues: 9 });
    let needs: Vec<(u64, String)> = needs_you(&work).into_iter().map(|n| (n.item.number, n.reason)).collect();
    assert_eq!(needs, [(220, "Review requested".to_string()), (221, "Checks failing".into()), (222, "Changes requested".into())]);
    assert_eq!(your_pull_requests(&work).iter().map(|i| i.number).collect::<Vec<_>>(), [223]);
}

#[test]
fn a_by_number_answer_is_a_bare_list() {
    assert_eq!(read_items(&json!([{"kind": "pull", "number": 3}, {"kind": "issue", "number": 4}])).len(), 2);
    assert!(read_items(&json!({"error": "x"})).is_empty());
}
