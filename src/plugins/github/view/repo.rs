//! A repository's page (`plugins/builtin/github/pages/GitHubRepoPage.vue`, rows 8.x of docs/parity/21-github.md): the title with Refresh
//! and a link to GitHub, the Pull requests and Issues tabs with their open counts, the pull request filters and their box, and the
//! list of item rows with its loading, failed and empty words and Load more. What the page does is `repo`; the issue filter bar is
//! piece J's.
use super::home::{Look, button};
use super::row::{RowArgs, item_row};
use crate::State;
use crate::icons::{CIRCLE_DOT, EXTERNAL_LINK, GIT_PULL_REQUEST, REFRESH_CW, SEARCH, TRIANGLE_ALERT};
use crate::plugins::github::items::item_resource_id;
use crate::plugins::github::repo::{FILTER_INPUT, PULL_FILTERS, PullFilter, Tab, fleet_numbers};
use crate::plugins::github::sessions::sessions_of;
use crate::plugins::github::{state, state_ref};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::confirm::spinner;
use crate::ui::page_header::heading_actions;
use crate::ui::rem;
use crate::ui::text_menu::text_input;
use crate::view::workflows::sheet;
use rune::{Color, Cursor, El, ScrollSpec, Tx, col, el, icon, row, text};

/// `.gh-repo__tab`: an icon, the words and the open count; the selected one has a 2 px accent line under it.
fn tab(t: &Theme, key: &str, on: bool, glyph: &'static rune::Icon, words: &str, count: Option<u64>) -> El {
    let ink = if on { t.text } else { t.muted };
    let mut e = row()
        .key(key.to_string())
        .role("tab")
        .label(words.to_string())
        .checked(on)
        .h(36.5)
        .px(12.0)
        .gap(7.0)
        .center_y()
        .mb(-1.0)
        .border_sides([0.0, 0.0, 2.0, 0.0], if on { t.accent } else { Color::TRANSPARENT })
        .border_layout()
        .hover_group()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(icon(glyph, 14.0).color(ink))
        .child(text(words.to_string()).font(sans(13.0, 500.0, 1.2)).color(ink).group_color(t.text));
    if let Some(n) = count {
        e = e.child(text(n.to_string()).key(format!("{key}-count")).font(mono(11.0, 400.0, 1.2)).color(t.muted.alpha(0.8)));
    }
    e
}

/// `.gh-repo__chip`: 28 high, a border; pressed it has none and is filled with the accent's wash.
fn chip(t: &Theme, f: PullFilter, on: bool, fleet_count: usize) -> El {
    let mut e = row()
        .key(format!("github-pull-filter-{}", f.id()))
        .role("button")
        .label(f.label())
        .checked(on)
        .h(28.0)
        .px(10.0)
        .gap(6.0)
        .center_y()
        .radius(RADIUS_BTN)
        .fill(if on { t.accent_dim } else { Color::TRANSPARENT })
        .border(1.0, if on { Color::TRANSPARENT } else { t.border })
        .border_layout()
        .hover_group()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(text(f.label()).font(sans(12.5, 400.0, 1.2)).color(if on { t.text } else { t.muted }).group_color(t.text))
        .on_press(move |s: &mut State| s.github_repo_filter(f));
    if f == PullFilter::Fleet && fleet_count > 0 {
        e = e.child(
            text(fleet_count.to_string()).key("github-pull-filter-fleet-count").font(mono(11.0, 400.0, 1.2)).color(t.muted.alpha(0.8)),
        );
    }
    e
}

/// `.gh-repo__search`: the filter box of the pull requests, off under In Fleet.
fn search_box(s: &State, t: &Theme, enabled: bool) -> El {
    let g = state_ref(&s.places);
    let input = text_input(&g.repo.text, mono(12.0, 400.0, 1.4))
        .key(FILTER_INPUT)
        .label("Filter pull requests")
        .single_line()
        .placeholder("Filter, e.g. label:bug author:octocat", t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .grow(1.0)
        .min_w(0.0)
        .on_edit(|s: &mut State| &mut state(&mut s.places).repo.text);
    let e = row()
        .key("github-repo-search")
        .grow(1.0)
        .min_w(220.0)
        .h(30.0)
        .px(10.0)
        .gap(8.0)
        .center_y()
        .radius(RADIUS_BTN)
        .fill(t.wash(3.0))
        .border(1.0, t.border)
        .border_layout()
        .child(icon(&SEARCH, 13.0).color(t.muted))
        .child(input);
    if enabled { e } else { e.opacity(0.5).disabled(true) }
}

fn header(s: &State, t: &Theme, owner: &str, repo: &str, loading: bool) -> El {
    let heading = row()
        .gap(6.0)
        .center_y()
        .child(text(format!("{owner} /")).key("github-repo-owner").font(sans(rem(1.5), 400.0, 1.25).tracking(-0.525)).color(t.muted))
        .child(text(repo.to_string()).key("github-repo-name").font(sans(rem(1.5), 600.0, 1.25).tracking(-0.525)).color(t.text));
    let refresh = button(
        t,
        "github-repo-refresh",
        Look::Ghost,
        Some(if loading { spinner(t.muted) } else { icon(&REFRESH_CW, 14.0).color(t.muted) }),
        "",
        !loading,
    )
    .label("Refresh")
    .help("Refresh")
    .on_press(|s: &mut State| s.github_repo_refresh());
    let url = format!("https://github.com/{owner}/{repo}");
    let link = button(t, "github-repo-link", Look::Plain, Some(icon(&EXTERNAL_LINK, 13.0).color(t.text)), "GitHub", true).on_press_tx(
        move |_: &mut State, tx: &mut Tx| {
            let _ = tx.open_url(&url);
        },
    );
    let _ = s;
    heading_actions("github-repo-header", heading, vec![refresh, link])
}

fn centred(t: &Theme, key: &str, parts: Vec<El>) -> El {
    let _ = t;
    row().key(key.to_string()).w_full().center().gap(8.0).px(16.0).py(40.0).children(parts)
}

fn list(s: &State, t: &Theme, owner: &str, repo: &str) -> El {
    let g = state_ref(&s.places);
    let l = s.github_repo_list();
    let mut out = col().key("github-repo-list").w_full().gap(1.0);
    if l.loading && l.items.is_empty() {
        return out.child(centred(t, "github-repo-loading", vec![spinner(t.muted)]));
    }
    if let Some(why) = l.error {
        return out.child(
            centred(
                t,
                "github-repo-error",
                vec![
                    icon(&TRIANGLE_ALERT, 14.0).color(t.error),
                    text(why.to_string()).key("github-repo-error-words").font(sans(13.0, 400.0, 1.4)).color(t.text),
                    button(t, "github-repo-retry", Look::Plain, None, "Try again", true).on_press(|s: &mut State| s.github_repo_refresh()),
                ],
            )
            .role("alert"),
        );
    }
    if l.items.is_empty() {
        let words = g.repo.empty_words();
        return out.child(centred(t, "github-repo-empty", vec![text(words).font(sans(13.0, 400.0, 1.4)).color(t.muted)]));
    }
    let _ = (owner, repo);
    for item in l.items {
        let sessions = sessions_of(s, item);
        let key = format!("github-row-{}", item_resource_id(item));
        out = out.child(item_row(
            s,
            t,
            &RowArgs {
                key: &key,
                item,
                sessions: &sessions,
                show_repo: false,
                reason: None,
                on_label: Some(|s, name| s.github_repo_toggle_label(name)),
            },
        ));
    }
    if l.has_more {
        let words = if l.loading_more { "Loading\u{2026}" } else { "Load more" };
        let more = row()
            .key("github-repo-more")
            .role("button")
            .label(words)
            .self_center()
            .mt(8.0)
            .px(12.0)
            .py(6.0)
            .radius(RADIUS_BTN)
            .hover_group()
            .hover(t.wash(5.0))
            .cursor(Cursor::Pointer)
            .focusable(true)
            .focus_ring(2.0, 1.0, t.accent)
            .child(text(words).font(sans(12.5, 400.0, 1.2)).color(t.muted).group_color(t.text));
        out = out.child(if l.loading_more { more.disabled(true) } else { more.on_press(|s: &mut State| s.github_repo_more()) });
    }
    out
}

/// The page, while the GitHub rail is on a repository.
pub fn repo(s: &State, t: &Theme) -> El {
    let g = state_ref(&s.places);
    let Some((owner, name)) = s.github_repo_shown().or_else(|| match &g.at {
        crate::plugins::github::Where::Repo { owner, repo } => Some((owner.clone(), repo.clone())),
        _ => None,
    }) else {
        return sheet(t, "github-page");
    };
    let r = &g.repo;
    let counts = g.work.work.as_ref().and_then(|w| w.repos.iter().find(|c| c.full_name.eq_ignore_ascii_case(&format!("{owner}/{name}"))));
    let l = s.github_repo_list();
    let tabs = row()
        .key("github-repo-tabs")
        .role("tablist")
        .label("Pull requests and issues")
        .w_full()
        .gap(4.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .border_layout()
        .child(
            tab(t, "github-repo-tab-pulls", r.tab == Tab::Pulls, &GIT_PULL_REQUEST, "Pull requests", counts.map(|c| c.open_pull_requests))
                .on_press(|s: &mut State| s.github_repo_tab(Tab::Pulls)),
        )
        .child(
            tab(t, "github-repo-tab-issues", r.tab == Tab::Issues, &CIRCLE_DOT, "Issues", counts.map(|c| c.open_issues))
                .on_press(|s: &mut State| s.github_repo_tab(Tab::Issues)),
        );
    let mut page =
        col().key("github-repo").w_full().max_w(1080.0).self_center().gap(12.0).child(header(s, t, &owner, &name, l.loading)).child(tabs);
    if r.tab == Tab::Pulls {
        let fleet = fleet_numbers(s, &owner, &name).len();
        let mut filters = row().key("github-repo-filters").w_full().flex_wrap().center_y().gap(6.0);
        for f in PULL_FILTERS {
            filters = filters.child(chip(t, f, r.filter == f, fleet));
        }
        page = page.child(filters.child(search_box(s, t, r.filter != PullFilter::Fleet)));
    } else {
        // The issue filter bar is piece J's: until it is built the tab shows its list under no bar.
        page = page.child(el().key("github-repo-issue-bar"));
    }
    page = page.child(list(s, t, &owner, &name));
    sheet(t, "github-page").child(col().w_full().grow(1.0).min_h(0.0).padding(24.0).child(page).scroll(ScrollSpec { stick_bottom: false }))
}
