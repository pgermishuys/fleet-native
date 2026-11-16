//! The `gh-parts` scene: every small GitHub part in each of its states on one page, so a picture shows them all
//! (nothing outside the scenes draws this).
use super::{CheckKind, ItemKind, check_icon, checks_summary, diff_stat, item_icon, label, pr_state_pill, reviewer_avatar};
use crate::fleet::pr_state::{CheckCounts, PrState, ReviewerState};
use crate::theme::{Theme, sans};
use rune::{El, col, row, text};

fn section(t: &Theme, title: &str, parts: Vec<El>) -> El {
    let heading = text(title.to_string()).font(sans(11.0, 600.0, 1.2)).color(t.muted).w(120.0);
    let mut line = row().w_full().gap(14.0).center_y().child(heading);
    for part in parts {
        line = line.child(part);
    }
    line.py(7.0)
}

pub fn parts(t: &Theme) -> El {
    let states = [
        (PrState::Open, "Ready to merge"),
        (PrState::Blocked, "1 failing · 2 threads"),
        (PrState::Draft, "Draft"),
        (PrState::Merged, "Merged"),
        (PrState::Closed, "Closed"),
    ];
    let pills = states.iter().map(|(s, w)| pr_state_pill(t, &format!("gh-pill-{w}"), ItemKind::Pull, *s, w)).collect();
    let issues = vec![
        pr_state_pill(t, "gh-pill-issue-open", ItemKind::Issue, PrState::Open, "Open"),
        pr_state_pill(t, "gh-pill-issue-closed", ItemKind::Issue, PrState::Closed, "Closed"),
    ];
    let icons = [PrState::Open, PrState::Blocked, PrState::Draft, PrState::Merged, PrState::Closed]
        .iter()
        .map(|s| item_icon(t, ItemKind::Pull, *s, 16.0))
        .chain([item_icon(t, ItemKind::Issue, PrState::Open, 16.0), item_icon(t, ItemKind::Issue, PrState::Closed, 16.0)])
        .collect();
    let checks = [CheckKind::Success, CheckKind::Failure, CheckKind::Pending, CheckKind::Skipped, CheckKind::Neutral]
        .iter()
        .map(|k| check_icon(t, *k, 16.0))
        .collect();
    let summary = |key: &str, counts: CheckCounts, bar_only: bool| checks_summary(t, key, counts, bar_only).w(300.0);
    let sums = vec![
        col()
            .gap(14.0)
            .child(summary("gh-sum-failing", CheckCounts { passed: 3, failing: 1, pending: 1 }, false))
            .child(summary("gh-sum-pending", CheckCounts { passed: 3, failing: 0, pending: 2 }, false))
            .child(summary("gh-sum-passed", CheckCounts { passed: 5, failing: 0, pending: 0 }, false))
            .child(summary("gh-sum-bar", CheckCounts { passed: 3, failing: 1, pending: 1 }, true)),
    ];
    let avatars = vec![
        reviewer_avatar(t, "gh-av-approved", "sam-example", Some(ReviewerState::Approved), 18.0).mx(6.0),
        reviewer_avatar(t, "gh-av-changes", "robin-example", Some(ReviewerState::ChangesRequested), 18.0).mx(6.0),
        reviewer_avatar(t, "gh-av-waiting", "kit-example", Some(ReviewerState::Requested), 18.0).mx(6.0),
        reviewer_avatar(t, "gh-av-none", "alex-example", None, 18.0),
        reviewer_avatar(t, "gh-av-big", "morgan-example", Some(ReviewerState::Approved), 28.0).mx(6.0),
    ];
    let labels = vec![
        label(t, "gh-label-bug", "bug", "d73a4a"),
        label(t, "gh-label-first", "good first issue", "7057ff"),
        label(t, "gh-label-docs", "documentation", "0075ca"),
        label(t, "gh-label-dark", "wontfix", "000000"),
        label(t, "gh-label-odd", "no colour", "nope"),
    ];
    col()
        .key("gh-parts")
        .w_full()
        .h_full()
        .padding(24.0)
        .fill(t.panel_bg)
        .radius(10.0)
        .child(section(t, "State pill", pills))
        .child(section(t, "Issue pill", issues))
        .child(section(t, "Item icon", icons))
        .child(section(t, "Check icon", checks))
        .child(section(t, "Checks summary", sums))
        .child(section(t, "Diff stat", vec![diff_stat(t, "gh-diff", 412, 88)]))
        .child(section(t, "Reviewer avatar", avatars))
        .child(section(t, "Label", labels))
}
