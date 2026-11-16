//! What the Context tab's pull request card says (`SmartLinkCard.vue`, rows 12.12 and 12.13 of
//! docs/parity/08-session-header-and-context.md): the pill, the meta line and the signals, worked out from the link so the
//! view only draws them. Plain data: no Rune, no I/O. Which signals are open is `ContextTab`'s (`checks_open`, `review_open`).
use super::pr_state::PrState;
use super::pr_state::{CheckCounts, Reviewer, ReviewerState, pr_state, pr_state_label, pr_words};
use super::smart_links::{
    CheckRun, CheckState, CheckSummary, Enrichment, LinkLabel, ReviewThread, SmartLink, SummaryState, check_state, ci_status,
    has_merge_conflict, is_pull_request, link_diff, link_href, link_labels, link_number, link_pr_facts, link_reviewers, link_title,
    needs_attention, review_threads, summarize_checks,
};
use serde_json::Value;

/// The state pill: for a pull request the words say what it waits on while it is open; an issue shows GitHub's own label.
#[derive(Clone, Debug, PartialEq)]
pub struct Pill {
    pub pull: bool,
    pub state: PrState,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub number: String,
    pub title: String,
    pub href: String,
    pub pill: Option<Pill>,
    /// Shown instead of the pill while GitHub has not answered.
    pub note: Option<&'static str>,
    /// `head → base`, for an open pull request.
    pub branch: Option<(String, String)>,
    pub diff: Option<(i64, i64)>,
    pub labels: Vec<LinkLabel>,
    pub merged: bool,
    /// An open, resolved pull request: the only kind with signals.
    pub open_pr: bool,
    pub checks: CheckSummary,
    pub counts: Option<CheckCounts>,
    pub runs: Vec<CheckRun>,
    pub reviewers: Vec<Reviewer>,
    pub threads: Vec<ReviewThread>,
    pub conflict: bool,
    pub base_ref: Option<String>,
    pub attention: bool,
}

fn string_meta(link: &SmartLink, key: &str) -> Option<String> {
    link.metadata.get(key).and_then(Value::as_str).map(str::to_string)
}

/// `pendingNote`.
pub fn pending_note(status: Enrichment) -> Option<&'static str> {
    match status {
        Enrichment::Pending => Some("Checking GitHub\u{2026}"),
        Enrichment::NotConnected => Some("Connect GitHub to see status"),
        Enrichment::NotFound => Some("GitHub couldn't find this"),
        Enrichment::Error => Some("Couldn't check GitHub"),
        Enrichment::Resolved => None,
    }
}

/// "Changes requested by sarah", "Approved by tvdb", "Waiting for kim", else "2 reviewers".
pub fn review_summary(reviewers: &[Reviewer]) -> String {
    let by = |state| reviewers.iter().filter(|r| r.state == state).map(|r| r.login.as_str()).collect::<Vec<_>>().join(", ");
    for (state, words) in [
        (ReviewerState::ChangesRequested, "Changes requested by"),
        (ReviewerState::Approved, "Approved by"),
        (ReviewerState::Requested, "Waiting for"),
    ] {
        let names = by(state);
        if !names.is_empty() {
            return format!("{words} {names}");
        }
    }
    format!("{} reviewer{}", reviewers.len(), if reviewers.len() == 1 { "" } else { "s" })
}

impl Card {
    pub fn of(link: &SmartLink) -> Card {
        let pull = is_pull_request(link);
        let resolved = link.enrichment == Enrichment::Resolved;
        let open_pr = pull && resolved && !link.is_terminal;
        let facts = link_pr_facts(link);
        let pill = (resolved && !link.status_label.is_empty()).then(|| {
            if !pull {
                let state = if link.status == "closed" { PrState::Closed } else { PrState::Open };
                return Pill { pull: false, state, label: link.status_label.clone() };
            }
            let state = pr_state(&facts);
            Pill { pull: true, state, label: if state == PrState::Open { pr_words(&facts) } else { pr_state_label(state).into() } }
        });
        let head = string_meta(link, "headRef");
        let base = string_meta(link, "baseRef");
        Card {
            number: link_number(link).map_or("null".to_string(), |n| n.to_string()),
            title: link_title(link),
            href: link_href(link),
            note: if pill.is_some() { None } else { pending_note(link.enrichment) },
            pill,
            branch: open_pr.then(|| head.zip(base.clone())).flatten(),
            diff: link_diff(link).filter(|_| open_pr),
            labels: link_labels(link),
            merged: pull && resolved && link.status == "merged",
            open_pr,
            checks: summarize_checks(link),
            counts: facts.check_counts,
            runs: ci_status(link).map(|c| c.check_runs).unwrap_or_default(),
            reviewers: link_reviewers(link),
            threads: review_threads(link),
            conflict: has_merge_conflict(link),
            base_ref: base,
            attention: needs_attention(link),
        }
    }

    /// The Checks signal is open to begin with when something fails.
    pub fn checks_open_by_default(&self) -> bool {
        self.checks.state == SummaryState::Failing
    }

    /// Whether a run is listed with the Send to agent button.
    pub fn is_failing(run: &CheckRun) -> bool {
        check_state(run) == CheckState::Failing
    }
}
