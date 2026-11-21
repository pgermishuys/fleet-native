//! The issue filter bar (`components/IssueFilterBar.vue`, `FilterExpressionField.vue` and `components/filters/*`, rows 9.x of
//! docs/parity/21-github.md): the expression field, the Open and Closed toggles, the Label, Author, Milestone and Assignee
//! popovers, and the sort menu. What the controls do is `bar`; this file only draws them and the menu that is open.
use crate::State;
use crate::icons::{ARROW_UP_DOWN, CHECK, CIRCLE_CHECK, CIRCLE_DOT, MILESTONE, SEARCH, TAG, USER, USER_CHECK, X};
use crate::plugins::github::bar::{EXPR_INPUT, Look, MENU_INPUT, Menu, Row, SORTS, sort_words};
use crate::plugins::github::filter::StateChoice;
use crate::plugins::github::state_ref;
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, button_ink};
use crate::ui::confirm::spinner;
use crate::ui::menu::{self, Item, Mark};
use crate::ui::overlay::{anchored, scrim};
use crate::ui::rem;
use crate::ui::text_menu::text_input;
use crate::view::gh::{label_color, reviewer_avatar};
use rune::{Align, Color, Cursor, El, Key, KeyEvent, ScrollSpec, Side, Tx, col, el, icon, row, text};
use std::rc::Rc;

fn row_key(i: usize) -> String {
    format!("github-issues-row-{i}")
}

/// `FilterExpressionField`: the field with its magnifier (a small spinner while searching) and, once there is a filter, Clear.
fn field(s: &State, t: &Theme, searching: bool) -> El {
    let g = state_ref(&s.places);
    let input = text_input(&g.repo.bar.expr, mono(12.0, 400.0, 1.4))
        .key(EXPR_INPUT)
        .label("Filter issues")
        .single_line()
        .placeholder("Filter issues\u{2026} e.g. is:open label:bug author:octocat", t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .grow(1.0)
        .min_w(0.0)
        .on_edit(|s: &mut State| &mut crate::plugins::github::state(&mut s.places).repo.bar.expr)
        .on_input_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| match ev.key {
            Key::Enter => {
                s.github_bar_commit();
                tx.blur();
                true
            }
            Key::Escape => {
                s.github_bar_drop_edit();
                tx.blur();
                true
            }
            _ => false,
        });
    // The input has 28 px of room on the right whether or not Clear is there.
    let clear = if s.github_bar_has_filters() {
        row()
            .key("github-issues-clear")
            .role("button")
            .label("Clear filters")
            .size(20.0, 20.0)
            .center()
            .radius(4.0)
            .hover_group()
            .cursor(Cursor::Pointer)
            .focusable(true)
            .focus_ring(2.0, 1.0, t.accent)
            .child(icon(&X, 11.0).color(t.muted).group_color(t.text))
            .on_press(|s: &mut State| s.github_bar_clear())
    } else {
        el().size(20.0, 20.0)
    };
    row()
        .key("github-issues-field")
        .w_full()
        .h(32.0)
        .pl(8.0)
        .pr(4.0)
        .gap(9.0)
        .center_y()
        .radius(RADIUS_BTN)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .focus_within_ring(1.0, -1.0, t.accent)
        .child(if searching { spinner(t.muted) } else { icon(&SEARCH, 13.0).color(t.muted) })
        .child(input)
        .child(clear)
}

/// `.filter-badge`: how many are chosen.
fn badge(t: &Theme, key: &str, n: usize) -> El {
    row()
        .key(key.to_string())
        .min_w(16.0)
        .h(16.0)
        .px(4.0)
        .center()
        .fill(t.accent.alpha(0.2))
        .child(text(n.to_string()).key(format!("{key}-count")).font(sans(10.0, 600.0, 1.0)).color(t.accent_text))
}

/// One of the four popovers' buttons: the `filter` look at `sm`, an icon, the words and, while something is chosen, the badge.
fn trigger(s: &State, t: &Theme, menu: Menu, glyph: &'static rune::Icon, words: &str, chosen: usize) -> El {
    let mut b = button(t, Variant::Filter { active: false }, Size::Sm, Some(glyph), words)
        .key(menu.trigger())
        .role("button")
        .focusable(true)
        .on_press_tx(move |s: &mut State, tx: &mut Tx| {
            s.github_bar_toggle(menu);
            tx.focus(MENU_INPUT, false);
        });
    if chosen > 0 {
        b = b.child(badge(t, &format!("{}-badge", menu.trigger()), chosen));
    }
    let _ = s;
    b
}

/// Open and Closed: `data-active` fills the chosen one with the accent's wash.
fn state_toggle(t: &Theme, key: &str, on: bool, glyph: &'static rune::Icon, words: &str, choice: StateChoice) -> El {
    let ink = if on { t.accent_text } else { t.muted };
    // `:size="12"` is overridden by the button's `size-4` rule on svgs without a size class: 14 px.
    let lead = if on { icon(glyph, rem(1.0)).color(ink) } else { icon(glyph, rem(1.0)).color(ink).group_color(t.text) };
    button_ink(t, Variant::Filter { active: on }, Size::Sm, Some(lead), words, Some(ink))
        .key(key.to_string())
        .role("button")
        .checked(on)
        .focusable(true)
        .on_press(move |s: &mut State| s.github_bar_state(choice))
}

/// The bar: the field, then the controls.
pub fn bar(s: &State, t: &Theme, searching: bool) -> El {
    let g = state_ref(&s.places);
    let f = &g.repo.issues;
    let controls = row()
        .key("github-issues-controls")
        .w_full()
        .flex_wrap()
        .center_y()
        .gap(2.0)
        .child(state_toggle(t, "github-issues-open", f.state == StateChoice::Open, &CIRCLE_DOT, "Open", StateChoice::Open))
        .child(state_toggle(t, "github-issues-closed", f.state == StateChoice::Closed, &CIRCLE_CHECK, "Closed", StateChoice::Closed))
        .child(el().w(1.0).h(16.0).mx(4.0).fill(t.border))
        .child(trigger(s, t, Menu::Label, &TAG, "Label", f.labels.len()))
        .child(trigger(s, t, Menu::Author, &USER, "Author", usize::from(f.author.is_some())))
        .child(trigger(s, t, Menu::Milestone, &MILESTONE, "Milestone", usize::from(f.milestone.is_some())))
        .child(trigger(s, t, Menu::Assignee, &USER_CHECK, "Assignee", usize::from(f.assignee.is_some())))
        .child(el().grow(1.0))
        .child(sort_button(t, sort_words(f)));
    col().key("github-issues-bar").w_full().gap(8.0).pt(10.0).pb(4.0).child(field(s, t, searching)).child(controls)
}

/// `SortControl`'s button: 28 high, 11 px, the text colour.
fn sort_button(t: &Theme, words: &str) -> El {
    row()
        .key(Menu::Sort.trigger())
        .role("button")
        .label(words.to_string())
        .h(28.0)
        .px(8.0)
        .py(2.0)
        .gap(4.0)
        .center_y()
        .radius(6.0)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(icon(&ARROW_UP_DOWN, 12.0).color(t.text))
        .child(text(words.to_string()).key("github-issues-sort-words").font(sans(11.0, 400.0, 1.4)).color(t.text))
        .on_press(|s: &mut State| s.github_bar_toggle(Menu::Sort))
}

/// `.filter-checkbox`: 14 square, radius 3, a border; chosen: the accent with a check.
fn checkbox(t: &Theme, on: bool) -> El {
    let b = el().size(14.0, 14.0).center().radius(3.0).border(1.0, if on { t.accent } else { t.border }).fill(if on {
        t.accent
    } else {
        Color::TRANSPARENT
    });
    if on { b.child(icon(&CHECK, 10.0).color(t.primary_fg)) } else { b }
}

/// `CommandItem`: a row of the list. The highlighted one is filled with the accent; what Fleet draws in `--muted` stays so.
fn item(s: &State, t: &Theme, r: &Row, at: usize, hl: bool) -> El {
    let _ = s;
    let (ink, fill) = if hl { (t.primary_fg, t.accent) } else { (t.text, Color::TRANSPARENT) };
    let check = |on: bool| icon(&CHECK, 13.0).color(ink).opacity(if on { 1.0 } else { 0.0 });
    let name = |words: &str| text(words.to_string()).font(sans(13.0, 400.0, 1.4286)).color(ink).truncate().min_w(0.0);
    let mut e = row()
        .key(row_key(at))
        .role("option")
        .label(r.title.clone())
        .w_full()
        .px(rem(0.5))
        .py(rem(0.375))
        .center_y()
        .radius(3.5)
        .cursor(Cursor::Default)
        .fill(fill);
    let inner = match &r.look {
        Look::Clear => row().child(text("Clear selection").font(sans(13.0, 400.0, 1.4286)).color(t.muted)),
        Look::Label { color, checked } => row()
            .gap(8.0)
            .center_y()
            .grow(1.0)
            .min_w(0.0)
            .child(checkbox(t, *checked))
            .child(el().size(10.0, 10.0).radius(5.0).fill(label_color(color)))
            .child(name(&r.title)),
        Look::User { checked } => row()
            .gap(8.0)
            .center_y()
            .grow(1.0)
            .min_w(0.0)
            .child(check(*checked))
            .child(reviewer_avatar(t, &format!("github-issues-avatar-{at}"), &r.title, None, 16.0))
            .child(name(&r.title)),
        Look::Plain { checked } => row().gap(8.0).center_y().grow(1.0).min_w(0.0).child(check(*checked)).child(name(&r.title)),
        Look::Milestone { checked, meta } => row()
            .gap(8.0)
            .center_y()
            .grow(1.0)
            .min_w(0.0)
            .child(check(*checked))
            .child(col().min_w(0.0).child(name(&r.title)).child(text(meta.clone()).font(sans(10.0, 400.0, 1.3)).color(t.muted))),
    };
    e = e.child(inner);
    e.on_hover(move |s: &mut State| crate::plugins::github::state(&mut s.places).repo.bar.hl = at)
        .on_press(move |s: &mut State| s.github_bar_pick(at))
}

fn on_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    match ev.key {
        Key::Down | Key::Up => {
            if let Some(at) = s.github_bar_step(if ev.key == Key::Down { 1 } else { -1 }) {
                tx.scroll_into_view(row_key(at));
            }
            true
        }
        Key::Enter => {
            s.github_bar_pick_highlighted();
            true
        }
        _ => false,
    }
}

/// `Command`: the search box, then the list (at most 224 high).
fn command(s: &State, t: &Theme, menu: Menu) -> El {
    let g = state_ref(&s.places);
    let input = row()
        .h(rem(2.25))
        .px(rem(0.75))
        .gap(rem(0.5))
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(icon(&SEARCH, rem(1.0)).color(t.text).opacity(0.5))
        .child(
            text_input(&g.repo.bar.query, sans(rem(0.875), 400.0, 1.3))
                .key(MENU_INPUT)
                .label(menu.placeholder().trim_end_matches('\u{2026}').to_string())
                .single_line()
                .placeholder(menu.placeholder(), t.muted)
                .color(t.text)
                .caret(t.text, t.accent.alpha(0.35))
                .grow(1.0)
                .h(rem(2.25))
                .py((rem(2.25) - rem(0.875) * 1.3) / 2.0 - 1.0)
                .on_edit(|s: &mut State| &mut crate::plugins::github::state(&mut s.places).repo.bar.query)
                .on_input_change(|s: &mut State, _: &mut Tx| s.github_bar_typed())
                .on_input_key(on_key),
        );
    let rows = s.github_bar_rows();
    let (loading, none) = s.github_bar_list_state();
    let mut list = col().key("github-issues-list").role("listbox").label("Choices").w_full().padding(rem(0.25));
    if loading {
        list = list.child(el().key("github-issues-loading").w_full().center().padding(12.0).child(spinner(t.muted)));
    } else if none && !g.repo.bar.query.text.is_empty() {
        // reka's `CommandEmpty` draws only once something is typed.
        list = list.child(
            el().w_full()
                .py(rem(1.5))
                .center()
                .child(text(menu.empty_words()).key("github-issues-empty").font(sans(rem(0.875), 400.0, 1.4)).color(t.text)),
        );
    }
    for (i, r) in rows.iter().enumerate() {
        list = list.child(item(s, t, r, i, g.repo.bar.hl == i));
    }
    // reka drops the highlight when the pointer leaves the list.
    let list = list.scroll(ScrollSpec { stick_bottom: false }).max_h(224.0).on_hover_change(|s: &mut State, inside: bool, _: &mut Tx| {
        if !inside {
            crate::plugins::github::state(&mut s.places).repo.bar.hl = usize::MAX;
        }
    });
    col().key("github-issues-command").w_full().child(input).child(list)
}

fn sort_entries(s: &State) -> Vec<menu::Entry> {
    let g = state_ref(&s.places);
    SORTS
        .iter()
        .enumerate()
        .map(|(i, (words, field, direction))| {
            let (field, direction) = (*field, *direction);
            let on = g.repo.issues.sort == field && g.repo.issues.direction == direction;
            menu::Entry::Item(
                Item::new(*words)
                    .id(format!("github-issues-sort-{i}"))
                    .mark(Mark::Radio(on))
                    .on(move |s: &mut State, _: &mut Tx| s.github_bar_sort(field, direction)),
            )
        })
        .collect()
}

/// The scrim and the open menu, anchored under its button (`align="start"`; the sort menu `align="end"`).
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let Some(open) = state_ref(&s.places).repo.bar.menu else { return Vec::new() };
    if s.active_rail() != Some(crate::Rail::Plugin(crate::plugins::github::ID)) {
        return Vec::new();
    }
    let close: Rc<dyn Fn(&mut State)> = Rc::new(|s: &mut State| s.github_bar_close());
    let key = if open == Menu::Sort { "github-issues-sort-menu" } else { "github-issues-menu" };
    let body = if open == Menu::Sort {
        menu::panel(t, key, sort_entries(s)).w(open.width())
    } else {
        let mut p = col()
            .key(key)
            .role("dialog")
            .label(open.placeholder().trim_end_matches('\u{2026}').to_string())
            .w(open.width())
            .items_stretch()
            .padding(rem(1.0))
            .fill(t.card_bg)
            .border(1.0, t.border)
            .border_layout()
            .child(command(s, t, open));
        for shadow in t.menu_shadows() {
            p = p.box_shadow(shadow);
        }
        p
    };
    let panel = {
        let close = close.clone();
        body.focus_return(open.trigger().to_string()).modal(true, move |s: &mut State| close(s))
    };
    let align = if open == Menu::Sort { Align::End } else { Align::Start };
    vec![
        scrim("github-issues-scrim", {
            let close = close.clone();
            move |s: &mut State| close(s)
        }),
        anchored(panel, open.trigger(), Side::Bottom, align, 4.0),
    ]
}
