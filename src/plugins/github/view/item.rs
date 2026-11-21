//! An item's page (`plugins/builtin/github/pages/GitHubWorkItemDetailPage.vue`, rows 10.x of docs/parity/21-github.md): loading and
//! failing, the header (the trail, the title, Refresh, GitHub and the split button, the line under the title) and the menu behind the
//! chevron. The description and the conversation are `item_body`; the rail beside them is `item_rail`.
use super::item_body::body;
use crate::State;
use crate::fleet::send_to_agent::FIX;
use crate::icons::{CHEVRON_DOWN, EXTERNAL_LINK, MESSAGE_SQUARE, PLUS, REFRESH_CW, SPARKLES, TRIANGLE_ALERT};
use crate::plugins::github::item_page::{Primary, Run, by_words, files_words, fix_label, fix_of, item_state, noun, primary, state_label};
use crate::plugins::github::items::{ItemDetail, ItemSummary, item_resource_id};
use crate::plugins::github::sessions::{ItemSession, open_session};
use crate::plugins::github::{Kind, Where, state, state_ref};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::age;
use crate::ui::confirm::spinner;
use crate::ui::menu::{self, Entry, Item};
use crate::ui::overlay::{anchored, scrim};
use crate::ui::page_header::{bar_lead, trail, trail_here, trail_link};
use crate::ui::rem;
use crate::view::gh::{ItemKind, diff_stat, pr_state_pill, reviewer_avatar};
use crate::view::session_row::RowClock;
use crate::view::workflows::sheet;
use rune::text::Span;
use rune::{Align, Color, Cursor, El, Side, Tx, col, icon, rich, row, text};

/// The chevron's menu.
pub const MENU: &str = "github-item-menu";
const MORE: &str = "github-item-more";

/// `.gh-btn`: 30 high, padding 0 11, a 1 px border, 12.5 px 500. `corners` squares the side that meets the split's other half.
fn gh_btn(t: &Theme, key: &str, look: Look, parts: Vec<El>, words: &str, enabled: bool) -> El {
    let (fill, edge, ink) = match look {
        Look::Plain => (t.card_bg, t.border, t.text),
        Look::Ghost => (Color::TRANSPARENT, Color::TRANSPARENT, t.muted),
        Look::Primary => (t.accent, Color::TRANSPARENT, t.primary_fg),
    };
    let mut b = row()
        .key(key.to_string())
        .role("button")
        .h(30.0)
        .px(11.0)
        .gap(6.0)
        .center()
        .radius(RADIUS_BTN)
        .fill(fill)
        .border(1.0, edge)
        .border_layout()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .children(parts);
    if look == Look::Primary {
        // `color-mix(in srgb, var(--accent) 88%, white)`
        b = b.hover(t.accent.mix(Color::WHITE, 0.12));
    }
    if !words.is_empty() {
        b = b.label(words.to_string()).child(text(words.to_string()).font(sans(12.5, 500.0, 1.2)).color(ink));
    }
    if enabled { b } else { b.opacity(0.7).disabled(true).cursor(Cursor::Default) }
}

#[derive(Clone, Copy, PartialEq)]
enum Look {
    Plain,
    Ghost,
    Primary,
}

/// `.gh-branch`: a branch name on the accent's wash, mono 11.5.
pub fn branch(t: &Theme, key: &str, name: &str) -> El {
    row()
        .key(key.to_string())
        .px(6.0)
        .py(1.0)
        .radius(5.0)
        .fill(t.accent_dim)
        .child(text(name.to_string()).font(mono(11.5, 400.0, 1.5)).color(t.text).wrap())
}

fn muted(t: &Theme, words: impl Into<String>) -> El {
    text(words.into()).font(sans(12.5, 400.0, 1.5)).color(t.muted)
}

/// The title and " #n" (muted, 400).
fn title(t: &Theme, summary: &ItemSummary) -> El {
    let head = &summary.title;
    let number = format!(" #{}", summary.number);
    let spans = vec![Span::new(head.len(), head.len() + number.len()).weight(400.0).color(t.muted)];
    rich(format!("{head}{number}"), spans)
        .key("github-item-title")
        .font(sans(rem(1.5), 600.0, 1.25).tracking(-0.525))
        .color(t.text)
        .min_w(0.0)
}

/// `.gh-item__by`: the state, the author's avatar, who did what to which branches, when, and the diff.
fn by_line(s: &State, t: &Theme, summary: &ItemSummary, detail: &ItemDetail) -> El {
    let pull = summary.kind == Kind::Pull;
    let clock = RowClock::now(s);
    let kind = if pull { ItemKind::Pull } else { ItemKind::Issue };
    let mut line = row().key("github-item-by").flex_wrap().center_y().gap(6.0).child(pr_state_pill(
        t,
        "github-item-state",
        kind,
        item_state(summary, Some(detail)),
        &state_label(summary, Some(detail)),
    ));
    if let Some(author) = &summary.author {
        line = line.child(reviewer_avatar(t, "github-item-author-avatar", author, None, 18.0));
    }
    let who = by_words(summary);
    let author_len = summary.author.as_deref().map_or(0, str::len);
    let mut sentence =
        row().key("github-item-by-words").flex_wrap().center_y().gap(4.0).child(
            rich(who.clone(), vec![Span::new(0, author_len).weight(500.0).color(t.text)]).font(sans(12.5, 400.0, 1.5)).color(t.muted),
        );
    if let (true, Some(head)) = (pull, summary.head_ref.as_deref()) {
        sentence = sentence.child(branch(t, "github-item-head", head)).child(muted(t, "\u{2192}")).child(branch(
            t,
            "github-item-base",
            summary.base_ref.as_deref().unwrap_or_default(),
        ));
    }
    line = line.child(sentence).child(muted(t, format!("\u{b7} {}", age::long(&summary.created_at, clock.now, clock.offset_secs))));
    if let (true, Some(add), Some(del)) = (pull, summary.additions, summary.deletions) {
        line = line.child(muted(t, "\u{b7}")).child(diff_stat(t, "github-item-diff", add, del));
        if let Some(files) = files_words(Some(detail)) {
            line = line.child(muted(t, files));
        }
    }
    line
}

/// What the split's main half draws: the icon, or a spinner while the fix is on its way.
fn primary_parts(t: &Theme, p: &Primary, sending: bool) -> Vec<El> {
    vec![if sending {
        spinner(t.primary_fg)
    } else {
        let glyph = match p.run {
            Run::Fix => &SPARKLES,
            Run::Open => &MESSAGE_SQUARE,
            Run::Start => &PLUS,
        };
        icon(glyph, 13.0).color(t.primary_fg)
    }]
}

/// `.gh-split`: the primary and its chevron half, joined (the divider is the primary text at 25 %).
fn split(t: &Theme, p: &Primary, sending: bool, off: bool) -> El {
    let main = gh_btn(t, "github-item-primary", Look::Primary, primary_parts(t, p, sending), &p.label, !off)
        .corners([RADIUS_BTN, 0.0, 0.0, RADIUS_BTN])
        .on_press_tx(|s: &mut State, tx: &mut Tx| s.github_item_primary(tx));
    let more = gh_btn(t, MORE, Look::Primary, vec![icon(&CHEVRON_DOWN, 13.0).color(t.primary_fg)], "", true)
        .label("More ways to work on this")
        .px(7.0)
        .corners([0.0, RADIUS_BTN, RADIUS_BTN, 0.0])
        .border_sides([0.0, 0.0, 0.0, 1.0], t.primary_fg.alpha(0.25))
        .on_press(|s: &mut State| {
            let p = &mut state(&mut s.places).item;
            p.menu = !p.menu;
        });
    row().key("github-item-split").child(main).child(more)
}

/// The failing-checks line a failed send leaves under the header.
fn send_error(t: &Theme, why: &str) -> El {
    text(why.to_string()).key("github-item-send-error").role("alert").font(sans(12.0, 400.0, 1.4)).color(t.error_text)
}

fn header(s: &State, t: &Theme, detail: &ItemDetail, refreshing: bool) -> El {
    let summary = &detail.summary;
    let (owner, repo) = (summary.owner.clone(), summary.repo.clone());
    let steps = vec![
        trail_link(t, "github-item-trail-home", "GitHub", |s: &mut State| state(&mut s.places).at = Where::Home),
        trail_link(t, "github-item-trail-repo", &format!("{owner} / {repo}"), {
            let (o, r) = (owner.clone(), repo.clone());
            move |s: &mut State| state(&mut s.places).at = Where::Repo { owner: o.clone(), repo: r.clone() }
        }),
        trail_here(t, if summary.kind == Kind::Pull { "Pull requests" } else { "Issues" }),
    ];
    let sessions = s.github_item_sessions();
    let first = sessions.first();
    let id = item_resource_id(summary);
    let fix = first.and_then(|f| fix_of(s, &f.id, &id));
    let label = fix.as_ref().and_then(|f| fix_label(summary, f.runs.len(), f.threads.len()));
    let sending = first.is_some_and(|f| s.agent_sends.is_sending(&f.id, FIX));
    let sent = first.is_some_and(|f| s.agent_sends.is_sent(&f.id, FIX));
    let p = primary(summary, first.is_some(), label, sent);

    let refresh = gh_btn(
        t,
        "github-item-refresh",
        Look::Ghost,
        vec![if refreshing { spinner(t.muted) } else { icon(&REFRESH_CW, 14.0).color(t.muted) }],
        "",
        !refreshing,
    )
    .label("Refresh")
    .help("Refresh")
    .on_press(|s: &mut State| s.github_item_refresh());
    let url = summary.url.clone();
    let link = gh_btn(t, "github-item-link", Look::Plain, vec![icon(&EXTERNAL_LINK, 13.0).color(t.text)], "GitHub", true).on_press_tx(
        move |_: &mut State, tx: &mut Tx| {
            let _ = tx.open_url(&url);
        },
    );
    let mut lead = vec![
        trail(t, "github-item-trail", steps),
        row().flex_wrap().center_y().gap(6.0).child(title(t, summary)),
        by_line(s, t, summary, detail),
    ];
    if let Some(why) = first.and_then(|f| s.agent_sends.error(&f.id)) {
        lead.push(send_error(t, why));
    }
    bar_lead(t, "github-item-head", lead, vec![refresh, link, split(t, &p, sending, sending || sent)])
}

/// The menu behind the chevron: In Fleet and each session, then Start another session (or the primary's words).
fn menu_entries(sessions: &[ItemSession], p: &Primary) -> Vec<Entry> {
    let mut out = Vec::new();
    if !sessions.is_empty() {
        out.push(Entry::Label("In Fleet".into()));
        for (i, session) in sessions.iter().enumerate() {
            let session = session.clone();
            out.push(Entry::Item(Item::new(session.title.clone()).id(format!("github-item-menu-session-{i}")).icon(&MESSAGE_SQUARE).on(
                move |s: &mut State, tx: &mut Tx| {
                    s.ui.open = None;
                    open_session(s, &session, tx);
                },
            )));
        }
        out.push(Entry::Separator);
    }
    let words = if sessions.is_empty() { p.label.clone() } else { "Start another session".into() };
    out.push(Entry::Item(Item::new(words).id("github-item-menu-start").icon(&PLUS).on(|s: &mut State, _: &mut Tx| {
        close(s);
        s.github_item_start();
    })));
    out
}

/// The menu, under the chevron and 256 wide (`w-64`).
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let g = state_ref(&s.places);
    if !g.item.menu || s.github_item_shown().is_none() {
        return Vec::new();
    }
    let Some(detail) = &g.item.detail else { return Vec::new() };
    let sessions = s.github_item_sessions();
    let first = sessions.first();
    let fix = first.and_then(|f| fix_of(s, &f.id, &item_resource_id(&detail.summary)));
    let label = fix.as_ref().and_then(|f| fix_label(&detail.summary, f.runs.len(), f.threads.len()));
    let p = primary(&detail.summary, first.is_some(), label, false);
    let panel = menu::panel(t, MENU, menu_entries(&sessions, &p)).w(256.0).focus_return(MORE.to_string()).modal(true, close);
    vec![scrim("github-item-menu-scrim", close), anchored(panel, MORE, Side::Bottom, Align::End, 4.0)]
}

fn close(s: &mut State) {
    state(&mut s.places).item.menu = false;
}

/// `.gh-item__state`: a line of words in the page's padding.
fn state_line(key: &str, parts: Vec<El>) -> El {
    row().key(key.to_string()).w_full().center_y().gap(10.0).px(28.0).py(24.0).children(parts)
}

/// The page, while the GitHub rail is on an item.
pub fn item(s: &State, t: &Theme) -> El {
    let g = state_ref(&s.places);
    let Some((_, _, kind, _)) = s.github_item_shown().or_else(|| g.item.asked.clone()) else { return sheet(t, "github-page") };
    let p = &g.item;
    let inner = if p.loading {
        state_line(
            "github-item-loading",
            vec![spinner(t.muted), text(format!("Loading the {}\u{2026}", noun(kind))).font(sans(14.0, 400.0, 1.4)).color(t.muted)],
        )
    } else if let Some(detail) = &p.detail {
        col()
            .key("github-item")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .child(header(s, t, detail, p.refreshing))
            .child(body(s, t, detail))
            .on_layout(|s: &mut State, [_, _, w, _]| state(&mut s.places).item.width = w)
            .scroll(rune::ScrollSpec { stick_bottom: false })
    } else {
        let why = p.error.clone().unwrap_or_else(|| format!("Couldn't load the {}.", noun(kind)));
        state_line(
            "github-item-error",
            vec![
                icon(&TRIANGLE_ALERT, 16.0).color(t.error),
                text(why).key("github-item-error-words").font(sans(14.0, 400.0, 1.4)).color(t.text),
                gh_btn(t, "github-item-retry", Look::Plain, vec![icon(&REFRESH_CW, 13.0).color(t.text)], "Try again", true)
                    .on_press(|s: &mut State| s.github_item_refresh()),
            ],
        )
        .role("alert")
    };
    // The shell pads a rail view's page by 24 (as it does a repository's); the page scrolls inside that.
    sheet(t, "github-page").child(col().w_full().grow(1.0).min_h(0.0).padding(24.0).child(inner))
}
