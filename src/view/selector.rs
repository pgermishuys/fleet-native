//! `SelectorDropdown.vue`: the menu the composer's agent and model chips open (docs/parity/05-composer.md, piece E).
//! A 380 px box above the chip with a filter, optional headings, rows with a description and notes at their right,
//! a star on model rows, and a "N more" cue where the list runs on. The rules are in `composer/selector.rs`, what it
//! keeps in `fleet/selector.rs`. The chip itself is in `view/composer.rs`.
use crate::composer::selector::{self as rules, Item, Shown};
use crate::fleet::usage::{ModelProviderUsage, Tone, usage_tooltip};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, sans};
use crate::ui::text_menu::text_input;
use crate::{State, Tx};
use rune::{Color, Cursor, El, Key, KeyEvent, Shadow, col, el, icon, row, text};

/// The usage bar's size (`.selector-dropdown__usage-bar`).
const BAR_W: f32 = 28.0;
const BAR_H: f32 = 4.0;
/// The filter box: 7 px above and below a 12 px line, 1 px border.
const FILTER_H: f32 = 31.0;
/// What the panel takes besides the list: its padding and border, the filter, and the gap below it.
const CHROME: f32 = 10.0 + FILTER_H + 8.0;

/// The whole panel for the menu `name` ("agent" or "model").
pub fn panel(s: &State, t: &Theme, name: &'static str) -> El {
    let shown = s.selector_shown(name);
    let selected = s.selector_selected(name);
    let count: usize = shown.iter().map(|v| v.rows.len()).sum();
    let (win_w, win_h) = s.ui.sel.window;
    let width = if win_w > 0.0 { rules::menu_width(win_w) } else { rules::MENU_WIDTH };
    let height = if win_h > 0.0 { rules::menu_height(win_h) } else { 320.0 };

    let mut list = col().key(format!("{name}-pop-list")).w_full().items_stretch().on_layout(|s: &mut State, [_, y, _, h]| {
        let bottom = y + h;
        if (s.ui.sel.list_bottom - bottom).abs() > 0.5 {
            s.ui.sel.list_bottom = bottom;
        }
    });
    for (list_i, section) in shown.iter().enumerate() {
        if let Some(title) = &section.title {
            list = list.child(heading(t, title, section.provider, section.usage.as_ref(), list_i));
        }
        for r in &section.rows {
            list = list.child(option(s, t, name, section, r.index, &r.item, r.item.id == selected));
        }
    }
    if count == 0 {
        list = list.child(
            text("No matching options").font(sans(12.0, 400.0, 1.2)).color(t.muted).padding(10.0).w_full().align(rune::Align::Center),
        );
    }
    let scroller = list
        .w_full()
        .scroll(rune::ScrollSpec { stick_bottom: false })
        .key(format!("{name}-pop-rows"))
        .max_h(height - CHROME)
        .shrink()
        .min_h(0.0);

    let mut p = col()
        .key(format!("{name}-pop-menu"))
        .role("listbox")
        .label(noun(name, " selector"))
        .w(width)
        .padding(4.0)
        .radius(RADIUS_CARD)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .items_stretch()
        .box_shadow(Shadow { x: 0.0, y: 10.0, blur: 30.0, spread: 0.0, color: Color::BLACK.alpha(0.3), inset: false })
        .child(filter(s, t, name))
        .child(scroller);
    let more = s.ui.sel.more_below(count);
    if more > 0 {
        p = p.child(more_cue(t, more));
    }
    p
}

/// What the menu is for: "Agent", "Effort" or "Model", then `after`.
fn noun(name: &str, after: &str) -> String {
    let word = match name {
        "agent" => "Agent",
        "wf-effort" => "Effort",
        _ => "Model",
    };
    format!("{word}{after}")
}

/// `.selector-dropdown__filter`: focused on open; the arrows move the highlight, Enter picks it, and the field keeps focus.
fn filter(s: &State, t: &Theme, name: &'static str) -> El {
    text_input(&s.ui.sel.filter, sans(12.0, 400.0, 1.2))
        .key(format!("{name}-filter"))
        .label(format!("Filter {}", noun(name, "").to_lowercase()))
        .placeholder(format!("Filter {}…", noun(name, "").to_lowercase()), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .single_line()
        .w_full()
        .on_edit(|s: &mut State| &mut s.ui.sel.filter)
        .on_submit(move |s: &mut State, tx: &mut Tx| s.pick_active(name, tx))
        // Typing puts the highlight on the first match.
        .on_input_change(|s: &mut State, _: &mut Tx| {
            s.keep_selector();
        })
        .on_input_key(move |s: &mut State, ev: &KeyEvent, tx: &mut Tx| match ev.key {
            Key::Down | Key::Up => {
                s.move_selector(name, ev.key == Key::Down, tx);
                true
            }
            _ => false,
        })
        .px(10.0)
        .py(7.0)
        .mb(8.0)
        .radius(RADIUS_BTN)
        .fill(t.wash(3.0))
        .border(1.0, t.border)
        .border_layout()
        .focus_within_ring(2.0, -1.0, t.accent)
}

/// `.selector-dropdown__heading`: 10px uppercase; a provider's is indented and lighter, and may end with its usage.
fn heading(t: &Theme, title: &str, provider: bool, usage: Option<&ModelProviderUsage>, at: usize) -> El {
    let style = if provider { sans(9.5, 500.0, 1.5) } else { sans(10.0, 600.0, 1.5) };
    let mut r = row()
        .w_full()
        .pt(8.0)
        .pb(3.0)
        .pr(10.0)
        .pl(if provider { 16.0 } else { 10.0 })
        .justify_between()
        .child(text(title.to_uppercase()).font(style.tracking(0.06 * if provider { 9.5 } else { 10.0 })).color(t.muted));
    if let Some(u) = usage {
        r = r.child(usage_bar(t, u, at));
    }
    r
}

/// `.selector-dropdown__usage`: "resets 2:05 PM" while dimmed, then a 28 x 4 bar filled to the percent, all in the
/// tone's colour (the track is that colour at 22%). Weight 400, no case change, no tracking.
fn usage_bar(t: &Theme, u: &ModelProviderUsage, at: usize) -> El {
    let ink = match u.tone {
        Tone::Quiet => t.muted,
        Tone::Warn => t.status_waiting_text,
        Tone::Used => t.error_text,
    };
    let mut group = row().key(format!("model-provider-usage{at}")).gap(6.0).center_y().help(usage_tooltip(u));
    if let Some(resets) = &u.reset_text {
        group = group.child(text(resets.clone()).font(sans(9.5, 400.0, 1.5)).color(ink).key(format!("model-provider-resets{at}")));
    }
    let fill = el().w(BAR_W * u.percent.clamp(0, 100) as f32 / 100.0).h(BAR_H).radius(999.0).fill(ink);
    group.child(row().w(BAR_W).h(BAR_H).radius(999.0).fill(ink.alpha(0.22)).child(fill))
}

fn option(s: &State, t: &Theme, name: &'static str, section: &Shown, index: usize, item: &Item, selected: bool) -> El {
    let active = s.ui.sel.active == index;
    let id = item.id.clone();
    let mut copy =
        col().gap(2.0).grow(1.0).min_w(0.0).child(text(item.label.clone()).font(sans(13.0, 500.0, 1.5)).color(t.text).truncate());
    if !item.description.is_empty() {
        copy = copy.child(text(item.description.clone()).font(sans(12.0, 400.0, 1.4)).color(t.muted).wrap());
    }
    let mut notes = col().gap(2.0).items_end();
    let mut any = false;
    if !item.meta.is_empty() && !section.hide_meta {
        notes = notes.child(text(item.meta.clone()).font(sans(12.0, 400.0, 1.5)).color(t.muted));
        any = true;
    }
    if let Some(size) = &item.context_size {
        notes = notes.child(text(size.clone()).font(sans(11.0, 400.0, 1.5)).color(t.muted).key(format!("{name}-ctx{index}")));
        any = true;
    }
    if let Some(price) = &item.price {
        let mut chip = text(price.clone()).font(sans(11.0, 400.0, 1.5)).color(t.muted).key(format!("{name}-price{index}"));
        if let Some(note) = &item.price_note {
            chip = chip.help(note.clone());
        }
        notes = notes.child(chip);
        any = true;
    }
    let mut button = row()
        .key(format!("{name}-pop-item{index}"))
        .role("option")
        .label(item.label.clone())
        .grow(1.0)
        .min_w(0.0)
        .px(10.0)
        .py(7.0)
        .gap(12.0)
        .justify_between()
        .radius(RADIUS_BTN - 2.0)
        .cursor(Cursor::Pointer)
        .on_hover(move |s: &mut State| s.ui.sel.active = index)
        .on_press_tx(move |s: &mut State, tx: &mut Tx| s.pick_selector(name, &id, tx))
        .child(copy);
    if selected {
        button = button.fill(t.wash(6.0));
    }
    if any {
        button = button.child(notes);
    }
    let mut r = row().key(format!("{name}-opt{index}")).w_full().items_stretch().radius(RADIUS_BTN - 2.0).on_layout(
        move |s: &mut State, [_, y, _, h]| {
            let rows = &mut s.ui.sel.rows;
            if rows.len() <= index {
                rows.resize(index + 1, (0.0, 0.0));
            }
            if (rows[index].0 - y).abs() > 0.5 || (rows[index].1 - h).abs() > 0.5 {
                rows[index] = (y, h);
            }
        },
    );
    if active {
        r = r.fill(t.wash(6.0));
    }
    if section_dimmed(section) {
        r = r.opacity(0.55);
    }
    r = r.child(button);
    if let Some(on) = item.starred {
        r = r.child(star(t, name, index, item.id.clone(), on, active));
    }
    r
}

/// Rows under a provider heading whose limit is used up or nearly dim, and stay pickable.
fn section_dimmed(section: &Shown) -> bool {
    section.usage.as_ref().is_some_and(|u| u.dimmed)
}

/// `.selector-dropdown__star`: shown on the highlighted row (hover or keys) and always when starred.
fn star(t: &Theme, name: &str, index: usize, id: String, on: bool, active: bool) -> El {
    let glyph = if on { icon(&STAR_FILLED, 14.0).color(t.idle) } else { icon(&STAR, 14.0).color(t.muted) };
    let label = if on { "Remove from favourites" } else { "Add to favourites" };
    let b = row()
        .key(format!("{name}-star{index}"))
        .role("button")
        .label(label)
        .px(8.0)
        .center_y()
        .cursor(Cursor::Pointer)
        .opacity(if on || active { 1.0 } else { 0.0 })
        .child(glyph);
    if on || active { b.help(label).on_press(move |s: &mut State| s.toggle_star(&id)) } else { b }
}

/// `.selector-dropdown__more`: a fade over the last visible row, with how many are below.
fn more_cue(t: &Theme, more: usize) -> El {
    el().key("selector-more")
        .pin(Some(1.0), None, Some(1.0), Some(1.0))
        .px(12.0)
        .pt(18.0)
        .pb(6.0)
        .radius(RADIUS_CARD - 1.0)
        .gradient(t.card_bg.alpha(0.0), t.card_bg)
        .center()
        .child(text(format!("{more} more")).font(sans(11.0, 400.0, 1.5)).color(t.muted))
}
