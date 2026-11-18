//! The smallest single replacement that turns one document into another (`lib/code-editor/minimal-change.ts`): the common
//! start and end stay, so the scroll and the picked lines mostly stay put when the agent's edit is applied.
//! A document is its lines. Positions count characters, and a line break always counts as one whatever the file's break is.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MinimalChange {
    pub from: usize,
    pub to: usize,
    /// The replacement, as lines.
    pub insert: Vec<String>,
    /// 1-based lines of the new document that the change covers.
    pub lines: (usize, usize),
}

impl MinimalChange {
    /// How many characters the replacement is, its breaks counted as one each.
    pub fn insert_len(&self) -> usize {
        self.insert.iter().map(|l| l.chars().count()).sum::<usize>() + self.insert.len().saturating_sub(1)
    }

    /// Where a position (a cursor) in the old document is in the new one: before the change it stays, after it moves by the
    /// change's size, inside it goes to the start of what replaced it.
    pub fn map(&self, pos: usize) -> usize {
        if pos <= self.from {
            pos
        } else if pos >= self.to {
            pos - (self.to - self.from) + self.insert_len()
        } else {
            self.from
        }
    }

    /// The old document with the change made: the new document.
    pub fn apply(&self, before: &[String]) -> Vec<String> {
        let chars: Vec<char> = flat(before);
        let mut out: String = chars[..self.from].iter().collect();
        out.push_str(&self.insert.join("\n"));
        out.extend(&chars[self.to..]);
        out.split('\n').map(str::to_string).collect()
    }
}

fn flat(lines: &[String]) -> Vec<char> {
    lines.join("\n").chars().collect()
}

/// The 1-based line a position is on.
fn line_at(chars: &[char], pos: usize) -> usize {
    1 + chars[..pos.min(chars.len())].iter().filter(|c| **c == '\n').count()
}

/// `None` when the documents are the same.
pub fn minimal_change(before: &[String], after: &[String]) -> Option<MinimalChange> {
    if before == after {
        return None;
    }
    let (a, b) = (flat(before), flat(after));
    let shorter = a.len().min(b.len());
    let mut start = 0;
    while start < shorter && a[start] == b[start] {
        start += 1;
    }
    let mut end = 0;
    while end < shorter - start && a[a.len() - 1 - end] == b[b.len() - 1 - end] {
        end += 1;
    }
    let insert_to = b.len() - end;
    let insert: String = b[start..insert_to].iter().collect();
    Some(MinimalChange {
        from: start,
        to: a.len() - end,
        insert: insert.split('\n').map(str::to_string).collect(),
        lines: (line_at(&b, start), line_at(&b, insert_to.max(start))),
    })
}
