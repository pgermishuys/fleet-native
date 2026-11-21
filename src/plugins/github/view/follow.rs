//! Follow a repository (`plugins/builtin/github/components/AddRepositoryDialog.vue`, rows 3.1 to 3.6 of docs/parity/21-github.md): a
//! dialog titled "Follow a repository" holding a command list: a search, then your repositories you do not follow, each with its
//! lock, language and stars. Drawn on `ui::dialog` and the command palette's list; the state and the matching are `follow.rs`.
use crate::State;
use crate::icons::{GITHUB, LOCK, SEARCH, STAR};
use crate::plugins::github::follow::INPUT;
use crate::plugins::github::repos::Repo;
use crate::plugins::github::state_ref;
use crate::theme::{Theme, sans};
use crate::ui::confirm::spinner;
use crate::ui::dialog::{Dialog, dialog};
use crate::ui::rem;
use crate::ui::text_menu::text_input;
use rune::{Color, Cursor, El, Key, KeyEvent, ScrollSpec, Tx, col, el, icon, row, text};

fn on_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    match ev.key {
        Key::Down | Key::Up => {
            if let Some(name) = s.github_follow_step(if ev.key == Key::Down { 1 } else { -1 }) {
                tx.scroll_into_view(row_key(&name));
            }
            true
        }
        Key::Enter => {
            s.github_follow_pick_active();
            true
        }
        _ => false,
    }
}

pub fn row_key(full_name: &str) -> String {
    format!("github-follow-row-{full_name}")
}

/// `.add-repo__item` in a `CommandItem`: the mark, the name, the lock, the language and the stars. The highlighted row's icons take
/// the accent's text colour; the language and the stars keep Fleet's `--muted`.
fn item(t: &Theme, r: &Repo, hl: bool, at: usize) -> El {
    let (ink, fill) = if hl { (t.primary_fg, t.accent) } else { (t.text, Color::TRANSPARENT) };
    let soft = if hl { t.primary_fg } else { t.muted };
    let name = r.full_name.clone();
    let mut e = row()
        .key(row_key(&r.full_name))
        .role("option")
        .label(r.full_name.clone())
        .w_full()
        .px(rem(0.5))
        .py(rem(0.375))
        .gap(rem(0.5))
        .center_y()
        .radius(3.5)
        .cursor(Cursor::Default)
        .fill(fill)
        .child(icon(&GITHUB, 14.0).color(soft))
        .child(text(r.full_name.clone()).font(sans(rem(0.875), 400.0, 1.43)).color(ink).truncate().grow(1.0).shrink().min_w(0.0));
    if r.private {
        e = e.child(icon(&LOCK, 12.0).color(soft));
    }
    if let Some(lang) = &r.language {
        e = e.child(
            el().px(6.0)
                .py(1.0)
                .radius(999.0)
                .fill(t.wash(7.0))
                .child(text(lang.clone()).font(sans(11.0, 400.0, 1.4)).color(t.muted).single_line()),
        );
    }
    if r.stars > 0 {
        e = e.child(
            row()
                .gap(2.0)
                .center_y()
                .child(icon(&STAR, 10.0).color(soft))
                .child(text(r.stars.to_string()).font(sans(11.0, 400.0, 1.4)).color(t.muted).single_line()),
        );
    }
    e.on_hover(move |s: &mut State| crate::plugins::github::state(&mut s.places).follow.active = Some(at))
        .on_press(move |s: &mut State| s.github_follow_pick(&name))
}

/// `CommandEmpty`: `py-6 text-center text-sm`. It renders only while something is typed (`!!filterState.search && count === 0`), so a
/// list that is still loading, or empty, says nothing until you type.
fn empty(t: &Theme, loading: bool) -> El {
    let words = if loading { "Loading repositories\u{2026}" } else { "No repositories found." };
    let line = if loading {
        row()
            .gap(6.0)
            .center_y()
            .child(spinner(t.muted))
            .child(text(words).key("github-follow-loading").font(sans(rem(0.875), 400.0, 1.4)).color(t.muted))
    } else {
        row().child(text(words).key("github-follow-empty").font(sans(rem(0.875), 400.0, 1.4)).color(t.text))
    };
    el().w_full().py(rem(1.5)).center().child(line)
}

fn command(s: &State, t: &Theme) -> El {
    let g = state_ref(&s.places);
    let input = row()
        .h(rem(2.25))
        .px(rem(0.75))
        .gap(rem(0.5))
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(icon(&SEARCH, rem(1.0)).color(t.text).opacity(0.5))
        .child(
            text_input(&g.follow.query, sans(rem(0.875), 400.0, 1.3))
                .key(INPUT)
                .label("Search your repositories")
                .single_line()
                .placeholder("Search your repositories\u{2026}", t.muted)
                .color(t.text)
                .caret(t.text, t.accent.alpha(0.35))
                .grow(1.0)
                .h(rem(2.25))
                .py((rem(2.25) - rem(0.875) * 1.3) / 2.0 - 1.0)
                .on_edit(|s: &mut State| &mut crate::plugins::github::state(&mut s.places).follow.query)
                .on_input_change(|s: &mut State, _: &mut Tx| s.github_follow_typed())
                .on_input_key(on_key),
        );
    let rows = s.github_follow_rows();
    let loading = g.repos.get().loading;
    let mut list = col().key("github-follow-list").role("listbox").label("Your repositories").w_full().padding(rem(0.25));
    if rows.is_empty() && !g.follow.query.text.is_empty() {
        list = list.child(empty(t, loading));
    }
    for (i, r) in rows.iter().enumerate() {
        list = list.child(item(t, r, g.follow.active == Some(i), i));
    }
    let list = list.scroll(ScrollSpec { stick_bottom: false }).max_h(300.0);
    col().key("github-follow-command").w_full().fill(t.card_bg).child(input).child(list)
}

/// The scrim and the dialog, while it is open.
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    if !state_ref(&s.places).follow.open {
        return Vec::new();
    }
    dialog(
        t,
        Dialog {
            key: "github-follow",
            title: "Follow a repository".into(),
            body: vec![command(s, t)],
            close_button: true,
            click_outside: true,
            dismiss: Some(State::github_follow_close),
            ..Dialog::default()
        },
    )
}
