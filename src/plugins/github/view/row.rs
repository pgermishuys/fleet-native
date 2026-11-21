//! `GitHubItemRow.vue`: a pull request or issue in a list. What it is on the left; whether it can merge and whether Fleet is on it on
//! the right. The pointer (or the keyboard) swaps the right side for "Start session" (or "Open session") and a link to GitHub.
use super::session_chip::session_chip;
use crate::fleet::pr_state::{Checks, PrState, ReviewerState, pr_state, pr_words};
use crate::icons::{EXTERNAL_LINK, MESSAGE_SQUARE, PLUS};
use crate::plugins::github::items::{ItemSummary, item_pr_facts};
use crate::plugins::github::sessions::{ItemSession, open_session, start_session};
use crate::plugins::github::{Kind, Where, state};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::age;
use crate::view::gh::{CheckKind, ItemKind, check_icon, diff_stat, item_icon, label, reviewer_avatar};
use crate::view::session_row::RowClock;
use crate::{State, Tx};
use rune::{Cursor, El, Key, KeyEvent, col, el, icon, row, text};

/// What a row is given (its props): the item, the sessions on it, and how to show it.
pub struct RowArgs<'a> {
    /// The row's key; its parts are `{key}-start`, `{key}-open-session`, `{key}-link`, `{key}-chip`, `{key}-reason`, `{key}-label-{name}`.
    pub key: &'a str,
    pub item: &'a ItemSummary,
    pub sessions: &'a [ItemSession],
    /// `owner/repo` before the number, for lists across repositories.
    pub show_repo: bool,
    /// Why the item is in a "Needs you" list: "Review requested", "client-tests failing".
    pub reason: Option<&'a str>,
    /// A press on a label (`@label`): the repository page filters by it. Without one, a label is only drawn.
    pub on_label: Option<fn(&mut State, &str)>,
}

/// The widest the actions are, so the right side holds the room either of its two states needs.
const ACTIONS_W: f32 = 144.0;

/// The row's own words: "Pull request #218: title · Checks failing · 2 comments".
pub fn description(item: &ItemSummary) -> String {
    let pull = item.kind == Kind::Pull;
    let mut parts = vec![format!("{} #{}: {}", if pull { "Pull request" } else { "Issue" }, item.number, item.title)];
    if pull && item.state == "open" {
        parts.push(pr_words(&item_pr_facts(item, None)));
    }
    if item.comments > 0 {
        parts.push(format!("{} comment{}", item.comments, if item.comments == 1 { "" } else { "s" }));
    }
    parts.join(" · ")
}

/// The state the icon draws: a pull request's by `prState`, an issue open or closed.
fn icon_state(item: &ItemSummary) -> PrState {
    if item.kind == Kind::Pull {
        pr_state(&item_pr_facts(item, None))
    } else if item.state == "closed" {
        PrState::Closed
    } else {
        PrState::Open
    }
}

/// Opens the item's own page.
pub fn open_item(s: &mut State, item: &ItemSummary) {
    state(&mut s.places).at = Where::of_item(item);
}

fn activates(ev: &KeyEvent) -> bool {
    let go = match &ev.key {
        Key::Enter => true,
        Key::Text(t) => t == " ",
        _ => false,
    };
    go && !ev.mods.ctrl && !ev.mods.cmd && !ev.mods.alt
}

fn button(t: &Theme, key: String, words: Option<&str>, glyph: &'static rune::Icon, primary: bool) -> El {
    let (fill, ink) = if primary { (t.accent, t.primary_fg) } else { (t.card_bg, t.text) };
    let mut b = row()
        .key(key)
        .h(24.0)
        .px(8.0)
        .gap(5.0)
        .center_y()
        .radius(6.0)
        .fill(fill)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .role("button");
    b = if primary { b } else { b.border(1.0, t.border) };
    b = b.child(icon(glyph, 12.0).color(ink));
    if let Some(words) = words {
        b = b.child(text(words.to_string()).font(sans(12.0, 500.0, 1.2)).color(ink)).label(words.to_string());
    }
    b
}

/// The right side at rest: why it is here, who is on it, who has said what, whether it builds, how big it is.
fn signals(s: &State, t: &Theme, a: &RowArgs, state: PrState) -> El {
    let item = a.item;
    let key = a.key;
    let mut out = row().key(format!("{key}-signals")).gap(12.0).center_y().justify_end();
    if let Some(reason) = a.reason {
        out = out.child(
            text(reason.to_string()).key(format!("{key}-reason")).font(sans(11.5, 400.0, 1.2)).color(t.pr_tone_text(PrState::Blocked)),
        );
    }
    if let Some(chip) = session_chip(t, &format!("{key}-chip"), a.sessions) {
        out = out.child(chip);
    }
    // Reviewers who have said something; waiting ones show on the pull request's page.
    let verdicts: Vec<_> =
        item.reviewers.iter().filter(|r| matches!(r.state, ReviewerState::Approved | ReviewerState::ChangesRequested)).take(3).collect();
    if !verdicts.is_empty() {
        let mut group = row().gap(5.0).pr(2.0).center_y();
        for r in verdicts {
            group = group.child(reviewer_avatar(t, &format!("{key}-reviewer-{}", r.login), &r.login, Some(r.state), 16.0));
        }
        out = out.child(group);
    }
    let _ = (s, state);
    let kind = match item_pr_facts(item, None).checks {
        Checks::Failing => Some(CheckKind::Failure),
        Checks::Pending => Some(CheckKind::Pending),
        Checks::Passing => Some(CheckKind::Success),
        Checks::None => None,
    };
    if let Some(kind) = kind {
        out = out.child(check_icon(t, kind, 14.0));
    }
    if let (Some(add), Some(del)) = (item.additions, item.deletions) {
        out = out.child(row().min_w(72.0).justify_end().child(diff_stat(t, &format!("{key}-diff"), add, del)));
    }
    out
}

/// "Start session" or "Open session", and the link to GitHub.
fn actions(t: &Theme, a: &RowArgs) -> El {
    let key = a.key;
    let item = a.item;
    let mut out = row().key(format!("{key}-actions")).gap(4.0).center_y();
    if let Some(first) = a.sessions.first() {
        let first = first.clone();
        out = out.child(
            button(t, format!("{key}-open-session"), Some("Open session"), &MESSAGE_SQUARE, false)
                .on_press_tx(move |s: &mut State, tx: &mut Tx| open_session(s, &first, tx)),
        );
    } else {
        let item = item.clone();
        out = out.child(
            button(t, format!("{key}-start"), Some("Start session"), &PLUS, true)
                .on_press(move |s: &mut State| start_session(s, &item, None)),
        );
    }
    let url = item.url.clone();
    out.child(
        button(t, format!("{key}-link"), None, &EXTERNAL_LINK, false)
            .fill(rune::Color::TRANSPARENT)
            .border(0.0, rune::Color::TRANSPARENT)
            .label("Open on GitHub")
            .help("Open on GitHub")
            .on_press_tx(move |_: &mut State, tx: &mut Tx| {
                let _ = tx.open_url(&url);
            }),
    )
}

/// The second line: where, who, when, which branch.
fn meta(s: &State, t: &Theme, a: &RowArgs) -> El {
    let (item, key) = (a.item, a.key);
    let small = || sans(12.0, 400.0, 1.2);
    let dot = |n: &str| text("·").key(format!("{key}-dot-{n}")).font(small()).color(t.muted);
    let mut line = row().w_full().min_w(0.0).gap(6.0).center_y().clip_rounded(0.0);
    if a.show_repo {
        line = line.child(text(format!("{}/{}", item.owner, item.repo)).key(format!("{key}-repo")).font(small()).color(t.muted));
    }
    line = line.child(text(format!("#{}", item.number)).key(format!("{key}-number")).font(small()).color(t.muted));
    if let Some(author) = &item.author {
        line = line
            .child(dot("author"))
            .child(reviewer_avatar(t, &format!("{key}-author-avatar"), author, None, 14.0))
            .child(text(author.clone()).key(format!("{key}-author")).font(small()).color(t.muted));
    }
    let clock = RowClock::now(s);
    line = line
        .child(dot("time"))
        .child(text(age::compact(&item.updated_at, clock.now, clock.offset_secs)).key(format!("{key}-age")).font(small()).color(t.muted));
    if let Some(branch) = &item.head_ref {
        line = line.child(dot("branch")).child(
            text(branch.clone()).key(format!("{key}-branch")).font(mono(11.0, 400.0, 1.2)).color(t.muted.alpha(0.85)).truncate().shrink(),
        );
    }
    line
}

/// One item in a list.
pub fn item_row(s: &State, t: &Theme, a: &RowArgs) -> El {
    let (item, key) = (a.item, a.key);
    let words = description(item);
    let state = icon_state(item);
    let kind = if item.kind == Kind::Pull { ItemKind::Pull } else { ItemKind::Issue };

    let mut title_line = row()
        .w_full()
        .min_w(0.0)
        .gap(8.0)
        .center_y()
        .child(text(item.title.clone()).key(format!("{key}-title")).font(sans(13.0, 500.0, 1.3)).color(t.text).truncate().shrink());
    for l in item.labels.iter().take(3) {
        let mut chip = label(t, &format!("{key}-label-{}", l.name), &l.name, &l.color);
        if let Some(on_label) = a.on_label {
            let name = l.name.clone();
            // A label press is its own: it doesn't open the row (`@click.stop`).
            chip = chip.cursor(Cursor::Pointer).on_press(move |s: &mut State| on_label(s, &name));
        }
        title_line = title_line.child(chip);
    }

    let middle = col().grow(1.0).min_w(0.0).gap(2.0).child(title_line).child(meta(s, t, a));
    let trail = row().min_w(ACTIONS_W).justify_end().child(signals(s, t, a, state).group_opacity(1.0, 0.0));

    let open = {
        let item = item.clone();
        move |s: &mut State| open_item(s, &item)
    };
    let line = row()
        .key(key.to_string())
        .w_full()
        .px(10.0)
        .py(8.0)
        .gap(10.0)
        .center_y()
        .radius(RADIUS_BTN)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        .role("button")
        .label(words.clone())
        .help(words)
        .on_press(open.clone())
        .on_key(move |s: &mut State, ev: &KeyEvent, _: &mut Tx| {
            let go = activates(ev);
            if go {
                open(s);
            }
            go
        })
        .child(el().w(18.0).mt(2.0).self_start().child(item_icon(t, kind, state, 15.0)))
        .child(middle)
        .child(trail);

    // The shell holds the pointer for the actions over the right side: they show while it is over either.
    el().key(format!("{key}-shell"))
        .w_full()
        .radius(RADIUS_BTN)
        .hover(t.wash(5.0))
        .hover_group()
        .child(line)
        .child(row().pin(None, Some(0.0), Some(10.0), Some(0.0)).center_y().child(actions(t, a).group_opacity(0.0, 1.0)))
}
