//! `@` and `$` references in a draft, as the composer draws and edits them: `composer-references.ts`
//! (`splitDraftReferences`, `pickedReferenceRanges`, `referenceKeyAction`, `caretOutsideReferences`).
//!
//! They are plain text in the message (the agent reads the path); the composer only tints them, and edits the ones
//! picked from the `@` list as one piece. No Rune in here. Positions are byte offsets on character boundaries, where
//! Fleet's are UTF-16 indices.
use super::refs::{SkillDelivery, inside_backticks};
use std::ops::Range;

/// What a reference is, and so how it is tinted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    File,
    Folder,
    Session,
    /// A skill; `note` when it goes to the agent as a note rather than with the message.
    Skill {
        note: bool,
    },
}

/// A reference in the text: where it is (`start` is the `@` or `$`, `end` just past its last character), what it is,
/// and whether it was picked from the list (drawn even with the caret at its end, and edited as one piece).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefSpan {
    pub range: Range<usize>,
    pub kind: RefKind,
    pub picked: bool,
}

/// A stretch of the draft: plain, or a reference (`DraftSegment`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub text: String,
    pub reference: Option<RefKind>,
    pub picked: bool,
}

/// What the draft knows besides its text: the tokens picked from the `@` list, the skills named and how each is
/// delivered, and tokens drawn plain unless picked again.
#[derive(Debug, Clone, Copy, Default)]
pub struct Known<'a> {
    pub sessions: &'a [&'a str],
    pub files: &'a [&'a str],
    pub skills: &'a [(&'a str, SkillDelivery)],
    pub plain: &'a [&'a str],
}

fn trim_trailing_punctuation(token: &str) -> &str {
    token.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}', '\'', '"'])
}

/// The references in `text`, in order. A token the caret is in (or at the end of) is still being typed, so it stays
/// plain until the caret moves past it, unless it was picked. A `$` token counts only when it names a known skill
/// outside backticks. (`/(^|\s)(@[^\s@][^\s]*|\$[^\s$][^\s]*)/g`.)
pub fn reference_spans(text: &str, caret: Option<usize>, known: &Known) -> Vec<RefSpan> {
    let mut spans = Vec::new();
    let mut prev_space = true;
    let mut chars = text.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        let opens = (c == '@' || c == '$') && prev_space && chars.peek().is_some_and(|(_, n)| !n.is_whitespace() && *n != c);
        if !opens {
            prev_space = c.is_whitespace();
            continue;
        }
        let token_end = text[start..].find(char::is_whitespace).map_or(text.len(), |e| start + e);
        while chars.peek().is_some_and(|(i, _)| *i < token_end) {
            chars.next();
        }
        prev_space = false;
        let token = trim_trailing_punctuation(&text[start..token_end]);
        let end = start + token.len();

        let skill = if c == '$' { known.skills.iter().find(|(name, _)| *name == &token[1..]).map(|(_, d)| *d) } else { None };
        if c == '$' && (skill.is_none() || inside_backticks(text, start)) {
            continue;
        }
        let is_session = known.sessions.contains(&token);
        let picked = is_session || known.files.contains(&token);
        let typing = caret.is_some_and(|k| k > start && k <= token_end);
        if token.chars().count() < 2 || (!picked && (known.plain.contains(&token) || typing)) {
            continue;
        }
        let kind = match skill {
            Some(delivery) => RefKind::Skill { note: delivery == SkillDelivery::Note },
            None if is_session => RefKind::Session,
            None if token.ends_with('/') => RefKind::Folder,
            None => RefKind::File,
        };
        spans.push(RefSpan { range: start..end, kind, picked });
    }
    spans
}

/// The draft cut into plain text and references (`splitDraftReferences`).
pub fn split_draft_references(text: &str, caret: Option<usize>, known: &Known) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut at = 0;
    for span in reference_spans(text, caret, known) {
        if span.range.start > at {
            out.push(Segment { text: text[at..span.range.start].into(), reference: None, picked: false });
        }
        out.push(Segment { text: text[span.range.clone()].into(), reference: Some(span.kind), picked: span.picked });
        at = span.range.end;
    }
    if at < text.len() {
        out.push(Segment { text: text[at..].into(), reference: None, picked: false });
    }
    out
}

/// Where the references picked from the `@` list are in `text`, in order (`pickedReferenceRanges`).
pub fn picked_reference_ranges(text: &str, sessions: &[&str], files: &[&str]) -> Vec<Range<usize>> {
    let known = Known { sessions, files, ..Known::default() };
    reference_spans(text, None, &known).into_iter().filter(|s| s.picked).map(|s| s.range).collect()
}

/// The keys that touch a picked reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKey {
    Backspace,
    Delete,
    Left,
    Right,
}

/// The selection to make before the key does its work, and whether the key's own work is then stopped: the arrows are;
/// Backspace and Delete go on to delete the selection (`ReferenceKeyAction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyAction {
    pub selection: (usize, usize),
    pub prevent_default: bool,
}

/// What a key does to a picked reference at a caret with no selection. Backspace after one, or Delete before it,
/// selects it whole and lets the field delete the selection, so it goes in one piece and undo puts it back. The
/// arrows step over it. None when the key doesn't touch one (`referenceKeyAction`).
pub fn reference_key_action(key: RefKey, caret: usize, ranges: &[Range<usize>]) -> Option<KeyAction> {
    for Range { start, end } in ranges.iter().cloned() {
        let touches_end = caret > start && caret <= end;
        let touches_start = caret >= start && caret < end;
        let action = |selection, prevent_default| Some(KeyAction { selection, prevent_default });
        match key {
            RefKey::Backspace if touches_end => return action((start, end), false),
            RefKey::Delete if touches_start => return action((start, end), false),
            RefKey::Left if touches_end => return action((start, start), true),
            RefKey::Right if touches_start => return action((end, end), true),
            _ => {}
        }
    }
    None
}

/// A caret inside a picked reference (after a click, say) moves to its nearer edge, a tie to the start; anywhere
/// else it stays (`caretOutsideReferences`).
pub fn caret_outside_references(caret: usize, ranges: &[Range<usize>]) -> usize {
    match ranges.iter().find(|r| caret > r.start && caret < r.end) {
        Some(r) if caret - r.start <= r.end - caret => r.start,
        Some(r) => r.end,
        None => caret,
    }
}
