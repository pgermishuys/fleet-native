//! Your work, the page of the GitHub rail (`plugins/builtin/github/pages/GitHubBrowserPage.vue`, rows 5.x of docs/parity/21-github.md):
//! the title with Refresh and Follow a repository, then what needs you, what Fleet is on, your pull requests and what is assigned to
//! you, each a list of the item rows. Not connected, it is the connect section instead (`home_connect`). What goes in which list is
//! `home::buckets`; the work itself is the plugin's `work` store.
use super::home_connect::connect;
use super::row::{RowArgs, item_row};
use crate::State;
use crate::fleet::pr_state::PrState;
use crate::icons::{CIRCLE_DOT, GITHUB, PLUS, REFRESH_CW, TRIANGLE_ALERT, ZAP};
use crate::plugins::github::home::{Buckets, buckets, header_line};
use crate::plugins::github::items::{ItemSummary, item_resource_id};
use crate::plugins::github::sessions::sessions_of;
use crate::plugins::github::state_ref;
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::confirm::spinner;
use crate::ui::empty_state;
use crate::ui::page_header::plain_actions;
use crate::view::workflows::sheet;
use rune::{Color, Cursor, El, ScrollSpec, col, icon, row, text};

/// `.gh-home__btn`: 30 high, padding 0 11, a 1 px border, 12.5 px 500; the ghost one has neither border nor fill and is muted; the
/// primary one is 34 high in the accent.
#[derive(Clone, Copy, PartialEq)]
pub enum Look {
    Plain,
    Ghost,
    Primary,
}

pub fn button(t: &Theme, key: &str, look: Look, lead: Option<El>, words: &str, enabled: bool) -> El {
    let (fill, edge, ink) = match look {
        Look::Plain => (t.card_bg, t.border, t.text),
        Look::Ghost => (Color::TRANSPARENT, Color::TRANSPARENT, t.muted),
        Look::Primary => (t.accent, Color::TRANSPARENT, t.primary_fg),
    };
    let mut b = row()
        .key(key.to_string())
        .role("button")
        .h(if look == Look::Primary { 34.0 } else { 30.0 })
        .px(11.0)
        .gap(6.0)
        .center()
        .radius(RADIUS_BTN)
        .fill(fill)
        .border(1.0, edge)
        .border_layout()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent);
    if let Some(lead) = lead {
        b = b.child(lead);
    }
    if !words.is_empty() {
        b = b.label(words.to_string()).child(text(words.to_string()).font(sans(12.5, 500.0, 1.2)).color(ink));
    }
    if enabled { b } else { b.opacity(0.7).disabled(true).cursor(Cursor::Default) }
}

/// `.gh-home__link`: a muted line of words that takes the text colour on hover.
pub fn link(t: &Theme, key: &str, words: &str) -> El {
    row()
        .key(key.to_string())
        .role("button")
        .label(words.to_string())
        .hover_group()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(text(words.to_string()).font(sans(12.5, 400.0, 1.4)).color(t.muted).group_color(t.text))
}

/// `.gh-home__bucket`: a title (an icon, the words, how many) and the rows under it, 2 apart.
fn bucket(t: &Theme, key: &str, glyph: Option<(&'static rune::Icon, Color)>, title: &str, rows: Vec<El>) -> El {
    let mut head = row().w_full().gap(7.0).center_y().px(10.0).mb(4.0);
    if let Some((glyph, ink)) = glyph {
        head = head.child(icon(glyph, 13.0).color(ink));
    }
    head = head
        .child(text(title.to_string()).font(sans(12.0, 600.0, 1.4)).color(t.muted))
        .child(text(rows.len().to_string()).key(format!("{key}-count")).font(mono(12.0, 400.0, 1.4)).color(t.muted.alpha(0.75)));
    col().key(key.to_string()).w_full().gap(2.0).child(head).children(rows)
}

fn rows(s: &State, t: &Theme, section: &str, list: &[(ItemSummary, Option<String>)]) -> Vec<El> {
    list.iter()
        .map(|(item, reason)| {
            let sessions = sessions_of(s, item);
            let key = format!("github-{section}-{}", item_resource_id(item));
            item_row(s, t, &RowArgs { key: &key, item, sessions: &sessions, show_repo: true, reason: reason.as_deref(), on_label: None })
        })
        .collect()
}

fn plain(list: &[ItemSummary]) -> Vec<(ItemSummary, Option<String>)> {
    list.iter().map(|i| (i.clone(), None)).collect()
}

fn lists(s: &State, t: &Theme, b: &Buckets, followed: usize) -> Vec<El> {
    let mut out = Vec::new();
    if !b.needs.is_empty() {
        let list: Vec<_> = b.needs.iter().map(|n| (n.item.clone(), Some(n.reason.clone()))).collect();
        let warn = Some((&TRIANGLE_ALERT, t.pr_tone(PrState::Blocked)));
        out.push(bucket(t, "github-needs-you", warn, "Needs you", rows(s, t, "needs", &list)));
    }
    if !b.in_fleet.is_empty() {
        out.push(bucket(t, "github-in-fleet", Some((&ZAP, t.accent)), "In Fleet", rows(s, t, "fleet", &plain(&b.in_fleet))));
    }
    if !b.pulls.is_empty() {
        out.push(bucket(t, "github-your-pulls", None, "Your pull requests", rows(s, t, "pulls", &plain(&b.pulls))));
    }
    if !b.assigned.is_empty() {
        out.push(bucket(
            t,
            "github-assigned",
            Some((&CIRCLE_DOT, t.muted)),
            "Assigned to you",
            rows(s, t, "assigned", &plain(&b.assigned)),
        ));
    }
    if b.is_empty() {
        let tail = if followed > 0 { " in the repositories you follow" } else { "" };
        out.push(
            empty_state::full(
                t,
                "github-empty",
                &GITHUB,
                "Nothing needs you",
                &format!("No review requests, open pull requests or assigned issues{tail}."),
                None,
            )
            .py(48.0),
        );
    }
    out
}

fn header(s: &State, t: &Theme) -> El {
    let g = state_ref(&s.places);
    let age = g.work.loaded_at.map(|at| s.clock - at);
    let line = header_line(g.bookmarks.list.len(), age);
    let loading = g.work.loading;
    let refresh = button(
        t,
        "github-refresh",
        Look::Ghost,
        Some(if loading { spinner(t.muted) } else { icon(&REFRESH_CW, 14.0).color(t.muted) }),
        "",
        !loading,
    )
    .label("Refresh")
    .help("Refresh")
    .on_press(|s: &mut State| s.github_work_refresh());
    let follow = button(t, "github-follow", Look::Plain, Some(icon(&PLUS, 14.0).color(t.text)), "Follow a repository", true)
        .on_press_tx(|s: &mut State, tx: &mut rune::Tx| s.github_follow_open(tx));
    plain_actions(t, "github-home-header", "Your work", Some(&line), vec![refresh, follow])
}

/// `.gh-home__error`: the reason, in the error colour.
fn error(t: &Theme, why: &str) -> El {
    row()
        .key("github-home-error")
        .role("alert")
        .gap(8.0)
        .center_y()
        .child(icon(&TRIANGLE_ALERT, 14.0).color(t.error_text))
        .child(text(why.to_string()).font(sans(13.0, 400.0, 1.4)).color(t.error_text).wrap().shrink().min_w(0.0))
}

fn loading(t: &Theme) -> El {
    row()
        .key("github-home-loading")
        .gap(8.0)
        .center_y()
        .child(spinner(t.muted))
        .child(text("Asking GitHub what needs you…").font(sans(14.0, 400.0, 1.4)).color(t.muted))
}

/// The page: the connect section until GitHub is connected (and the status is known), then Your work.
pub fn home(s: &State, t: &Theme) -> El {
    let g = state_ref(&s.places);
    let mut page = col().key("github-home").w_full().max_w(1080.0).self_center().gap(22.0);
    if !g.auth.checking && !g.auth.connected {
        page = page.child(connect(s, t));
    } else if g.auth.connected {
        page = page.child(header(s, t));
        if let Some(why) = &g.work.error {
            page = page.child(error(t, why));
        }
        match (&g.work.work, g.work.loading) {
            (None, true) => page = page.child(loading(t)),
            (Some(work), _) => page = page.children(lists(s, t, &buckets(s, work), g.bookmarks.list.len())),
            (None, false) => {}
        }
    }
    sheet(t, "github-page").child(col().w_full().grow(1.0).min_h(0.0).padding(24.0).child(page).scroll(ScrollSpec { stick_bottom: false }))
}
