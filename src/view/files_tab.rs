//! Ports `components/canvas/FilesCanvas.vue` and the names half of `components/session/FileBrowserPanel.vue`: the Files tab's body.
//! A toolbar (the search box and Refresh), the "Search in" switch, and under them the tree or what stands in for it: "Loading
//! files…", the error, or "No files found". The names search replaces the tree with its results once two characters are typed;
//! "File contents" does the same with Find in files' lines (`find_in_files`, component 12), and adds Aa and .* to the box.
use super::file_tree::tree_row;
use super::find_in_files::{find_results, hint, hit_key, toggle};
use crate::State;
use crate::fleet::file_browser::SearchIn;
use crate::fleet::find_text::FindText;
use crate::icons::*;
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::ui::{confirm, empty_state};
use rune::{Cursor, Cycle, El, Key, KeyEvent, Tx, col, el, icon, row, text, text_input};

pub const FIELD: &str = "files-search";

/// `Spinner` at `size`: the turning circle.
pub fn spinner(ink: rune::Color, size: f32) -> El {
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
    let contents = browser.search_in == SearchIn::Contents;
    let field = text_input(&browser.field, sans(13.0, 400.0, 1.3))
        .key(FIELD)
        .label(if contents { "Search inside files" } else { "Search files" })
        .placeholder(if contents { "Search inside files\u{2026}" } else { "Search files\u{2026}" }, t.muted)
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
            // Enter opens the first result as a kept tab; in contents mode, the active line.
            Key::Enter => {
                if s.file_browser.search_in == SearchIn::Contents {
                    if let Some(hit) = s.find_text.enter_hit() {
                        s.open_hit(&hit.path, hit.line, tx);
                    }
                } else {
                    s.open_first_found(tx);
                }
                true
            }
            Key::Down | Key::Up if s.file_browser.search_in == SearchIn::Contents => {
                s.find_text.move_active(if ev.key == Key::Down { 1 } else { -1 });
                if let Some(a) = &s.find_text.active {
                    tx.scroll_into_view(hit_key(&a.path, a.line));
                }
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
    if contents {
        search = search
            .child(
                toggle(t, "find-match-case", "Match case", "Aa", s.find_text.match_case)
                    .on_press(|s: &mut State| s.find_text.match_case = !s.find_text.match_case),
            )
            .child(
                toggle(t, "find-regex", "Regular expression", ".*", s.find_text.regex)
                    .on_press(|s: &mut State| s.find_text.regex = !s.find_text.regex),
            );
    }
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
    let contents = button("files-mode-contents", "File contents", mode == SearchIn::Contents)
        .on_press(|s: &mut State| s.file_browser.search_in = SearchIn::Contents);
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

/// The results of the names search: "Searching…", the error, "No files found" or one button per path.
fn results(s: &State, t: &Theme) -> El {
    let search = &s.file_browser.search;
    if search.loading {
        return col()
            .key("files-searching")
            .w_full()
            .items_center()
            .gap(8.0)
            .px(16.0)
            .py(24.0)
            .child(spinner(t.muted, 20.0))
            .child(text("Searching\u{2026}").font(sans(12.0, 400.0, 1.5)).color(t.muted));
    }
    if let Some(why) = &search.error {
        return col().key("files-search-error-wrap").w_full().padding(12.0).items_stretch().child(confirm::error_note(
            t,
            "files-search-error",
            why,
            None,
        ));
    }
    let found = s.file_browser.found();
    if found.is_empty() {
        return empty_state::compact_described(t, "files-search-empty", None, "No files found", "Try a different search query");
    }
    let last = found.len() - 1;
    col().key("files-results").w_full().items_stretch().children(found.iter().enumerate().map(|(i, path)| {
        let open = path.clone();
        let mut r = row()
            .key(format!("file-result-{path}"))
            .role("button")
            .label(path.clone())
            .w_full()
            .px(12.0)
            .py(8.0)
            .cursor(Cursor::Pointer)
            .hover(t.main_bg)
            .focusable(true)
            .focus_ring(2.0, -2.0, t.accent)
            .child(text(path.clone()).font(sans(12.0, 400.0, 1.5)).color(t.text).truncate().min_w(0.0).grow(1.0));
        if i != last {
            r = r.border_sides([0.0, 0.0, 1.0, 0.0], t.border).border_layout();
        }
        r.on_press_tx(move |s: &mut State, tx| s.open_found_file(&open, false, tx))
    }))
}

fn content(s: &State, t: &Theme) -> El {
    let browser = &s.file_browser;
    if browser.search_in == SearchIn::Contents && FindText::is_active(&browser.field.text) {
        return find_results(s, t);
    }
    if browser.is_searching() {
        return results(s, t);
    }
    // In contents mode a short query leaves the tree, with a hint under it.
    if browser.search_in == SearchIn::Contents {
        return col().key("files-contents-idle").w_full().items_stretch().child(tree_body(s, t)).child(hint(s, t));
    }
    tree_body(s, t)
}

fn tree_body(s: &State, t: &Theme) -> El {
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
