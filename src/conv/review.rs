//! What the Changes review draws from one file's diff lines, as plain logic with no Rune in it (`lib/diff-review.ts`):
//! hunks (the changes with a little context around them), the runs of unchanged lines folded between them, word marks
//! on changed lines, side-by-side rows, and the text a revert leaves behind.
//!
//! Marks are **byte** offsets into a line's content (Fleet's are UTF-16 offsets); both slice the same text. The 70 %
//! rule counts UTF-16 units, as Fleet does, so a line gets marks in both or in neither.
use super::diff::{DiffLine, Kind};

/// A start and an end, as byte offsets into a line's content.
pub type Span = (usize, usize);

const CONTEXT: usize = 3;
/// A hunk this long is a rewrite: pairing its lines up only adds noise.
const MAX_MARKED_HUNK: usize = 600;

/// A diff line with its place in the file's full line list and the spans of it that differ from its partner line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewLine {
    pub line: DiffLine,
    pub index: usize,
    pub marks: Vec<Span>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewHunk {
    pub id: String,
    /// The nearest line above the hunk that starts at the margin: "export interface Note".
    pub name: String,
    /// First line number the hunk shows (new side, or old side for a pure deletion at the top).
    pub start_line: u32,
    pub lines: Vec<ReviewLine>,
    /// Indices (into the file's full line list) of the first and last line of the hunk.
    pub from: usize,
    pub to: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewFold {
    pub id: String,
    pub hidden: usize,
    pub lines: Vec<ReviewLine>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewItem {
    Hunk(ReviewHunk),
    Fold(ReviewFold),
}

impl ReviewItem {
    pub fn id(&self) -> &str {
        match self {
            ReviewItem::Hunk(h) => &h.id,
            ReviewItem::Fold(f) => &f.id,
        }
    }
}

// ── Word marks ──────────────────────────────────────────────────────────────────────────────────────────────────────

/// `\s+`, `[A-Za-z0-9_$]+`, or any one other character.
fn tokens(text: &str) -> Vec<&str> {
    let class = |c: char| {
        if c.is_whitespace() {
            1
        } else if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
            2
        } else {
            0
        }
    };
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let k = class(c);
        let mut end = i + c.len_utf8();
        if k != 0 {
            while let Some(&(j, d)) = chars.peek() {
                if class(d) != k {
                    break;
                }
                end = j + d.len_utf8();
                chars.next();
            }
        }
        out.push(&text[start..end]);
        start = end;
    }
    out
}

fn utf16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// The spans of two paired lines that differ: what is left after the tokens they share at the start and at the end.
/// A side is `None` when it has nothing of its own (the other line only added to it); both are when the lines share
/// nothing, or the change covers most of the line, for marks to help.
pub fn word_marks(from: &str, to: &str) -> (Option<Span>, Option<Span>) {
    let a = tokens(from);
    let b = tokens(to);
    let mut head = 0;
    while head < a.len() && head < b.len() && a[head] == b[head] {
        head += 1;
    }
    let mut tail = 0;
    while tail < a.len() - head && tail < b.len() - head && a[a.len() - 1 - tail] == b[b.len() - 1 - tail] {
        tail += 1;
    }
    let prefix: usize = a[..head].iter().map(|t| t.len()).sum();
    let suffix: usize = a[a.len() - tail..].iter().map(|t| t.len()).sum();
    if prefix + suffix == 0 {
        return (None, None);
    }
    let (from_end, to_end) = (from.len() - suffix, to.len() - suffix);
    let (prefix16, suffix16) = (utf16(&from[..prefix]), utf16(&from[from.len() - suffix..]));
    let (from16, to16) = (utf16(from), utf16(to));
    let longest = from16.max(to16) as f64;
    let covered = (from16 - suffix16).max(to16 - suffix16) - prefix16;
    if covered as f64 > longest * 0.7 {
        return (None, None);
    }
    ((from_end > prefix).then_some((prefix, from_end)), (to_end > prefix).then_some((prefix, to_end)))
}

/// Marks each run of removed lines followed by a run of added lines, pairing them in order.
fn mark_words(lines: &mut [ReviewLine]) {
    let mut i = 0;
    while i < lines.len() {
        if lines[i].line.kind != Kind::Remove {
            i += 1;
            continue;
        }
        let r0 = i;
        while i < lines.len() && lines[i].line.kind == Kind::Remove {
            i += 1;
        }
        let a0 = i;
        while i < lines.len() && lines[i].line.kind == Kind::Add {
            i += 1;
        }
        for p in 0..(a0 - r0).min(i - a0) {
            let (from, to) = word_marks(&lines[r0 + p].line.content, &lines[a0 + p].line.content);
            if let Some(span) = from {
                lines[r0 + p].marks = vec![span];
            }
            if let Some(span) = to {
                lines[a0 + p].marks = vec![span];
            }
        }
    }
}

// ── Hunks and folds ─────────────────────────────────────────────────────────────────────────────────────────────────

fn name_for(lines: &[ReviewLine], before: usize) -> String {
    for line in lines[..=before].iter().rev() {
        if line.line.kind == Kind::Remove {
            continue;
        }
        let c = &line.line.content;
        let first = c.chars().next();
        if !c.trim().is_empty() && !first.is_some_and(|f| f.is_whitespace() || matches!(f, '}' | ')' | ']')) {
            let text = c.trim();
            return if utf16(text) > 64 { format!("{}…", text.chars().take(63).collect::<String>()) } else { text.to_string() };
        }
    }
    String::new()
}

/// The hunks of a file's diff and the unchanged runs between them. No changes gives no items.
pub fn build_review(diff_lines: &[DiffLine]) -> Vec<ReviewItem> {
    build_review_with(diff_lines, CONTEXT)
}

pub fn build_review_with(diff_lines: &[DiffLine], context: usize) -> Vec<ReviewItem> {
    let all: Vec<ReviewLine> =
        diff_lines.iter().enumerate().map(|(index, l)| ReviewLine { line: l.clone(), index, marks: Vec::new() }).collect();
    let changed: Vec<usize> = all.iter().filter(|l| l.line.kind != Kind::Context).map(|l| l.index).collect();
    if changed.is_empty() {
        return Vec::new();
    }
    // Group changes whose context would touch or overlap.
    let mut groups: Vec<(usize, usize)> = Vec::new();
    for index in changed {
        match groups.last_mut() {
            Some(last) if index - last.1 - 1 <= context * 2 => last.1 = index,
            _ => groups.push((index, index)),
        }
    }
    let fold = |cursor: usize, end: usize| {
        ReviewItem::Fold(ReviewFold { id: format!("fold-{cursor}"), hidden: end - cursor, lines: all[cursor..end].to_vec() })
    };
    let mut items = Vec::new();
    let mut cursor = 0;
    for (n, (first, last)) in groups.into_iter().enumerate() {
        let from = first.saturating_sub(context);
        let to = (last + context).min(all.len() - 1);
        if from > cursor {
            items.push(fold(cursor, from));
        }
        let mut lines = all[from..=to].to_vec();
        if lines.len() <= MAX_MARKED_HUNK {
            mark_words(&mut lines);
        }
        let head = lines.iter().find(|l| l.line.kind != Kind::Remove).unwrap_or(&lines[0]);
        let start_line = head.line.new.or(head.line.old).unwrap_or(1);
        items.push(ReviewItem::Hunk(ReviewHunk { id: format!("hunk-{n}"), name: name_for(&all, from), start_line, lines, from, to }));
        cursor = to + 1;
    }
    if cursor < all.len() {
        items.push(fold(cursor, all.len()));
    }
    items
}

// ── Split rows and marked pieces ────────────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitRow<'a> {
    pub left: Option<&'a ReviewLine>,
    pub right: Option<&'a ReviewLine>,
}

/// Old and new side by side: context on both, a run of removals against the run of additions that follows it.
pub fn split_rows(lines: &[ReviewLine]) -> Vec<SplitRow<'_>> {
    let mut rows = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].line.kind == Kind::Context {
            rows.push(SplitRow { left: Some(&lines[i]), right: Some(&lines[i]) });
            i += 1;
            continue;
        }
        let r0 = i;
        while i < lines.len() && lines[i].line.kind == Kind::Remove {
            i += 1;
        }
        let a0 = i;
        while i < lines.len() && lines[i].line.kind == Kind::Add {
            i += 1;
        }
        let (removes, adds) = (&lines[r0..a0], &lines[a0..i]);
        for p in 0..removes.len().max(adds.len()) {
            rows.push(SplitRow { left: removes.get(p), right: adds.get(p) });
        }
    }
    rows
}

/// A line's text in plain and marked pieces, in order.
pub fn marked_segments<'a>(content: &'a str, marks: &[(usize, usize)]) -> Vec<(&'a str, bool)> {
    if marks.is_empty() {
        return vec![(content, false)];
    }
    let mut out = Vec::new();
    let mut at = 0;
    for &(start, end) in marks {
        if start > at {
            out.push((&content[at..start], false));
        }
        out.push((&content[start..end], true));
        at = end;
    }
    if at < content.len() {
        out.push((&content[at..], false));
    }
    out
}

// ── Revert ──────────────────────────────────────────────────────────────────────────────────────────────────────────

/// One line of a file and what ended it: "\r\n", "\n", "\r", or "" for a last line with no final newline.
#[derive(Clone, Debug)]
struct RawLine<'a> {
    text: &'a str,
    eol: &'a str,
}

/// Lines the way the diff reads them (CRLF, LF and lone CR all end a line), each with its own ending.
fn raw_lines(text: &str) -> Vec<RawLine<'_>> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let (mut at, mut i) = (0, 0);
    while i < bytes.len() {
        let len = match bytes[i] {
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => 2,
            b'\r' | b'\n' => 1,
            _ => {
                i += 1;
                continue;
            }
        };
        out.push(RawLine { text: &text[at..i], eol: &text[i..i + len] });
        i += len;
        at = i;
    }
    if at < text.len() {
        out.push(RawLine { text: &text[at..], eol: "" });
    }
    out
}

/// The file's text with one range of its diff undone: added lines inside `[from, to]` are dropped and removed lines
/// come back. Everything outside the range stays as the file has it now.
///
/// With `raw` (the base and current text as read) every line keeps the ending it had in the file it came from, so a
/// CRLF, LF or mixed file is changed only in the reverted lines, and the final newline (or its absence) and a BOM stay
/// as they are. Without it lines are joined with "\n".
pub fn text_with_range_reverted(
    diff_lines: &[DiffLine],
    from: usize,
    to: usize,
    trailing_newline: bool,
    raw: Option<(&str, &str)>,
) -> String {
    if let Some((before, after)) = raw {
        return raw_text_with_range_reverted(diff_lines, from, to, before, after);
    }
    let mut out: Vec<&str> = Vec::new();
    for (index, line) in diff_lines.iter().enumerate() {
        let inside = index >= from && index <= to;
        match line.kind {
            Kind::Context => out.push(&line.content),
            Kind::Add if !inside => out.push(&line.content),
            Kind::Remove if inside => out.push(&line.content),
            _ => {}
        }
    }
    let text = out.join("\n");
    if trailing_newline && !out.is_empty() { text + "\n" } else { text }
}

/// The numbered line of its own file, or the diff's text with no ending when the file has no such line.
fn pick<'a>(lines: &[RawLine<'a>], n: Option<u32>, fallback: &'a str) -> RawLine<'a> {
    lines.get(n.unwrap_or(1) as usize - 1).cloned().unwrap_or(RawLine { text: fallback, eol: "" })
}

fn raw_text_with_range_reverted(diff_lines: &[DiffLine], from: usize, to: usize, before: &str, after: &str) -> String {
    let (before, after) = (raw_lines(before), raw_lines(after));
    let mut kept: Vec<RawLine> = Vec::new();
    for (index, line) in diff_lines.iter().enumerate() {
        let inside = index >= from && index <= to;
        match line.kind {
            Kind::Remove if inside => kept.push(pick(&before, line.old, &line.content)),
            Kind::Remove => {}
            Kind::Add if inside => {}
            _ => kept.push(pick(&after, line.new, &line.content)),
        }
    }
    if kept.is_empty() {
        return String::new();
    }
    // A line that lost its ending (it was the old last line) takes the file's usual one; the last line ends the way the
    // file ends now. Ties go to the ending seen first.
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for line in after.iter().chain(before.iter()).filter(|l| !l.eol.is_empty()) {
        match counts.iter_mut().find(|c| c.0 == line.eol) {
            Some(c) => c.1 += 1,
            None => counts.push((line.eol, 1)),
        }
    }
    let mut usual = "\n";
    let mut best = 0;
    for (eol, n) in counts {
        if n > best {
            usual = eol;
            best = n;
        }
    }
    let ends_with_newline = after.last().is_none_or(|l| !l.eol.is_empty());
    let last = kept.len() - 1;
    let mut out = String::new();
    for (i, line) in kept.iter().enumerate() {
        out.push_str(line.text);
        if i < last || ends_with_newline {
            out.push_str(if line.eol.is_empty() { usual } else { line.eol });
        }
    }
    out
}
