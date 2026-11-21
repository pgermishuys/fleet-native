//! The GitHub rail's panel, `[github-panel]` (`plugins/builtin/github/GitHubPanel.vue`): who is signed in, Your work with how many
//! things need you, the repositories you follow with their open counts, Stop following and Follow a repository. It reads the
//! plugin's stores (`work`, `bookmarks`) and the connection; nothing here asks a server (`keep` does, in `mod.rs`).
use crate::State;
use crate::fleet::pr_state::PrState;
use crate::icons::{FOLDER_GIT_2, INBOX, PLUS, SETTINGS, X};
use crate::plugins::github::bookmarks::Bookmark;
use crate::plugins::github::items::{Work, needs_you};
use crate::plugins::github::{Where, state_ref};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::{empty_state, rem};
use crate::view::files_tab;
use crate::view::gh::reviewer_avatar;
use rune::{Cursor, El, ScrollSpec, col, el, icon, row, text};

pub fn panel(s: &State, t: &Theme) -> El {
    let g = state_ref(&s.places);
    let work = g.work.work.as_ref();
    let mut body = col().w_full().gap(1.0).child(account(s, t, work)).child(home_row(s, t, work));
    if g.auth.connected {
        body = body.child(heading(t)).children(repositories(s, t, work));
        body = body.child(add_row(t));
    }
    col()
        .key("github-panel")
        .label("GitHub")
        .w(s.sidebar_w)
        .h_full()
        .min_h(0.0)
        .px(6.0)
        .py(4.0)
        .child(body.scroll(ScrollSpec { stick_bottom: false }).w_full().grow(1.0).min_h(0.0))
}

/// `.panel-account`: the avatar (a plain circle until the work says who), the name over the status, and the settings gear.
fn account(s: &State, t: &Theme, work: Option<&Work>) -> El {
    let connected = state_ref(&s.places).auth.connected;
    let login = work.and_then(|w| w.login.as_deref());
    let avatar = match login {
        Some(login) => reviewer_avatar(t, "github-panel-avatar", login, None, 24.0),
        None => el().key("github-panel-avatar").size(24.0, 24.0).radius(24.0).fill(t.wash(10.0)),
    };
    let status = row().gap(5.0).center_y().child(el().size(6.0, 6.0).radius(6.0).fill(if connected { t.running } else { t.error })).child(
        text(if connected { "Connected" } else { "Not connected" }).key("github-panel-status").font(sans(11.0, 400.0, 1.5)).color(t.muted),
    );
    let who = col()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .child(text(login.unwrap_or("GitHub").to_string()).key("github-panel-name").font(sans(12.5, 600.0, 1.5)).color(t.text).truncate())
        .child(status);
    let gear = el()
        .key("github-panel-settings")
        .role("button")
        .label("GitHub settings")
        .help("GitHub settings")
        .size(22.0, 22.0)
        .center()
        .radius(5.0)
        .cursor(Cursor::Pointer)
        .hover(t.wash(8.0))
        .focus_ring(2.0, -2.0, t.accent)
        .child(icon(&SETTINGS, 13.0).color(t.muted).group_color(t.text))
        .hover_group()
        .on_press(|s: &mut State| s.open_settings("plugin:github", None));
    col()
        .w_full()
        .mb(6.0)
        .child(row().w_full().gap(9.0).center_y().px(6.0).pt(6.0).pb(10.0).child(avatar).child(who).child(gear))
        .child(el().w_full().h(1.0).fill(t.border))
}

/// `.panel-row`: 32 high, an icon, the name and whatever ends the row.
fn line(t: &Theme, key: String, label: String, current: bool, glyph: &'static rune::Icon) -> El {
    row()
        .key(key)
        .role("button")
        .label(label)
        .w_full()
        .min_w(0.0)
        .min_h(32.0)
        .gap(8.0)
        .center_y()
        .px(8.0)
        .radius(RADIUS_BTN)
        .cursor(Cursor::Pointer)
        .fill(if current { t.wash(9.0) } else { rune::Color::TRANSPARENT })
        .hover(if current { rune::Color::TRANSPARENT } else { t.wash(5.0) })
        .focus_ring(2.0, -2.0, t.accent)
        .child(icon(glyph, 14.0).color(t.muted))
}

/// `data-testid="github-panel-home"`: Your work, hot with how many things need you.
fn home_row(s: &State, t: &Theme, work: Option<&Work>) -> El {
    let current = state_ref(&s.places).at == Where::Home;
    let need = work.map_or(0, |w| needs_you(w).len());
    let mut r = line(t, "github-panel-home".into(), "Your work".into(), current, &INBOX)
        .hover_group()
        .child(name(t, "Your work".into(), None, current))
        .on_press(|s: &mut State| crate::plugins::github::state(&mut s.places).at = Where::Home);
    if need > 0 {
        r = r.child(count(need.to_string(), t.pr_tone_text(PrState::Blocked)).help(format!("{need} need you")));
    }
    r
}

fn name(t: &Theme, name: String, owner: Option<String>, current: bool) -> El {
    let font = sans(13.0, if current { 500.0 } else { 400.0 }, 1.4);
    let ink = if current { t.text } else { t.text.alpha(0.86) };
    let mut r = row().grow(1.0).shrink().min_w(0.0);
    if let Some(owner) = owner {
        r = r.child(text(owner).font(font).color(t.muted));
    }
    r.child(text(name).font(font).color(ink).group_color(t.text).truncate())
}

/// `.panel-row__count`: mono 11, tabular, muted unless hot.
fn count(words: String, ink: rune::Color) -> El {
    text(words).font(mono(11.0, 400.0, 1.5)).color(ink)
}

/// `.panel-heading`.
fn heading(t: &Theme) -> El {
    text("REPOSITORIES")
        .key("github-panel-heading")
        .font(sans(11.0, 600.0, 1.5).tracking(0.66))
        .color(t.muted.alpha(0.8))
        .mx(8.0)
        .mt(12.0)
        .mb(4.0)
}

fn repositories(s: &State, t: &Theme, work: Option<&Work>) -> Vec<El> {
    let b = &state_ref(&s.places).bookmarks;
    if b.list.is_empty() {
        return vec![if b.loading {
            row()
                .key("github-bookmarks-loading")
                .label("Loading the repositories you follow…")
                .w_full()
                .center()
                .padding(12.0)
                .child(files_tab::spinner(t.muted, 16.0))
        } else if let Some(why) = &b.error {
            compact_error(t, "Couldn't load the repositories you follow.", why)
        } else {
            empty_state::compact_bare(t, "github-panel-empty", "Follow a repository to see its pull requests and issues here")
        }];
    }
    let at = &state_ref(&s.places).at;
    b.list.iter().map(|mark| bookmark(t, mark, current_repo(at, mark), counts_for(work, &mark.full_name))).collect()
}

/// `isCurrent`: the repository's page or anything under it, ignoring case; `acme/rock` is not `acme/rocket`.
pub fn current_repo(at: &Where, mark: &Bookmark) -> bool {
    match at {
        Where::Home => false,
        Where::Repo { owner, repo } | Where::Item { owner, repo, .. } => {
            format!("{owner}/{repo}").to_lowercase() == mark.full_name.to_lowercase()
        }
    }
}

/// `countsFor`: "{open pull requests} · {open issues}" from the work answer, matched ignoring case.
pub fn counts_for(work: Option<&Work>, full_name: &str) -> Option<String> {
    let r = work?.repos.iter().find(|r| r.full_name.eq_ignore_ascii_case(full_name))?;
    Some(format!("{} · {}", r.open_pull_requests, r.open_issues))
}

/// `.bookmark-item`: the row and its Stop following button, which takes the counts' place while the row has the pointer.
fn bookmark(t: &Theme, mark: &Bookmark, current: bool, counts: Option<String>) -> El {
    let (owner, repo) = (mark.owner.clone(), mark.name.clone());
    let full = mark.full_name.clone();
    let mut r = line(t, format!("github-bookmark-{full}"), full.clone(), current, &FOLDER_GIT_2)
        .help(full.clone())
        .child(name(t, repo.clone(), Some(format!("{owner}/")), current))
        .on_press(move |s: &mut State| {
            crate::plugins::github::state(&mut s.places).at = Where::Repo { owner: owner.clone(), repo: repo.clone() }
        });
    if let Some(counts) = counts {
        r = r.child(count(counts, t.muted).group_color(rune::Color::TRANSPARENT).help("Open pull requests · open issues"));
    }
    let gone = full.clone();
    let stop = el()
        .key(format!("github-bookmark-remove-{full}"))
        .role("button")
        .label(format!("Stop following {full}"))
        .help(format!("Stop following {full}"))
        .size(22.0, 22.0)
        .center()
        .radius(5.0)
        .pin(None, Some(5.0), Some(5.0), None)
        .cursor(Cursor::Pointer)
        .hover(t.wash(8.0))
        .focus_ring(2.0, -2.0, t.accent)
        .hover_group()
        .group_opacity(0.0, 1.0)
        .child(icon(&X, 11.0).color(t.muted).group_color(t.text))
        .on_press(move |s: &mut State| s.github_bookmark_remove_and_reload(&gone));
    el().w_full().hover_group().child(r).child(stop)
}

/// `ErrorNote` compact: no tint, 12 px, a 14 px icon, the reason under the message (medium weight when there is one).
fn compact_error(t: &Theme, message: &str, why: &str) -> El {
    row()
        .key("github-bookmarks-error")
        .role("alert")
        .gap(8.0)
        .items_start()
        .mx(12.0)
        .child(icon(&crate::icons::CIRCLE_ALERT, 14.0).color(t.error_text).mt(1.0))
        .child(
            col()
                .shrink()
                .min_w(0.0)
                .child(text(message.to_string()).font(sans(rem(0.85714), 500.0, 1.33)).color(t.error_text).wrap())
                .child(text(why.to_string()).font(sans(rem(0.85714), 400.0, 1.33)).color(t.error_text).wrap().mt(2.0)),
        )
}

/// `data-testid="github-panel-add"`: Follow a repository.
fn add_row(t: &Theme) -> El {
    line(t, "github-panel-add".into(), "Follow a repository".into(), false, &PLUS)
        .hover_group()
        .child(text("Follow a repository").font(sans(13.0, 400.0, 1.4)).color(t.muted).group_color(t.text).grow(1.0).truncate())
        .on_press_tx(|s: &mut State, tx: &mut rune::Tx| s.github_follow_open(tx))
}
