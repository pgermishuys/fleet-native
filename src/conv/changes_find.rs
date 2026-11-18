//! Find in Changes, the logic with no Rune state in it (`lib/changes-find.ts`): what the bar searches in a session's diffs, the keys that
//! step through the matches, the selection that fills the bar, and what stepping to a match has to open so the line is drawn.
//!
//! The search reads the diff data, not what is drawn, so it covers files that are shut, runs of unchanged lines that are folded
//! and lines past a long diff's "Show more". The marks are piece K's.
use super::find::count_occurrences;
use super::review::ReviewItem;
use rune::{Key, KeyEvent};

/// A one-line selection longer than this is not taken for a query.
pub const MAX_SELECTED_QUERY: usize = 200;

/// One line of a file's diff the bar can land on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindLine {
    /// The line's index in the file's full line list (unique within the file).
    pub index: usize,
    pub text: String,
    /// The hunk or fold it sits in.
    pub item_id: String,
    pub in_fold: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindFile {
    pub path: String,
    pub lines: Vec<FindLine>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangesHit {
    pub path: String,
    pub line: usize,
    pub item_id: String,
    pub in_fold: bool,
    /// Which of the line's matches this is, from the left.
    pub ordinal: usize,
}

/// `findLines`: a file's lines in the order the review draws them: each hunk's lines, and the unchanged runs folded between them.
pub fn find_lines(items: &[ReviewItem]) -> Vec<FindLine> {
    let mut out = Vec::new();
    for item in items {
        let (lines, in_fold) = match item {
            ReviewItem::Hunk(h) => (&h.lines, false),
            ReviewItem::Fold(f) => (&f.lines, true),
        };
        for line in lines {
            out.push(FindLine { index: line.index, text: line.line.content.clone(), item_id: item.id().to_string(), in_fold });
        }
    }
    out
}

/// `findChangesHits`: every match in the files, top to bottom, case-blind and without overlaps. Each line counts once, though Split
/// draws an unchanged line twice.
pub fn find_changes_hits(files: &[FindFile], query: &str) -> Vec<ChangesHit> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for file in files {
        for line in &file.lines {
            for ordinal in 0..count_occurrences(&line.text, query) {
                hits.push(ChangesHit {
                    path: file.path.clone(),
                    line: line.index,
                    item_id: line.item_id.clone(),
                    in_fold: line.in_fold,
                    ordinal,
                });
            }
        }
    }
    hits
}

/// `stepShortcut`: F3 and Ctrl+G (Cmd+G on a Mac) step through the matches, with Shift going back. 1, -1, or 0 for any other key.
pub fn step_shortcut(ev: &KeyEvent, apple: bool) -> i32 {
    let m = ev.mods;
    let primary = if apple { m.cmd && !m.ctrl } else { m.ctrl && !m.logo };
    let is_step = (matches!(ev.key, Key::Function(3)) && !m.ctrl && !m.cmd) || (primary && ev.is_command('g'));
    match (is_step && !m.alt, m.shift) {
        (false, _) => 0,
        (true, true) => -1,
        (true, false) => 1,
    }
}

/// `selectedQuery`: the text selected inside Changes becomes the query when it is on one line and short enough. `selection` is
/// the selected text, or `None` when nothing is selected inside Changes (the caller decides that; a selection elsewhere is not ours).
pub fn selected_query(selection: Option<&str>) -> String {
    match selection {
        Some(text) if !text.contains('\n') && text.chars().count() <= MAX_SELECTED_QUERY => text.to_string(),
        _ => String::new(),
    }
}

/// What the review has to do to draw a match's line, as `revealHit` does it (`ChangesReview.vue`). Each part is only what is
/// still undone, so applying a plan twice does nothing the second time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reveal {
    /// The file's section is shut: open it.
    pub open_file: bool,
    /// The file is viewed as Rendered: show its diff again.
    pub show_diff: bool,
    /// The line is in an unchanged run that is folded: open the run.
    pub open_fold: bool,
    /// The line is past the cap of a long diff: draw the rest.
    pub show_all: bool,
}

/// The state of one file's section the plan is made from.
#[derive(Clone, Copy, Debug, Default)]
pub struct Section {
    pub open: bool,
    pub rendered: bool,
    pub fold_open: bool,
    /// The hit's line is among those drawn now (its fold counts as drawn: the fold row is, and `open_fold` opens it).
    pub drawn: bool,
}

pub fn reveal_plan(hit: &ChangesHit, section: Section) -> Reveal {
    Reveal { open_file: !section.open, show_diff: section.rendered, open_fold: hit.in_fold && !section.fold_open, show_all: !section.drawn }
}
