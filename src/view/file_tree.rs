//! Ports `components/session/FileBrowserTreeNode.vue`: one row of the Files tab's tree. A folder is a chevron and a name (a
//! spinner while it loads); a file is a dot and a name, with the A / M / D badge and the `+N / -N` counts of a changed file at
//! the right, and the selected one filled with `--primary`. Rows indent `8 + 16 × depth`.
use crate::State;
use crate::fleet::diffs::{FileDiff, FileStatus};
use crate::fleet::file_browser::Row;
use crate::icons::*;
use crate::theme::{Theme, mono, sans};
use rune::{Cursor, El, el, icon, row, text};

pub fn row_key(path: &str, directory: bool) -> String {
    format!("{}-node-{path}", if directory { "dir" } else { "file" })
}

/// Fleet's rows are `padding: 4px 8px` with the indent on the left.
fn frame(depth: usize) -> El {
    row().w_full().gap(8.0).center_y().pl(8.0 + 16.0 * depth as f32).pr(8.0).py(4.0).radius(4.0)
}

/// `.file-browser-tree-node__loading` and `__empty`: a muted italic 12 px line one level in.
fn note(t: &Theme, key: String, depth: usize, words: &str) -> El {
    row()
        .key(key)
        .w_full()
        .pl(8.0 + 16.0 * depth as f32)
        .pr(8.0)
        .py(4.0)
        .child(text(words.to_string()).font(sans(12.0, 400.0, 1.5).italic()).color(t.muted))
}

pub fn tree_row(s: &State, t: &Theme, row_: &Row) -> El {
    match row_ {
        Row::Dir { entry, depth, open, loading } => {
            let path = entry.path.clone();
            let mut r = frame(*depth)
                .key(row_key(&entry.path, true))
                .role("treeitem")
                .label(entry.name.clone())
                .cursor(Cursor::Pointer)
                .hover(t.main_bg)
                .focusable(true)
                .focus_ring(2.0, -2.0, t.accent)
                .child(icon(if *open { &CHEVRON_DOWN } else { &CHEVRON_RIGHT }, 16.0).color(t.muted))
                .child(name(&entry.name, t.text, 400.0));
            if *loading {
                r = r.child(crate::ui::confirm::spinner(t.muted));
            }
            r.on_press(move |s: &mut State| s.toggle_files_dir(&path))
        }
        Row::File { entry, depth, selected } => {
            let (press, double) = (entry.path.clone(), entry.path.clone());
            let (ink, weight, dot) = if *selected { (t.primary_fg, 600.0, rune::Color::WHITE) } else { (t.text, 400.0, t.muted) };
            let mut r = frame(*depth)
                .key(row_key(&entry.path, false))
                .role("treeitem")
                .label(entry.name.clone())
                .cursor(Cursor::Pointer)
                .focusable(true)
                .focus_ring(2.0, -2.0, t.accent)
                // A selected row stays filled on hover.
                .hover(if *selected { t.accent } else { t.main_bg })
                .child(el().size(8.0, 8.0).min_w(8.0).radius(4.0).fill(dot))
                .child(name(&entry.name, ink, weight));
            if *selected {
                r = r.fill(t.accent);
            }
            if let Some(info) = s.diffs.by_file(&entry.path) {
                r = r.child(marks(t, info));
            }
            r.on_press_tx(move |s: &mut State, tx| s.press_file(&press, tx))
                .on_double_press_tx(move |s: &mut State, tx| s.double_press_file(&double, tx))
        }
        Row::Loading { depth } => note(t, format!("tree-loading-{depth}"), *depth, "Loading\u{2026}"),
        Row::Empty { depth } => note(t, format!("tree-empty-{depth}"), *depth, "No files"),
    }
}

/// `.file-browser-tree-node__name`: 14 px, one line with an ellipsis, taking what the row leaves.
fn name(words: &str, ink: rune::Color, weight: f32) -> El {
    text(words.to_string()).font(sans(14.0, weight, 1.5)).color(ink).truncate().min_w(0.0).grow(1.0)
}

/// A changed file's badge and counts; screen readers hear "added, 4 additions".
fn marks(t: &Theme, info: &FileDiff) -> El {
    let (letter, fill, status) = match info.status {
        FileStatus::Added => ("A", t.badge_added(), "added"),
        FileStatus::Modified => ("M", t.badge_modified(), "modified"),
        FileStatus::Deleted => ("D", t.badge_deleted(), "deleted"),
    };
    let mut said = vec![status.to_string()];
    let plural = |n: u64| if n == 1 { "" } else { "s" };
    if info.additions > 0 {
        said.push(format!("{} addition{}", info.additions, plural(info.additions)));
    }
    if info.deletions > 0 {
        said.push(format!("{} deletion{}", info.deletions, plural(info.deletions)));
    }
    let mut counts = Vec::new();
    if info.additions > 0 {
        counts.push(format!("+{}", info.additions));
    }
    if info.deletions > 0 {
        counts.push(format!("-{}", info.deletions));
    }
    let mut r = row().key(format!("file-marks-{}", info.file)).label(said.join(", ")).gap(6.0).center_y().min_w(0.0);
    r = r.child(
        el().size(16.0, 16.0)
            .min_w(16.0)
            .center()
            .radius(2.0)
            .fill(fill)
            .child(text(letter).font(sans(10.0, 700.0, 1.0)).color(t.primary_fg)),
    );
    if !counts.is_empty() {
        r = r.child(text(counts.join(" / ")).font(mono(11.0, 400.0, 1.5)).color(t.muted));
    }
    r
}
