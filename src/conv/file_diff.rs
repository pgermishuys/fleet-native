//! The rows of a file's Diff view, as plain logic with no Rune in it (`@codemirror/merge`'s `unifiedMergeView` as `merge.ts` sets it
//! up, rows 6.3, 6.4 and 6.9 of docs/parity/11-files.md): the whole file with the base's removed lines shown above the lines that
//! replaced them, the changed words marked on both, and the long runs of unchanged lines folded to a bar.
use super::changed_lines::base_lines;
use super::diff::diff_script;
use super::review::{Span, word_marks};

/// `collapseUnchanged: { margin: 3, minSize: 8 }`.
pub const MARGIN: usize = 3;
pub const MIN_FOLD: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Same,
    /// A line the text has and the base hasn't (changed or inserted).
    Added,
    /// A line of the base the text no longer has: no number of its own.
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub kind: Kind,
    pub text: String,
    /// The 1-based line in the file's text; none for a removed line.
    pub no: Option<usize>,
    /// The 0-based line of the base this row is (or is the same as); none for an added line.
    pub base: Option<usize>,
    /// The changed words, as byte ranges of `text`.
    pub marks: Vec<Span>,
}

/// What the view draws: a row (by index into the rows), or a bar standing for `hidden` unchanged rows from `first`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Row(usize),
    Fold { first: usize, hidden: usize },
}

/// An empty file has no lines, as `Text.of([""])` diffs to a pure insertion: no empty removed line above a file that was added.
fn lines_of_base(base: &[String]) -> &[String] {
    if base.len() == 1 && base[0].is_empty() { &[] } else { base }
}

/// Every line of `doc`, with the lines of `base` that are gone drawn above the ones that replaced them. Each run of removed lines
/// followed by a run of added ones is paired in order for the changed words (`highlightChanges`).
pub fn rows(base: &[String], doc: &[String]) -> Vec<Row> {
    let base = lines_of_base(base);
    let (mut a, mut b) = (0usize, 0usize);
    let mut out: Vec<Row> = Vec::with_capacity(doc.len());
    for step in diff_script(base, doc) {
        match step {
            0 => {
                out.push(Row { kind: Kind::Same, text: doc[b].clone(), no: Some(b + 1), base: Some(a), marks: Vec::new() });
                a += 1;
                b += 1;
            }
            -1 => {
                out.push(Row { kind: Kind::Removed, text: base[a].clone(), no: None, base: Some(a), marks: Vec::new() });
                a += 1;
            }
            _ => {
                out.push(Row { kind: Kind::Added, text: doc[b].clone(), no: Some(b + 1), base: None, marks: Vec::new() });
                b += 1;
            }
        }
    }
    mark_words(&mut out);
    out
}

/// The last version of a deleted file: every line of the base as a removed line, numbered as the file had them.
pub fn deleted_rows(before: &str) -> Vec<Row> {
    let lines = base_lines(before);
    let lines = lines_of_base(&lines);
    lines
        .iter()
        .enumerate()
        .map(|(i, l)| Row { kind: Kind::Removed, text: l.clone(), no: Some(i + 1), base: Some(i), marks: Vec::new() })
        .collect()
}

fn mark_words(rows: &mut [Row]) {
    let mut i = 0;
    while i < rows.len() {
        if rows[i].kind != Kind::Removed {
            i += 1;
            continue;
        }
        let r0 = i;
        while i < rows.len() && rows[i].kind == Kind::Removed {
            i += 1;
        }
        let a0 = i;
        while i < rows.len() && rows[i].kind == Kind::Added {
            i += 1;
        }
        for p in 0..(a0 - r0).min(i - a0) {
            let (from, to) = word_marks(&rows[r0 + p].text, &rows[a0 + p].text);
            rows[r0 + p].marks = from.into_iter().collect();
            rows[a0 + p].marks = to.into_iter().collect();
        }
    }
}

/// Folds each run of unchanged rows, keeping `margin` rows beside a change (none at the start or end of the file); a run folds only
/// when at least `min` rows go into the bar. `opened` says which folds (by first row) the user has opened. A file with nothing
/// changed shows whole.
pub fn items(rows: &[Row], margin: usize, min: usize, opened: &dyn Fn(usize) -> bool) -> Vec<Item> {
    let mut out = Vec::with_capacity(rows.len());
    if rows.iter().all(|r| r.kind == Kind::Same) {
        return (0..rows.len()).map(Item::Row).collect();
    }
    let mut i = 0;
    while i < rows.len() {
        if rows[i].kind != Kind::Same {
            out.push(Item::Row(i));
            i += 1;
            continue;
        }
        let start = i;
        while i < rows.len() && rows[i].kind == Kind::Same {
            i += 1;
        }
        let from = start + if start == 0 { 0 } else { margin };
        let to = if i == rows.len() { i } else { i.saturating_sub(margin) };
        if to > from && to - from >= min && !opened(from) {
            out.extend((start..from).map(Item::Row));
            out.push(Item::Fold { first: from, hidden: to - from });
            out.extend((to..i).map(Item::Row));
        } else {
            out.extend((start..i).map(Item::Row));
        }
    }
    out
}

/// What the fold's bar says.
pub fn fold_words(hidden: usize) -> String {
    format!("{hidden} unchanged {}", if hidden == 1 { "line" } else { "lines" })
}
