//! The bracket the caret sits against and the one it pairs with (`matchBrackets()`, row 5.9 of docs/parity/11-files.md). The scan
//! counts brackets of one kind only and looks at plain characters, so a bracket inside a string or a comment counts: Fleet's reads the
//! syntax tree. It gives up after the same 10,000 characters CodeMirror's `maxScanDistance` does.
use super::code_edit::Pos;

const PAIRS: [(char, char); 3] = [('(', ')'), ('[', ']'), ('{', '}')];
const MAX_SCAN: usize = 10_000;

/// The two places of a pair of brackets, when the character before the caret (else the one after it) is a bracket with a partner.
pub fn matching(lines: &[String], caret: Pos) -> Option<(Pos, Pos)> {
    let line = lines.get(caret.line)?;
    let before = line[..caret.byte.min(line.len())].chars().next_back().map(|c| (c, caret.byte - c.len_utf8()));
    let after = line.get(caret.byte..).and_then(|r| r.chars().next()).map(|c| (c, caret.byte));
    [before, after].into_iter().flatten().find_map(|(c, byte)| {
        let at = Pos::new(caret.line, byte);
        let (open, close, forward) = PAIRS.iter().find_map(|&(o, k)| {
            if c == o {
                Some((o, k, true))
            } else if c == k {
                Some((o, k, false))
            } else {
                None
            }
        })?;
        partner(lines, at, open, close, forward).map(|p| (at, p))
    })
}

fn partner(lines: &[String], at: Pos, open: char, close: char, forward: bool) -> Option<Pos> {
    let mut depth = 0usize;
    let mut scanned = 0usize;
    if forward {
        for (n, line) in lines.iter().enumerate().skip(at.line) {
            let from = if n == at.line { at.byte } else { 0 };
            for (b, c) in line[from..].char_indices() {
                scanned += 1;
                if scanned > MAX_SCAN {
                    return None;
                }
                if c == open {
                    depth += 1;
                } else if c == close {
                    depth -= 1;
                    if depth == 0 {
                        return Some(Pos::new(n, from + b));
                    }
                }
            }
        }
    } else {
        for n in (0..=at.line).rev() {
            let line = lines.get(n)?;
            let to = if n == at.line { at.byte + close.len_utf8() } else { line.len() };
            for (b, c) in line[..to].char_indices().rev() {
                scanned += 1;
                if scanned > MAX_SCAN {
                    return None;
                }
                if c == close {
                    depth += 1;
                } else if c == open {
                    depth -= 1;
                    if depth == 0 {
                        return Some(Pos::new(n, b));
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Vec<String> {
        s.lines().map(String::from).collect()
    }

    #[test]
    fn a_bracket_before_the_caret_pairs_across_lines() {
        let l = text("f(a,\n  g(b))\n");
        assert_eq!(matching(&l, Pos::new(0, 2)), Some((Pos::new(0, 1), Pos::new(1, 6))));
        assert_eq!(matching(&l, Pos::new(1, 7)), Some((Pos::new(1, 6), Pos::new(0, 1))));
    }

    #[test]
    fn the_bracket_after_the_caret_counts_when_none_is_before_it() {
        let l = text("x = [1, [2]]");
        assert_eq!(matching(&l, Pos::new(0, 4)), Some((Pos::new(0, 4), Pos::new(0, 11))));
        assert_eq!(matching(&l, Pos::new(0, 1)), None);
    }

    #[test]
    fn an_unpaired_bracket_marks_nothing() {
        assert_eq!(matching(&text("a(b"), Pos::new(0, 2)), None);
    }
}
