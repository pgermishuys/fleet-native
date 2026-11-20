//! Ports `NSD/FolderPicker.vue`: the folder chip under the box and the menu it opens above itself, list view only (rows B2.1-B2.103;
//! the path and clone views are pieces G and H). What the rows are is `fleet::new_session::folder`; what the keys do is
//! `fleet::new_session::folder_menu`.
use crate::fleet::new_session::folder::{self, FolderOption, Group, Mark};
use crate::fleet::new_session::folder_menu::{CHIP, Make, SEARCH_KEY, View, row_key};
use crate::fleet::new_session::plan::tilde_path;
use crate::fleet::new_session::request::Folder;
use crate::icons::*;
use crate::theme::{Theme, mono, sans};
use crate::ui::chip::{Chip, Option_, option, pop, pop_label, pop_note, pop_separator};
use crate::ui::text_menu::text_input;
use crate::view::folder_menu_path;
use crate::{State, Tx};
use rune::{Align, Cursor, Cycle, El, Key, KeyEvent, ScrollSpec, Side, col, icon, row, text};

/// `.ns-pop`'s width, and the list's height limit (`min(340px, 50vh)`; the window is never shorter than 680).
const WIDTH: f32 = 344.0;
const LIST_MAX_H: f32 = 340.0;

/// The folder chip (`FolderPicker.vue`'s trigger, B2.1-B2.17): a mark for the kind of folder (a spinner while a clone for it runs),
/// its name, a `new` tag for a folder made from this menu, a chevron; the accent-tinted border while nothing is chosen.
pub fn trigger(s: &State, t: &Theme) -> El {
    let chosen = s.new_draft.draft().and_then(|d| d.folder.as_ref());
    let cloning = s.folder_menu.make == Make::Cloning && s.folder_menu.clone_name.is_some() && !s.folder_menu.clone_superseded;
    let glyph = match chosen {
        Some(Folder::None) => &MESSAGE_SQUARE,
        Some(Folder::Directory(_)) => &FOLDER,
        _ => &FOLDER_GIT_2,
    };
    let path = match chosen {
        Some(Folder::Repository(p) | Folder::Directory(p)) => Some(p.clone()),
        _ => None,
    };
    let disabled = s.new_draft.draft().is_some_and(|d| d.is_starting);
    Chip::new(CHIP, s.folder_chip_label(chosen))
        .glyph(glyph)
        .busy(cloning)
        .attention(chosen.is_none() && !cloning)
        .new_tag(!cloning && s.folder_is_new(chosen))
        .open(s.folder_menu_open())
        .help(path)
        .disabled(disabled)
        .view(t, |s: &mut State, tx: &mut Tx| s.open_folder_menu(tx))
}

/// The spinner a row shows while its create, clone or check runs.
pub fn spinner(t: &Theme) -> El {
    static FADE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
    icon(&LOADER_CIRCLE, 14.0).color(t.muted).cycle(Cycle { period: 1.2, phase: 0.0, stops: &FADE, still: 1.0, fps: 30.0 })
}

fn mark(t: &Theme, o: &FolderOption, busy: bool) -> El {
    if busy {
        return spinner(t);
    }
    let (glyph, color) = match o.mark {
        Mark::Repository => (&FOLDER_GIT_2, t.muted),
        Mark::Directory => (&FOLDER, t.muted),
        Mark::Browse => (&FOLDER_OPEN, t.muted),
        Mark::Create => (&FOLDER_PLUS, t.accent),
        Mark::Clone => (&GITHUB, t.muted),
        Mark::Chat => (&MESSAGE_SQUARE, t.muted),
    };
    icon(glyph, 14.0).color(color)
}

/// `.ns-folder-search`: the magnifier and the box. Down, Up and Enter move and run the highlighted row (B2.45-B2.47); everything
/// else, Tab and Esc included, falls through (B2.49).
fn search(s: &State, t: &Theme) -> El {
    let field = text_input(&s.folder_menu.query, sans(13.0, 400.0, 1.3))
        .key(SEARCH_KEY)
        .label(if s.folder_allow_create() { "Search, or create a folder" } else { "Search repositories" })
        .placeholder(folder::placeholder(s.folder_allow_create()), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .single_line()
        .grow(1.0)
        .min_w(0.0)
        .on_edit(|s: &mut State| &mut s.folder_menu.query)
        .on_input_change(|s: &mut State, _: &mut Tx| s.folder_query_changed())
        .on_input_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| match ev.key {
            Key::Down => {
                s.folder_step(1, tx);
                true
            }
            Key::Up => {
                s.folder_step(-1, tx);
                true
            }
            Key::Enter => {
                let at = s.folder_menu.highlight;
                s.run_folder_row(at, tx);
                true
            }
            _ => false,
        });
    row()
        .key("new-session-folder-search-box")
        .items_center()
        .gap(7.0)
        .mx(-1.0)
        .mt(-1.0)
        .mb(4.0)
        .px(8.0)
        .pt(5.0)
        .pb(7.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(icon(&SEARCH, 14.0).color(t.muted))
        .child(field)
}

pub fn alert(t: &Theme, key: &str, words: String) -> El {
    row()
        .key(key.to_string())
        .role("alert")
        .mx(8.0)
        .mb(8.0)
        .child(text(words).font(sans(12.5, 400.0, 1.4)).color(t.error_text).wrap().w_full())
}

fn list(s: &State, t: &Theme) -> El {
    let rows = s.folder_rows();
    let hl = s.folder_menu.highlight;
    let busy = s.folder_menu.make;
    let mut c = col().key("new-session-folder-list").role("listbox").label("Folders").w_full();
    for (i, o) in rows.iter().enumerate() {
        if folder::starts_other(&rows, i) {
            c = c.child(pop_separator(t));
        } else if let Some(label) = folder::group_label(&rows, i) {
            c = c.child(pop_label(t, label));
        }
        let making = o.group == Group::New && busy != Make::Idle;
        let title = match (o.group == Group::New, busy) {
            (true, Make::Creating) => "Creating".to_string(),
            (true, Make::Cloning) => "Cloning".to_string(),
            _ => o.title.clone(),
        };
        let detail = if making && busy == Make::Cloning { s.clone_status_text() } else { o.detail.clone() };
        let spec = Option_ {
            key: row_key(i, o),
            icon: None,
            title,
            detail: Some(detail),
            mono_detail: o.mono,
            checked: o.selected,
            disabled: false,
            highlighted: i == hl,
            bold: o.title_name.clone(),
            hint: None,
            tag: None,
            no_mark: false,
            mono_title: false,
        };
        let at = i;
        c = c.child(
            option(t, spec, Some(mark(t, o, making)), move |s, tx| s.run_folder_row(at, tx))
                .on_hover(move |s: &mut State| s.folder_menu.highlight = at),
        );
    }
    if let Some(note) = s.folder_empty_note() {
        c = c.child(pop_note(t, &note));
    }
    c.scroll(ScrollSpec { stick_bottom: false }).max_h(LIST_MAX_H)
}

/// `.ns-folder-root`: the Location the new folder or clone goes in. Fleet's is a native `<select>`; here the box opens its list
/// right under it (an operating-system menu can't be drawn from inside a popover).
fn location(s: &State, t: &Theme) -> Option<El> {
    if !s.folder_shows_location() {
        return None;
    }
    let current = s.folder_root().unwrap_or_default();
    let font = mono(11.5, 400.0, 1.3);
    let select = row()
        .key("new-session-folder-root")
        .role("combobox")
        .label("Location")
        .grow(1.0)
        .min_w(0.0)
        .px(4.0)
        .py(2.0)
        .gap(4.0)
        .center_y()
        .radius(5.0)
        .border(1.0, t.border)
        .cursor(Cursor::Pointer)
        .hover(t.wash(6.0))
        .focus_ring(2.0, 0.0, t.accent)
        .on_press(|s: &mut State| s.folder_menu.root_list = !s.folder_menu.root_list)
        .child(text(tilde_path(&current)).font(font).color(t.text).truncate().grow(1.0).min_w(0.0))
        .child(icon(&CHEVRON_DOWN, 12.0).color(t.muted));
    let mut c = col().w_full().mt(4.0).border_sides([1.0, 0.0, 0.0, 0.0], t.border).child(
        row()
            .px(8.0)
            .pt(6.0)
            .pb(2.0)
            .gap(6.0)
            .center_y()
            .child(icon(&MAP_PIN, 12.0).color(t.muted))
            .child(text("Location").font(sans(12.0, 400.0, 1.3)).color(t.muted))
            .child(select),
    );
    if s.folder_menu.root_list {
        for (n, root) in s.feeds.roots.iter().enumerate() {
            let value = root.clone();
            c = c.child(
                row()
                    .key(format!("new-session-folder-root-{n}"))
                    .role("option")
                    .mx(8.0)
                    .px(4.0)
                    .py(3.0)
                    .radius(5.0)
                    .cursor(Cursor::Pointer)
                    .hover(t.wash(6.0))
                    .on_press(move |s: &mut State| s.pick_folder_root(value.clone()))
                    .child(text(tilde_path(root)).font(font).color(t.text).truncate()),
            );
        }
    }
    Some(c)
}

/// The server's words for a failed create, open or clone, with "Use that folder" after a 409 (B2.100-B2.102).
pub fn make_error(s: &State, t: &Theme) -> Option<El> {
    let error = s.folder_menu.error.as_ref()?;
    let mut line = row()
        .key("new-session-folder-error")
        .role("alert")
        .mx(8.0)
        .mb(8.0)
        .gap(6.0)
        .flex_wrap()
        .child(text(error.clone()).font(sans(12.5, 400.0, 1.4)).color(t.error_text).wrap().w_full());
    if s.folder_menu.existing.is_some() {
        line = line.child(
            row()
                .key("new-session-folder-use-existing")
                .cursor(Cursor::Pointer)
                .focus_ring(2.0, 0.0, t.accent)
                .on_press(|s: &mut State| s.use_existing_folder())
                .child(text("Use that folder").font(sans(12.5, 400.0, 1.4)).color(t.accent_text)),
        );
    }
    Some(line)
}

/// The menu and what dismisses it: it opens above the chip, 6px off, its left edge on the chip's (B2.25).
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let body = if s.folder_menu.warning.is_some() {
        vec![folder_menu_path::warning_panel(s, t)]
    } else if s.folder_menu.view == View::Path {
        folder_menu_path::path_view(s, t)
    } else if s.folder_menu.view == View::Clone {
        crate::view::folder_menu_clone::clone_view(s, t)
    } else {
        list_view(s, t)
    };
    let panel =
        pop(t, "new-session-folder-pop", WIDTH, body).focus_return(CHIP.to_string()).modal(true, |s: &mut State| s.close_folder_menu());
    vec![
        crate::ui::overlay::scrim("new-session-folder-scrim", |s: &mut State| s.close_folder_menu()),
        crate::ui::overlay::anchored(panel, CHIP, Side::Top, Align::Start, 6.0),
    ]
}

fn list_view(s: &State, t: &Theme) -> Vec<El> {
    let mut body = vec![search(s, t)];
    if let Some(c) = s.folder_invalid_char() {
        body.push(alert(t, "new-session-folder-invalid", format!("“{c}” can't be in a folder name.")));
    }
    body.push(list(s, t));
    body.extend(location(s, t));
    body.extend(make_error(s, t));
    body
}
