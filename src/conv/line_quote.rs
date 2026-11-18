//! Which lines a selection in the review stands for, and what Copy link and Quote put on the clipboard (`lib/review-selection.ts`
//! and `lib/line-quote.ts`; rows 12.1, 12.2, 12.4 and 12.5 of docs/parity/12-changes-and-review.md). Plain Rust, no view.

/// A diff row a selection touches: its line number on each side, `None` where the side has no such line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedRow {
    pub old: Option<u32>,
    pub new: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Old,
    New,
}

/// The lines a selection in a diff spans, on one side: new (the file as it is) or old (as it was).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReviewSelection {
    pub side: Side,
    pub first: u32,
    pub last: u32,
}

/// `selectionLines`: a selection that takes in any line the file still has is about the file as it is, in new numbers (removed
/// lines in it have none and drop out); one with removed lines only is about the old file, in old numbers.
pub fn selection_lines(rows: &[SelectedRow]) -> Option<ReviewSelection> {
    let side = if rows.iter().any(|r| r.new.is_some()) { Side::New } else { Side::Old };
    let numbers: Vec<u32> = rows.iter().filter_map(|r| if side == Side::New { r.new } else { r.old }).collect();
    Some(ReviewSelection { side, first: *numbers.iter().min()?, last: *numbers.iter().max()? })
}

/// One selected text, as the review reads it: the row it belongs to and how much of its text the selection takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Touched {
    pub row: SelectedRow,
    /// The text has characters and the selection starts at its very end.
    pub starts_at_end: bool,
    /// The text has characters and the selection ends at its very start.
    pub ends_at_start: bool,
}

/// `touchesOnly`: with more than one row, a first row the selection only reaches the end of, and a last row it only reaches the
/// start of, are left out.
pub fn rows_of(mut touched: Vec<Touched>) -> Vec<SelectedRow> {
    if touched.len() > 1 && touched[0].starts_at_end {
        touched.remove(0);
    }
    if touched.len() > 1 && touched[touched.len() - 1].ends_at_start {
        touched.pop();
    }
    touched.into_iter().map(|t| t.row).collect()
}

/// "L7" or "L18–27".
pub fn line_range_label(first: u32, last: u32) -> String {
    if first == last { format!("L{first}") } else { format!("L{first}\u{2013}{last}") }
}

/// "1 line" or "10 lines".
pub fn line_count_label(first: u32, last: u32) -> String {
    match last - first + 1 {
        1 => "1 line".to_string(),
        n => format!("{n} lines"),
    }
}

/// "src/app.ts:7" or "src/app.ts:18-27", the form a message's @ reference takes.
pub fn line_reference(path: &str, first: u32, last: u32) -> String {
    if first == last { format!("{path}:{first}") } else { format!("{path}:{first}-{last}") }
}

/// Lines `first` to `last` (1-based) of `text`, without their line endings.
pub fn slice_lines(text: &str, first: u32, last: u32) -> String {
    let from = first.saturating_sub(1) as usize;
    let count = (last as usize).saturating_sub(from);
    text.split('\n').skip(from).take(count).map(|l| l.strip_suffix('\r').unwrap_or(l)).collect::<Vec<_>>().join("\n")
}

/// A heading, then the lines in a fence that is longer than any run of backticks inside them, tagged with the file's extension.
pub fn line_quote(heading: &str, path: &str, text: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in text.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    let fence = "`".repeat((longest + 1).max(3));
    let name = path.rsplit('/').next().unwrap_or(path);
    let extension = name.rsplit_once('.').map(|(_, e)| e).filter(|e| !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric()));
    format!("{heading}\n{fence}{}\n{text}\n{fence}\n", extension.unwrap_or("").to_ascii_lowercase())
}

impl ReviewSelection {
    /// "L2–4", or "Old L11" for lines only the old side has.
    pub fn label(&self) -> String {
        format!("{}{}", if self.side == Side::Old { "Old " } else { "" }, line_range_label(self.first, self.last))
    }

    pub fn count(&self) -> String {
        line_count_label(self.first, self.last)
    }

    /// What Copy link copies; old lines have no place in the file to link to.
    pub fn link(&self, path: &str) -> Option<String> {
        (self.side == Side::New).then(|| line_reference(path, self.first, self.last))
    }

    /// The quote of these lines out of the file's `before` or `after`.
    pub fn quote(&self, path: &str, before: &str, after: &str) -> String {
        let text = slice_lines(if self.side == Side::New { after } else { before }, self.first, self.last);
        let reference = line_reference(path, self.first, self.last);
        line_quote(&if self.side == Side::New { reference } else { format!("{reference} (before)") }, path, &text)
    }
}
