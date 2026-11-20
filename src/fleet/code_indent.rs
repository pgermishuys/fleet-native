//! What typing does besides inserting the character (component 11, piece U; row 13.1 of docs/parity/11-files.md, which Fleet gets from
//! CodeMirror's `closeBrackets()`, `indentOnInput()` and `insertNewlineAndIndent`): brackets and quotes close, a closing one typed
//! over its twin steps past it, Backspace between a pair takes both, Enter indents by the language's rule, and a closing bracket
//! typed on a line of its own goes back to the indent of the line that opened it. All of it is `Editor` methods over the file's lines.
//!
//! Where it differs from CodeMirror: it doesn't remember which brackets *it* closed (CodeMirror steps over only those), and it reads
//! plain characters, not the syntax tree, so a bracket inside a string or comment counts. The language's rule is chosen by the file's
//! name: Python adds a level after `:`; the languages with braces after an opening bracket; anything else copies the line's indent.
use super::brackets;
use super::code_edit::{Editor, INDENT, Kind, Pos, indent_of};
use crate::conv::file_language::language_for;

/// How a file indents the line after Enter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Indent {
    /// A language with brackets: a level after an opening one, a closing one dedents.
    #[default]
    Braces,
    /// Python: a level after `:`, and after an opening bracket too.
    Python,
    /// Plain text and markup: the line's own indent.
    Copy,
}

/// The rule for `path`.
pub fn indent_for(path: &str) -> Indent {
    match language_for(path) {
        Some("python") => Indent::Python,
        Some("markdown" | "yaml" | "xml" | "html" | "properties" | "toml" | "ini" | "csv" | "diff" | "vue") | None => Indent::Copy,
        Some(_) => Indent::Braces,
    }
}

const PAIRS: [(char, char); 3] = [('(', ')'), ('[', ']'), ('{', '}')];
const QUOTES: [char; 3] = ['"', '\'', '`'];
/// What a bracket may close in front of (CodeMirror's `before`): the end of the line, a space, or one of these.
const CLOSE_BEFORE: &str = ")]}:;>,";

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn closer_of(c: char) -> Option<char> {
    PAIRS.iter().find(|(o, _)| *o == c).map(|(_, k)| *k)
}

fn is_closer(c: char) -> bool {
    PAIRS.iter().any(|(_, k)| *k == c)
}

impl Editor {
    fn char_before(&self, lines: &[String]) -> Option<char> {
        lines.get(self.caret.line)?[..self.caret.byte].chars().next_back()
    }

    fn char_after(&self, lines: &[String]) -> Option<char> {
        lines.get(self.caret.line)?[self.caret.byte..].chars().next()
    }

    /// A key that types `text`: one character may close a bracket or quote, step over its twin, or dedent a closing bracket at the
    /// start of a line; anything longer (an input method's commit) is inserted as it is.
    pub fn type_text(&mut self, lines: &mut Vec<String>, text: &str, indent: Indent, now: f64) {
        let mut chars = text.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else { return self.insert(lines, text, now) };
        let selected = self.has_selection();
        // Over a selection an opener wraps it, keeping the text selected (`closeBrackets` `handleOpen` with a range).
        if selected && let Some(close) = closer_of(c).or_else(|| QUOTES.contains(&c).then_some(c)) {
            let (a, b) = self.range();
            let inner = text_of(lines, a, b);
            self.change(lines, a, b, &format!("{c}{inner}{close}"), Kind::Other, now);
            let from = Pos::new(a.line, a.byte + c.len_utf8());
            let to = end_after(from, &inner);
            (self.anchor, self.caret) = (from, to);
            return;
        }
        if selected {
            return self.insert(lines, text, now);
        }
        // Past a twin that is already there.
        if (is_closer(c) || QUOTES.contains(&c)) && self.char_after(lines) == Some(c) {
            let to = Pos::new(self.caret.line, self.caret.byte + c.len_utf8());
            self.set_caret(to, false);
            return;
        }
        let (before, after) = (self.char_before(lines), self.char_after(lines));
        let room = after.is_none_or(|a| a.is_whitespace() || CLOSE_BEFORE.contains(a));
        if let Some(close) = closer_of(c).filter(|_| room) {
            return self.insert_pair(lines, c, close, now);
        }
        if QUOTES.contains(&c) && room && !before.is_some_and(is_word) && !(c == '\'' && before == Some('\\')) {
            return self.insert_pair(lines, c, c, now);
        }
        // A closing bracket typed with only space before it takes the indent of the line its opener is on.
        if is_closer(c) && indent == Indent::Braces && self.dedent_closer(lines, c, now) {
            return;
        }
        self.insert(lines, text, now);
    }

    fn insert_pair(&mut self, lines: &mut Vec<String>, open: char, close: char, now: f64) {
        let at = self.caret;
        self.change(lines, at, at, &format!("{open}{close}"), Kind::Type, now);
        let inside = Pos::new(at.line, at.byte + open.len_utf8());
        // The step's selection after it is the caret inside the pair, so typing on groups with it.
        self.set_caret(inside, false);
        self.settle_last_step();
    }

    /// Makes the last undo step end with the selection as it is now (it was taken before the caret was put inside a pair).
    fn settle_last_step(&mut self) {
        let sel = (self.anchor, self.caret);
        self.set_last_after(sel);
    }

    fn dedent_closer(&mut self, lines: &mut Vec<String>, c: char, now: f64) -> bool {
        let at = self.caret;
        let Some(text) = lines.get(at.line) else { return false };
        if !text[..at.byte].chars().all(|x| x == ' ' || x == '\t') {
            return false;
        }
        // Look for the opener with the closer in place, then put the line back as it was.
        let original = text.clone();
        lines[at.line].insert(at.byte, c);
        let found = brackets::matching(lines, Pos::new(at.line, at.byte + c.len_utf8()));
        lines[at.line] = original;
        let Some((_, opener)) = found else { return false };
        if opener.line >= at.line {
            return false;
        }
        let want = indent_of(&lines[opener.line]).to_string();
        self.change(lines, Pos::new(at.line, 0), at, &format!("{want}{c}"), Kind::Other, now);
        true
    }

    /// Backspace with the caret between an empty pair (`()`, `""`) takes both. True when it did.
    pub fn backspace_pair(&mut self, lines: &mut Vec<String>, now: f64) -> bool {
        if self.has_selection() {
            return false;
        }
        let (Some(b), Some(a)) = (self.char_before(lines), self.char_after(lines)) else { return false };
        let pair = closer_of(b) == Some(a) || (QUOTES.contains(&b) && a == b);
        if !pair {
            return false;
        }
        let c = self.caret;
        let from = Pos::new(c.line, c.byte - b.len_utf8());
        let to = Pos::new(c.line, c.byte + a.len_utf8());
        let keep = (self.anchor, self.caret);
        self.change_from(lines, from, to, "", Kind::Delete, now, keep);
        true
    }

    /// Enter: a new line indented by the file's rule (`insertNewlineAndIndent`). Between a pair of brackets it opens a block: the
    /// closing one goes on a line of its own at the old indent.
    pub fn newline_in(&mut self, lines: &mut Vec<String>, indent: Indent, now: f64) {
        let (a, b) = self.range();
        let before = &lines[a.line][..a.byte];
        let after = &lines[b.line][b.byte..];
        let base = indent_of(before).to_string();
        let trimmed = before.trim_end();
        let last = trimmed.chars().next_back();
        let opens = last.is_some_and(|c| closer_of(c).is_some());
        let deeper = match indent {
            Indent::Copy => false,
            Indent::Braces => opens,
            Indent::Python => opens || last == Some(':'),
        };
        let level = if deeper { format!("{base}{INDENT}") } else { base.clone() };
        let between = opens && after.trim_start().chars().next() == last.and_then(closer_of) && indent != Indent::Copy;
        if between {
            // The caret ends on the middle line.
            let text = format!("\n{level}\n{base}");
            self.change(lines, a, b, &text, Kind::Other, now);
            let caret = Pos::new(a.line + 1, level.len());
            self.set_caret(caret, false);
            self.settle_last_step();
            return;
        }
        // Spaces left before the caret on the line it breaks are dropped (the line's trailing space), as CodeMirror does.
        self.change(lines, a, b, &format!("\n{level}"), Kind::Other, now);
    }
}

impl Editor {
    /// Replaces each of `edits` (non-overlapping, in reading order) as one step of the history, so one undo takes them all back
    /// (CodeMirror's replace all is one transaction). The caret ends after the last replacement.
    pub fn replace_all(&mut self, lines: &mut Vec<String>, edits: &[(Pos, Pos, String)], now: f64) {
        let (Some(first), Some(last)) = (edits.first(), edits.last()) else { return };
        let mut text = String::new();
        let mut at = first.0;
        for (a, b, with) in edits {
            text.push_str(&text_of(lines, at, *a));
            text.push_str(with);
            at = *b;
        }
        self.change(lines, first.0, last.1, &text, Kind::Other, now);
    }
}

fn text_of(lines: &[String], a: Pos, b: Pos) -> String {
    super::code_edit::text_between(lines, a, b)
}

fn end_after(at: Pos, text: &str) -> Pos {
    match text.rfind('\n') {
        Some(i) => Pos::new(at.line + text.matches('\n').count(), text.len() - i - 1),
        None => Pos::new(at.line, at.byte + text.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> Vec<String> {
        text.split('\n').map(String::from).collect()
    }

    fn at(lines: &[String], line: usize, byte: usize) -> Editor {
        let _ = lines;
        let mut e = Editor::default();
        e.set_caret(Pos::new(line, byte), false);
        e
    }

    #[test]
    fn an_opening_bracket_closes_and_the_caret_goes_inside() {
        let mut lines = doc("call");
        let mut e = at(&lines, 0, 4);
        e.type_text(&mut lines, "(", Indent::Braces, 0.0);
        assert_eq!(lines[0], "call()");
        assert_eq!(e.caret, Pos::new(0, 5));
        e.type_text(&mut lines, "x", Indent::Braces, 0.1);
        e.type_text(&mut lines, ")", Indent::Braces, 0.2);
        assert_eq!(lines[0], "call(x)");
        assert_eq!(e.caret, Pos::new(0, 7), "the typed ) went over the one that was there");
    }

    #[test]
    fn a_bracket_does_not_close_in_front_of_a_word() {
        let mut lines = doc("ab");
        let mut e = at(&lines, 0, 0);
        e.type_text(&mut lines, "(", Indent::Braces, 0.0);
        assert_eq!(lines[0], "(ab");
    }

    #[test]
    fn a_quote_closes_unless_it_follows_a_word() {
        let mut lines = doc("x = ");
        let mut e = at(&lines, 0, 4);
        e.type_text(&mut lines, "\"", Indent::Braces, 0.0);
        assert_eq!(lines[0], "x = \"\"");
        let mut lines = doc("don");
        let mut e = at(&lines, 0, 3);
        e.type_text(&mut lines, "'", Indent::Braces, 0.0);
        assert_eq!(lines[0], "don'", "an apostrophe after a word is just one");
    }

    #[test]
    fn typing_an_opener_over_a_selection_wraps_it_and_keeps_it_selected() {
        let mut lines = doc("a b c");
        let mut e = at(&lines, 0, 2);
        e.set_caret(Pos::new(0, 3), true);
        e.type_text(&mut lines, "[", Indent::Braces, 0.0);
        assert_eq!(lines[0], "a [b] c");
        assert_eq!(e.selected_text(&lines), "b");
        e.undo(&mut lines);
        assert_eq!(lines[0], "a b c");
    }

    #[test]
    fn backspace_between_a_pair_takes_both_and_one_undo_puts_them_back() {
        let mut lines = doc("f()");
        let mut e = at(&lines, 0, 2);
        assert!(e.backspace_pair(&mut lines, 0.0));
        assert_eq!(lines[0], "f");
        assert_eq!(e.caret, Pos::new(0, 1));
        e.undo(&mut lines);
        assert_eq!(lines[0], "f()");
        assert_eq!(e.caret, Pos::new(0, 2));
        let mut other = doc("f(x)");
        let mut e = at(&other, 0, 2);
        assert!(!e.backspace_pair(&mut other, 0.0), "a pair with something inside is left to a plain Backspace");
    }

    #[test]
    fn python_indents_after_a_colon_and_the_braces_after_an_opener() {
        let mut lines = doc("def f():");
        let mut e = at(&lines, 0, 8);
        e.newline_in(&mut lines, Indent::Python, 0.0);
        assert_eq!(lines[1], "  ");
        assert_eq!(e.caret, Pos::new(1, 2));
        let mut lines = doc("  if (x) {");
        let mut e = at(&lines, 0, 10);
        e.newline_in(&mut lines, Indent::Braces, 0.0);
        assert_eq!(lines[1], "    ");
        let mut lines = doc("a:");
        let mut e = at(&lines, 0, 2);
        e.newline_in(&mut lines, Indent::Braces, 0.0);
        assert_eq!(lines[1], "", "a colon means nothing in a language with braces");
    }

    #[test]
    fn enter_between_braces_puts_the_closing_one_on_its_own_line() {
        let mut lines = doc("  fn f() {}");
        let mut e = at(&lines, 0, 10);
        e.newline_in(&mut lines, Indent::Braces, 0.0);
        assert_eq!(lines.join("\n"), "  fn f() {\n    \n  }");
        assert_eq!(e.caret, Pos::new(1, 4));
        e.undo(&mut lines);
        assert_eq!(lines.join("\n"), "  fn f() {}");
    }

    #[test]
    fn replace_all_is_one_step_and_one_undo_takes_every_replacement_back() {
        let mut lines = doc("page one\nthe page\nno");
        let mut e = at(&lines, 0, 0);
        let edits = vec![(Pos::new(0, 0), Pos::new(0, 4), "rows".to_string()), (Pos::new(1, 4), Pos::new(1, 8), "rows\nmore".to_string())];
        e.replace_all(&mut lines, &edits, 0.0);
        assert_eq!(lines.join("\n"), "rows one\nthe rows\nmore\nno");
        assert!(e.undo(&mut lines));
        assert_eq!(lines.join("\n"), "page one\nthe page\nno");
        assert!(!e.undo(&mut lines), "it was one step");
    }

    #[test]
    fn plain_text_copies_the_indent_and_nothing_more() {
        let mut lines = doc("  - item:");
        let mut e = at(&lines, 0, 9);
        e.newline_in(&mut lines, Indent::Copy, 0.0);
        assert_eq!(lines[1], "  ");
    }

    #[test]
    fn the_rule_follows_the_files_language() {
        assert_eq!(indent_for("src/paginate.py"), Indent::Python);
        assert_eq!(indent_for("src/app.ts"), Indent::Braces);
        assert_eq!(indent_for("README.md"), Indent::Copy);
        assert_eq!(indent_for("notes.txt"), Indent::Copy);
    }
}
