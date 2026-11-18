//! Selecting lines in the review (`ChangesReview.vue` "Selected lines", rows 12.1 to 12.8 of docs/parity/12-changes-and-review.md):
//! the selection Rune reports in a file's lines becomes line numbers, the bar under the review offers Copy link, Quote and Add to
//! message, and a toast says what was copied. The line text elements are keyed `review-tx-<path>-<index>` (Unified) or
//! `review-txo-` / `review-txn-` (Split's old and new cell), `index` being the line's place in the file's lines.
use crate::State;
use crate::conv::line_quote::{ReviewSelection, SelectedRow, Side, Touched, line_range_label, rows_of, selection_lines};
use crate::conv::review::{ReviewItem, ReviewLine};
use crate::fleet::diff_contents::Entry;
use crate::fleet::review::Picked;
use rune::{TextSelection, Tx};
use std::time::Duration;

/// How long the toast stays (`setTimeout(…, 2200)`).
pub const TOAST_SECONDS: f64 = 2.2;

/// One selected text, as `State::read_pick` takes it.
#[derive(Clone, Debug)]
pub struct PickedText {
    pub key: String,
    pub start: usize,
    pub end: usize,
    pub len: usize,
}

pub fn text_key(path: &str, index: usize, split_side: Option<Side>) -> String {
    match split_side {
        None => format!("review-tx-{path}-{index}"),
        Some(Side::Old) => format!("review-txo-{path}-{index}"),
        Some(Side::New) => format!("review-txn-{path}-{index}"),
    }
}

/// The side a key belongs to in Split (`None` in Unified) and its line's index.
fn parse_key(path: &str, key: &str) -> Option<(Option<Side>, usize)> {
    let (side, rest) = if let Some(r) = key.strip_prefix("review-txo-") {
        (Some(Side::Old), r)
    } else if let Some(r) = key.strip_prefix("review-txn-") {
        (Some(Side::New), r)
    } else {
        (None, key.strip_prefix("review-tx-")?)
    };
    let index = rest.strip_prefix(path)?.strip_prefix('-')?.parse().ok()?;
    Some((side, index))
}

fn line_at(items: &[ReviewItem], index: usize) -> Option<&ReviewLine> {
    items.iter().find_map(|item| match item {
        ReviewItem::Hunk(h) => h.lines.iter().find(|l| l.index == index),
        ReviewItem::Fold(f) => f.lines.iter().find(|l| l.index == index),
    })
}

/// The texts a Rune selection holds, for `State::read_pick`; `backward` says the drag went up, so the anchor is the last text.
pub fn picked_texts(sel: &TextSelection) -> (Vec<PickedText>, bool) {
    let texts = sel
        .texts()
        .iter()
        .filter_map(|t| {
            let r = t.range();
            Some(PickedText { key: t.key()?.to_string(), start: r.start, end: r.end, len: t.len() })
        })
        .collect();
    (texts, sel.backward())
}

impl State {
    /// A file's lines hear the selection change: which lines it spans, or none when it left them (12.1, 12.2). In Split a drag
    /// that began in one column counts that column's lines only.
    pub fn read_pick(&mut self, path: &str, texts: &[PickedText], backward: bool) {
        let Some(Entry::Ready(item)) = self.diff_contents.entry(path) else { return };
        let items = crate::view::review::review_items(item);
        let parsed: Vec<(PickedText, Option<Side>, usize)> =
            texts.iter().filter_map(|t| parse_key(path, &t.key).map(|(side, i)| (t.clone(), side, i))).collect();
        // The anchor is where the drag began: the first text, or the last when the drag went up.
        let anchor = if backward { parsed.last() } else { parsed.first() };
        let column = anchor.and_then(|(_, side, _)| *side);
        let rows: Vec<&(PickedText, Option<Side>, usize)> =
            parsed.iter().filter(|(_, side, _)| column.is_none() || *side == column).collect();
        let n = rows.len();
        let touched: Vec<Touched> = rows
            .iter()
            .enumerate()
            .filter_map(|(at, (t, side, index))| {
                let l = line_at(&items, *index)?;
                let row = match side {
                    None => SelectedRow { old: l.line.old, new: l.line.new },
                    Some(Side::Old) => SelectedRow { old: l.line.old, new: None },
                    Some(Side::New) => SelectedRow { old: None, new: l.line.new },
                };
                let nothing = t.start >= t.end && t.len > 0;
                Some(Touched { row, starts_at_end: nothing && at == 0, ends_at_start: nothing && at + 1 == n })
            })
            .collect();
        let next = selection_lines(&rows_of(touched)).map(|lines| Picked { path: path.to_string(), lines });
        if next != self.review.picked {
            self.review.picked = next;
        }
    }

    /// The selection left the file's lines.
    pub fn clear_pick_state(&mut self, path: &str) {
        if self.review.picked.as_ref().is_some_and(|p| p.path == path) {
            self.review.picked = None;
        }
    }

    /// `clearSelected`: the selection and the bar go.
    pub fn clear_pick(&mut self, tx: &mut Tx) {
        tx.clear_text_selection();
        self.review.picked = None;
    }

    /// The quote of the selected lines, once the file's contents are here (`selectedQuote`).
    fn picked_quote(&self) -> Option<String> {
        let p = self.review.picked.as_ref()?;
        match self.diff_contents.entry(&p.path)? {
            Entry::Ready(item) => Some(p.lines.quote(&p.path, &item.before, &item.after)),
            _ => None,
        }
    }

    /// Shows `words` in the toast for 2.2 s (`toast`).
    pub fn review_toast(&mut self, words: String, tx: &mut Tx) {
        self.review.toast = Some((words, self.clock + TOAST_SECONDS));
        tx.wake_after(Duration::from_millis(100));
    }

    /// From `poll`: the toast goes when its time is up.
    pub fn keep_review_toast(&mut self, now: f64, tx: &mut Tx) -> bool {
        match &self.review.toast {
            Some((_, until)) if now >= *until => {
                self.review.toast = None;
                true
            }
            Some(_) => {
                tx.wake_after(Duration::from_millis(100));
                false
            }
            None => false,
        }
    }

    /// Copy link: `path:18-27`, new lines only (`copySelectedLink`). Rune's copy has no answer to wait for, so "Couldn't copy to the
    /// clipboard" can't be told apart from a copy that worked.
    pub fn copy_pick_link(&mut self, tx: &mut Tx) {
        let Some(link) = self.review.picked.as_ref().and_then(|p| p.lines.link(&p.path)) else { return };
        tx.copy(link.clone());
        self.review_toast(format!("Copied {link}"), tx);
    }

    /// Quote: the lines as a fenced block (`copySelectedQuote`).
    pub fn copy_pick_quote(&mut self, tx: &mut Tx) {
        let (Some(quote), Some(p)) = (self.picked_quote(), self.review.picked.clone()) else { return };
        tx.copy(quote);
        self.review_toast(format!("Copied {} as a quote", line_range_label(p.lines.first, p.lines.last)), tx);
    }

    /// Add to message: new lines go into the draft as an `@path:18-27` reference, old lines as the quote; the selection clears and the
    /// composer takes focus (`addSelectedToMessage`).
    pub fn add_pick_to_message(&mut self, tx: &mut Tx) {
        let Some(p) = self.review.picked.clone() else { return };
        let session = self.canvas_session();
        if p.lines.side == Side::New {
            self.append_draft_reference(
                &session,
                &format!("@{}", crate::conv::line_quote::line_reference(&p.path, p.lines.first, p.lines.last)),
            );
        } else {
            let Some(quote) = self.picked_quote() else { return };
            self.append_draft_text(&session, &quote);
        }
        self.clear_pick(tx);
        tx.focus("composer", false);
    }

    /// Esc in Changes with lines selected clears them ("Clear (Esc)").
    pub fn pick_selection(&self) -> Option<&ReviewSelection> {
        self.review.picked.as_ref().map(|p| &p.lines)
    }
}
