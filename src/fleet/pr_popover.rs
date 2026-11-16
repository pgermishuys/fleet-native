//! What the pull request pill's popover says (`PullRequestPill.vue`, rows 11.3 to 11.8): worked out from the session's pull
//! request link, so the view only draws it. Plain data: no Rune, no I/O.
use super::pr_state::{CheckCounts, Lifecycle, PrState, ReviewDecision, Reviewer, ReviewerState, block_reasons, pr_state, pr_state_label};
use super::smart_links::{
    CheckRun, CheckState, ReviewThread, SmartLink, check_state, ci_status, link_diff, link_href, link_number, link_pr_facts,
    link_reviewers, link_title, review_threads,
};
use serde_json::Value;

/// How many rows of checks and threads the lists show.
pub const MAX_RUNS: usize = 4;
pub const MAX_THREADS: usize = 3;

/// What stands at the right of a listed check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunEnd {
    /// A failing check has the Send to agent button.
    Send,
    /// "running" or the conclusion.
    Word(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListedRun {
    pub run: CheckRun,
    pub state: CheckState,
    pub end: RunEnd,
}

/// The footer's first button.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Primary {
    Archive,
    /// "Fix failing checks", "Address review" or "Fix checks and address review".
    Fix(&'static str),
    MergeOnGithub,
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrPopover {
    pub link: SmartLink,
    pub state: PrState,
    pub label: &'static str,
    pub repo: String,
    pub number: String,
    pub title: String,
    pub href: String,
    pub diff: Option<(i64, i64)>,
    pub head_ref: Option<String>,
    pub base_ref: String,
    pub reviewers: Vec<Reviewer>,
    pub merged: bool,
    pub open: bool,
    /// The checks bar's counts: the section shows when the pull request has any check at all.
    pub counts: Option<CheckCounts>,
    pub has_runs: bool,
    pub listed: Vec<ListedRun>,
    pub failing: Vec<CheckRun>,
    /// Every open thread (the list shows the first three, "Fix" sends them all).
    pub threads: Vec<ReviewThread>,
    pub changes_by: Vec<String>,
    pub conflict: bool,
    pub primary: Primary,
}

fn string_meta(link: &SmartLink, key: &str) -> Option<String> {
    link.metadata.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

impl PrPopover {
    pub fn of(link: &SmartLink) -> PrPopover {
        let facts = link_pr_facts(link);
        let state = pr_state(&facts);
        let runs = ci_status(link).map(|c| c.check_runs).unwrap_or_default();
        let failing: Vec<CheckRun> = runs.iter().filter(|r| check_state(r) == CheckState::Failing).cloned().collect();
        // The bar counts every check; the list is only the ones still running or failing.
        let listed = runs
            .iter()
            .filter(|r| matches!(check_state(r), CheckState::Failing | CheckState::Running))
            .take(MAX_RUNS)
            .map(|r| {
                let st = check_state(r);
                let end = if st == CheckState::Failing {
                    RunEnd::Send
                } else if st == CheckState::Running {
                    RunEnd::Word("running".into())
                } else {
                    RunEnd::Word(r.conclusion.clone().unwrap_or_default())
                };
                ListedRun { run: r.clone(), state: st, end }
            })
            .collect();
        let threads = review_threads(link);
        let reviewers = link_reviewers(link);
        let changes_by = reviewers.iter().filter(|r| r.state == ReviewerState::ChangesRequested).map(|r| r.login.clone()).collect();
        let open = facts.state == Lifecycle::Open;
        let merged = facts.state == Lifecycle::Merged;
        let fix = match (!failing.is_empty(), !threads.is_empty()) {
            (true, true) => Some("Fix checks and address review"),
            (true, false) => Some("Fix failing checks"),
            (false, true) => Some("Address review"),
            _ => None,
        };
        let primary = if merged {
            Primary::Archive
        } else if let (true, Some(label)) = (open, fix) {
            Primary::Fix(label)
        } else if open && block_reasons(&facts).is_empty() && facts.review == Some(ReviewDecision::Approved) {
            Primary::MergeOnGithub
        } else {
            Primary::None
        };
        PrPopover {
            state,
            label: pr_state_label(state),
            repo: string_meta(link, "repo").unwrap_or_default(),
            number: link_number(link).map(|n| n.to_string()).unwrap_or_default(),
            title: link_title(link),
            href: link_href(link),
            diff: link_diff(link),
            head_ref: string_meta(link, "headRef"),
            base_ref: string_meta(link, "baseRef").unwrap_or_else(|| "main".into()),
            reviewers,
            merged,
            open,
            counts: facts.check_counts,
            has_runs: !runs.is_empty(),
            listed,
            failing,
            threads,
            changes_by,
            conflict: facts.conflict,
            primary,
            link: link.clone(),
        }
    }

    /// "<names> requested changes" or "N unresolved thread(s)", when the review section shows at all.
    pub fn review_line(&self) -> Option<String> {
        if !self.changes_by.is_empty() {
            return Some(format!("{} requested changes", self.changes_by.join(", ")));
        }
        let n = self.threads.len();
        (n > 0).then(|| format!("{n} unresolved thread{}", if n == 1 { "" } else { "s" }))
    }

    pub fn shows_review(&self) -> bool {
        !self.threads.is_empty() || !self.changes_by.is_empty()
    }
}
