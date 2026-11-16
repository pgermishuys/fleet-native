//! Smart-link data, pull request state and the badge (docs/parity/08-session-header-and-context.md, piece F). Ports
//! `pr-state.test.ts` (4), `lib/smart-links.test.ts` (7), `stores/smart-links.test.ts` (11) and
//! `use-smart-link-updates.test.ts` (5) under their names; the prompt texts are compared byte for byte with what Fleet's
//! own `formatCheckFailurePrompt`, `formatReviewThreadPrompt` and `formatFixPrompt` produced for the same input
//! (`tests/fixtures/smart-link-prompts.json`). The store's requests are read from a fake server.
use super::super::events::{Event, Kind};
use super::super::link::{Job, Link, Post};
use super::super::pr_state::*;
use super::super::row_chips::PrBadge;
use super::super::smart_links::*;
use super::super::state::Topic;
use super::reconnect::app;
use crate::State;
use rune::Runtime;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

// ─── pr-state.test.ts ───────────────────────────────────────────────────────

fn facts(change: impl FnOnce(&mut PrFacts)) -> PrFacts {
    let mut f = PrFacts::default();
    change(&mut f);
    f
}

fn counts(passed: u32, failing: u32, pending: u32) -> Option<CheckCounts> {
    Some(CheckCounts { passed, failing, pending })
}

// pr-state.test.ts: "blocks an open pull request on failing checks, conflicts, requested changes or open threads"
#[test]
fn blocks_an_open_pull_request_on_failing_checks_conflicts_requested_changes_or_open_threads() {
    assert_eq!(pr_state(&facts(|_| {})), PrState::Open);
    assert_eq!(pr_state(&facts(|f| f.checks = Checks::Failing)), PrState::Blocked);
    assert_eq!(pr_state(&facts(|f| f.conflict = true)), PrState::Blocked);
    assert_eq!(pr_state(&facts(|f| f.review = Some(ReviewDecision::ChangesRequested))), PrState::Blocked);
    assert_eq!(pr_state(&facts(|f| f.unresolved_threads = 1)), PrState::Blocked);
    let all = facts(|f| {
        f.checks = Checks::Failing;
        f.conflict = true;
        f.review = Some(ReviewDecision::ChangesRequested);
        f.unresolved_threads = 2;
    });
    assert_eq!(block_reasons(&all), [BlockReason::Checks, BlockReason::Conflict, BlockReason::Changes, BlockReason::Threads]);
}

// pr-state.test.ts: "keeps drafts grey and finished pull requests in their own colour"
#[test]
fn keeps_drafts_grey_and_finished_pull_requests_in_their_own_colour() {
    assert_eq!(pr_state(&facts(|f| (f.draft, f.checks) = (true, Checks::Failing))), PrState::Draft);
    assert_eq!(pr_state(&facts(|f| (f.state, f.checks) = (Lifecycle::Merged, Checks::Failing))), PrState::Merged);
    assert_eq!(pr_state(&facts(|f| f.state = Lifecycle::Closed)), PrState::Closed);
    assert!(block_reasons(&facts(|f| (f.state, f.checks) = (Lifecycle::Merged, Checks::Failing))).is_empty());
}

// pr-state.test.ts: "says what the pull request is waiting on"
#[test]
fn says_what_the_pull_request_is_waiting_on() {
    let words = |change: fn(&mut PrFacts)| pr_words(&facts(change));
    assert_eq!(
        words(|f| {
            f.checks = Checks::Failing;
            f.unresolved_threads = 2;
            f.check_counts = counts(3, 1, 1);
        }),
        "1 failing · 2 threads"
    );
    assert_eq!(
        words(|f| {
            f.checks = Checks::Failing;
            f.check_counts = counts(3, 1, 0);
        }),
        "Checks failing"
    );
    assert_eq!(
        words(|f| {
            f.checks = Checks::Pending;
            f.check_counts = counts(3, 0, 2);
        }),
        "3 of 5 checks done"
    );
    assert_eq!(words(|f| f.checks = Checks::Pending), "Checks running");
    assert_eq!(words(|f| f.review = Some(ReviewDecision::Approved)), "Ready to merge");
    assert_eq!(words(|f| f.review = Some(ReviewDecision::ReviewRequired)), "Waiting for review");
    assert_eq!(words(|f| f.review = Some(ReviewDecision::ChangesRequested)), "Changes requested");
    assert_eq!(words(|f| f.conflict = true), "Conflicts");
    assert_eq!(words(|_| {}), "Checks passed");
    assert_eq!(words(|f| f.checks = Checks::None), "Open");
    assert_eq!(words(|f| f.draft = true), "Draft");
    assert_eq!(words(|f| f.state = Lifecycle::Merged), "Merged");
}

#[test]
fn the_rest_of_the_words_are_a_count_and_the_extremes_keep_their_form() {
    let all = facts(|f| {
        f.checks = Checks::Failing;
        f.conflict = true;
        f.review = Some(ReviewDecision::ChangesRequested);
        f.unresolved_threads = 1;
        f.check_counts = counts(0, 2, 0);
    });
    assert_eq!(pr_words(&all), "2 failing · Conflicts +2");
    assert_eq!(pr_words(&facts(|f| (f.draft, f.checks) = (true, Checks::Failing))), "Draft · checks failing");
    assert_eq!(pr_words(&facts(|f| f.state = Lifecycle::Closed)), "Closed");
    assert_eq!(pr_words(&facts(|f| (f.review, f.checks) = (Some(ReviewDecision::Approved), Checks::None))), "Approved");
    assert_eq!(pr_words(&facts(|f| f.unresolved_threads = 1)), "1 thread");
    assert_eq!(pr_state_label(PrState::Blocked), "Blocked");
    assert_eq!(check_count_words(CheckCounts { passed: 3, failing: 1, pending: 1 }), "3 passed · 1 failing · 1 running");
    assert_eq!(checks_headline(Checks::Pending), "Some checks haven't completed yet");
}

// pr-state.test.ts: "reads GitHub's checks rollup and reviewers"
#[test]
fn reads_githubs_checks_rollup_and_reviewers() {
    assert_eq!(checks_from_rollup(Some("failure")), Checks::Failing);
    assert_eq!(checks_from_rollup(Some("pending")), Checks::Pending);
    assert_eq!(checks_from_rollup(Some("success")), Checks::Passing);
    assert_eq!(checks_from_rollup(None), Checks::None);
    let reviewers =
        parse_reviewers(Some(&json!([{"login": "sarah", "avatarUrl": "", "state": "CHANGES_REQUESTED"}, {"state": "APPROVED"}, "junk"])));
    assert_eq!(reviewers, [Reviewer { login: "sarah".into(), avatar_url: None, state: ReviewerState::ChangesRequested }]);
}

// ─── lib/smart-links.test.ts ────────────────────────────────────────────────

fn wire(change: impl FnOnce(&mut Value)) -> Value {
    let mut w = json!({
        "id": "l1", "sessionId": "s1", "url": "https://github.com/owner/repo/pull/187", "providerId": "github",
        "resourceType": "pull_request", "resourceId": "owner/repo#187", "title": "owner/repo #187: feat(client): mock API",
        "status": "open", "statusLabel": "Open", "metadataJson": null, "isDismissed": false, "isTerminal": false,
        "createdAt": "2026-09-12T00:00:00Z", "updatedAt": "2026-09-12T00:00:00Z", "relationship": "own",
        "enrichmentStatus": "resolved", "lastCheckedAt": null,
    });
    change(&mut w);
    w
}

fn plain() -> SmartLink {
    parse_wire_link(&wire(|_| {}))
}

fn run(name: &str, status: &str, conclusion: Option<&str>) -> Value {
    json!({"id": name.len(), "name": name, "status": status, "conclusion": conclusion, "htmlUrl": format!("https://ci.example.test/{name}"),
        "workflowName": "CI", "startedAt": null, "completedAt": null})
}

fn with_checks(runs: Vec<Value>, extra: Value) -> SmartLink {
    let mut meta =
        json!({"owner": "owner", "repo": "repo", "number": 187, "ci": {"headSha": "abc1234def", "ciStatus": "failure", "checkRuns": runs}});
    for (k, v) in extra.as_object().into_iter().flatten() {
        meta[k] = v.clone();
    }
    parse_wire_link(&wire(|w| w["metadataJson"] = json!(meta.to_string())))
}

fn check_run(v: &Value) -> CheckRun {
    ci_status(&with_checks(vec![v.clone()], json!({}))).unwrap().check_runs.remove(0)
}

// smart-links.test.ts: "parses wire links and defaults unknown relationship and status"
#[test]
fn parses_wire_links_and_defaults_unknown_relationship_and_status() {
    let link = parse_wire_link(&wire(|w| {
        w["relationship"] = json!("weird");
        w.as_object_mut().unwrap().remove("enrichmentStatus");
        w["metadataJson"] = json!("{not json");
    }));
    assert_eq!(link.relationship, Relationship::Mentioned);
    assert_eq!(link.enrichment, Enrichment::Resolved);
    assert!(link.metadata.is_empty());
}

// smart-links.test.ts: "strips the owner/repo prefix from titles and falls back to a number"
#[test]
fn strips_the_owner_repo_prefix_from_titles_and_falls_back_to_a_number() {
    assert_eq!(link_title(&plain()), "feat(client): mock API");
    assert_eq!(link_title(&parse_wire_link(&wire(|w| w["title"] = json!("owner/repo#187")))), "Pull request #187");
    assert_eq!(link_number(&plain()), Some(187));
}

#[test]
fn a_number_comes_from_the_metadata_then_the_resource_id_then_the_url_and_an_issue_says_so() {
    let from = |change: fn(&mut Value)| parse_wire_link(&wire(change));
    assert_eq!(link_number(&from(|w| w["metadataJson"] = json!(r#"{"number": 9}"#))), Some(9));
    assert_eq!(link_number(&from(|w| w["resourceId"] = json!("owner/repo"))), Some(187), "the url has it");
    assert_eq!(link_number(&from(|w| (w["resourceId"], w["url"]) = (json!("x"), json!("https://example.test/x")))), None);
    let issue = from(|w| {
        w["resourceType"] = json!("issue");
        w["title"] = json!("");
    });
    assert_eq!(link_title(&issue), "Issue #187");
    assert!(!has_title(&issue));
    assert!(has_title(&plain()));
    assert_eq!(link_href(&plain()), "https://github.com/owner/repo/pull/187");
    assert_eq!(
        link_href(&from(|w| w["metadataJson"] = json!(r#"{"htmlUrl": "https://github.com/owner/repo/pull/187/files"}"#))),
        "https://github.com/owner/repo/pull/187/files"
    );
}

// smart-links.test.ts: "summarizes checks in words"
#[test]
fn summarizes_checks_in_words() {
    let mixed = with_checks(
        vec![
            run("client-tests", "completed", Some("failure")),
            run("e2e", "in_progress", None),
            run("build", "completed", Some("success")),
        ],
        json!({}),
    );
    assert_eq!(summarize_checks(&mixed).text, "1 failing · 1 running · 1 passed");
    let skipped = with_checks(vec![run("a", "completed", Some("success")), run("b", "completed", Some("skipped"))], json!({}));
    assert_eq!(summarize_checks(&skipped).text, "All 2 passed");
    assert_eq!(summarize_checks(&plain()).state, SummaryState::None);
}

// smart-links.test.ts: "needs attention for failing checks or conflicts on open pull requests only"
#[test]
fn needs_attention_for_failing_checks_or_conflicts_on_open_pull_requests_only() {
    assert!(needs_attention(&with_checks(vec![run("a", "completed", Some("failure"))], json!({}))));
    assert!(needs_attention(&with_checks(vec![run("a", "completed", Some("success"))], json!({"mergeable": false}))));
    assert!(!needs_attention(&with_checks(vec![run("a", "completed", Some("success"))], json!({"mergeable": null}))));
    let mut merged = with_checks(vec![run("a", "completed", Some("failure"))], json!({}));
    (merged.is_terminal, merged.status) = (true, "merged".into());
    assert!(!needs_attention(&merged));
}

// smart-links.test.ts: "puts origin, own and pinned links in the header and hides unknown mentions"
#[test]
fn puts_origin_own_and_pinned_links_in_the_header_and_hides_unknown_mentions() {
    let with = |relationship: &str, enrichment: &str, dismissed: bool| {
        parse_wire_link(&wire(|w| {
            w["relationship"] = json!(relationship);
            w["enrichmentStatus"] = json!(enrichment);
            w["isDismissed"] = json!(dismissed);
        }))
    };
    assert!(is_header_link(&with("origin", "resolved", false)));
    assert!(!is_header_link(&with("mentioned", "resolved", false)));
    assert!(!is_visible_link(&with("mentioned", "not_found", false)));
    assert!(is_visible_link(&with("pinned", "not_found", false)));
    assert!(!is_visible_link(&with("own", "resolved", true)));
}

fn prompt_fixture() -> (Value, Value) {
    let f: Value = serde_json::from_str(&super::fixture("smart-link-prompts.json")).unwrap();
    (f["metadata"].clone(), f["expected"].clone())
}

fn prompt_link(metadata: &Value) -> SmartLink {
    parse_wire_link(&wire(|w| w["metadataJson"] = json!(metadata.to_string())))
}

// smart-links.test.ts: "includes captured failure logs when sending a check to the agent"
#[test]
fn includes_captured_failure_logs_when_sending_a_check_to_the_agent() {
    let failing = run("client-tests", "completed", Some("failure"));
    let link = with_checks(
        vec![failing.clone()],
        json!({"ciFailures": [{"sha": "abc1234def", "checkRunName": "client-tests", "checkRunId": 1, "conclusion": "failure", "htmlUrl": "https://ci.example.test/job", "logContent": "Expected 2, got 3", "detectedAt": ""}]}),
    );
    let prompt = format_check_failure_prompt(&link, &check_run(&failing));
    assert!(prompt.contains("[CI Failure — owner/repo PR #187]"));
    assert!(prompt.contains("Commit: abc1234"));
    assert!(prompt.contains("Expected 2, got 3"));
    assert!(prompt.contains("BEGIN UNTRUSTED CONTENT"));
}

#[test]
fn the_prompts_are_what_fleet_writes_byte_for_byte() {
    let (metadata, expected) = prompt_fixture();
    let link = prompt_link(&metadata);
    let runs = ci_status(&link).unwrap().check_runs;
    let threads = review_threads(&link);
    assert_eq!(format_check_failure_prompt(&link, &runs[0]), expected["failing"].as_str().unwrap());
    assert_eq!(
        format_check_failure_prompt(&link, &runs[1]),
        expected["timedOut"].as_str().unwrap(),
        "no captured failure: the run's own words"
    );
    assert_eq!(format_review_thread_prompt(&link, &threads[0]), expected["thread0"].as_str().unwrap());
    assert_eq!(format_review_thread_prompt(&link, &threads[1]), expected["thread1"].as_str().unwrap(), "no comment, no line");
    assert_eq!(format_fix_prompt(&link, &runs, &threads), expected["fix"].as_str().unwrap());
    let bare = parse_wire_link(&wire(|w| {
        w["metadataJson"] = json!(json!({"number": 5}).to_string());
        w["resourceId"] = json!("owner/repo#5");
    }));
    assert_eq!(format_check_failure_prompt(&bare, &check_run(&run("lint", "completed", None))), expected["bare"].as_str().unwrap());
}

// smart-links.test.ts: "formats how long ago GitHub was checked"
#[test]
fn formats_how_long_ago_github_was_checked() {
    let now = to_ms("2026-09-12T12:00:00Z");
    assert_eq!(format_checked_ago(Some("2026-09-12T11:59:58Z"), now, 0).as_deref(), Some("just now"));
    assert_eq!(format_checked_ago(Some("2026-09-12T11:59:48Z"), now, 0).as_deref(), Some("just now"));
    assert_eq!(format_checked_ago(Some("2026-09-12T11:56:00Z"), now, 0).as_deref(), Some("4 minutes ago"));
    assert_eq!(format_checked_ago(None, now, 0), None);
}

#[test]
fn the_ago_words_follow_the_browsers_for_hours_days_weeks_and_far_dates() {
    let now = to_ms("2026-09-12T12:00:00Z");
    let ago = |iso: &str| format_checked_ago(Some(iso), now, 0).unwrap();
    assert_eq!(ago("2026-09-12T11:00:00Z"), "1 hour ago");
    assert_eq!(ago("2026-09-12T09:00:00Z"), "3 hours ago");
    assert_eq!(ago("2026-09-11T10:00:00Z"), "yesterday");
    assert_eq!(ago("2026-09-09T12:00:00Z"), "3 days ago");
    assert_eq!(ago("2026-09-05T12:00:00Z"), "last week");
    assert_eq!(ago("2026-08-22T12:00:00Z"), "3 weeks ago");
    assert_eq!(ago("2026-06-01T12:00:00Z"), "Jun 1");
    assert_eq!(ago("2026-09-12T12:30:00Z"), "in 30 minutes");
    assert_eq!(format_checked_ago(Some("not a time"), now, 0), None);
}

fn to_ms(iso: &str) -> i64 {
    crate::fleet::work::parse_iso_ms(iso).unwrap()
}

#[test]
fn the_parts_the_views_draw_read_from_the_metadata() {
    let link = with_checks(
        vec![run("a", "completed", Some("success"))],
        json!({"additions": 412, "deletions": 88, "labels": [{"name": "bug", "color": "d73a4a"}, {"color": "ffffff"}, "junk"],
            "reviewers": [{"login": "sam-example", "state": "APPROVED"}], "reviewDecision": "APPROVED", "mergeable": false}),
    );
    assert_eq!(link_diff(&link), Some((412, 88)));
    assert_eq!(link_diff(&plain()), None);
    assert_eq!(link_labels(&link), [LinkLabel { name: "bug".into(), color: "d73a4a".into() }]);
    assert_eq!(link_reviewers(&link).len(), 1);
    assert!(has_merge_conflict(&link));
    let facts = link_pr_facts(&link);
    assert_eq!((facts.review, facts.conflict, facts.checks), (Some(ReviewDecision::Approved), true, Checks::Passing));
    assert_eq!(pr_words(&facts), "Conflicts");
}

// ─── the store, in memory ───────────────────────────────────────────────────

fn linked(id: &str, relationship: &str, change: impl FnOnce(&mut Value)) -> Value {
    wire(|w| {
        w["id"] = json!(id);
        w["url"] = json!(format!("https://github.com/o/r/pull/{id}"));
        w["resourceId"] = json!(format!("o/r#{id}"));
        w["title"] = json!(format!("o/r #{id}: Link {id}"));
        w["relationship"] = json!(relationship);
        change(w);
    })
}

fn ids(links: Vec<&SmartLink>) -> Vec<&str> {
    links.into_iter().map(|l| l.id.as_str()).collect()
}

// stores/smart-links.test.ts: "orders header links origin, own, pinned and leaves mentions out"
#[test]
fn orders_header_links_origin_own_pinned_and_leaves_mentions_out() {
    let mut store = SmartLinks::default();
    store.set_links(
        "s1",
        &[linked("1", "mentioned", |_| {}), linked("2", "pinned", |_| {}), linked("3", "own", |_| {}), linked("4", "origin", |_| {})],
    );
    assert_eq!(ids(store.header_links("s1")), ["4", "3", "2"]);
}

// stores/smart-links.test.ts: "applies pushed updates in place"
#[test]
fn applies_pushed_updates_in_place() {
    let mut store = SmartLinks::default();
    store.set_links("s1", &[linked("1", "mentioned", |_| {})]);
    store.upsert_link(&linked("1", "own", |w| (w["status"], w["isTerminal"]) = (json!("merged"), json!(true))));
    store.upsert_link(&linked("2", "mentioned", |_| {}));
    let seen: Vec<_> = store.visible_links("s1").iter().map(|l| (l.id.clone(), l.relationship, l.status.clone())).collect();
    assert_eq!(seen, [("1".into(), Relationship::Own, "merged".into()), ("2".into(), Relationship::Mentioned, "open".into())]);
}

// stores/smart-links.test.ts: "reports the newest check time"
#[test]
fn reports_the_newest_check_time() {
    let mut store = SmartLinks::default();
    store.set_links(
        "s1",
        &[
            linked("1", "own", |w| w["lastCheckedAt"] = json!("2026-09-12T10:00:00Z")),
            linked("2", "mentioned", |w| w["lastCheckedAt"] = json!("2026-09-12T11:00:00Z")),
        ],
    );
    assert_eq!(store.last_checked_at("s1").as_deref(), Some("2026-09-12T11:00:00Z"));
}

// stores/smart-links.test.ts: "picks the session's own open pull request before one it started from or pinned, and open before merged"
#[test]
fn picks_the_sessions_own_open_pull_request_before_one_it_started_from_or_pinned_and_open_before_merged() {
    let merged = |w: &mut Value| (w["status"], w["isTerminal"]) = (json!("merged"), json!(true));
    let mut store = SmartLinks::default();
    store.set_links(
        "s1",
        &[
            linked("1", "pinned", |_| {}),
            linked("2", "own", merged),
            linked("3", "origin", |_| {}),
            linked("4", "own", |_| {}),
            linked("5", "own", |w| {
                w["resourceType"] = json!("issue");
                w["url"] = json!("https://github.com/o/r/issues/5");
            }),
        ],
    );
    assert_eq!(store.session_pull_request("s1").map(|l| l.id.as_str()), Some("4"));
    store.set_links("s1", &[linked("1", "pinned", |_| {}), linked("2", "own", merged)]);
    assert_eq!(store.session_pull_request("s1").map(|l| l.id.as_str()), Some("1"));
}

#[test]
fn a_badge_waits_for_github_to_answer_and_says_what_the_pull_request_waits_on() {
    let mut store = SmartLinks::default();
    store.set_links("s1", &[linked("187", "own", |w| w["enrichmentStatus"] = json!("not_connected"))]);
    assert_eq!(store.pr_badge("s1"), None, "a link GitHub has not answered for has no badge");
    store.set_links(
        "s1",
        &[linked("187", "own", |w| w["metadataJson"] = json!(json!({"number": 187, "ci": {"headSha": "a", "checkRuns": [run("t", "completed", Some("failure")), run("b", "completed", Some("success"))]}}).to_string()))],
    );
    let badge = store.pr_badge("s1").unwrap();
    assert_eq!((badge.number, badge.state, badge.checks), (187, PrState::Blocked, Checks::Failing));
    assert_eq!(badge.description, "Pull request #187 · Checks failing");
    assert!(badge.shows_checks());
    assert_eq!(store.pr_badge("nobody"), None);
    let _: Option<PrBadge> = None;
}

// ─── the store and the events, on the wire ──────────────────────────────────

/// What the fake server saw and what it answers: one answer per `METHOD path` (the next `answer` replaces it), `0` closes
/// the connection without a word (a Fleet that is not there).
#[derive(Default)]
struct Fake {
    calls: Vec<(String, String, Value)>,
    answers: HashMap<String, (u16, String)>,
}

impl Fake {
    fn count(&self, method: &str, path: &str) -> usize {
        self.calls.iter().filter(|(m, p, _)| m == method && p == path).count()
    }
}

fn serve(mut stream: TcpStream, fake: Arc<Mutex<Fake>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let (mut head, mut length) = (String::new(), 0usize);
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        if line == "\r\n" {
            break;
        }
        if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            length = n.trim().parse().unwrap_or(0);
        }
        head.push_str(&line);
    }
    let mut raw = vec![0; length];
    let _ = reader.read_exact(&mut raw);
    let mut words = head.lines().next().unwrap_or("").split(' ');
    let (method, path) = (words.next().unwrap_or("").to_string(), words.next().unwrap_or("").to_string());
    let mut fake = fake.lock().unwrap();
    let key = format!("{method} {path}");
    let (code, body) = match fake.answers.get(&key) {
        Some(answer) => answer.clone(),
        _ if path.starts_with("/hubs/session-events/negotiate") => (200, "{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}".into()),
        _ => (404, "{}".into()),
    };
    fake.calls.push((method, path, serde_json::from_slice(&raw).unwrap_or(Value::Null)));
    if code == 0 {
        return;
    }
    let _ = stream.write_all(
        format!("HTTP/1.1 {code} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len())
            .as_bytes(),
    );
}

struct Live {
    rt: Runtime<State>,
    jobs: Receiver<Job>,
    fake: Arc<Mutex<Fake>>,
}

impl Live {
    fn start() -> Live {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let fake = Arc::new(Mutex::new(Fake::default()));
        let shared = fake.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let fake = shared.clone();
                std::thread::spawn(move || serve(stream, fake));
            }
        });
        let (tx, jobs) = channel::<Job>();
        let tx = Mutex::new(tx);
        let post = Post::new(move |job| {
            let _ = tx.lock().unwrap().send(job);
        });
        let mut rt = app();
        let link = Link::start(base, post);
        rt.update(move |s, _| s.link = Some(link));
        Live { rt, jobs, fake }
    }

    fn answer(&self, method: &str, path: &str, code: u16, body: Value) {
        self.fake.lock().unwrap().answers.insert(format!("{method} {path}"), (code, body.to_string()));
    }

    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        crate::test_wait::drive(&mut self.rt, &self.jobs, what, done, |s| {
            format!("{} header links, {} sessions", s.smart_links.header_by_session.len(), s.smart_links.by_session.len())
        });
    }

    /// Runs the app until the fake server has seen `n` calls of `method path` and their answers are applied.
    fn wait_calls(&mut self, method: &str, path: &str, n: usize) {
        let (fake, m, p) = (self.fake.clone(), method.to_string(), path.to_string());
        self.wait(&format!("{n} {method} {path}"), move |_| fake.lock().unwrap().count(&m, &p) >= n);
        // The answer is applied by a job queued after the call was counted: let the queue empty.
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(2));
            while let Ok(job) = self.jobs.try_recv() {
                self.rt.update(job);
            }
        }
    }

    fn count(&self, method: &str, path: &str) -> usize {
        self.fake.lock().unwrap().count(method, path)
    }
}

const ALL: &str = "/api/smart-links";

// stores/smart-links.test.ts: "loads a session's links once"
#[test]
fn loads_a_sessions_links_once() {
    let mut live = Live::start();
    live.answer("GET", "/api/sessions/s1/smart-links/all", 200, json!([linked("1", "own", |_| {})]));
    live.rt.update(|s, _| {
        s.ensure_links_loaded("s1");
        s.ensure_links_loaded("s1");
    });
    live.wait("the links", |s| s.smart_links.by_session.contains_key("s1"));
    live.rt.update(|s, _| s.ensure_links_loaded("s1"));
    live.wait_calls("GET", "/api/sessions/s1/smart-links/all", 1);
    assert_eq!(live.count("GET", "/api/sessions/s1/smart-links/all"), 1);
    assert_eq!(live.rt.state.smart_links.visible_links("s1").len(), 1);
}

// stores/smart-links.test.ts: "knows a session's links are loading, and why a load failed, until one succeeds"
#[test]
fn knows_a_sessions_links_are_loading_and_why_a_load_failed_until_one_succeeds() {
    let mut live = Live::start();
    let all = "/api/sessions/s1/smart-links/all";
    live.answer("GET", all, 503, json!({"error": "nope"}));
    live.rt.update(|s, _| s.ensure_links_loaded("s1"));
    assert!(live.rt.state.smart_links.is_loading("s1"));
    live.wait("the refusal", |s| !s.smart_links.is_loading("s1"));
    assert!(live.rt.state.smart_links.load_error("s1").unwrap().contains("503"));

    live.answer("GET", all, 0, Value::Null);
    live.rt.update(|s, _| s.ensure_links_loaded("s1"));
    live.wait("the failed try", |s| !s.smart_links.is_loading("s1") && s.smart_links.load_error("s1") == Some("Couldn't reach Fleet."));

    live.answer("GET", all, 200, json!([]));
    live.rt.update(|s, _| s.ensure_links_loaded("s1"));
    live.wait("the empty list", |s| s.smart_links.by_session.contains_key("s1"));
    assert!(!live.rt.state.smart_links.is_loading("s1"));
    assert_eq!(live.rt.state.smart_links.load_error("s1"), None);
    assert!(live.rt.state.smart_links.by_session["s1"].is_empty());
}

// stores/smart-links.test.ts: "pins optimistically and rolls back when the server refuses"
#[test]
fn pins_optimistically_and_rolls_back_when_the_server_refuses() {
    let mut live = Live::start();
    live.rt.update(|s, _| s.smart_links.set_links("s1", &[linked("1", "mentioned", |_| {})]));
    let relationship = |live: &Live| live.rt.state.smart_links.visible_links("s1")[0].relationship;

    live.answer("PATCH", "/api/sessions/s1/smart-links/1/pin", 204, Value::Null);
    live.rt.update(|s, _| s.set_link_pinned("s1", "1", true));
    assert_eq!(relationship(&live), Relationship::Pinned, "at once, before the server answers");
    live.wait_calls("PATCH", "/api/sessions/s1/smart-links/1/pin", 1);
    assert_eq!(relationship(&live), Relationship::Pinned);

    live.answer("PATCH", "/api/sessions/s1/smart-links/1/unpin", 500, Value::Null);
    live.rt.update(|s, _| s.set_link_pinned("s1", "1", false));
    assert_eq!(relationship(&live), Relationship::Mentioned, "unpinned at once");
    live.wait_calls("PATCH", "/api/sessions/s1/smart-links/1/unpin", 1);
    assert_eq!(relationship(&live), Relationship::Pinned, "put back when the server refuses");
}

#[test]
fn dismissing_hides_the_link_at_once_and_brings_it_back_when_the_server_refuses() {
    let mut live = Live::start();
    live.rt.update(|s, _| {
        s.smart_links.set_links("s1", &[linked("1", "own", |_| {})]);
        s.smart_links.header_links_loaded(&[linked("1", "own", |_| {})]);
    });
    live.answer("PATCH", "/api/sessions/s1/smart-links/1/dismiss", 500, Value::Null);
    live.rt.update(|s, _| s.dismiss_link("s1", "1"));
    assert!(live.rt.state.smart_links.visible_links("s1").is_empty());
    assert!(live.rt.state.smart_links.session_pull_request("s1").is_none(), "the header link goes too");
    live.wait_calls("PATCH", "/api/sessions/s1/smart-links/1/dismiss", 1);
    assert_eq!(live.rt.state.smart_links.visible_links("s1").len(), 1);
    assert!(live.rt.state.smart_links.session_pull_request("s1").is_some());
}

// stores/smart-links.test.ts: "explains a URL that isn't a GitHub pull request or issue"
#[test]
fn explains_a_url_that_isnt_a_github_pull_request_or_issue() {
    let mut live = Live::start();
    let answers: Arc<Mutex<Vec<Option<String>>>> = Arc::default();
    let post = |live: &mut Live, url: &'static str, n: usize| {
        let sink = answers.clone();
        live.rt.update(move |s, _| s.add_link("s1", url, move |_, _, message| sink.lock().unwrap().push(message)));
        let sink = answers.clone();
        live.wait("the answer", move |_| sink.lock().unwrap().len() >= n);
    };
    live.answer("POST", "/api/sessions/s1/smart-links", 400, json!("bad"));
    post(&mut live, "https://example.com", 1);
    live.answer("POST", "/api/sessions/s1/smart-links", 200, linked("9", "pinned", |_| {}));
    post(&mut live, "https://github.com/o/r/pull/9", 2);
    assert_eq!(*answers.lock().unwrap(), [Some("Paste a link to a GitHub pull request or issue.".to_string()), None]);
    assert_eq!(ids(live.rt.state.smart_links.header_links("s1")), ["9"]);
    let posted = live
        .fake
        .lock()
        .unwrap()
        .calls
        .iter()
        .filter(|c| c.0 == "POST" && c.1.ends_with("/smart-links"))
        .map(|c| c.2.clone())
        .collect::<Vec<_>>();
    assert_eq!(posted, [json!({"url": "https://example.com"}), json!({"url": "https://github.com/o/r/pull/9"})]);
}

#[test]
fn a_refusal_or_a_missing_fleet_says_which_when_attaching() {
    let mut live = Live::start();
    let said: Arc<Mutex<Vec<Option<String>>>> = Arc::default();
    for (code, n) in [(500, 1), (0, 2)] {
        live.answer("POST", "/api/sessions/s1/smart-links", code, Value::Null);
        let sink = said.clone();
        live.rt.update(move |s, _| s.add_link("s1", "https://github.com/o/r/pull/9", move |_, _, m| sink.lock().unwrap().push(m)));
        let sink = said.clone();
        live.wait("the answer", move |_| sink.lock().unwrap().len() >= n);
    }
    assert_eq!(
        *said.lock().unwrap(),
        [Some("Couldn't attach the link (HTTP 500).".to_string()), Some("Couldn't reach Fleet to attach the link.".to_string())]
    );
}

// stores/smart-links.test.ts: "loads every session's header links in one request and keeps them current from pushes"
#[test]
fn loads_every_sessions_header_links_in_one_request_and_keeps_them_current_from_pushes() {
    let mut live = Live::start();
    live.answer(
        "GET",
        ALL,
        200,
        json!([
            linked("1", "own", |w| w["sessionId"] = json!("s1")),
            linked("2", "origin", |w| {
                w["sessionId"] = json!("s2");
                w["resourceType"] = json!("issue");
                w["url"] = json!("https://github.com/o/r/issues/2");
            }),
        ]),
    );
    live.rt.update(|s, _| {
        s.ensure_header_links_loaded();
        s.ensure_header_links_loaded();
    });
    live.wait("the header links", |s| s.smart_links.header_loaded);
    live.rt.update(|s, _| s.ensure_header_links_loaded());
    assert_eq!(live.count("GET", ALL), 1);
    let id = |s: &State, session: &str| s.smart_links.session_pull_request(session).map(|l| l.id.clone());
    assert_eq!(id(&live.rt.state, "s1").as_deref(), Some("1"));
    assert_eq!(id(&live.rt.state, "s2"), None);

    // A pushed update for a session that was never opened lands in the header links only.
    live.rt.update(|s, _| s.smart_links.apply_pushed(&linked("3", "own", |w| w["sessionId"] = json!("s2"))));
    assert_eq!(id(&live.rt.state, "s2").as_deref(), Some("3"));
    assert!(!live.rt.state.smart_links.by_session.contains_key("s2"));
    live.rt.update(|s, _| {
        s.smart_links.apply_pushed(&linked("3", "own", |w| (w["sessionId"], w["isDismissed"]) = (json!("s2"), json!(true))))
    });
    assert_eq!(id(&live.rt.state, "s2"), None);
}

// stores/smart-links.test.ts: "keeps the badges and keeps applying pushes when the reload after a reconnect fails"
#[test]
fn keeps_the_badges_and_keeps_applying_pushes_when_the_reload_after_a_reconnect_fails() {
    let mut live = Live::start();
    live.answer("GET", ALL, 200, json!([linked("1", "own", |_| {})]));
    live.rt.update(|s, _| s.ensure_header_links_loaded());
    live.wait("the header links", |s| s.smart_links.header_loaded);
    let id = |s: &State| s.smart_links.session_pull_request("s1").map(|l| l.id.clone());

    // The socket is back but the server isn't answering yet.
    live.answer("GET", ALL, 503, json!({}));
    live.rt.update(|s, _| s.reload_header_links());
    live.wait_calls("GET", ALL, 2);
    assert_eq!(id(&live.rt.state).as_deref(), Some("1"));

    live.rt.update(|s, _| {
        s.smart_links.apply_pushed(&linked("1", "own", |w| (w["status"], w["isTerminal"]) = (json!("merged"), json!(true))))
    });
    assert_eq!(live.rt.state.smart_links.session_pull_request("s1").unwrap().status, "merged");

    live.answer("GET", ALL, 0, Value::Null);
    live.rt.update(|s, _| s.reload_header_links());
    live.wait_calls("GET", ALL, 3);
    live.rt.update(|s, _| s.smart_links.apply_pushed(&linked("2", "pinned", |_| {})));
    assert!(live.rt.state.smart_links.header_by_session["s1"].iter().any(|l| l.id == "2"));
}

// stores/smart-links.test.ts: "finds the sessions working on a pull request"
#[test]
fn finds_the_sessions_working_on_a_pull_request() {
    let mut store = SmartLinks::default();
    store.header_links_loaded(&[
        linked("7", "own", |w| (w["sessionId"], w["resourceId"]) = (json!("s1"), json!("O/R#7"))),
        linked("7", "pinned", |w| (w["id"], w["sessionId"]) = (json!("x"), json!("s2"))),
        linked("8", "own", |w| w["sessionId"] = json!("s3")),
    ]);
    assert_eq!(store.sessions_for("o/r#7"), ["s1", "s2"]);
    // Once a session's full list is loaded, it decides.
    store.set_links(
        "s2",
        &[linked("7", "pinned", |w| (w["id"], w["sessionId"], w["isDismissed"]) = (json!("x"), json!("s2"), json!(true)))],
    );
    assert_eq!(store.sessions_for("o/r#7"), ["s1"]);
}

#[test]
fn a_focus_request_grows_a_nonce_so_asking_twice_still_flashes() {
    let mut store = SmartLinks::default();
    store.request_focus("s1", "l1");
    let first = store.focus.clone().unwrap();
    store.request_focus("s1", "l1");
    let second = store.focus.clone().unwrap();
    assert_eq!((second.session_id.as_str(), second.target.as_str()), ("s1", "l1"));
    assert!(second.nonce > first.nonce);
}

#[test]
fn the_origin_records_are_read_from_the_session() {
    let records = read_origin_records(
        &json!([{"sourceType": "automation", "title": "Nightly", "summary": null, "actionId": "a1", "createdAt": "2026-09-12T00:00:00Z"}, "junk"]),
    );
    assert_eq!(
        records,
        [OriginRecord {
            source_type: "automation".into(),
            title: Some("Nightly".into()),
            summary: None,
            action_id: "a1".into(),
            created_at: "2026-09-12T00:00:00Z".into()
        }]
    );
    assert!(read_origin_records(&json!({"not": "a list"})).is_empty());
}

#[test]
fn the_origin_is_read_from_the_machine_the_session_is_on() {
    let mut live = Live::start();
    live.answer(
        "GET",
        "/api/sessions/s1/origin",
        200,
        json!([{"sourceType": "automation", "title": "Nightly", "actionId": "a1", "createdAt": ""}]),
    );
    live.rt.update(|s, _| s.load_origin_records("s1"));
    live.wait("the origin", |s| s.smart_links.origins.contains_key("s1"));
    assert_eq!(live.rt.state.smart_links.origins["s1"][0].title.as_deref(), Some("Nightly"));
}

// ─── use-smart-link-updates.test.ts ─────────────────────────────────────────

fn pushed(topic: &Topic, kind: Kind, payload: Value) -> impl FnOnce(&mut State, &mut rune::Tx) + Send + use<> {
    let (topic, event) = (topic.clone(), Event::Other(kind, payload));
    move |s, _| s.dispatch(&topic, &[event])
}

// use-smart-link-updates.test.ts: "loads every session's header links when mounted"
// (Fleet's composable runs when the app mounts; here the connection does it, at its start.)
#[test]
fn loads_every_sessions_header_links_when_connected() {
    let mut live = Live::start();
    live.answer("GET", ALL, 200, json!([linked("1", "own", |_| {})]));
    live.rt.update(|s, _| s.load_smart_links());
    live.wait("the header links", |s| s.smart_links.header_loaded);
    assert_eq!(live.count("GET", ALL), 1);
    assert!(live.rt.state.smart_links.session_pull_request("s1").is_some());
}

// use-smart-link-updates.test.ts: "applies a pushed link"
#[test]
fn applies_a_pushed_link() {
    let mut rt = app();
    rt.update(|s, _| s.smart_links.header_links_loaded(&[]));
    rt.update(pushed(&Topic::Sessions, Kind::SmartLinkUpdated, linked("7", "own", |_| {})));
    assert_eq!(rt.state.smart_links.session_pull_request("s1").map(|l| l.id.clone()).as_deref(), Some("7"));
    // …and a full list that is loaded takes it too.
    rt.update(|s, _| s.smart_links.set_links("s1", &[]));
    rt.update(pushed(&Topic::Sessions, Kind::SmartLinkUpdated, linked("8", "pinned", |_| {})));
    assert_eq!(ids(rt.state.smart_links.visible_links("s1")), ["8"]);
}

// use-smart-link-updates.test.ts: "ignores other events and links without an id or a session"
#[test]
fn ignores_other_events_and_links_without_an_id_or_a_session() {
    let mut rt = app();
    rt.update(|s, _| s.smart_links.header_links_loaded(&[]));
    let none = |rt: &Runtime<State>| rt.state.smart_links.header_by_session.is_empty();
    rt.update(pushed(&Topic::Sessions, Kind::SessionTokens, json!({"id": "l1", "sessionId": "s1"})));
    rt.update(pushed(&Topic::Sessions, Kind::SmartLinkUpdated, json!({"sessionId": "s1"})));
    rt.update(pushed(&Topic::Sessions, Kind::SmartLinkUpdated, json!({"id": "l1"})));
    rt.update(pushed(&Topic::Sessions, Kind::SmartLinkUpdated, Value::Null));
    assert!(none(&rt));
}

// use-smart-link-updates.test.ts: "loads the links again after a reconnect"
#[test]
fn loads_the_links_again_after_a_reconnect() {
    let mut live = Live::start();
    live.answer("GET", ALL, 200, json!([linked("1", "own", |_| {})]));
    live.rt.update(|s, _| s.on_reconnected());
    live.wait("the header links", |s| s.smart_links.header_loaded);
    assert_eq!(live.count("GET", ALL), 1);
}

// use-smart-link-updates.test.ts: "stops listening when unmounted"
// (The app never unmounts; what it listens to is the `sessions` topic and nothing else.)
#[test]
fn listens_to_the_sessions_topic_and_to_no_other() {
    let mut rt = app();
    rt.update(|s, _| s.smart_links.header_links_loaded(&[]));
    rt.update(pushed(&Topic::Session("s1".into()), Kind::SmartLinkUpdated, linked("7", "own", |_| {})));
    assert!(rt.state.smart_links.header_by_session.is_empty());
}

// ─── the session list's badge, from links ───────────────────────────────────

#[test]
fn the_rows_badges_come_out_of_the_links_the_same_as_the_ones_written_on_the_rows() {
    let project = crate::mock::rows::chips();
    let mut store = SmartLinks::default();
    store.header_links_loaded(&crate::mock::links::chips_links(&project));
    let mut checked = 0;
    for session in project.sessions.iter().filter(|s| s.row.pr.is_some()) {
        assert_eq!(store.pr_badge(&session.id), session.row.pr, "{}", session.title);
        checked += 1;
    }
    assert_eq!(checked, 7);
}

#[test]
fn a_link_a_real_server_has_not_been_able_to_check_reads_and_has_no_badge() {
    // Captured from a scratch Fleet v0.72.0 with no GitHub connected (an invented repository).
    let wire = json!({"id": "985d20f5-fa72-4c95-adad-02ae2ee6940d", "sessionId": "daed022c", "url": "https://github.com/example/demo/pull/187",
        "providerId": "github", "resourceType": "pull_request", "resourceId": "example/demo#187", "title": "example/demo#187", "status": "",
        "statusLabel": "", "metadataJson": null, "isDismissed": false, "isTerminal": false, "createdAt": "2026-11-16T09:32:37.1878690Z",
        "updatedAt": "2026-11-16T09:32:37.1920537Z", "relationship": "pinned", "enrichmentStatus": "not_connected",
        "lastCheckedAt": "2026-11-16T09:32:37.1920537Z"});
    let link = parse_wire_link(&wire);
    assert_eq!(
        (link_number(&link), link_title(&link), link.enrichment),
        (Some(187), "Pull request #187".to_string(), Enrichment::NotConnected)
    );
    let mut store = SmartLinks::default();
    store.header_links_loaded(&[wire]);
    assert_eq!(store.session_pull_request("daed022c").map(|l| l.id.as_str()), Some("985d20f5-fa72-4c95-adad-02ae2ee6940d"));
    assert_eq!(store.pr_badge("daed022c"), None);
}
