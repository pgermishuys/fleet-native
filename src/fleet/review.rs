//! The review's own state per session (`ChangesReview.vue`, rows 5.x and 6.x of docs/parity/12-changes-and-review.md): which file
//! sections are shut, which start shut, the layout the user picked and the order the files are drawn in. Plain Rust with no Rune
//! in it; `view::review` draws it and the `State` half at the bottom asks for the contents of every open file.
use super::diffs::FileDiff;
use crate::State;
use crate::conv::review::ReviewItem;
use std::collections::HashSet;

/// A long list opens its first few files; the rest are one click away (`AUTO_OPEN`).
pub const AUTO_OPEN: usize = 8;
/// Past this many changed lines a diff starts shut whatever its place (`BIG_DIFF_LINES`): drawing thousands of lines freezes the panel.
pub const BIG_DIFF_LINES: u64 = 1500;
/// Unified below this much panel, Split from it, until the user picks (`SPLIT_FROM_WIDTH`).
pub const SPLIT_FROM_WIDTH: f32 = 1100.0;

/// A diff with thousands of changed lines draws its first hunks; the rest is one click away (`MAX_DRAWN_LINES`).
pub const MAX_DRAWN_LINES: usize = 600;

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
    layout_pref: Option<Layout>,
    /// Unchanged runs the reader opened (`path`, fold id) and files drawn whole past the cap.
    open_folds: HashSet<(String, String)>,
    drawn_all: HashSet<String>,
}

impl Review {
    /// Another session: a fresh review (Fleet mounts one per session).
    pub fn open(&mut self, session: &str) {
        if self.session != session {
            *self = Review { session: session.to_string(), layout_pref: self.layout_pref, ..Default::default() };
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

    /// The layout drawn: the user's pick, else by the panel's width.
    pub fn layout(&self, panel_w: f32) -> Layout {
        self.layout_pref.unwrap_or(if panel_w >= SPLIT_FROM_WIDTH { Layout::Split } else { Layout::Unified })
    }

    pub fn set_layout(&mut self, layout: Layout) {
        self.layout_pref = Some(layout);
    }
}

impl State {
    /// The review's files in drawing order.
    pub fn review_files(&self) -> Vec<FileDiff> {
        sorted(self.diffs.files())
    }

    /// Settles which sections start shut and asks for the contents of every open one (H narrows this to what is near). Nothing
    /// while the panel is closed: a closed panel shows no diffs and asks for none.
    pub fn review_sync(&mut self) {
        let session = self.diffs.session().to_string();
        if session.is_empty() || !self.content_open {
            return;
        }
        self.review.open(&session);
        let files = self.review_files();
        self.review.decide(&files);
        let open: Vec<&FileDiff> = files.iter().filter(|f| self.review.is_open(&f.file)).collect();
        for file in open {
            self.load_diff_file(&session, file);
        }
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
        let mut r = Review::default();
        assert_eq!(r.layout(1099.0), Layout::Unified);
        assert_eq!(r.layout(1100.0), Layout::Split);
        r.set_layout(Layout::Unified);
        assert_eq!(r.layout(1400.0), Layout::Unified);
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
