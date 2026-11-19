//! Go to file (`components/canvas/GoToFileDialog.vue`, rows 9.1 to 9.10 of docs/parity/11-files.md): a dialog with a box, the files
//! already open (nothing typed) or the ones the machine found, and a foot of keys. The state is `fleet::go_to_file`.
use crate::State;
use crate::fleet::canvases::{file_icon, file_name};
use crate::fleet::go_to_file::folder_of;
use crate::icons::SEARCH;
use crate::theme::{Theme, sans};
use crate::ui::overlay::centred;
use crate::ui::rem;
use rune::{Color, Cursor, El, Key, KeyEvent, ScrollSpec, Tx, col, el, icon, row, text, text_input};

pub const INPUT: &str = "go-to-file-input";
const LIST: &str = "go-to-file-list";

/// `max-height: min(360px, 55vh)`.
fn list_height(window_h: f32) -> f32 {
    360f32.min(window_h * 0.55)
}

fn on_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let items = s.go_to_items();
    match ev.key {
        Key::Down | Key::Up if !items.is_empty() => {
            s.go_to.step(if ev.key == Key::Down { 1 } else { -1 }, items.len());
            tx.scroll_into_view(format!("go-to-file-item-{}", items[s.go_to.active]));
            true
        }
        Key::Enter => {
            s.go_to_pick_active(tx);
            true
        }
        _ => false,
    }
}

fn row_of(t: &Theme, path: &str, active: bool) -> El {
    let (name, folder) = (file_name(path).to_string(), folder_of(path).to_string());
    let (ink, fill) = if active { (t.text, t.accent_dim) } else { (t.muted, Color::TRANSPARENT) };
    let target = path.to_string();
    let mut r = row()
        .key(format!("go-to-file-item-{path}"))
        .role("option")
        .label(path.to_string())
        .w_full()
        .h(32.0)
        .px(10.0)
        .gap(8.0)
        .items_center()
        .radius(7.0)
        .fill(fill)
        .cursor(Cursor::Pointer)
        .child(icon(file_icon(path), 14.0).color(ink))
        .child(text(name).font(sans(13.0, 400.0, 1.4)).color(t.text).single_line());
    if !folder.is_empty() {
        r = r.child(text(folder).font(sans(12.0, 400.0, 1.4)).color(ink).truncate().shrink().min_w(0.0));
    }
    r.on_hover(move |s: &mut State| {
        if let Some(i) = s.go_to_items().iter().position(|p| *p == target) {
            s.go_to.active = i;
        }
    })
    .on_press_tx({
        let path = path.to_string();
        move |s: &mut State, tx: &mut Tx| s.go_to_pick(&path, tx)
    })
}

fn quiet(t: &Theme, words: &str, size: f32, weight: f32) -> El {
    el().w_full().px(10.0).py(6.0).child(text(words.to_string()).font(sans(size, weight, 1.4)).color(t.muted).wrap())
}

fn keys(t: &Theme, caps: &[&str], what: &str) -> El {
    let mut r = row().gap(3.0).items_center();
    for cap in caps {
        r = r.child(text(cap.to_string()).font(sans(10.5, 500.0, 1.4)).color(t.muted));
    }
    r.child(text(what.to_string()).font(sans(11.5, 400.0, 1.4)).color(t.muted))
}

/// The panel: `DialogContent` with no padding, 448 wide, no close button.
pub fn panel(s: &State, t: &Theme) -> El {
    let typed = !s.go_to.query.text.trim().is_empty();
    let items = s.go_to_items();
    let input = row()
        .h(46.0)
        .px(14.0)
        .gap(8.0)
        .items_center()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(icon(&SEARCH, 15.0).color(t.muted))
        .child(
            text_input(&s.go_to.query, sans(14.0, 400.0, 1.3))
                .key(INPUT)
                .label("Go to file")
                .single_line()
                .placeholder("Go to file…", t.muted)
                .color(t.text)
                .caret(t.text, t.accent.alpha(0.35))
                .grow(1.0)
                .min_w(0.0)
                .on_edit(|s: &mut State| &mut s.go_to.query)
                .on_input_key(on_key),
        );
    let search = &s.go_to.search;
    let mut list = col().key(LIST).role("listbox").label("Files").w_full().padding(6.0);
    if items.is_empty() {
        list = if typed && search.loading {
            list.child(quiet(t, "Searching…", 12.0, 400.0))
        } else if let (true, Some(why)) = (typed, &search.error) {
            list.child(crate::ui::confirm::error_note(t, "go-to-file-error", "Couldn't search for files.", Some(why)))
        } else if typed {
            list.child(quiet(t, "No files match.", 12.0, 400.0))
        } else {
            list.child(quiet(t, "Type to find a file in this session.", 12.0, 400.0))
        };
    } else {
        if !typed {
            list = list.child(quiet(t, "Open", 11.0, 600.0));
        }
        for (i, path) in items.iter().enumerate() {
            list = list.child(row_of(t, path, i == s.go_to.active));
        }
    }
    let list = list.scroll(ScrollSpec { stick_bottom: false }).max_h(list_height(s.ui.sel.window.1));
    let foot = row()
        .w_full()
        .px(14.0)
        .py(8.0)
        .gap(14.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], t.border)
        .child(keys(t, &["↑", "↓"], "to move"))
        .child(keys(t, &["Enter"], "to open"))
        .child(keys(t, &["Esc"], "to close"));
    let panel = col()
        .key("go-to-file")
        .role("dialog")
        .label("Go to file")
        .w(rem(32.0))
        .max_w(rem(32.0))
        // `DialogContent` is a grid with `gap-4`, so the box, the list and the foot stand 16 apart.
        .gap(16.0)
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .clip_rounded(0.0)
        .child(input)
        .child(list)
        .child(foot)
        .modal(true, |s: &mut State| s.go_to_file_close());
    crate::ui::dialog::lift(t, panel)
}

/// The scrim and the dialog (`Open::GoToFile`).
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    vec![crate::ui::dialog::overlay("go-to-file"), centred("go-to-file-frame", panel(s, t))]
}
