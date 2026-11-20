//! Pull requests and issues as the GitHub pages show them (`lib/github-items.ts`): list rows (`ItemSummary`) and a page
//! (`ItemDetail`), read from Fleet's GitHub answers, and the words and facts made from them. Pure: no Rune, no I/O.
//!
//! The types are read from `serde_json::Value`, as `src/fleet/` does: a field the server adds is ignored, one it leaves out is
//! the empty value, and reviewers are read the one way (`parse_reviewers`) wherever they appear.
use super::where_::{Kind, Where};
use crate::fleet::new_session::request::GitHubPreset;
use crate::fleet::pr_state::{
    CheckCounts, Lifecycle, PrFacts, PrState, Reviewer, checks_from_counts, checks_from_rollup, parse_reviewers, pr_state,
    pr_words, review_decision,
};
use crate::fleet::smart_links::{
    SmartLink, SummaryState, is_pull_request, link_diff, link_href, link_labels, link_number, link_reviewers, link_title, summarize_checks,
};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemLabel {
    pub name: String,
    pub color: String,
}

/// `GitHubItemSummary`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemSummary {
    pub kind: Kind,
    pub owner: String,
    pub repo: String,
    pub number: u64,
    pub title: String,
    pub url: String,
    /// open, closed or merged.
    pub state: String,
    pub is_draft: bool,
    pub author: Option<String>,
    pub author_avatar_url: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub comments: u64,
    pub labels: Vec<ItemLabel>,
    pub head_ref: Option<String>,
    pub base_ref: Option<String>,
    pub additions: Option<i64>,
    pub deletions: Option<i64>,
    /// success, failure, pending or none; `None` for issues.
    pub checks: Option<String>,
    pub review_decision: Option<String>,
    /// MERGEABLE, CONFLICTING or UNKNOWN.
    pub mergeable: Option<String>,
    pub reviewers: Vec<Reviewer>,
    pub assignees: Vec<String>,
}

/// `GitHubItemPage`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemPage {
    pub items: Vec<ItemSummary>,
    pub total_count: u64,
    pub end_cursor: Option<String>,
    pub has_next_page: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckResult {
    Success,
    Failure,
    Pending,
    Skipped,
    Neutral,
}

/// `GitHubCheck`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub name: String,
    pub state: CheckResult,
    pub url: Option<String>,
    pub workflow_name: Option<String>,
    pub check_run_id: Option<i64>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimelineKind {
    Comment,
    Review,
    Merged,
    Closed,
    Reopened,
    Ready,
    Draft,
    Referenced,
}

/// The item a "referenced" entry points at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    pub kind: Kind,
    pub repository: String,
    pub number: u64,
}

/// `GitHubTimelineEntry`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelineEntry {
    pub kind: TimelineKind,
    pub author: Option<String>,
    pub author_avatar_url: Option<String>,
    pub created_at: String,
    pub body: Option<String>,
    /// A review's verdict, or the referencing item's state.
    pub state: Option<String>,
    pub url: Option<String>,
    pub reference: Option<Reference>,
}

/// `GitHubItemDetail`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemDetail {
    pub summary: ItemSummary,
    pub body: Option<String>,
    pub changed_files: Option<i64>,
    pub merged_at: Option<String>,
    pub merged_by: Option<String>,
    pub checks: Vec<Check>,
    pub unresolved_threads: u32,
    pub timeline: Vec<TimelineEntry>,
}

/// `GitHubRepoCounts`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoCounts {
    pub full_name: String,
    pub open_pull_requests: u64,
    pub open_issues: u64,
}

/// `GitHubWork`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Work {
    pub login: Option<String>,
    pub avatar_url: Option<String>,
    pub review_requested: Vec<ItemSummary>,
    pub authored: Vec<ItemSummary>,
    pub assigned: Vec<ItemSummary>,
    pub repos: Vec<RepoCounts>,
}

// ─── reading ────────────────────────────────────────────────────────────────

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn opt_text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn whole(v: &Value, key: &str) -> Option<i64> {
    v.get(key).and_then(Value::as_f64).map(|n| n as i64)
}

fn count(v: &Value, key: &str) -> u64 {
    whole(v, key).map_or(0, |n| n.max(0) as u64)
}

fn kind_of(word: Option<&str>) -> Kind {
    if word == Some("pull") { Kind::Pull } else { Kind::Issue }
}

fn list<'a>(v: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    v.get(key).and_then(Value::as_array).into_iter().flatten()
}

impl ItemSummary {
    pub fn read(v: &Value) -> ItemSummary {
        ItemSummary {
            kind: kind_of(v.get("kind").and_then(Value::as_str)),
            owner: text(v, "owner"),
            repo: text(v, "repo"),
            number: count(v, "number"),
            title: text(v, "title"),
            url: text(v, "url"),
            state: text(v, "state"),
            is_draft: v.get("isDraft").and_then(Value::as_bool).unwrap_or(false),
            author: opt_text(v, "author"),
            author_avatar_url: opt_text(v, "authorAvatarUrl"),
            created_at: text(v, "createdAt"),
            updated_at: text(v, "updatedAt"),
            comments: count(v, "comments"),
            labels: list(v, "labels")
                .filter_map(|l| Some(ItemLabel { name: l.get("name")?.as_str()?.to_string(), color: text(l, "color") }))
                .collect(),
            head_ref: opt_text(v, "headRef"),
            base_ref: opt_text(v, "baseRef"),
            additions: whole(v, "additions"),
            deletions: whole(v, "deletions"),
            checks: opt_text(v, "checks"),
            review_decision: opt_text(v, "reviewDecision"),
            mergeable: opt_text(v, "mergeable"),
            reviewers: parse_reviewers(v.get("reviewers")),
            assignees: list(v, "assignees").filter_map(|a| a.as_str().map(str::to_string)).collect(),
        }
    }
}

impl ItemPage {
    pub fn read(v: &Value) -> ItemPage {
        ItemPage {
            items: list(v, "items").map(ItemSummary::read).collect(),
            total_count: count(v, "totalCount"),
            end_cursor: opt_text(v, "endCursor"),
            has_next_page: v.get("hasNextPage").and_then(Value::as_bool).unwrap_or(false),
        }
    }
}

/// The items of a by-number answer (`…/items?numbers=`), a bare list.
pub fn read_items(v: &Value) -> Vec<ItemSummary> {
    v.as_array().into_iter().flatten().map(ItemSummary::read).collect()
}

impl Check {
    fn read(v: &Value) -> Check {
        let state = match v.get("state").and_then(Value::as_str) {
            Some("success") => CheckResult::Success,
            Some("failure") => CheckResult::Failure,
            Some("skipped") => CheckResult::Skipped,
            Some("neutral") => CheckResult::Neutral,
            _ => CheckResult::Pending,
        };
        Check {
            name: text(v, "name"),
            state,
            url: opt_text(v, "url"),
            workflow_name: opt_text(v, "workflowName"),
            check_run_id: whole(v, "checkRunId"),
            started_at: opt_text(v, "startedAt"),
            completed_at: opt_text(v, "completedAt"),
        }
    }
}

impl TimelineEntry {
    /// `None` for a kind this app doesn't know, so a newer server's entry is left out rather than drawn wrong.
    fn read(v: &Value) -> Option<TimelineEntry> {
        let kind = match v.get("kind").and_then(Value::as_str)? {
            "comment" => TimelineKind::Comment,
            "review" => TimelineKind::Review,
            "merged" => TimelineKind::Merged,
            "closed" => TimelineKind::Closed,
            "reopened" => TimelineKind::Reopened,
            "ready" => TimelineKind::Ready,
            "draft" => TimelineKind::Draft,
            "referenced" => TimelineKind::Referenced,
            _ => return None,
        };
        let reference = v.get("reference").filter(|r| r.is_object()).map(|r| Reference {
            kind: kind_of(r.get("kind").and_then(Value::as_str)),
            repository: text(r, "repository"),
            number: count(r, "number"),
        });
        Some(TimelineEntry {
            kind,
            author: opt_text(v, "author"),
            author_avatar_url: opt_text(v, "authorAvatarUrl"),
            created_at: text(v, "createdAt"),
            body: opt_text(v, "body"),
            state: opt_text(v, "state"),
            url: opt_text(v, "url"),
            reference,
        })
    }
}

impl ItemDetail {
    pub fn read(v: &Value) -> ItemDetail {
        ItemDetail {
            summary: ItemSummary::read(v.get("summary").unwrap_or(&Value::Null)),
            body: opt_text(v, "body"),
            changed_files: whole(v, "changedFiles"),
            merged_at: opt_text(v, "mergedAt"),
            merged_by: opt_text(v, "mergedBy"),
            checks: list(v, "checks").map(Check::read).collect(),
            unresolved_threads: count(v, "unresolvedThreads") as u32,
            timeline: list(v, "timeline").filter_map(TimelineEntry::read).collect(),
        }
    }
}

impl Work {
    pub fn read(v: &Value) -> Work {
        let items = |key: &str| list(v, key).map(ItemSummary::read).collect();
        Work {
            login: opt_text(v, "login"),
            avatar_url: opt_text(v, "avatarUrl"),
            review_requested: items("reviewRequested"),
            authored: items("authored"),
            assigned: items("assigned"),
            repos: list(v, "repos")
                .map(|r| RepoCounts {
                    full_name: text(r, "fullName"),
                    open_pull_requests: count(r, "openPullRequests"),
                    open_issues: count(r, "openIssues"),
                })
                .collect(),
        }
    }
}

/// `getJson`'s failure message: 401 sends the user to Settings, else the server's `error`, else the status.
pub fn error_words(status: u16, body: &str) -> String {
    if status == 401 {
        return "Connect GitHub in Settings to see this.".into();
    }
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| format!("GitHub didn't answer (HTTP {status})."))
}

// ─── words and facts ────────────────────────────────────────────────────────

/// A body as GitHub shows it: HTML comments (bots leave notes in them) are hidden there, so they are here.
pub fn visible_body(body: Option<&str>) -> String {
    let mut rest = body.unwrap_or_default();
    let mut out = String::new();
    while let Some(open) = rest.find("<!--") {
        let Some(close) = rest[open + 4..].find("-->") else { break };
        out.push_str(&rest[..open]);
        rest = &rest[open + 4 + close + 3..];
    }
    out.push_str(rest);
    out.trim().to_string()
}

/// `owner/repo#123`, the key smart links use.
pub fn item_resource_id(item: &ItemSummary) -> String {
    format!("{}/{}#{}", item.owner, item.repo, item.number)
}

/// Skipped and neutral checks count as passed.
pub fn check_counts(checks: &[Check]) -> CheckCounts {
    let n = |f: fn(CheckResult) -> bool| checks.iter().filter(|c| f(c.state)).count() as u32;
    CheckCounts {
        passed: n(|s| matches!(s, CheckResult::Success | CheckResult::Skipped | CheckResult::Neutral)),
        failing: n(|s| s == CheckResult::Failure),
        pending: n(|s| s == CheckResult::Pending),
    }
}

/// What the pull request state language needs from a list row, or from a page when it has checks.
pub fn item_pr_facts(item: &ItemSummary, detail: Option<&ItemDetail>) -> PrFacts {
    let counts = detail.filter(|d| !d.checks.is_empty()).map(|d| check_counts(&d.checks));
    let decision = item.review_decision.as_ref().map(|d| Value::String(d.clone()));
    PrFacts {
        state: match item.state.as_str() {
            "merged" => Lifecycle::Merged,
            "closed" => Lifecycle::Closed,
            _ => Lifecycle::Open,
        },
        draft: item.is_draft,
        checks: counts.map_or_else(|| checks_from_rollup(item.checks.as_deref()), checks_from_counts),
        conflict: item.mergeable.as_deref() == Some("CONFLICTING"),
        review: review_decision(decision.as_ref()),
        unresolved_threads: detail.map_or(0, |d| d.unresolved_threads),
        check_counts: counts,
    }
}

/// What "Start session" hands the New session page.
pub fn item_session_preset(item: &ItemSummary, body: Option<&str>) -> GitHubPreset {
    let pull = item.kind == Kind::Pull;
    GitHubPreset {
        source_type: if pull { "github-pull-request" } else { "github-issue" }.into(),
        owner: item.owner.clone(),
        repo: item.repo.clone(),
        number: item.number,
        title: item.title.clone(),
        body: body.map(str::to_string),
        html_url: item.url.clone(),
        repo_full_name: format!("{}/{}", item.owner, item.repo),
        suggested_branch: if pull { item.head_ref.clone() } else { None },
    }
}

/// A session's link as a list row, for "In Fleet": the watcher already keeps its state, checks and reviews, so the row needs
/// no request to GitHub. `None` when the link has no number, owner or repository.
pub fn summary_from_link(link: &SmartLink) -> Option<ItemSummary> {
    let number = link_number(link)?;
    let owner = link.metadata.get("owner")?.as_str()?.to_string();
    let repo = link.metadata.get("repo")?.as_str()?.to_string();
    let pull = is_pull_request(link);
    let checks = summarize_checks(link);
    let diff = link_diff(link);
    let meta = |key: &str| link.metadata.get(key).and_then(Value::as_str).map(str::to_string);
    let draft = link.status == "draft";
    Some(ItemSummary {
        kind: if pull { Kind::Pull } else { Kind::Issue },
        owner,
        repo,
        number,
        title: link_title(link),
        url: link_href(link),
        state: if draft || link.status.is_empty() { "open".into() } else { link.status.clone() },
        is_draft: draft,
        author: meta("author"),
        author_avatar_url: meta("authorAvatarUrl"),
        created_at: link.created_at.clone(),
        // When GitHub last saw a change, not when Fleet last checked.
        updated_at: meta("updatedAt").unwrap_or_else(|| link.updated_at.clone()),
        comments: 0,
        labels: link_labels(link).into_iter().map(|l| ItemLabel { name: l.name, color: l.color }).collect(),
        head_ref: meta("headRef"),
        base_ref: meta("baseRef"),
        additions: diff.map(|d| d.0),
        deletions: diff.map(|d| d.1),
        checks: pull.then(|| {
            match checks.state {
                SummaryState::Failing => "failure",
                SummaryState::Running => "pending",
                SummaryState::Passed => "success",
                SummaryState::None => "none",
            }
            .to_string()
        }),
        review_decision: meta("reviewDecision"),
        mergeable: match link.metadata.get("mergeable").and_then(Value::as_bool) {
            Some(false) => Some("CONFLICTING".into()),
            Some(true) => Some("MERGEABLE".into()),
            None => None,
        },
        reviewers: link_reviewers(link),
        assignees: Vec::new(),
    })
}

/// A pull request that needs the user, and why: "Review requested", "Checks failing", "Changes requested".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeedsYou {
    pub item: ItemSummary,
    pub reason: String,
}

/// Pull requests waiting on the user's review, then their own that something blocks (a failing check, a conflict, requested
/// changes).
pub fn needs_you(work: &Work) -> Vec<NeedsYou> {
    let asked = work.review_requested.iter().map(|item| NeedsYou { item: item.clone(), reason: "Review requested".into() });
    let blocked = work
        .authored
        .iter()
        .filter(|item| pr_state(&item_pr_facts(item, None)) == PrState::Blocked)
        .map(|item| NeedsYou { item: item.clone(), reason: pr_words(&item_pr_facts(item, None)) });
    asked.chain(blocked).collect()
}

/// The user's open pull requests that nothing blocks.
pub fn your_pull_requests(work: &Work) -> Vec<ItemSummary> {
    work.authored.iter().filter(|item| pr_state(&item_pr_facts(item, None)) != PrState::Blocked).cloned().collect()
}

impl Where {
    /// `itemRoute`: the page for a pull request or an issue.
    pub fn of_item(item: &ItemSummary) -> Where {
        Where::Item { owner: item.owner.clone(), repo: item.repo.clone(), kind: item.kind, number: item.number }
    }
}
