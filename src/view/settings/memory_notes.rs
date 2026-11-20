//! Settings → Memory's notes (`MemorySection.vue`): the toolbar, the heading over each list, the add form and the lists
//! themselves. A note's row is `memory_row.rs`, the expired fold `memory_expired.rs`, Clear all `memory_clear.rs`. The small
//! pieces every one of them uses (a link button, an icon button, the filled button) are here too.
use super::memory_row;
use crate::State;
use crate::fleet::settings::memory::{List, MemoryPage};
use crate::fleet::settings::memory_notes::{Group, ListView, Section};
use crate::icons::{CHEVRON_DOWN, FOLDER_GIT_2, PLUS, SEARCH, SERVER};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, sans};
use crate::ui::text_menu::text_input;
use crate::ui::{Open, rem, select};
use rune::{Cursor, El, Icon, col, icon, row, text};
use std::rc::Rc;

// ─── Buttons ────────────────────────────────────────────────────────────────

pub fn pressable(e: El, key: String, label: &str, disabled: bool, press: impl Fn(&mut State) + 'static) -> El {
    e.key(key).role("button").label(label.to_string()).focusable(true).cursor(Cursor::Pointer).disabled(disabled).on_press(press)
}

/// `.memory-link`: 12px muted, padding 2/6, radius 4, an accent outline on focus.
pub fn link(
    t: &Theme,
    key: String,
    glyph: Option<&'static Icon>,
    label: &str,
    aria: &str,
    disabled: bool,
    press: impl Fn(&mut State) + 'static,
) -> El {
    let mut e = row().gap(3.5).px(7.0).py(2.0).radius(4.0).center_y().focus_ring(2.0, 1.0, t.accent);
    if let Some(glyph) = glyph {
        e = e.child(icon(glyph, 13.0).color(t.muted));
    }
    pressable(e.child(text(label.to_string()).font(sans(12.0, 400.0, 1.33)).color(t.muted)), key, aria, disabled, press)
}

/// `.memory-icon-btn`: 26 × 26, radius 8, a 14px icon; the page's own colour behind it on hover.
pub fn icon_button(
    t: &Theme,
    key: String,
    glyph: &'static Icon,
    aria: &str,
    tip: Option<&str>,
    disabled: bool,
    press: impl Fn(&mut State) + 'static,
) -> El {
    let mut e = row()
        .size(26.0, 26.0)
        .justify_center()
        .center_y()
        .radius(RADIUS_BTN)
        .hover(t.main_bg)
        .focus_ring(2.0, 1.0, t.accent)
        .child(icon(glyph, 14.0).color(t.muted));
    if let Some(tip) = tip {
        e = e.help(tip.to_string());
    }
    pressable(e, key, aria, disabled, press)
}

/// `rounded-md bg-accent px-3 py-1 text-xs font-medium text-primary-foreground disabled:opacity-60`.
pub fn solid(t: &Theme, key: String, label: &str, disabled: bool, press: impl Fn(&mut State) + 'static) -> El {
    let e = row()
        .px(10.5)
        .py(3.5)
        .radius(6.0)
        .fill(t.accent)
        .opacity(if disabled { 0.6 } else { 1.0 })
        .focus_ring(2.0, 1.0, t.accent)
        .child(text(label.to_string()).font(sans(rem(0.75), 500.0, 1.33)).color(t.primary_fg));
    pressable(e, key, label, disabled, press)
}

/// `rounded-md border border-border bg-card-bg p-2 text-sm`, the text area of the add form and of an edit.
pub fn area(t: &Theme, edit: &rune::TextEdit, focused: bool, placeholder: &str, label: &str) -> El {
    text_input(edit, sans(rem(0.875), 400.0, 1.43))
        .placeholder(placeholder.to_string(), t.muted)
        .label(label.to_string())
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .h(51.0)
        .px(7.0)
        .py(7.0)
        .radius(6.0)
        .border(1.0, if focused { t.accent } else { t.border })
        .border_layout()
        .fill(t.card_bg)
}

// ─── The toolbar ────────────────────────────────────────────────────────────

/// `.memory-field`: a muted icon and a control, padding 5/10, a 1px border that goes accent while the control has focus.
fn field(t: &Theme, focused: bool) -> El {
    row()
        .gap(7.0)
        .px(10.0)
        .py(5.0)
        .center_y()
        .radius(RADIUS_BTN)
        .border(1.0, if focused { t.accent } else { t.border })
        .border_layout()
        .fill(t.main_bg)
}

fn repository_field(page: &MemoryPage, t: &Theme) -> El {
    let empty = page.overview.as_ref().is_none_or(|o| o.repositories.is_empty());
    let name = if empty { "No repositories yet" } else { page.repository_name() };
    let open = field(t, false)
        .role("combobox")
        .label("Repository")
        .focusable(true)
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 1.0, t.accent)
        .child(icon(&FOLDER_GIT_2, 14.0).color(t.muted))
        .child(text(name.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.text))
        .child(icon(&CHEVRON_DOWN, 12.0).color(t.muted));
    let e = open.key("memory-repository".to_string());
    if empty { e } else { e.on_press(|s: &mut State| s.ui.open = Some(Open::Select("memory-repository"))) }
}

/// The open list under the repository select: each repository by its name.
pub fn repository_list(s: &State, t: &Theme) -> Vec<El> {
    let Some(overview) = &s.settings.memory.overview else { return vec![] };
    let options: Vec<(&str, &str)> = overview.repositories.iter().map(|r| (r.path.as_str(), r.name.as_str())).collect();
    let choose: select::Choose = Rc::new(|s: &mut State, path: &str| {
        s.ui.open = None;
        s.memory_choose_repository(path);
    });
    select::content(t, "memory-repository-list", "memory-repository", 220.0, &options, s.settings.memory.repository.as_deref(), choose)
}

fn search_field(s: &State, t: &Theme) -> El {
    let edit = &s.settings.memory_ui.search;
    field(t, edit.focused).child(icon(&SEARCH, 14.0).color(t.muted)).child(
        text_input(edit, sans(rem(0.875), 400.0, 1.43))
            .key("memory-search".to_string())
            .single_line()
            .placeholder("Search notes".to_string(), t.muted)
            .label("Search notes".to_string())
            .color(t.text)
            .caret(t.text, t.accent.alpha(0.35))
            .w(140.0)
            .on_edit(|s: &mut State| &mut s.settings.memory_ui.search),
    )
}

/// The repository select and the search box, and the count line.
pub fn toolbar(s: &State, t: &Theme) -> El {
    let page = &s.settings.memory;
    row()
        .key("memory-toolbar")
        .w_full()
        .mt(17.5)
        .flex_wrap()
        .center_y()
        .justify_between()
        .gap(7.0)
        .child(row().flex_wrap().center_y().gap(7.0).child(repository_field(page, t)).child(search_field(s, t)))
        .child(text(page.count_line()).key("memory-count".to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted))
}

// ─── A list ─────────────────────────────────────────────────────────────────

/// The title over a list and its buttons.
fn heading(s: &State, t: &Theme, v: &ListView) -> El {
    let page = &s.settings.memory;
    let glyph = if v.list == List::Machine { &SERVER } else { &FOLDER_GIT_2 };
    let count = if page.search_on() {
        format!("{} {}", v.total, if v.total == 1 { "note" } else { "notes" })
    } else {
        format!("{}/{}", v.total, v.max)
    };
    let word = v.list.word();
    let list = v.list;
    let mut buttons = row().center_y().gap(3.5);
    if list == List::Machine || page.repository.is_some() {
        buttons =
            buttons.child(link(t, format!("memory-add-{word}"), Some(&PLUS), "Add a note", "Add a note", false, move |s: &mut State| {
                s.memory_start_adding(list)
            }));
    }
    if v.total > 0 {
        let label = format!("Clear these {}", v.total);
        buttons = buttons.child(link(t, format!("memory-clear-{word}"), None, &label, &label, page.saving, move |s: &mut State| {
            s.memory_clear_list(list)
        }));
    }
    row()
        .key(format!("memory-heading-{word}"))
        .w_full()
        .mt(17.5)
        .center_y()
        .justify_between()
        .gap(7.0)
        .child(
            row()
                .center_y()
                .gap(7.0)
                .child(icon(glyph, 13.0).color(t.muted))
                .child(text(v.title.to_uppercase()).font(sans(11.0, 600.0, 1.3).tracking(0.025 * 11.0)).color(t.muted))
                .child(text(count).font(sans(11.0, 400.0, 1.3)).color(t.muted)),
        )
        .child(buttons)
}

/// The form that opens under a heading: a text area, Save note and Cancel.
fn add_form(s: &State, t: &Theme, v: &ListView) -> El {
    let ui = &s.settings.memory_ui;
    let word = v.list.word();
    let list = v.list;
    let placeholder = match list {
        List::Machine => "Something true for every repository on this machine".to_string(),
        List::Repository => format!("Something true for {} on any machine", s.settings.memory.repository_name()),
    };
    let field = area(t, &ui.draft, ui.draft.focused, &placeholder, "Add a note")
        .key(format!("memory-draft-{word}"))
        .on_edit(|s: &mut State| &mut s.settings.memory_ui.draft)
        .on_submit(|s: &mut State, _| s.memory_save_draft())
        .on_input_key(|s: &mut State, ev: &rune::KeyEvent, _: &mut crate::Tx| {
            let esc = ev.key == rune::Key::Escape;
            if esc {
                s.memory_cancel_adding();
            }
            esc
        });
    let save_off = s.settings.memory.saving || ui.draft.text.trim().is_empty();
    col()
        .key(format!("memory-add-form-{word}"))
        .w_full()
        .mt(7.0)
        .gap(7.0)
        .px(10.5)
        .py(10.5)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(field)
        .child(
            row()
                .gap(7.0)
                .center_y()
                .child(solid(t, format!("memory-save-{word}"), "Save note", save_off, |s: &mut State| s.memory_save_draft()))
                .child(link(t, format!("memory-cancel-{word}"), None, "Cancel", "Cancel", false, |s: &mut State| s.memory_cancel_adding())),
        )
}

/// `EmptyState bordered compact`, its dashes and all.
fn empty(t: &Theme, key: String, words: &str) -> El {
    col()
        .key(key)
        .w_full()
        .mt(7.0)
        .items_center()
        .px(10.5)
        .py(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .border_dash(3.0, 3.0)
        .child(text(words.to_string()).font(sans(rem(0.875), 500.0, 1.43)).color(t.text).align(rune::Align::Center).wrap())
}

/// The heading row over the two groups of a list: what it is, and how full.
fn group_head(t: &Theme, key: String, title: &str, meta: &str) -> El {
    row()
        .key(key)
        .w_full()
        .mt(12.0)
        .justify_between()
        .gap(12.0)
        .child(text(title.to_string()).font(sans(12.0, 600.0, 1.33)).color(t.text))
        .child(text(meta.to_string()).font(sans(12.0, 400.0, 1.33)).color(t.muted))
}

fn section(s: &State, t: &Theme, v: &ListView, sec: &Section) -> Vec<El> {
    let word = v.list.word();
    let searching = !s.settings.memory_ui.search.text.trim().is_empty();
    let mut out = vec![];
    if let (Some(title), Some(meta)) = (sec.title, &sec.meta) {
        let id = if sec.group == Group::Search { "search" } else { "always" };
        out.push(group_head(t, format!("memory-group-{word}-{id}"), title, meta));
    }
    if !sec.items.is_empty() {
        let key = if sec.group == Group::Search { format!("memory-list-{word}-search") } else { format!("memory-list-{word}") };
        let mut list = col()
            .key(key)
            .w_full()
            .mt(7.0)
            .radius(RADIUS_CARD)
            .border(1.0, t.border)
            .border_layout()
            .fill(t.main_bg)
            .clip_rounded(RADIUS_CARD);
        for (at, note) in sec.items.iter().enumerate() {
            list = list.child(memory_row::row_for(s, t, note, at == 0));
        }
        out.push(list);
    } else if s.settings.memory_ui.adding != Some(v.list) || sec.group == Group::Search {
        let words = if searching { "No notes match" } else { sec.empty.as_str() };
        out.push(empty(t, format!("memory-empty-{word}-{}", sec.group as u8), words));
    }
    out
}

/// Both lists, one after the other, each with its heading, its add form and its notes.
pub fn lists(s: &State, t: &Theme) -> Vec<El> {
    let query = s.settings.memory_ui.search.text.clone();
    let mut out = vec![];
    for v in s.settings.memory.lists(&query) {
        out.push(heading(s, t, &v));
        if s.settings.memory_ui.adding == Some(v.list) {
            out.push(add_form(s, t, &v));
        }
        for sec in &v.sections {
            out.extend(section(s, t, &v, sec));
        }
    }
    out
}
