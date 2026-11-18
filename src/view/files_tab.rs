//! Ports `components/canvas/FilesCanvas.vue` and the names half of `components/session/FileBrowserPanel.vue`: the Files tab's body.
//! A toolbar (the search box and Refresh), the "Search in" switch, and under them the tree or what stands in for it: "Loading
//! files…", the error, or "No files found". "File contents" is Find in files' (component 12) and drawn off until then; the names
//! search is piece E's, so the box takes what is typed and nothing yet answers it.
use super::file_tree::tree_row;
use crate::State;
use crate::fleet::file_browser::SearchIn;
use crate::icons::*;
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::ui::{confirm, empty_state};
use rune::{Cursor, Cycle, El, Key, KeyEvent, Tx, col, el, icon, row, text, text_input};

pub const FIELD: &str = "files-search";

/// "File contents" waits for Find in files (component 12, piece P).
pub const CONTENTS_OFF: &str = "Searching inside files comes with Find in files";

/// `Spinner` at `size`: the turning circle.
fn spinner(ink: rune::Color, size: f32) -> El {
    static FADE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
    icon(&LOADER_CIRCLE, size).color(ink).cycle(Cycle { period: 1.2, phase: 0.0, stops: &FADE, still: 1.0, fps: 30.0 })
}

pub fn files_tab(s: &State, t: &Theme) -> El {
    let panel =
        col().key("files-panel").w_full().gap(6.0).pt(8.0).items_stretch().child(toolbar(s, t)).child(switch(s, t)).child(content(s, t));
    col().key("files-canvas").w_full().grow(1.0).min_h(0.0).items_stretch().child(panel).scroll(rune::ScrollSpec { stick_bottom: false })
}

fn toolbar(s: &State, t: &Theme) -> El {
    let browser = &s.file_browser;
    let field = text_input(&browser.field, sans(13.0, 400.0, 1.3))
        .key(FIELD)
        .label("Search files")
        .placeholder("Search files\u{2026}", t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .single_line()
        .grow(1.0)
        .min_w(0.0)
        .on_edit(|s: &mut State| &mut s.file_browser.field)
        .on_input_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| match ev.key {
            // Esc clears the query and leaves the field.
            Key::Escape => {
                s.clear_files_search();
                tx.blur();
                true
            }
            _ => false,
        });
    let mut search = row()
        .key("files-search-box")
        .grow(1.0)
        .min_w(0.0)
        .h(30.0)
        .px(10.0)
        .gap(6.0)
        .center_y()
        .radius(RADIUS_BTN)
        .fill(t.wash(5.0))
        .focus_within_ring(1.0, -1.0, t.accent.alpha(0.55))
        .child(icon(&SEARCH, 14.0).color(t.muted))
        .child(field);
    if !browser.field.text.is_empty() {
        search = search.child(
            el().key("files-search-clear")
                .role("button")
                .label("Clear search")
                .help("Clear search")
                .padding(2.0)
                .radius(2.0)
                .cursor(Cursor::Pointer)
                .hover(t.border)
                .hover_group()
                .child(icon(&X, 14.0).color(t.muted).group_color(t.text))
                .on_press(|s: &mut State| s.clear_files_search()),
        );
    }
    let loading = browser.root_loading();
    let mut refresh = el()
        .key("files-refresh")
        .role("button")
        .label("Refresh files")
        .help("Refresh files")
        .size(28.0, 28.0)
        .min_w(28.0)
        .center()
        .radius(RADIUS_BTN)
        .focus_ring(2.0, -2.0, t.accent);
    refresh = if loading {
        refresh.opacity(0.5).disabled(true).child(spinner(t.muted, 14.0))
    } else {
        refresh
            .cursor(Cursor::Pointer)
            .hover(t.wash(6.0))
            .hover_group()
            .child(icon(&ROTATE_CW, 14.0).color(t.muted).group_color(t.text))
            .on_press(|s: &mut State| s.refresh_files())
    };
    row().key("files-toolbar").w_full().gap(4.0).center_y().pl(10.0).pr(8.0).child(search).child(refresh)
}

/// `.file-browser-panel__modes`: File names and File contents, the pressed one raised on `--card-bg` with a 1 px ring.
fn switch(s: &State, t: &Theme) -> El {
    let mode = s.file_browser.search_in;
    let button = |key: &'static str, words: &'static str, on: bool| {
        let b = row().key(key).role("button").label(words).grow(1.0).px(8.0).py(3.0).center().radius(6.0).cursor(Cursor::Pointer);
        let b = if on { b.fill(t.card_bg).border(1.0, t.border) } else { b.hover(t.wash(4.0)) };
        b.child(text(words).font(sans(12.0, 400.0, 1.5)).color(if on { t.text } else { t.muted }))
    };
    let names = button("files-mode-names", "File names", mode == SearchIn::Names)
        .on_press(|s: &mut State| s.file_browser.search_in = SearchIn::Names);
    // Pressing File contents does nothing yet: it is drawn off, with the reason as its tooltip.
    let contents = button("files-mode-contents", "File contents", false).opacity(0.5).disabled(true).help(CONTENTS_OFF);
    row()
        .key("files-modes")
        .role("group")
        .label("Search in")
        .mx(10.0)
        .padding(2.0)
        .gap(2.0)
        .radius(RADIUS_BTN)
        .fill(t.wash(5.0))
        .child(names)
        .child(contents)
}

fn content(s: &State, t: &Theme) -> El {
    let browser = &s.file_browser;
    let tree = browser.rows();
    if browser.root_loading() && browser.root().is_empty() {
        return col()
            .key("files-loading")
            .w_full()
            .items_center()
            .gap(8.0)
            .px(16.0)
            .py(24.0)
            .child(spinner(t.muted, 20.0))
            .child(text("Loading files\u{2026}").font(sans(12.0, 400.0, 1.5)).color(t.muted));
    }
    if let Some(why) = browser.error() {
        return col().key("files-error-wrap").w_full().padding(12.0).items_stretch().child(confirm::error_note(
            t,
            "files-error",
            why,
            None,
        ));
    }
    if browser.root().is_empty() {
        return empty_state::compact_described(
            t,
            "files-empty",
            Some(&FOLDER_TREE),
            "No files found",
            "The session directory is empty or not yet initialized.",
        );
    }
    col().key("files-tree").role("tree").w_full().items_stretch().children(tree.iter().map(|r| tree_row(s, t, r)))
}
