//! Which lines of a text differ from its git base (`lib/code-editor/agent-lines.ts`, `changedLines` and `baseText`): the stripe
//! in Edit and the lines that flash. A line diff; a deletion marks the line after it. Lines are the file's own lines (the
//! text split on its break), so the base's breaks and BOM never count.
use super::diff::diff_script;

/// The git base as lines: a BOM isn't content, and any line break splits (`baseText`). `a\n` is two lines, as the buffer's are.
pub fn base_lines(before: &str) -> Vec<String> {
    let text = before.strip_prefix('\u{feff}').unwrap_or(before);
    let mut lines = vec![String::new()];
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                lines.push(String::new());
            }
            '\n' => lines.push(String::new()),
            c => lines.last_mut().expect("one line at least").push(c),
        }
    }
    lines
}

/// 1-based lines of `doc` that differ from `base`, in order. A deletion marks the line after it (the last line when the deletion
/// was at the end).
pub fn changed_lines(base: &[String], doc: &[String]) -> Vec<usize> {
    let mut marked: Vec<usize> = Vec::new();
    let mut mark = |line: usize| {
        if marked.last() != Some(&line) {
            marked.push(line);
        }
    };
    // `at` is the next line of `doc` the script reaches; a run of added lines marks them, a run of removals alone marks `at + 1`.
    let (mut at, mut added, mut removed) = (0usize, 0usize, false);
    let end_run = |at: usize, added: usize, removed: bool, mark: &mut dyn FnMut(usize)| {
        if added > 0 {
            for line in at - added..at {
                mark(line + 1);
            }
        } else if removed {
            mark((at + 1).min(doc.len()));
        }
    };
    for step in diff_script(base, doc) {
        match step {
            0 => {
                end_run(at, added, removed, &mut mark);
                (added, removed) = (0, false);
                at += 1;
            }
            -1 => removed = true,
            _ => {
                added += 1;
                at += 1;
            }
        }
    }
    end_run(at, added, removed, &mut mark);
    marked
}
