//! The review's own state per session (`ChangesReview.vue`, rows 5.x and 6.x of docs/parity/12-changes-and-review.md): which file
//! sections are shut, which start shut, the layout the user picked and the order the files are drawn in. Plain Rust with no Rune
//! in it; `view::review` draws it and the `State` half at the bottom asks for the contents of every open file.
use super::diff_contents::Entry;
use super::diffs::FileDiff;
use crate::State;
use crate::conv::review::ReviewItem;
use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;

/// A long list opens its first few files; the rest are one click away (`AUTO_OPEN`).
pub const AUTO_OPEN: usize = 8;
/// Past this many changed lines a diff starts shut whatever its place (`BIG_DIFF_LINES`): drawing thousands of lines freezes the panel.
pub const BIG_DIFF_LINES: u64 = 1500;
/// Unified below this much panel, Split from it, until the user picks (`SPLIT_FROM_WIDTH`).
pub const SPLIT_FROM_WIDTH: f32 = 1100.0;

/// A diff with thousands of changed lines draws its first hunks; the rest is one click away (`MAX_DRAWN_LINES`).
pub const MAX_DRAWN_LINES: usize = 600;

/// How far past the visible part of the review a file still counts as near (`NEAR_MARGIN`, "600px 0px").
pub const NEAR_MARGIN: f32 = 600.0;
/// What the review's scroll box is taken to show before it has said (the panel's usual height).
const FIRST_VIEWPORT: f32 = 900.0;
/// The body's top padding, a card's header and its two border rows, and the gap between cards (`review.rs`).
const BODY_TOP: f32 = 12.0;
const CARD_CHROME: f32 = 38.0;
const CARD_GAP: f32 = 10.0;
/// One line's height in a file's lines (`view::review_lines::LINE_H`).
const LINE: f32 = 20.0;
/// A body that is one line of words (binary, too large, failed): `.rfile__note`.
const NOTE_BODY: f32 = 44.0;

/// `pendingHeight`: the room a file not drawn yet holds, and keeps while it loads.
pub fn pending_height(file: &FileDiff) -> f32 {
    (file.additions + file.deletions + 8).min(MAX_DRAWN_LINES as u64) as f32 * LINE
}

/// The files within `NEAR_MARGIN` of the visible part, from where each one's card is taken to lie. `body` is the height an open
/// file's body takes as far as is known: the pending height for a file not loaded, which also stands in for one that has (it draws
/// about as many rows), and a short note's for one that has no lines to draw.
pub fn near_files(files: &[FileDiff], body: impl Fn(&FileDiff) -> f32, offset: f32, viewport: f32) -> Vec<String> {
    let (from, to) = (offset - NEAR_MARGIN, offset + viewport + NEAR_MARGIN);
    let mut top = BODY_TOP;
    let mut near = Vec::new();
    for file in files {
        let height = CARD_CHROME + body(file);
        if top < to && top + height > from {
            near.push(file.file.clone());
        }
        top += height + CARD_GAP;
    }
    near
}

/// One item of a file's review as drawn: a hunk may be cut to its first `take` lines.
pub struct Shown<'a> {
    pub item: &'a ReviewItem,
    pub take: usize,
}

/// `visibleItems`: every item, or the first `MAX_DRAWN_LINES` changed lines (a hunk that is the whole file is cut too).
pub fn visible(items: &[ReviewItem], all: bool) -> Vec<Shown<'_>> {
    let mut out = Vec::new();
    let mut drawn = 0usize;
    for item in items {
        if !all && drawn >= MAX_DRAWN_LINES {
            break;
        }
        match item {
            ReviewItem::Hunk(h) => {
                let take = if all { h.lines.len() } else { h.lines.len().min(MAX_DRAWN_LINES - drawn) };
                out.push(Shown { item, take });
                drawn += h.lines.len();
            }
            ReviewItem::Fold(f) => out.push(Shown { item, take: f.lines.len() }),
        }
    }
    out
}

/// `hiddenChanges`: the changed lines `visible` leaves out.
pub fn hidden_changes(items: &[ReviewItem], all: bool) -> usize {
    if all {
        return 0;
    }
    let total: usize = items.iter().map(|i| if let ReviewItem::Hunk(h) = i { h.lines.len() } else { 0 }).sum();
    let drawn: usize = visible(items, false).iter().map(|s| if matches!(s.item, ReviewItem::Hunk(_)) { s.take } else { 0 }).sum();
    total - drawn
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Unified,
    Split,
}

/// The user's pick, else Unified below `SPLIT_FROM_WIDTH` of panel and Split from it.
pub fn layout_for(pick: Option<Layout>, panel_w: f32) -> Layout {
    pick.unwrap_or(if panel_w >= SPLIT_FROM_WIDTH { Layout::Split } else { Layout::Unified })
}

/// `GENERATED_FILE`: lockfiles and generated files nobody reads. Names are matched case-blind, as Fleet's pattern is.
pub fn is_generated(path: &str) -> bool {
    const NAMES: [&str; 14] = [
        "package-lock.json",
        "npm-shrinkwrap.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "bun.lock",
        "bun.lockb",
        "cargo.lock",
        "gemfile.lock",
        "poetry.lock",
        "uv.lock",
        "composer.lock",
        "go.sum",
        "packages.lock.json",
        "pipfile.lock",
    ];
    let lower = path.to_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    NAMES.contains(&name) || name == "flake.lock" || [".min.js", ".min.css", ".lock", ".map"].iter().any(|ext| lower.ends_with(ext))
}

/// `startsCollapsed`.
pub fn starts_collapsed(file: &FileDiff) -> bool {
    is_generated(&file.file) || file.additions + file.deletions > BIG_DIFF_LINES
}

/// The files in the order the review draws them: by path (`localeCompare`, approximated by comparing case-blind first).
pub fn sorted(files: &[FileDiff]) -> Vec<FileDiff> {
    let mut out = files.to_vec();
    out.sort_by(|a, b| a.file.to_lowercase().cmp(&b.file.to_lowercase()).then_with(|| b.file.cmp(&a.file)));
    out
}

/// `dirOf`: the path up to and with its last slash.
pub fn dir_of(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..=i])
}

/// `nameOf`.
pub fn name_of(path: &str) -> &str {
    path.rfind('/').map_or(path, |i| &path[i + 1..])
}

#[derive(Debug, Default)]
pub struct Review {
    session: String,
    collapsed: HashSet<String>,
    /// The start-shut decision has been made for this review (once, when the first list arrives).
    decided: bool,
    /// Unchanged runs the reader opened (`path`, fold id) and files drawn whole past the cap.
    open_folds: HashSet<(String, String)>,
    drawn_all: HashSet<String>,
    /// Files that have come near the visible part: they stay drawn (`near`).
    near: HashSet<String>,
    /// Where the scroll box is, left by the view's `on_scroll_offset` (which may not change state) for `State::review_sync`.
    pub probe: Rc<Cell<Option<rune::ScrollPos>>>,
    /// The last place `keep_near` acted on.
    seen: Cell<Option<rune::ScrollPos>>,
    /// A revert is on its way: another can't start (`reverting`).
    pub reverting: bool,
    /// What the last revert that did nothing or failed says, above the files until dismissed (`notice`).
    pub notice: Option<String>,
    /// The file whose diff is asked for again once the list a revert asked for has come.
    pub reload_after_list: Option<String>,
}

impl Review {
    /// Another session: a fresh review (Fleet mounts one per session).
    pub fn open(&mut self, session: &str) {
        if self.session != session {
            *self = Review { session: session.to_string(), probe: self.probe.clone(), ..Default::default() };
        }
    }

    /// Decides which sections start shut, once, from the list as it first arrives: everything past the first `AUTO_OPEN`, and
    /// whatever `starts_collapsed` says wherever it sits. `files` is in drawing order.
    pub fn decide(&mut self, files: &[FileDiff]) {
        if self.decided || files.is_empty() {
            return;
        }
        self.decided = true;
        self.collapsed =
            files.iter().enumerate().filter(|(i, f)| *i >= AUTO_OPEN || starts_collapsed(f)).map(|(_, f)| f.file.clone()).collect();
    }

    pub fn fold_open(&self, path: &str, id: &str) -> bool {
        self.open_folds.contains(&(path.to_string(), id.to_string()))
    }

    pub fn open_fold(&mut self, path: &str, id: &str) {
        self.open_folds.insert((path.to_string(), id.to_string()));
    }

    pub fn is_drawn_all(&self, path: &str) -> bool {
        self.drawn_all.contains(path)
    }

    pub fn show_all(&mut self, path: &str) {
        self.drawn_all.insert(path.to_string());
    }

    /// The box moved since last asked: the place is now the one acted on.
    pub fn moved(&self) -> bool {
        let at = self.probe.get();
        at.is_some() && self.seen.replace(at) != at
    }

    pub fn near_count(&self) -> usize {
        self.near.len()
    }

    pub fn is_near(&self, path: &str) -> bool {
        self.near.contains(path)
    }

    /// `markNear`: for a file something else needs drawn (find, a request to focus it). Returns whether any was new.
    pub fn mark_near(&mut self, paths: impl IntoIterator<Item = String>) -> bool {
        paths.into_iter().fold(false, |any, p| self.near.insert(p) | any)
    }

    pub fn is_open(&self, path: &str) -> bool {
        !self.collapsed.contains(path)
    }

    pub fn toggle(&mut self, path: &str) {
        if !self.collapsed.remove(path) {
            self.collapsed.insert(path.to_string());
        }
    }

    /// `allCollapsed`: with files, every one of them shut (Expand all then shows instead of Collapse all).
    pub fn all_collapsed(&self, files: &[FileDiff]) -> bool {
        !files.is_empty() && files.iter().all(|f| !self.is_open(&f.file))
    }

    pub fn toggle_all(&mut self, files: &[FileDiff]) {
        self.collapsed = if self.all_collapsed(files) { HashSet::new() } else { files.iter().map(|f| f.file.clone()).collect() };
    }
}

impl State {
    /// The review's files in drawing order.
    pub fn review_files(&self) -> Vec<FileDiff> {
        sorted(self.diffs.files())
    }

    /// Settles which sections start shut and asks for the contents of every open file that is near the visible part (`near`). Nothing
    /// while the panel is closed: a closed panel shows no diffs and asks for none.
    pub fn review_sync(&mut self) {
        let session = self.diffs.session().to_string();
        if session.is_empty() || !self.content_open {
            return;
        }
        self.review.open(&session);
        let files = self.review_files();
        self.review.decide(&files);
        let (offset, viewport) = self.review.probe.get().map_or((0.0, FIRST_VIEWPORT), |at| (at.offset[1], at.viewport[1]));
        let body = |f: &FileDiff| match self.diff_contents.entry(&f.file) {
            _ if !self.review.is_open(&f.file) => 0.0,
            Some(Entry::Ready(item)) if item.binary || item.truncated => NOTE_BODY,
            Some(Entry::Error(_)) => NOTE_BODY,
            _ => pending_height(f),
        };
        let near = near_files(&files, body, offset, viewport);
        self.review.mark_near(near);
        let open: Vec<&FileDiff> = files.iter().filter(|f| self.review.is_open(&f.file) && self.review.is_near(&f.file)).collect();
        for file in open {
            self.load_diff_file(&session, file);
        }
    }

    /// The review scrolled or changed its height: files that came near are asked for. Returns whether the view must redraw.
    pub fn keep_review_near(&mut self) -> bool {
        if !self.content_open || !self.review.moved() {
            return false;
        }
        let before = self.review.near_count();
        self.review_sync();
        self.review.near_count() != before
    }

    /// The layout drawn: the user's pick (remembered, `weave:changes-layout`), else by the panel's width.
    pub fn review_layout(&self) -> Layout {
        layout_for(self.sidebar.changes_layout(), self.right_shown_w())
    }

    pub fn set_review_layout(&mut self, layout: Layout) {
        self.sidebar.save_changes_layout(layout);
    }

    /// Wrap long lines, remembered across sessions and launches.
    pub fn toggle_review_wrap(&mut self) {
        let on = !self.sidebar.changes_wrap();
        self.sidebar.save_changes_wrap(on);
    }

    /// A file's header was pressed (click, Enter or Space).
    pub fn toggle_review_file(&mut self, path: &str) {
        self.review.toggle(path);
        self.review_sync();
    }

    pub fn toggle_review_all(&mut self) {
        let files = self.review_files();
        self.review.toggle_all(&files);
        self.review_sync();
    }

    /// Try again on a file whose diff failed.
    pub fn retry_review_file(&mut self, path: &str) {
        let session = self.diffs.session().to_string();
        if let Some(file) = self.diffs.by_file(path).cloned() {
            self.reload_diff_file(&session, &file);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, add: u64, del: u64) -> FileDiff {
        FileDiff { file: path.into(), additions: add, deletions: del, ..Default::default() }
    }

    #[test]
    fn generated_files_are_known_by_name_and_ending() {
        for p in
            ["package-lock.json", "web/yarn.lock", "Cargo.lock", "a/b/go.sum", "dist/app.min.js", "x.js.map", "other.lock", "flake.lock"]
        {
            assert!(is_generated(p), "{p}");
        }
        for p in ["src/lock.rs", "go.summary", "lockfile.txt", "mapper.js"] {
            assert!(!is_generated(p), "{p}");
        }
    }

    #[test]
    fn the_first_eight_open_and_generated_or_huge_ones_start_shut() {
        let mut files: Vec<FileDiff> = (0..10).map(|i| file(&format!("src/f{i}.rs"), 1, 1)).collect();
        files[2] = file("src/yarn.lock", 3, 3);
        files[3] = file("src/big.rs", 1000, 501);
        let mut r = Review::default();
        r.open("s1");
        r.decide(&files);
        let shut: Vec<_> = files.iter().filter(|f| !r.is_open(&f.file)).map(|f| f.file.as_str()).collect();
        assert_eq!(shut, ["src/yarn.lock", "src/big.rs", "src/f8.rs", "src/f9.rs"]);
        // Decided once: a later list doesn't reshuffle what the reader opened.
        r.toggle("src/f8.rs");
        r.decide(&files);
        assert!(r.is_open("src/f8.rs"));
    }

    #[test]
    fn collapse_all_then_expand_all() {
        let files = vec![file("a", 1, 0), file("b", 1, 0)];
        let mut r = Review::default();
        r.open("s");
        r.decide(&files);
        assert!(!r.all_collapsed(&files));
        r.toggle_all(&files);
        assert!(r.all_collapsed(&files));
        r.toggle_all(&files);
        assert!(files.iter().all(|f| r.is_open(&f.file)));
        assert!(!r.all_collapsed(&[]));
    }

    #[test]
    fn layout_follows_the_width_until_picked() {
        assert_eq!(layout_for(None, 1099.0), Layout::Unified);
        assert_eq!(layout_for(None, 1100.0), Layout::Split);
        assert_eq!(layout_for(Some(Layout::Unified), 1400.0), Layout::Unified);
        assert_eq!(layout_for(Some(Layout::Split), 400.0), Layout::Split);
    }

    #[test]
    fn a_long_diff_shows_its_first_600_changed_lines_and_cuts_a_hunk() {
        use crate::conv::diff::parse_diff_lines;
        use crate::conv::review::build_review;
        let before: String = (0..1200).map(|i| format!("row {i}\n")).collect();
        let after: String = (0..1200).map(|i| if i % 2 == 1 { format!("row {i}!\n") } else { format!("row {i}\n") }).collect();
        let items = build_review(&parse_diff_lines(&before, &after));
        let drawn: usize = visible(&items, false).iter().map(|s| s.take).sum();
        assert_eq!(drawn, MAX_DRAWN_LINES, "a hunk that is the whole file is cut");
        assert!(hidden_changes(&items, false) > 1500 - 600 && hidden_changes(&items, true) == 0);
    }

    #[test]
    fn paths_sort_and_split() {
        let files = vec![file("src/notes.ts", 0, 0), file("README.md", 0, 0)];
        assert_eq!(sorted(&files)[0].file, "README.md");
        assert_eq!((dir_of("src/a/b.rs"), name_of("src/a/b.rs")), ("src/a/", "b.rs"));
        assert_eq!((dir_of("b.rs"), name_of("b.rs")), ("", "b.rs"));
    }
}
