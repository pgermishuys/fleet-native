//! An item's side rail (`GitHubWorkItemDetailPage.vue`'s `.gh-item__rail`, rows 12.x of docs/parity/21-github.md): the sessions on it,
//! its checks, reviewers, linked pull requests, labels, assignees and branch, each section only when it has something. Below 820 px
//! the page is one column and the rail comes first (`@container (max-width: 820px)`).
use crate::State;
use crate::fleet::pr_state::{ReviewerState, reviewer_words};
use crate::icons::{EXTERNAL_LINK, GIT_BRANCH, MESSAGE_SQUARE};
use crate::plugins::github::item_page::{linked_pull_requests, reference_state};
use crate::plugins::github::items::{CheckResult, ItemDetail, check_counts};
use crate::plugins::github::sessions::{ItemSession, open_session};
use crate::plugins::github::view::item::branch;
use crate::plugins::github::{Kind, state_ref};
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::view::gh::{CheckKind, ItemKind, check_icon, checks_summary, item_icon, label, reviewer_avatar};
use crate::view::session_row::glyph_el;
use rune::{Cursor, El, Tx, col, icon, row, text};

/// The rail's width beside the conversation (`minmax(0, 1fr) 300px`).
pub const RAIL_W: f32 = 300.0;
/// `@container (max-width: 820px)`: one column, the rail first.
pub const ONE_COLUMN: f32 = 820.0;
/// `@container (max-width: 520px)`: the main column's side padding shrinks to 16.
pub const TIGHT: f32 = 520.0;

/// The page's width, measured one frame late; a page not yet measured is laid out wide.
pub fn width(s: &State) -> f32 {
    let p = &state_ref(&s.places).item;
    p.pinned_width.unwrap_or(p.width)
}

pub fn one_column(s: &State) -> bool {
    let w = width(s);
    w > 0.0 && w <= ONE_COLUMN
}

pub fn tight(s: &State) -> bool {
    let w = width(s);
    w > 0.0 && w <= TIGHT
}

/// The sections that have something to show, in the order Fleet draws them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sections {
    pub checks: bool,
    pub reviewers: bool,
    pub linked: bool,
    pub labels: bool,
    pub assignees: bool,
    pub branch: bool,
}

pub fn sections(detail: &ItemDetail) -> Sections {
    let pull = detail.summary.kind == Kind::Pull;
    Sections {
        checks: pull && !detail.checks.is_empty(),
        reviewers: pull && (!detail.summary.reviewers.is_empty() || detail.unresolved_threads > 0),
        linked: !linked_pull_requests(detail).is_empty(),
        labels: !detail.summary.labels.is_empty(),
        assignees: !detail.summary.assignees.is_empty(),
        branch: pull && detail.summary.head_ref.is_some(),
    }
}

/// `.gh-rail__heading`: 11 px 600 in capitals, spaced .06em, muted.
fn heading(t: &Theme, words: &str) -> El {
    text(words.to_uppercase()).font(sans(11.0, 600.0, 1.4).tracking(0.66)).color(t.muted)
}

fn muted(t: &Theme, key: &str, words: String) -> El {
    text(words).key(key.to_string()).font(sans(12.0, 400.0, 1.4)).color(t.muted)
}

/// `.gh-rail__section`: a heading over its rows, gap 8.
fn section(t: &Theme, key: &str, words: &str, rows: Vec<El>) -> El {
    col().key(key.to_string()).min_w(0.0).gap(8.0).child(heading(t, words)).children(rows)
}

/// `.gh-rail__list`: rows 2 apart.
fn list(rows: Vec<El>) -> El {
    col().w_full().min_w(0.0).gap(2.0).children(rows)
}

fn one_session(t: &Theme, i: usize, session: &ItemSession) -> El {
    let glyph = if session.live { glyph_el(t, session.glyph) } else { None }.unwrap_or_else(|| icon(&MESSAGE_SQUARE, 12.0).color(t.text));
    let session = session.clone();
    let words = session.words.clone();
    let name = session.title.clone();
    row()
        .key(format!("github-item-session-{i}"))
        .role("button")
        .label(format!("{name}, {words}"))
        .w_full()
        .min_w(0.0)
        .px(8.0)
        .py(6.0)
        .gap(8.0)
        .center_y()
        .radius(RADIUS_BTN)
        .border(1.0, t.accent.alpha(0.3))
        .border_layout()
        .fill(t.accent_dim)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(glyph)
        .child(text(name).font(sans(12.5, 500.0, 1.4)).color(t.text).truncate().grow(1.0).shrink())
        .child(text(words).font(sans(12.0, 400.0, 1.4)).color(t.muted))
        .on_press_tx(move |s: &mut State, tx: &mut Tx| open_session(s, &session, tx))
}

fn sessions(t: &Theme, sessions: &[ItemSession]) -> El {
    let body = if sessions.is_empty() {
        vec![muted(t, "github-item-no-session", "No session is working on this yet.".into())]
    } else {
        vec![list(sessions.iter().enumerate().map(|(i, x)| one_session(t, i, x)).collect())]
    };
    section(t, "github-rail-sessions", "In Fleet", body)
}

fn check_kind(state: CheckResult) -> CheckKind {
    match state {
        CheckResult::Success => CheckKind::Success,
        CheckResult::Failure => CheckKind::Failure,
        CheckResult::Pending => CheckKind::Pending,
        CheckResult::Skipped => CheckKind::Skipped,
        CheckResult::Neutral => CheckKind::Neutral,
    }
}

fn checks(t: &Theme, detail: &ItemDetail) -> El {
    let rows = detail
        .checks
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let title = match &c.workflow_name {
                Some(w) => format!("{w} / {}", c.name),
                None => c.name.clone(),
            };
            let mut line = row()
                .key(format!("github-rail-check-{i}"))
                .w_full()
                .min_h(24.0)
                .gap(8.0)
                .center_y()
                .child(check_icon(t, check_kind(c.state), 13.0))
                .child(text(c.name.clone()).font(sans(12.5, 400.0, 1.4)).color(t.text).truncate().grow(1.0).shrink().help(title));
            if let Some(url) = c.url.clone() {
                line = line.child(
                    row()
                        .key(format!("github-rail-check-{i}-link"))
                        .role("link")
                        .label(format!("Open {} on GitHub", c.name))
                        .center()
                        .cursor(Cursor::Pointer)
                        .focusable(true)
                        .focus_ring(2.0, 1.0, t.accent)
                        .child(icon(&EXTERNAL_LINK, 12.0).color(t.muted))
                        .on_press_tx(move |_: &mut State, tx: &mut Tx| {
                            let _ = tx.open_url(&url);
                        }),
                );
            }
            line
        })
        .collect();
    section(
        t,
        "github-rail-checks",
        "Checks",
        vec![checks_summary(t, "github-rail-checks-summary", check_counts(&detail.checks), false), list(rows)],
    )
}

fn person(t: &Theme, key: &str, login: &str, state: Option<ReviewerState>) -> El {
    let mut line = row()
        .key(key.to_string())
        .w_full()
        .min_h(24.0)
        .gap(8.0)
        .center_y()
        .child(reviewer_avatar(t, &format!("{key}-avatar"), login, state, 20.0))
        .child(text(login.to_string()).font(sans(12.5, 400.0, 1.4)).color(t.text).truncate().shrink());
    if let Some(state) = state {
        let ink = match state {
            ReviewerState::Approved => t.pr_tone_text(crate::fleet::pr_state::PrState::Open),
            ReviewerState::ChangesRequested => t.pr_tone_text(crate::fleet::pr_state::PrState::Blocked),
            _ => t.muted,
        };
        // `margin-left: auto`
        line = line.child(
            row()
                .grow(1.0)
                .justify_end()
                .child(text(reviewer_words(state)).key(format!("{key}-verdict")).font(sans(11.5, 400.0, 1.4)).color(ink)),
        );
    }
    line
}

fn reviewers(t: &Theme, detail: &ItemDetail) -> El {
    let mut body = vec![list(
        detail.summary.reviewers.iter().map(|r| person(t, &format!("github-rail-reviewer-{}", r.login), &r.login, Some(r.state))).collect(),
    )];
    let n = detail.unresolved_threads;
    if n > 0 {
        body.push(muted(t, "github-rail-threads", format!("{n} unresolved thread{}", if n == 1 { "" } else { "s" })));
    }
    section(t, "github-rail-reviewers", "Reviewers", body)
}

fn linked(t: &Theme, detail: &ItemDetail) -> El {
    let rows = linked_pull_requests(detail)
        .into_iter()
        .enumerate()
        .map(|(i, e)| {
            let words = format!("#{} {}", e.reference.as_ref().map_or(0, |r| r.number), e.body.as_deref().unwrap_or_default());
            let entry = e.clone();
            row()
                .key(format!("github-rail-linked-{i}"))
                .role("link")
                .label(words.clone())
                .w_full()
                .min_w(0.0)
                .py(4.0)
                .gap(8.0)
                .center_y()
                .cursor(Cursor::Pointer)
                .focusable(true)
                .focus_ring(2.0, 1.0, t.accent)
                .child(item_icon(t, ItemKind::Pull, reference_state(e), 13.0))
                .child(text(words).font(sans(12.5, 500.0, 1.4)).color(t.text).truncate().grow(1.0).shrink())
                .on_press_tx(move |s: &mut State, tx: &mut Tx| s.github_item_open_reference(&entry, tx))
        })
        .collect();
    section(t, "github-rail-linked", "Linked pull requests", vec![list(rows)])
}

fn labels(t: &Theme, detail: &ItemDetail) -> El {
    let chips: Vec<El> =
        detail.summary.labels.iter().enumerate().map(|(i, l)| label(t, &format!("github-rail-label-{i}"), &l.name, &l.color)).collect();
    section(t, "github-rail-labels", "Labels", vec![row().w_full().flex_wrap().gap(6.0).children(chips)])
}

fn assignees(t: &Theme, detail: &ItemDetail) -> El {
    let rows = detail.summary.assignees.iter().map(|l| person(t, &format!("github-rail-assignee-{l}"), l, None)).collect();
    section(t, "github-rail-assignees", "Assignees", vec![list(rows)])
}

fn the_branch(t: &Theme, head: &str) -> El {
    let line = row().gap(6.0).center_y().child(icon(&GIT_BRANCH, 12.0).color(t.muted)).child(branch(t, "github-rail-branch-name", head));
    section(t, "github-rail-branch", "Branch", vec![line])
}

/// `.gh-item__rail`. Beside the conversation it is 300 wide with a rule on its left; in one column it is the full width with the
/// rule under it.
pub fn rail(s: &State, t: &Theme, detail: &ItemDetail, one: bool) -> El {
    let has = sections(detail);
    let mut out = col()
        .key("github-item-rail")
        .role("complementary")
        .label("Details")
        .min_w(0.0)
        .gap(20.0)
        .pt(20.0)
        .px(20.0)
        .pb(32.0)
        .fill(t.wash(2.0))
        .border_layout()
        .child(sessions(t, &s.github_item_sessions()));
    out = if one {
        out.w_full().border_sides([0.0, 0.0, 1.0, 0.0], t.border)
    } else {
        out.w(RAIL_W).self_stretch().border_sides([0.0, 0.0, 0.0, 1.0], t.border)
    };
    if has.checks {
        out = out.child(checks(t, detail));
    }
    if has.reviewers {
        out = out.child(reviewers(t, detail));
    }
    if has.linked {
        out = out.child(linked(t, detail));
    }
    if has.labels {
        out = out.child(labels(t, detail));
    }
    if has.assignees {
        out = out.child(assignees(t, detail));
    }
    if let (true, Some(head)) = (has.branch, detail.summary.head_ref.as_deref()) {
        out = out.child(the_branch(t, head));
    }
    out
}
