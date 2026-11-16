//! One way to say what state a pull request is in (`lib/pr-state.ts`), used by every GitHub surface: the session list's
//! badge, the header pill, the Context card and (later) the GitHub pages. Each surface turns what it knows into
//! `PrFacts`; the state, the colour and the words come from here. Pure: no Rune, no I/O.
use serde_json::Value;

/// The colour a pull request shows in. `Blocked` is an open pull request something stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrState {
    Open,
    Draft,
    Blocked,
    Merged,
    Closed,
}

/// `ChecksState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Checks {
    None,
    Passing,
    Failing,
    Pending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewDecision {
    Approved,
    ChangesRequested,
    ReviewRequired,
}

/// GitHub's own state; a draft is `Open` with `draft` set on the facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    Open,
    Closed,
    Merged,
}

/// What stops an open pull request, in the order the words list them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockReason {
    Checks,
    Conflict,
    Changes,
    Threads,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CheckCounts {
    pub passed: u32,
    pub failing: u32,
    pub pending: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrFacts {
    pub state: Lifecycle,
    pub draft: bool,
    pub checks: Checks,
    pub conflict: bool,
    pub review: Option<ReviewDecision>,
    pub unresolved_threads: u32,
    /// Check counts, when known, for "1 failing" and "3 of 5 checks done".
    pub check_counts: Option<CheckCounts>,
}

impl Default for PrFacts {
    fn default() -> Self {
        PrFacts {
            state: Lifecycle::Open,
            draft: false,
            checks: Checks::Passing,
            conflict: false,
            review: None,
            unresolved_threads: 0,
            check_counts: None,
        }
    }
}

pub fn block_reasons(facts: &PrFacts) -> Vec<BlockReason> {
    if facts.state != Lifecycle::Open {
        return Vec::new();
    }
    let mut reasons = Vec::new();
    if facts.checks == Checks::Failing {
        reasons.push(BlockReason::Checks);
    }
    if facts.conflict {
        reasons.push(BlockReason::Conflict);
    }
    if facts.review == Some(ReviewDecision::ChangesRequested) {
        reasons.push(BlockReason::Changes);
    }
    if facts.unresolved_threads > 0 {
        reasons.push(BlockReason::Threads);
    }
    reasons
}

pub fn pr_state(facts: &PrFacts) -> PrState {
    match facts.state {
        Lifecycle::Merged => PrState::Merged,
        Lifecycle::Closed => PrState::Closed,
        // A draft isn't asking for anything yet, so it stays grey whatever its checks say.
        Lifecycle::Open if facts.draft => PrState::Draft,
        Lifecycle::Open if !block_reasons(facts).is_empty() => PrState::Blocked,
        Lifecycle::Open => PrState::Open,
    }
}

fn plural(count: u32, word: &str) -> String {
    format!("{count} {word}{}", if count == 1 { "" } else { "s" })
}

fn reason_words(reason: BlockReason, facts: &PrFacts) -> String {
    match reason {
        BlockReason::Checks => match facts.check_counts.map_or(0, |c| c.failing) {
            0 => "Checks failing".into(),
            failing => format!("{failing} failing"),
        },
        BlockReason::Conflict => "Conflicts".into(),
        BlockReason::Changes => "Changes requested".into(),
        BlockReason::Threads => plural(facts.unresolved_threads, "thread"),
    }
}

/// A few words for what the pull request is waiting on: "1 failing · 2 threads", "3 of 5 checks done", "Ready to merge".
/// The pill and the badge's tooltip show them.
pub fn pr_words(facts: &PrFacts) -> String {
    match pr_state(facts) {
        PrState::Merged => return "Merged".into(),
        PrState::Closed => return "Closed".into(),
        PrState::Draft => return if facts.checks == Checks::Failing { "Draft · checks failing" } else { "Draft" }.into(),
        PrState::Blocked => {
            let words: Vec<String> = block_reasons(facts).into_iter().map(|r| reason_words(r, facts)).collect();
            // "1 failing" alone doesn't say what failed.
            if words.len() == 1 && words[0].ends_with(" failing") {
                return "Checks failing".into();
            }
            // Two reasons fit a pill; the rest are a count, and the popover lists them.
            return if words.len() > 2 { format!("{} +{}", words[..2].join(" · "), words.len() - 2) } else { words.join(" · ") };
        }
        PrState::Open => {}
    }
    if facts.checks == Checks::Pending {
        return match facts.check_counts {
            Some(c) if c.passed + c.failing + c.pending > 0 => format!("{} of {} checks done", c.passed, c.passed + c.failing + c.pending),
            _ => "Checks running".into(),
        };
    }
    match facts.review {
        Some(ReviewDecision::Approved) => return if facts.checks == Checks::None { "Approved" } else { "Ready to merge" }.into(),
        Some(ReviewDecision::ReviewRequired) => return "Waiting for review".into(),
        _ => {}
    }
    if facts.checks == Checks::Passing { "Checks passed" } else { "Open" }.into()
}

/// A label for the state alone: "Open", "Blocked", "Draft", "Merged", "Closed".
pub fn pr_state_label(state: PrState) -> &'static str {
    match state {
        PrState::Open => "Open",
        PrState::Draft => "Draft",
        PrState::Blocked => "Blocked",
        PrState::Merged => "Merged",
        PrState::Closed => "Closed",
    }
}

/// GitHub's own words for the checks, so they read the same here as on github.com.
pub fn checks_headline(checks: Checks) -> &'static str {
    match checks {
        Checks::Failing => "Some checks were not successful",
        Checks::Pending => "Some checks haven't completed yet",
        Checks::Passing => "All checks have passed",
        Checks::None => "No checks",
    }
}

/// "3 passed · 1 failing · 1 running".
pub fn check_count_words(counts: CheckCounts) -> String {
    [(counts.passed, "passed"), (counts.failing, "failing"), (counts.pending, "running")]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, w)| format!("{n} {w}"))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// GitHub's rollup ("success", "failure", "pending", "none") in Fleet's words.
pub fn checks_from_rollup(rollup: Option<&str>) -> Checks {
    match rollup {
        Some("success") => Checks::Passing,
        Some("failure") => Checks::Failing,
        Some("pending") => Checks::Pending,
        _ => Checks::None,
    }
}

pub fn checks_from_counts(counts: CheckCounts) -> Checks {
    if counts.failing > 0 {
        Checks::Failing
    } else if counts.pending > 0 {
        Checks::Pending
    } else if counts.passed > 0 {
        Checks::Passing
    } else {
        Checks::None
    }
}

pub fn review_decision(value: Option<&Value>) -> Option<ReviewDecision> {
    match value?.as_str()? {
        "APPROVED" => Some(ReviewDecision::Approved),
        "CHANGES_REQUESTED" => Some(ReviewDecision::ChangesRequested),
        "REVIEW_REQUIRED" => Some(ReviewDecision::ReviewRequired),
        _ => None,
    }
}

/// A reviewer's latest verdict, or `Requested` while they haven't answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewerState {
    Approved,
    ChangesRequested,
    Commented,
    Dismissed,
    Requested,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reviewer {
    pub login: String,
    pub avatar_url: Option<String>,
    pub state: ReviewerState,
}

pub fn reviewer_words(state: ReviewerState) -> &'static str {
    match state {
        ReviewerState::Approved => "Approved",
        ReviewerState::ChangesRequested => "Changes requested",
        ReviewerState::Commented => "Commented",
        ReviewerState::Dismissed => "Dismissed",
        ReviewerState::Requested => "Review requested",
    }
}

/// Entries without a login are left out; an unknown state reads as a comment.
pub fn parse_reviewers(value: Option<&Value>) -> Vec<Reviewer> {
    let Some(list) = value.and_then(Value::as_array) else { return Vec::new() };
    list.iter()
        .filter_map(|entry| {
            let login = entry.get("login")?.as_str()?.to_string();
            let avatar_url = entry.get("avatarUrl").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
            let state = match entry.get("state").and_then(Value::as_str) {
                Some("APPROVED") => ReviewerState::Approved,
                Some("CHANGES_REQUESTED") => ReviewerState::ChangesRequested,
                Some("DISMISSED") => ReviewerState::Dismissed,
                Some("REQUESTED") => ReviewerState::Requested,
                _ => ReviewerState::Commented,
            };
            Some(Reviewer { login, avatar_url, state })
        })
        .collect()
}
