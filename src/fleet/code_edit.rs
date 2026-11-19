//! The editing model of the code view's editor spike (component 11, piece P; row 13.1 of docs/parity/11-files.md, which Fleet does
//! with CodeMirror's `EditorState`, `history()` and the default keymap): a caret and an anchor in a file kept as lines, every change
//! one replacement of a range, and an undo history that groups typing as CodeMirror's does (a pause of 500 ms or a caret that moved
//! starts a new step). Plain Rust: the view draws from it and the keys call it.
use crate::conv::minimal_change::MinimalChange;
use std::ops::Range;

/// A place in the text: a line (0-based) and a byte inside it, always on a character boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pos {
    pub line: usize,
    pub byte: usize,
}

impl Pos {
    pub fn new(line: usize, byte: usize) -> Pos {
        Pos { line, byte }
    }
}

/// CodeMirror's `newGroupDelay`: typing closer together than this undoes as one step.
const GROUP_DELAY: f64 = 0.5;
/// CodeMirror's `indentUnit` as Fleet sets it: two spaces (row 13.1).
pub const INDENT: &str = "  ";

/// What kind of edit a step was, for grouping: typed characters join the step before, as deletions by Backspace do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Type,
    Delete,
    Other,
}

/// One undoable change: `removed` was replaced by `inserted` starting at `at`, with the selection before and after it.
#[derive(Clone, Debug)]
struct Step {
    at: Pos,
    removed: String,
    inserted: String,
    before: (Pos, Pos),
    after: (Pos, Pos),
    kind: Kind,
    time: f64,
}

/// The editor's state over one file's lines: the selection (an anchor and the caret, the same when nothing is selected), the column
/// up and down keep to, the history, and an input method's composition drawn at the caret.
#[derive(Clone, Debug, Default)]
pub struct Editor {
    pub anchor: Pos,
    pub caret: Pos,
    /// The column Up and Down aim for, kept across short lines (CodeMirror's `goalColumn`), in characters.
    goal: Option<usize>,
    undo: Vec<Step>,
    redo: Vec<Step>,
    /// An input method's composition and its cursor (bytes of it): drawn at the caret, not yet in the text.
    pub preedit: Option<(String, Option<(usize, usize)>)>,
}

/// The text between `a` and `b` (`a <= b`), its lines joined by `\n`.
pub fn text_between(lines: &[String], a: Pos, b: Pos) -> String {
    if a.line == b.line {
        return lines[a.line][a.byte..b.byte].to_string();
    }
    let mut out = lines[a.line][a.byte..].to_string();
    for l in &lines[a.line + 1..b.line] {
        out.push('\n');
        out.push_str(l);
    }
    out.push('\n');
    out.push_str(&lines[b.line][..b.byte]);
    out
}

/// Replaces `a..b` with `text` (whose `\n`s make new lines) and returns where the inserted text ends. Only the lines touched are
/// rebuilt; the lines after shift in the vector, which for a file of twenty thousand lines is a copy of their pointers.
pub fn replace(lines: &mut Vec<String>, a: Pos, b: Pos, text: &str) -> Pos {
    if lines.is_empty() {
        lines.push(String::new());
    }
    let head = &lines[a.line][..a.byte];
    let tail = &lines[b.line][b.byte..];
    let mut parts = text.split('\n');
    let first = parts.next().unwrap_or("");
    let rest: Vec<&str> = parts.collect();
    let end;
    let made: Vec<String> = if rest.is_empty() {
        end = Pos::new(a.line, head.len() + first.len());
        vec![format!("{head}{first}{tail}")]
    } else {
        let mut made = Vec::with_capacity(rest.len() + 1);
        made.push(format!("{head}{first}"));
        for p in &rest[..rest.len() - 1] {
            made.push(p.to_string());
        }
        let last = rest[rest.len() - 1];
        end = Pos::new(a.line + rest.len(), last.len());
        made.push(format!("{last}{tail}"));
        made
    };
    lines.splice(a.line..=b.line, made);
    end
}

/// The characters before `at`, a break counting one (the position `MinimalChange` speaks in).
pub fn offset_of(lines: &[String], at: Pos) -> usize {
    let before: usize = lines.iter().take(at.line).map(|l| l.chars().count() + 1).sum();
    before + lines.get(at.line).map_or(0, |l| l[..at.byte.min(l.len())].chars().count())
}

/// The inverse of [`offset_of`]; past the end is the end.
pub fn pos_of(lines: &[String], mut offset: usize) -> Pos {
    for (i, l) in lines.iter().enumerate() {
        let n = l.chars().count();
        if offset <= n || i + 1 == lines.len() {
            let byte = l.char_indices().nth(offset).map_or(l.len(), |(b, _)| b);
            return Pos::new(i, byte);
        }
        offset -= n + 1;
    }
    Pos::default()
}

/// Where `text` inserted at `at` ends.
fn end_of(at: Pos, text: &str) -> Pos {
    match text.rfind('\n') {
        Some(i) => Pos::new(at.line + text.matches('\n').count(), text.len() - i - 1),
        None => Pos::new(at.line, at.byte + text.len()),
    }
}

fn prev_char(line: &str, byte: usize) -> usize {
    line[..byte].char_indices().next_back().map_or(0, |(i, _)| i)
}

fn next_char(line: &str, byte: usize) -> usize {
    line[byte..].chars().next().map_or(byte, |c| byte + c.len_utf8())
}

/// The column (in characters, a tab to the next stop of four) of `byte` in `line`.
pub fn column(line: &str, byte: usize) -> usize {
    line[..byte].chars().fold(0, |col, c| if c == '\t' { col + 4 - col % 4 } else { col + 1 })
}

/// The byte of `line` nearest column `goal`, not past its end.
fn byte_at_column(line: &str, goal: usize) -> usize {
    let mut col = 0;
    for (i, c) in line.char_indices() {
        let w = if c == '\t' { 4 - col % 4 } else { 1 };
        if col + w > goal {
            return if goal - col > w / 2 { i + c.len_utf8() } else { i };
        }
        col += w;
    }
    line.len()
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// CodeMirror's `cursorGroupForward` on one line: past the spaces, then past a run of word characters or of other punctuation.
fn word_end(line: &str, byte: usize) -> usize {
    let mut chars = line[byte..].char_indices().map(|(i, c)| (byte + i, c)).peekable();
    while chars.peek().is_some_and(|(_, c)| c.is_whitespace()) {
        chars.next();
    }
    let Some(&(_, first)) = chars.peek() else { return line.len() };
    let word = is_word(first);
    for (i, c) in chars {
        if c.is_whitespace() || is_word(c) != word {
            return i;
        }
    }
    line.len()
}

fn word_start(line: &str, byte: usize) -> usize {
    let mut chars = line[..byte].char_indices().rev().peekable();
    while chars.peek().is_some_and(|(_, c)| c.is_whitespace()) {
        chars.next();
    }
    let Some(&(_, first)) = chars.peek() else { return 0 };
    let word = is_word(first);
    let mut at = byte;
    for (i, c) in chars {
        if c.is_whitespace() || is_word(c) != word {
            return at;
        }
        at = i;
    }
    0
}

/// The leading spaces and tabs of `line`.
pub fn indent_of(line: &str) -> &str {
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

/// A motion of the caret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Left,
    Right,
    WordLeft,
    WordRight,
    Up(usize),
    Down(usize),
    LineStart,
    LineEnd,
    DocStart,
    DocEnd,
}

impl Editor {
    /// The selection in order, start first.
    pub fn range(&self) -> (Pos, Pos) {
        if self.anchor <= self.caret { (self.anchor, self.caret) } else { (self.caret, self.anchor) }
    }

    pub fn has_selection(&self) -> bool {
        self.anchor != self.caret
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The selected bytes of line `i`, and whether the selection runs on past its end (drawn as one more space, as CodeMirror does).
    pub fn selected_on(&self, lines: &[String], i: usize) -> Option<(Range<usize>, bool)> {
        let (a, b) = self.range();
        if a == b || i < a.line || i > b.line {
            return None;
        }
        let len = lines.get(i).map_or(0, String::len);
        let from = if i == a.line { a.byte } else { 0 };
        let to = if i == b.line { b.byte } else { len };
        Some((from..to, i < b.line))
    }

    pub fn selected_text(&self, lines: &[String]) -> String {
        let (a, b) = self.range();
        text_between(lines, a, b)
    }

    /// Keeps the caret and anchor inside the text after it changed under the editor (the agent's edit, a new read).
    pub fn clamp(&mut self, lines: &[String]) {
        let fit = |p: Pos| {
            let line = p.line.min(lines.len().saturating_sub(1));
            let text = lines.get(line).map_or("", String::as_str);
            let mut byte = p.byte.min(text.len());
            while !text.is_char_boundary(byte) {
                byte -= 1;
            }
            Pos::new(line, byte)
        };
        self.caret = fit(self.caret);
        self.anchor = fit(self.anchor);
    }

    /// The text changed under the editor without it (the agent's edit on a clean buffer, `Transaction.remote`): the selection moves with
    /// the change. The history is dropped: its steps name places in the text from before, and CodeMirror's rebases them, which this
    /// model doesn't (a gap in docs/parity/11-files.md).
    pub fn remote(&mut self, before: &[String], change: &MinimalChange, after: &[String]) {
        let map = |p: Pos| pos_of(after, change.map(offset_of(before, p)));
        self.anchor = map(self.anchor);
        self.caret = map(self.caret);
        self.goal = None;
        self.undo.clear();
        self.redo.clear();
        self.preedit = None;
    }

    /// Makes `change` as one step of its own, never merged with the typing before it or after it (`isolateHistory`): "Use the agent's".
    pub fn isolated(&mut self, lines: &mut Vec<String>, change: &MinimalChange, now: f64) {
        let (a, b) = (pos_of(lines, change.from), pos_of(lines, change.to));
        let before = (self.anchor, self.caret);
        self.change_from(lines, a, b, &change.insert.join("\n"), Kind::Other, now, before);
    }

    pub fn set_caret(&mut self, at: Pos, extend: bool) {
        self.caret = at;
        if !extend {
            self.anchor = at;
        }
        self.goal = None;
    }

    pub fn select_all(&mut self, lines: &[String]) {
        self.anchor = Pos::default();
        let last = lines.len().saturating_sub(1);
        self.caret = Pos::new(last, lines.get(last).map_or(0, String::len));
        self.goal = None;
    }

    /// Moves the caret; with `extend` the anchor stays (Shift). Left and Right with a selection and no Shift go to its edge.
    pub fn motion(&mut self, lines: &[String], m: Move, extend: bool) {
        let c = self.caret;
        let line = |i: usize| lines.get(i).map_or("", String::as_str);
        let last = lines.len().saturating_sub(1);
        if !extend && self.has_selection() && matches!(m, Move::Left | Move::Right) {
            let (a, b) = self.range();
            return self.set_caret(if m == Move::Left { a } else { b }, false);
        }
        let mut goal = None;
        let to = match m {
            Move::Left if c.byte > 0 => Pos::new(c.line, prev_char(line(c.line), c.byte)),
            Move::Left if c.line > 0 => Pos::new(c.line - 1, line(c.line - 1).len()),
            Move::Left => c,
            Move::Right if c.byte < line(c.line).len() => Pos::new(c.line, next_char(line(c.line), c.byte)),
            Move::Right if c.line < last => Pos::new(c.line + 1, 0),
            Move::Right => c,
            Move::WordLeft if c.byte == 0 && c.line > 0 => Pos::new(c.line - 1, line(c.line - 1).len()),
            Move::WordLeft => Pos::new(c.line, word_start(line(c.line), c.byte)),
            Move::WordRight if c.byte == line(c.line).len() && c.line < last => Pos::new(c.line + 1, 0),
            Move::WordRight => Pos::new(c.line, word_end(line(c.line), c.byte)),
            Move::Up(n) | Move::Down(n) => {
                let col = self.goal.unwrap_or_else(|| column(line(c.line), c.byte));
                goal = Some(col);
                let to_line = if let Move::Up(_) = m { c.line.saturating_sub(n) } else { (c.line + n).min(last) };
                if to_line == c.line {
                    // CodeMirror moves to the start of the first line on Up there, and to the end of the last on Down.
                    if let Move::Up(_) = m { Pos::new(c.line, 0) } else { Pos::new(c.line, line(c.line).len()) }
                } else {
                    Pos::new(to_line, byte_at_column(line(to_line), col))
                }
            }
            // `cursorLineBoundaryBackward`: to the first non-space character, then to the very start.
            Move::LineStart => {
                let indent = indent_of(line(c.line)).len();
                Pos::new(c.line, if c.byte == indent { 0 } else { indent })
            }
            Move::LineEnd => Pos::new(c.line, line(c.line).len()),
            Move::DocStart => Pos::default(),
            Move::DocEnd => Pos::new(last, line(last).len()),
        };
        self.caret = to;
        if !extend {
            self.anchor = to;
        }
        self.goal = goal;
    }

    /// Replaces the selection with `text` as one step of the history (joined to the one before when it is typing that follows on).
    fn change(&mut self, lines: &mut Vec<String>, a: Pos, b: Pos, text: &str, kind: Kind, now: f64) {
        let before = (self.anchor, self.caret);
        self.change_from(lines, a, b, text, kind, now, before);
    }

    /// [`Self::change`] with the selection as it was before the key (a deletion moves the caret to find its range first).
    #[allow(clippy::too_many_arguments)]
    fn change_from(&mut self, lines: &mut Vec<String>, a: Pos, b: Pos, text: &str, kind: Kind, now: f64, before: (Pos, Pos)) {
        let removed = text_between(lines, a, b);
        let end = replace(lines, a, b, text);
        self.anchor = end;
        self.caret = end;
        self.goal = None;
        self.redo.clear();
        let after = (end, end);
        if let Some(last) = self.undo.last_mut()
            && kind != Kind::Other
            && last.kind == kind
            && now - last.time < GROUP_DELAY
            && last.after == before
        {
            match kind {
                // Typing on from where the last character went.
                Kind::Type if removed.is_empty() && end_of(last.at, &last.inserted) == a => {
                    last.inserted.push_str(text);
                    last.after = after;
                    last.time = now;
                    return;
                }
                // Backspace on from where the last deletion stopped.
                Kind::Delete if text.is_empty() && last.inserted.is_empty() && b == last.at => {
                    last.removed.insert_str(0, &removed);
                    last.at = a;
                    last.after = after;
                    last.time = now;
                    return;
                }
                _ => {}
            }
        }
        self.undo.push(Step { at: a, removed, inserted: text.to_string(), before, after, kind, time: now });
    }

    /// Types `text` over the selection (a key, a paste, an input method's commit).
    pub fn insert(&mut self, lines: &mut Vec<String>, text: &str, now: f64) {
        let (a, b) = self.range();
        let kind = if text.contains('\n') || a != b { Kind::Other } else { Kind::Type };
        self.change(lines, a, b, text, kind, now);
    }

    /// A paste: never joined to typing.
    pub fn paste(&mut self, lines: &mut Vec<String>, text: &str, now: f64) {
        let (a, b) = self.range();
        self.change(lines, a, b, text, Kind::Other, now);
    }

    /// Enter: a new line indented as the one it breaks (`insertNewlineAndIndent`, short of the language's own indent rules).
    pub fn newline(&mut self, lines: &mut Vec<String>, now: f64) {
        let (a, _) = self.range();
        let indent = indent_of(&lines[a.line][..a.byte]).to_string();
        self.insert(lines, &format!("\n{indent}"), now);
    }

    /// Tab: the indent unit at the caret (`insertTab` with Fleet's two spaces).
    pub fn tab(&mut self, lines: &mut Vec<String>, now: f64) {
        self.insert(lines, INDENT, now);
    }

    /// Backspace (or Delete with `forward`): the selection, else one character (a word with `word`); at a line's edge it joins lines.
    pub fn delete(&mut self, lines: &mut Vec<String>, forward: bool, word: bool, now: f64) {
        if self.has_selection() {
            let (a, b) = self.range();
            return self.change(lines, a, b, "", Kind::Other, now);
        }
        let keep = (self.anchor, self.caret, self.goal);
        let m = match (forward, word) {
            (false, false) => Move::Left,
            (false, true) => Move::WordLeft,
            (true, false) => Move::Right,
            (true, true) => Move::WordRight,
        };
        self.motion(lines, m, true);
        let (a, b) = self.range();
        if a == b {
            (self.anchor, self.caret, self.goal) = keep;
            return;
        }
        self.change_from(lines, a, b, "", if forward { Kind::Other } else { Kind::Delete }, now, (keep.0, keep.1));
    }

    /// Undoes the last step and puts the selection back as it was before it. Returns whether there was one.
    pub fn undo(&mut self, lines: &mut Vec<String>) -> bool {
        let Some(step) = self.undo.pop() else { return false };
        replace(lines, step.at, end_of(step.at, &step.inserted), &step.removed);
        (self.anchor, self.caret) = step.before;
        self.goal = None;
        self.redo.push(step);
        true
    }

    pub fn redo(&mut self, lines: &mut Vec<String>) -> bool {
        let Some(mut step) = self.redo.pop() else { return false };
        replace(lines, step.at, end_of(step.at, &step.removed), &step.inserted);
        (self.anchor, self.caret) = step.after;
        self.goal = None;
        // A redone step never joins what is typed next.
        step.time = f64::NEG_INFINITY;
        self.undo.push(step);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> Vec<String> {
        text.split('\n').map(String::from).collect()
    }

    fn joined(lines: &[String]) -> String {
        lines.join("\n")
    }

    #[test]
    fn replacing_a_range_across_lines_rebuilds_only_those_lines() {
        let mut lines = doc("one\ntwo\nthree\nfour");
        let end = replace(&mut lines, Pos::new(1, 1), Pos::new(2, 2), "X\nY");
        assert_eq!(joined(&lines), "one\ntX\nYree\nfour");
        assert_eq!(end, Pos::new(2, 1));
        let end = replace(&mut lines, Pos::new(0, 3), Pos::new(1, 0), "");
        assert_eq!(joined(&lines), "onetX\nYree\nfour");
        assert_eq!(end, Pos::new(0, 3));
    }

    #[test]
    fn typing_quickly_undoes_as_one_step_and_a_pause_starts_another() {
        let mut lines = doc("def f():\n    pass");
        let mut e = Editor::default();
        e.set_caret(Pos::new(1, 8), false);
        for (i, c) in ["  #", " o", "k"].iter().enumerate() {
            e.insert(&mut lines, c, i as f64 * 0.1);
        }
        e.insert(&mut lines, "!", 2.0);
        assert_eq!(lines[1], "    pass  # ok!");
        assert!(e.undo(&mut lines));
        assert_eq!(lines[1], "    pass  # ok");
        assert!(e.undo(&mut lines));
        assert_eq!(lines[1], "    pass");
        assert_eq!(e.caret, Pos::new(1, 8));
        assert!(!e.undo(&mut lines));
        assert!(e.redo(&mut lines) && e.redo(&mut lines));
        assert_eq!(lines[1], "    pass  # ok!");
        assert_eq!(e.caret, Pos::new(1, 15));
    }

    #[test]
    fn moving_the_caret_between_keys_starts_a_new_step() {
        let mut lines = doc("ab");
        let mut e = Editor::default();
        e.insert(&mut lines, "x", 0.0);
        e.motion(&lines, Move::LineEnd, false);
        e.insert(&mut lines, "y", 0.1);
        assert_eq!(lines[0], "xaby");
        e.undo(&mut lines);
        assert_eq!(lines[0], "xab");
    }

    #[test]
    fn backspaces_group_and_join_lines() {
        let mut lines = doc("ab\ncd");
        let mut e = Editor::default();
        e.set_caret(Pos::new(1, 1), false);
        e.delete(&mut lines, false, false, 0.0);
        e.delete(&mut lines, false, false, 0.1);
        e.delete(&mut lines, false, false, 0.2);
        assert_eq!(joined(&lines), "ad");
        assert_eq!(e.caret, Pos::new(0, 1));
        e.undo(&mut lines);
        assert_eq!(joined(&lines), "ab\ncd");
        assert_eq!(e.caret, Pos::new(1, 1));
    }

    #[test]
    fn up_and_down_keep_their_column_across_a_short_line() {
        let lines = doc("abcdefgh\nab\nabcdefgh");
        let mut e = Editor::default();
        e.set_caret(Pos::new(0, 6), false);
        e.motion(&lines, Move::Down(1), false);
        assert_eq!(e.caret, Pos::new(1, 2));
        e.motion(&lines, Move::Down(1), false);
        assert_eq!(e.caret, Pos::new(2, 6));
        e.motion(&lines, Move::Down(1), false);
        assert_eq!(e.caret, Pos::new(2, 8), "Down on the last line goes to its end");
    }

    #[test]
    fn shift_extends_and_typing_replaces_the_selection() {
        let mut lines = doc("return page_size");
        let mut e = Editor::default();
        e.set_caret(Pos::new(0, 7), false);
        e.motion(&lines, Move::WordRight, true);
        assert_eq!(e.selected_text(&lines), "page_size");
        e.insert(&mut lines, "n", 0.0);
        assert_eq!(lines[0], "return n");
        e.undo(&mut lines);
        assert_eq!(lines[0], "return page_size");
        assert_eq!(e.selected_text(&lines), "page_size", "undo brings the selection back");
    }

    #[test]
    fn enter_keeps_the_indent_and_home_goes_to_it_first() {
        let mut lines = doc("    x = 1");
        let mut e = Editor::default();
        e.set_caret(Pos::new(0, 9), false);
        e.newline(&mut lines, 0.0);
        assert_eq!(joined(&lines), "    x = 1\n    ");
        e.insert(&mut lines, "y", 0.1);
        e.motion(&lines, Move::LineStart, false);
        assert_eq!(e.caret, Pos::new(1, 4));
        e.motion(&lines, Move::LineStart, false);
        assert_eq!(e.caret, Pos::new(1, 0));
    }

    #[test]
    fn wide_and_multibyte_characters_move_as_one() {
        let lines = doc("aé日b");
        let mut e = Editor::default();
        e.motion(&lines, Move::Right, false);
        e.motion(&lines, Move::Right, false);
        assert_eq!(e.caret.byte, 3);
        e.motion(&lines, Move::Right, false);
        assert_eq!(e.caret.byte, 6);
        e.motion(&lines, Move::Left, false);
        assert_eq!(e.caret.byte, 3);
    }

    #[test]
    fn a_selection_tells_each_line_what_of_it_is_selected() {
        let lines = doc("ab\ncd\nef");
        let mut e = Editor::default();
        e.set_caret(Pos::new(0, 1), false);
        e.set_caret(Pos::new(2, 1), true);
        assert_eq!(e.selected_on(&lines, 0), Some((1..2, true)));
        assert_eq!(e.selected_on(&lines, 1), Some((0..2, true)));
        assert_eq!(e.selected_on(&lines, 2), Some((0..1, false)));
        assert_eq!(e.selected_text(&lines), "b\ncd\ne");
    }
}
