//! `AutocompletePopup.vue`: the list the `/`, `$` and `@` triggers open above the composer's frame (docs/parity/05-composer.md,
//! piece K). What it lists and how a key moves in it is `composer::triggers`; what feeds it is `fleet::finder`; the model it
//! draws is `fleet::autocomplete::Popup`. Rows take the press without taking focus from the text area.
use super::parts::{diamond, quad, triangle};
use crate::composer::triggers::{Group, Item};
use crate::fleet::autocomplete::Popup;
use crate::icons::*;
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::{State, Tx};
use rune::{Color, Cursor, Cycle, El, Shadow, Span, col, el, icon, rich, row, text};

/// The popup stops growing here and scrolls (`max-height: 340px`).
const MAX_H: f32 = 340.0;
/// The list's padding, and what the footer takes back so its rule runs edge to edge.
const PAD: f32 = 6.0;

const GROUPS: [(Group, &str, Option<&str>); 5] = [
    (Group::Command, "Commands", None),
    (Group::Skill, "Skills", None),
    (Group::Session, "Sessions", Some("Tab to attach")),
    (Group::File, "Files & folders", None),
    (Group::Agent, "Agents", None),
];

/// The popup's box, `width` wide (the composer frame's), to anchor 6 px above that frame.
pub fn popup(s: &State, t: &Theme, p: &Popup, width: f32) -> El {
    let selected = p.items.iter().find(|i| Some(&i.value) == p.selected.as_ref());
    let footer = footer(t, p, selected);
    let mut list = col().key("autocomplete-list").w_full().items_stretch().pt(PAD).px(PAD).pb(if footer.is_some() { 4.0 } else { PAD });
    if p.items.is_empty() {
        list = list.child(state(t, p));
    }
    let mut first = true;
    for (group, label, hint) in GROUPS {
        let rows: Vec<(usize, &Item)> = p.items.iter().enumerate().filter(|(_, i)| i.group == group).collect();
        if rows.is_empty() {
            continue;
        }
        let mut section = col().w_full().items_stretch().mt(if first { 0.0 } else { 2.0 });
        first = false;
        let hint = hint.or(if group == Group::File { p.file_hint.as_deref() } else { None });
        section = section.child(group_label(t, label, hint, group));
        for (index, item) in rows {
            section = section.child(option(s, t, p, index, item, Some(&item.value) == p.selected.as_ref()));
        }
        list = list.child(section);
    }
    let scroller =
        list.scroll(rune::ScrollSpec { stick_bottom: false }).key("autocomplete-scroll").w_full().shrink().min_h(0.0).max_h(MAX_H);
    let mut frame = col()
        .key("autocomplete-popup")
        .role("listbox")
        .label("Autocomplete suggestions")
        .w(width)
        .max_h(MAX_H)
        .items_stretch()
        .radius(RADIUS_CARD)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .box_shadow(Shadow { x: 0.0, y: 16.0, blur: 40.0, spread: -12.0, color: Color::BLACK.alpha(0.45), inset: false })
        .child(scroller);
    if let Some(footer) = footer {
        frame = frame.child(footer);
    }
    frame
}

/// `.autocomplete-popup__group-label`: 11px/600 uppercase, `--muted`, with the hint at the right in plain weight.
fn group_label(t: &Theme, label: &str, hint: Option<&str>, group: Group) -> El {
    let mut r = row()
        .key(format!("autocomplete-group-{group:?}"))
        .w_full()
        .pt(6.0)
        .px(8.0)
        .pb(2.0)
        .fill(t.card_bg)
        .sticky_top(-PAD)
        .child(text(label.to_uppercase()).font(sans(11.0, 600.0, 1.5).tracking(0.04 * 11.0)).color(t.muted));
    if let Some(hint) = hint {
        r = r.child(el().grow(1.0)).child(text(hint).font(sans(11.0, 400.0, 1.5)).color(t.muted));
    }
    r
}

/// The row's icon slot, 16×16.
fn glyph(t: &Theme, item: &Item) -> El {
    let slot = el().size(16.0, 16.0).center();
    match item.group {
        Group::Command => slot.child(icon(&TERMINAL, 14.0).color(t.muted)),
        Group::Skill => slot.child(icon(&SCROLL_TEXT, 14.0).color(t.running)),
        Group::Session => match item.session.as_ref().map(|x| (x.status.as_str(), x.activity.as_deref())) {
            Some(("active", _)) => slot.child(quad(t.text)),
            Some(("waiting_input", _)) => slot.child(diamond(t.idle)),
            Some(("error", _)) => slot.child(triangle(t.error)),
            _ => slot,
        },
        Group::Agent => {
            let mut wrap = el().size(16.0, 16.0).center().child(icon(&BOT, 14.0).color(t.muted));
            if let Some(color) = item.meta.as_deref().and_then(hex) {
                wrap = wrap.child(el().absolute(10.0, 10.0).size(7.0, 7.0).radius(3.5).fill(color).border(1.0, t.card_bg).border_layout());
            }
            slot.child(wrap)
        }
        Group::File if item.is_directory() => slot.child(icon(&FOLDER, 14.0).color(t.accent)),
        Group::File => slot.child(icon(&FILE_TEXT, 14.0).color(t.muted)),
    }
}

/// `#RRGGBB` as a colour (an agent's own).
fn hex(value: &str) -> Option<Color> {
    rune::color::hex(value.trim())
}

/// A label with the first match of what's typed marked in `--accent-text`/600 (session and skill labels only).
fn label(t: &Theme, p: &Popup, item: &Item) -> El {
    let style = sans(13.5, 400.0, 1.4);
    let query = p.query.trim();
    let marked = matches!(item.group, Group::Session | Group::Skill) && !query.is_empty();
    let at = marked.then(|| item.label.to_lowercase().find(&query.to_lowercase())).flatten();
    // A lowercase that changes a character's length moves the offsets: mark only where they still fit the label.
    match at.filter(|a| item.label.is_char_boundary(*a) && item.label.is_char_boundary(a + query.len())) {
        Some(a) if item.label.to_lowercase().len() == item.label.len() => {
            rich(item.label.clone(), vec![Span::new(a, a + query.len()).color(t.accent_text).weight(600.0)])
                .font(style)
                .color(t.text)
                .truncate()
                .shrink()
        }
        _ => text(item.label.clone()).font(style).color(t.text).truncate().shrink(),
    }
}

fn option(s: &State, t: &Theme, p: &Popup, index: usize, item: &Item, selected: bool) -> El {
    let disabled = item.disabled_reason.is_some();
    let note = item.disabled_reason.clone().or_else(|| item.description.clone());
    let mut content = row().grow(1.0).min_w(0.0).gap(9.0).center_y().child(label(t, p, item));
    if let Some(note) = note {
        content = content.child(text(note).font(sans(12.0, 400.0, 1.4)).color(t.muted).truncate().grow(1.0).min_w(0.0));
    }
    let mut main = row()
        .key(format!("autocomplete-option-{index}"))
        .role("option")
        .label(item.label.clone())
        .grow(1.0)
        .min_w(0.0)
        .gap(9.0)
        .px(8.0)
        .py(6.0)
        .center_y()
        .radius(7.0)
        .focusable(false)
        .child(glyph(t, item))
        .child(content);
    if let Some(session) = &item.session {
        main = main.child(text(s.mention_age(session.updated_at)).font(sans(12.0, 400.0, 1.4).features("'tnum' 1")).color(t.muted));
    }
    let value = item.value.clone();
    let mut wrap = row().key(format!("autocomplete-row-{index}")).w_full().radius(7.0).hover(t.accent_dim).child(main);
    if selected {
        wrap = wrap.fill(t.accent_dim);
    }
    if disabled {
        return wrap.opacity(0.5);
    }
    wrap = wrap.on_press_tx({
        let value = value.clone();
        move |s: &mut State, tx: &mut Tx| s.autocomplete_pick(&value, false, tx)
    });
    if item.is_directory() {
        wrap = wrap.child(
            el().key(format!("autocomplete-open-{index}"))
                .role("button")
                .label(format!("Open {}", item.label))
                .help("Open folder (Tab)")
                .w(32.0)
                .center()
                .radius(7.0)
                .focusable(false)
                .hover(t.accent_dim)
                .cursor(Cursor::Pointer)
                .on_press_tx(move |s: &mut State, tx: &mut Tx| s.autocomplete_pick(&value, true, tx))
                .child(icon(&CHEVRON_RIGHT, 14.0).color(t.muted)),
        );
    }
    wrap.cursor(Cursor::Pointer)
}

/// Loading, an error or "No results": what the list says when it has nothing to list.
fn state(t: &Theme, p: &Popup) -> El {
    let line = row().key("autocomplete-state").w_full().center().gap(8.0).px(16.0).py(14.0);
    if p.loading {
        static FADE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
        return line
            .child(icon(&LOADER_CIRCLE, 14.0).color(t.muted).cycle(Cycle { period: 1.2, phase: 0.0, stops: &FADE, still: 1.0, fps: 30.0 }))
            .child(text("Loading suggestions…").font(sans(12.0, 400.0, 1.5)).color(t.muted));
    }
    if let Some(error) = &p.error {
        return line
            .role("alert")
            .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text))
            .child(text(error.clone()).font(sans(12.0, 400.0, 1.5)).color(t.error_text).wrap());
    }
    line.child(text(p.empty_text.clone().unwrap_or_else(|| "No results".into())).font(sans(12.0, 400.0, 1.5)).color(t.muted).wrap())
}

/// `<kbd>`: a 1px border, radius 3, 11px, 4px at each side.
fn kbd(t: &Theme, key: &str) -> El {
    row().px(4.0).radius(3.0).border(1.0, t.border).border_layout().child(text(key).font(sans(11.0, 400.0, 1.5)).color(t.muted))
}

fn say(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(12.0, 400.0, 1.5)).color(t.muted)
}

/// The line under the list for a selected skill, session or file (`.autocomplete-popup__hint`); commands and agents have none.
fn footer(t: &Theme, p: &Popup, selected: Option<&Item>) -> Option<El> {
    let item = selected?;
    let line =
        row().key("autocomplete-footer").w_full().px(14.0).pt(7.0).pb(8.0).gap(4.0).center_y().border_sides([1.0, 0.0, 0.0, 0.0], t.border);
    Some(match item.group {
        Group::Skill => match item.disabled_reason.as_ref().or(item.warning.as_ref()) {
            Some(why) => {
                let ink = if item.disabled_reason.is_some() { t.muted } else { t.idle_text };
                line.child(text(why.clone()).font(sans(12.0, 400.0, 1.5)).color(ink).wrap())
            }
            None => {
                let l = line.child(kbd(t, "Enter")).child(say(t, "add skill"));
                if p.skill_hint.is_empty() { l } else { l.child(say(t, &format!("· {}", p.skill_hint))) }
            }
        },
        Group::Session => line.child(text(p.session_note).font(sans(12.0, 400.0, 1.5)).color(t.muted).wrap()),
        Group::File if item.is_directory() => {
            line.child(kbd(t, "Tab")).child(say(t, "open folder ·")).child(kbd(t, "Enter")).child(say(t, "reference it"))
        }
        Group::File => line.child(kbd(t, "Enter")).child(say(t, "reference it")),
        _ => return None,
    })
}
