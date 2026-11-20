//! Find in a file, the matching with no Rune state in it (CodeMirror's `SearchQuery` as `search({ top: true })` uses it, row 12.1 and
//! 12.2 of docs/parity/11-files.md): a query, the three boxes under the field (match case, regexp, by word), and where each match
//! sits in the file's lines. A match never spans lines (the panel has one field line and Fleet's files are searched line by line).
use std::ops::Range;

/// Matches beyond this many are not drawn or stepped to; a one-letter query in a huge file would otherwise cost a frame.
pub const MAX_MATCHES: usize = 20_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Options {
    /// "match case": off, `a` finds `A`.
    pub case: bool,
    /// "regexp": the query is a regular expression, not text.
    pub regexp: bool,
    /// "by word": a match must not touch a word character on either side.
    pub word: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    /// The line, from 0.
    pub line: usize,
    /// Bytes of that line.
    pub range: Range<usize>,
}

/// `wordChar` of CodeMirror's default: letters, digits and the underscore.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// CodeMirror's `wholeWord` test: a side only counts when the match's own edge is a word character.
fn on_word_edges(line: &str, r: &Range<usize>) -> bool {
    let first = line[r.clone()].chars().next();
    let last = line[r.clone()].chars().next_back();
    let before = line[..r.start].chars().next_back();
    let after = line[r.end..].chars().next();
    let side = |inside: Option<char>, outside: Option<char>| !(inside.is_some_and(is_word) && outside.is_some_and(is_word));
    side(first, before) && side(last, after)
}

/// What the query compiles to, or `None` for an empty or invalid one (an unfinished `(` finds nothing, as in CodeMirror).
pub fn compile(query: &str, o: Options) -> Option<regex::Regex> {
    if query.is_empty() {
        return None;
    }
    let source = if o.regexp { query.to_string() } else { regex::escape(query) };
    regex::RegexBuilder::new(&source).case_insensitive(!o.case).multi_line(true).build().ok()
}

/// Every match of `query` in `lines`, in reading order. Empty matches (`a*` between letters) are left out.
pub fn find_matches(lines: &[String], query: &str, o: Options) -> Vec<Match> {
    let Some(re) = compile(query, o) else { return Vec::new() };
    let mut out = Vec::new();
    for (line, text) in lines.iter().enumerate() {
        for m in re.find_iter(text) {
            let range = m.range();
            if range.is_empty() || (o.word && !on_word_edges(text, &range)) {
                continue;
            }
            out.push(Match { line, range });
            if out.len() >= MAX_MATCHES {
                return out;
            }
        }
    }
    out
}

/// The match after (`1`) or before (`-1`) `current`, wrapping round the ends; from nothing, the first or the last.
pub fn step(count: usize, current: Option<usize>, direction: i32) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(match (current, direction >= 0) {
        (Some(i), true) => (i + 1) % count,
        (Some(i), false) => (i + count - 1) % count,
        (None, true) => 0,
        (None, false) => count - 1,
    })
}

/// `\n`, `\r`, `\t` and `\\` in the replace field mean what they say (CodeMirror's `SearchQuery.unquote`, which Fleet leaves on).
fn unquote(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.clone().next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            _ => {
                out.push('\\');
                continue;
            }
        }
        chars.next();
    }
    out
}

/// What replacing the match `m` of `lines` puts there (`SearchQuery.getReplacement`): the replace field with its escapes, and with
/// regexp on `$&` for the match and `$1` for a group (`$$` is a dollar).
pub fn replacement(lines: &[String], m: &Match, query: &str, o: Options, replace: &str) -> String {
    let text = unquote(replace);
    if !o.regexp {
        return text;
    }
    let (Some(re), Some(line)) = (compile(query, o), lines.get(m.line)) else { return text };
    let caps = re.captures_at(line, m.range.start).filter(|c| c.get(0).is_some_and(|w| w.range() == m.range));
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some('$') => {
                chars.next();
                out.push('$');
            }
            Some('&') => {
                chars.next();
                out.push_str(&line[m.range.clone()]);
            }
            Some(d) if d.is_ascii_digit() => {
                let mut n = String::new();
                while let Some(&d) = chars.peek().filter(|d| d.is_ascii_digit()) {
                    n.push(d);
                    chars.next();
                }
                let group = n.parse::<usize>().ok().and_then(|i| caps.as_ref()?.get(i));
                match group {
                    Some(g) => out.push_str(g.as_str()),
                    // A group the pattern doesn't have stays as it was typed.
                    None => {
                        out.push('$');
                        out.push_str(&n);
                    }
                }
            }
            _ => out.push('$'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(String::from).collect()
    }

    fn at(m: &[Match]) -> Vec<(usize, usize)> {
        m.iter().map(|m| (m.line, m.range.start)).collect()
    }

    #[test]
    fn text_is_found_whatever_its_case_until_match_case_is_on() {
        let l = lines("Page one\nthe page\nPAGE");
        assert_eq!(at(&find_matches(&l, "page", Options::default())), [(0, 0), (1, 4), (2, 0)]);
        assert_eq!(at(&find_matches(&l, "page", Options { case: true, ..Options::default() })), [(1, 4)]);
    }

    #[test]
    fn the_query_is_text_unless_regexp_is_on() {
        let l = lines("a.c abc");
        assert_eq!(at(&find_matches(&l, "a.c", Options::default())), [(0, 0)]);
        assert_eq!(at(&find_matches(&l, "a.c", Options { regexp: true, ..Options::default() })), [(0, 0), (0, 4)]);
    }

    #[test]
    fn by_word_leaves_out_matches_inside_a_longer_word() {
        let l = lines("page pages last_page page_1 (page)");
        let word = Options { word: true, ..Options::default() };
        assert_eq!(at(&find_matches(&l, "page", word)), [(0, 0), (0, 29)]);
        assert_eq!(find_matches(&l, "page", Options::default()).len(), 5);
    }

    #[test]
    fn an_empty_or_unfinished_query_finds_nothing() {
        let l = lines("(a)");
        assert!(find_matches(&l, "", Options::default()).is_empty());
        assert!(find_matches(&l, "(", Options { regexp: true, ..Options::default() }).is_empty());
        assert_eq!(find_matches(&l, "(", Options::default()).len(), 1);
        assert!(find_matches(&l, "x*", Options { regexp: true, ..Options::default() }).is_empty());
    }

    #[test]
    fn stepping_wraps_at_both_ends() {
        assert_eq!(step(3, None, 1), Some(0));
        assert_eq!(step(3, None, -1), Some(2));
        assert_eq!(step(3, Some(2), 1), Some(0));
        assert_eq!(step(3, Some(0), -1), Some(2));
        assert_eq!(step(0, Some(0), 1), None);
    }

    #[test]
    fn a_match_after_multibyte_text_has_byte_offsets() {
        let l = lines("é page");
        assert_eq!(at(&find_matches(&l, "page", Options::default())), [(0, 3)]);
    }

    #[test]
    fn a_replacement_takes_its_escapes_and_with_regexp_its_groups() {
        let l = lines("page_size = 10");
        let m = &find_matches(&l, "page_(\\w+)", Options { regexp: true, ..Options::default() })[0];
        let re = Options { regexp: true, ..Options::default() };
        assert_eq!(replacement(&l, m, "page_(\\w+)", re, "rows_$1 \\t$&$$"), "rows_size \tpage_size$");
        assert_eq!(replacement(&l, m, "page_(\\w+)", re, "$3"), "$3", "a group the pattern lacks stays as typed");
        let plain = &find_matches(&l, "page_size", Options::default())[0];
        assert_eq!(replacement(&l, plain, "page_size", Options::default(), "a\\nb$1"), "a\nb$1", "no regexp: `$1` is text");
    }
}
