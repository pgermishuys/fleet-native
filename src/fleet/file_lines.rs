//! Picking lines in a file tab and what the lines bar does with them (`lib/code-editor/line-pick.ts` and `FileCanvas.vue`'s
//! "Picked lines", rows 8.1 to 8.7 and 4.9 of docs/parity/11-files.md). A press on a line number picks that line, Shift extends from
//! where the pick started, a drag down the numbers extends until the button comes up. The pick isn't the text selection: it lives
//! on the file's buffer, stays through scrolling and a click in the text, goes on Esc or when text is selected, and moves with the
//! agent's edit as CodeMirror's state does. A text selection counts as lines for the bar too.
use super::file_buffers::Status;
use crate::State;
use crate::conv::line_quote::{line_count_label, line_quote, line_range_label, line_reference};
use crate::conv::minimal_change::MinimalChange;
use crate::fleet::canvases::FileView;
use rune::{KeyEvent, Tx};
use std::time::Duration;

/// How long the tab's toast stays (`setTimeout(…, 2600)`).
pub const TOAST_SECONDS: f64 = 2.6;

/// Where a pick started and where it ends, as 1-based lines, either way round (`Pick`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LinePick {
    pub anchor: u32,
    pub head: u32,
}

/// First and last line, 1-based, first ≤ last (`LineRange`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LineRange {
    pub first: u32,
    pub last: u32,
}

impl LinePick {
    /// `pickedLines`.
    pub fn range(&self) -> LineRange {
        LineRange { first: self.anchor.min(self.head), last: self.anchor.max(self.head) }
    }
}

impl LineRange {
    pub fn label(&self) -> String {
        line_range_label(self.first, self.last)
    }

    pub fn count(&self) -> String {
        line_count_label(self.first, self.last)
    }

    pub fn contains(&self, line: u32) -> bool {
        (self.first..=self.last).contains(&line)
    }
}

/// A press on line `at`'s number (`onNumberMouseDown`): Shift extends from where the current pick started; otherwise a new pick of
/// that line, a press on the picked line included.
pub fn press(current: Option<LinePick>, at: u32, shift: bool) -> LinePick {
    let anchor = match current {
        Some(p) if shift => p.anchor,
        _ => at,
    };
    LinePick { anchor, head: at }
}

/// Where line `n` (1-based) starts in the text, counting characters and a break as one, as `MinimalChange` counts.
fn line_start(lines: &[String], n: u32) -> usize {
    lines.iter().take(n.saturating_sub(1) as usize).map(|l| l.chars().count() + 1).sum()
}

/// The 1-based line a position is on.
fn line_at(lines: &[String], pos: usize) -> u32 {
    let mut at = 0;
    for (i, l) in lines.iter().enumerate() {
        let end = at + l.chars().count();
        if pos <= end {
            return i as u32 + 1;
        }
        at = end + 1;
    }
    lines.len().max(1) as u32
}

/// The pick after the agent's edit (`tr.changes.mapPos`): each end is the start of its line, mapped through the change into the new
/// text. `before` and `after` are the lines either side of it.
pub fn map_pick(pick: LinePick, before: &[String], change: &MinimalChange, after: &[String]) -> LinePick {
    let map = |line: u32| line_at(after, change.map(line_start(before, line)));
    LinePick { anchor: map(pick.anchor), head: map(pick.head) }
}

/// One code line's text in a selection: its line, and how much of it the selection takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedText {
    pub line: u32,
    pub start: usize,
    pub end: usize,
}

/// `selectedLines`: the lines a selection spans; one that ends at the start of a line doesn't take that line in. `None` for a
/// selection that holds nothing (a click in the text).
pub fn selected_lines(texts: &[SelectedText]) -> Option<LineRange> {
    let first = texts.iter().min_by_key(|t| t.line)?;
    let last = texts.iter().max_by_key(|t| t.line)?;
    if texts.len() == 1 && first.start >= first.end {
        return None;
    }
    let mut range = LineRange { first: first.line, last: last.line };
    if range.last > range.first && last.end == 0 {
        range.last -= 1;
    }
    Some(range)
}

/// The text keys of the code view are `code-text-<line>`.
pub fn text_line(key: &str) -> Option<u32> {
    key.strip_prefix("code-text-")?.parse().ok()
}

/// What the file tab holds of the lines that isn't the buffer's: the text selection as lines, and the toast.
#[derive(Clone, Debug, Default)]
pub struct FileLines {
    /// The selected lines and the file (`session`, `path`) they are in.
    pub selected: Option<(String, String, LineRange)>,
    /// The tab's own toast (`toastMessage`) and the clock time it goes at.
    pub toast: Option<(String, f64)>,
}

impl State {
    /// The open file tab's session, machine and path, while its code shows (`showEditor`): ready, and not Rendered.
    fn file_lines_target(&self) -> Option<(String, String, String)> {
        let file = self.open_canvases().active().and_then(|c| c.file.clone())?;
        if file.view == FileView::Rendered && crate::fleet::canvases::has_rendered_view(&file.path) {
            return None;
        }
        let session = self.canvas_session();
        let machine = self.machine_key_of(&session).to_string();
        let ready = self.file_buffers.info(&machine, &session, &file.path).is_some_and(|i| i.status == Status::Ready);
        ready.then_some((machine, session, file.path))
    }

    /// The picked lines of a file, as the code view draws them.
    pub fn file_pick(&self, machine: &str, session: &str, path: &str) -> Option<LineRange> {
        self.file_buffers.buffer(machine, session, path)?.pick().map(|p| p.range())
    }

    /// What the bar is about: the picked lines, or else the lines the text selection spans (`pickedLines ?? selectedLines`).
    pub fn file_lines(&self) -> Option<(String, LineRange)> {
        let (machine, session, path) = self.file_lines_target()?;
        if let Some(range) = self.file_pick(&machine, &session, &path) {
            return Some((path, range));
        }
        match &self.file_lines.selected {
            Some((s, p, range)) if *s == session && *p == path => Some((path, *range)),
            _ => None,
        }
    }

    /// A press on line `at`'s number: picks it, or with Shift extends the pick to it. Returns where the pick started, which a drag
    /// keeps. The code takes focus so Esc reaches it.
    pub fn press_line_number(&mut self, path: &str, at: u32, shift: bool, tx: &mut Tx) -> u32 {
        let session = self.canvas_session();
        let machine = self.machine_key_of(&session).to_string();
        let current = self.file_buffers.buffer(&machine, &session, path).and_then(|b| b.pick());
        let pick = press(current, at, shift);
        self.file_buffers.set_pick(&machine, &session, path, Some(pick));
        tx.focus("file-code", false);
        pick.anchor
    }

    /// The pointer, dragging down the numbers from `anchor`, is over line `over`.
    pub fn drag_line_number(&mut self, path: &str, anchor: u32, over: u32) {
        let session = self.canvas_session();
        let machine = self.machine_key_of(&session).to_string();
        let count = self.file_buffers.buffer(&machine, &session, path).map_or(1, |b| b.lines().len().max(1) as u32);
        let pick = LinePick { anchor, head: over.clamp(1, count) };
        if self.file_buffers.buffer(&machine, &session, path).and_then(|b| b.pick()) != Some(pick) {
            self.file_buffers.set_pick(&machine, &session, path, Some(pick));
        }
    }

    /// The code's selection changed: the lines it spans, and text selected takes the pick away (`tr.isUserEvent("select")`).
    pub fn read_file_selection(&mut self, path: &str, texts: &[SelectedText]) {
        let session = self.canvas_session();
        let lines = selected_lines(texts);
        if lines.is_some() {
            let machine = self.machine_key_of(&session).to_string();
            self.file_buffers.set_pick(&machine, &session, path, None);
        }
        let next = lines.map(|range| (session, path.to_string(), range));
        if next != self.file_lines.selected {
            self.file_lines.selected = next;
        }
    }

    /// The selection left the code.
    pub fn clear_file_selection_state(&mut self, path: &str) {
        if self.file_lines.selected.as_ref().is_some_and(|(_, p, _)| p == path) {
            self.file_lines.selected = None;
        }
    }

    /// `clearLines`: the pick goes, or the text selection the bar was showing.
    pub fn clear_file_lines(&mut self, tx: &mut Tx) {
        let Some((machine, session, path)) = self.file_lines_target() else { return };
        if self.file_pick(&machine, &session, &path).is_some() {
            self.file_buffers.set_pick(&machine, &session, &path, None);
            return;
        }
        tx.clear_text_selection();
        self.file_lines.selected = None;
    }

    /// Shows `words` in the tab's toast for 2.6 s (`toast`).
    pub fn file_toast(&mut self, words: String, tx: &mut Tx) {
        self.file_lines.toast = Some((words, self.clock + TOAST_SECONDS));
        tx.wake_after(Duration::from_millis(100));
    }

    /// From `poll`: the toast goes when its time is up.
    pub fn keep_file_toast(&mut self, now: f64, tx: &mut Tx) -> bool {
        match &self.file_lines.toast {
            Some((_, until)) if now >= *until => {
                self.file_lines.toast = None;
                true
            }
            Some(_) => {
                tx.wake_after(Duration::from_millis(100));
                false
            }
            None => false,
        }
    }

    /// Copy link: `path:17-21` (`copyLink`). Rune's copy has no answer to wait for, so "Couldn't copy to the clipboard" can't be
    /// told apart from a copy that worked.
    pub fn copy_file_link(&mut self, tx: &mut Tx) {
        let Some((path, range)) = self.file_lines() else { return };
        let link = line_reference(&path, range.first, range.last);
        tx.copy(link.clone());
        self.file_toast(format!("Copied {link}"), tx);
    }

    /// Quote: the reference, then the lines in a fence (`copyQuote`).
    pub fn copy_file_quote(&mut self, tx: &mut Tx) {
        let Some((path, range)) = self.file_lines() else { return };
        let Some((machine, session, _)) = self.file_lines_target() else { return };
        let Some(buffer) = self.file_buffers.buffer(&machine, &session, &path) else { return };
        let lines = buffer.lines();
        let (from, to) = ((range.first as usize).saturating_sub(1).min(lines.len()), (range.last as usize).min(lines.len()));
        let text = lines[from..to].join("\n");
        tx.copy(line_quote(&line_reference(&path, range.first, range.last), &path, &text));
        self.file_toast(format!("Copied {} as a quote", range.label()), tx);
    }

    /// Add to message: `@path:17-21` into the session's draft, the lines cleared, the composer focused (`addToMessage`).
    pub fn add_file_lines_to_message(&mut self, tx: &mut Tx) {
        let Some((path, range)) = self.file_lines() else { return };
        let session = self.canvas_session();
        self.append_draft_reference(&session, &format!("@{}", line_reference(&path, range.first, range.last)));
        self.clear_file_lines(tx);
        tx.focus("composer", false);
    }

    /// Esc in the code, with lines picked or selected, clears them ("Clear (Esc)", the editor's keymap), and stops there.
    pub fn file_lines_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if ev.key != rune::Key::Escape || !tx.focused().is_some_and(|k| k.starts_with("file-") || k.starts_with("code-")) {
            return false;
        }
        if self.file_lines().is_none() {
            return false;
        }
        self.clear_file_lines(tx);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conv::minimal_change::minimal_change;

    fn doc(n: usize) -> Vec<String> {
        (1..=n).map(|i| format!("line {i}")).collect()
    }

    fn text(line: u32, start: usize, end: usize) -> SelectedText {
        SelectedText { line, start, end }
    }

    // line-pick.test.ts
    #[test]
    fn shift_click_extends_from_where_the_pick_started_either_way() {
        let p = press(None, 18, false);
        let p = press(Some(p), 27, true);
        assert_eq!(p.range(), LineRange { first: 18, last: 27 });
        let p = press(Some(p), 12, true);
        assert_eq!(p.range(), LineRange { first: 12, last: 18 });
    }

    // line-pick.test.ts
    #[test]
    fn a_plain_click_starts_a_new_pick() {
        let p = press(Some(LinePick { anchor: 18, head: 27 }), 5, false);
        assert_eq!(p.range(), LineRange { first: 5, last: 5 });
    }

    // line-pick.test.ts
    #[test]
    fn moves_with_lines_added_above_it() {
        let before = doc(30);
        let mut after = vec!["new".to_string(), "new".to_string()];
        after.extend(before.iter().cloned());
        let change = minimal_change(&before, &after).unwrap();
        let moved = map_pick(LinePick { anchor: 3, head: 6 }, &before, &change, &after);
        assert_eq!(moved.range(), LineRange { first: 5, last: 8 });
    }

    #[test]
    fn stays_put_when_the_edit_is_below_it() {
        let before = doc(30);
        let mut after = before.clone();
        after[20] = "changed".into();
        let change = minimal_change(&before, &after).unwrap();
        assert_eq!(map_pick(LinePick { anchor: 6, head: 3 }, &before, &change, &after), LinePick { anchor: 6, head: 3 });
    }

    // line-pick.test.ts, `selectedLines`
    #[test]
    fn spans_the_selection_s_lines_not_the_line_it_ends_at_the_start_of() {
        assert_eq!(selected_lines(&[]), None);
        // From the start of line 2 to the start of line 4.
        assert_eq!(selected_lines(&[text(2, 0, 3), text(3, 0, 5), text(4, 0, 0)]), Some(LineRange { first: 2, last: 3 }));
        // Inside line 2.
        assert_eq!(selected_lines(&[text(2, 1, 2)]), Some(LineRange { first: 2, last: 2 }));
        // A click in the text selects nothing.
        assert_eq!(selected_lines(&[text(2, 1, 1)]), None);
    }

    #[test]
    fn a_code_text_key_names_its_line() {
        assert_eq!(text_line("code-text-17"), Some(17));
        assert_eq!(text_line("code-no-17"), None);
    }
}
