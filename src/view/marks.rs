//! Find's marks on text that isn't markdown: tool output and a user's plain words. Markdown marks its own matches
//! (`prose`). The colours are Fleet's (`main.css`, `mark.find-hit`): `--status-waiting` at 38% for every match, solid with
//! `#1a1400` ink for the current one.
use crate::conv::find::{Marks, plain_ranges};
use crate::fleet::find::MARK;
use crate::theme::Theme;
use rune::TextHighlight;
use std::ops::Range;

/// A wash behind each of `ranges` (bytes of the text drawn), the current one solid and keyed so a scroll can find it.
pub fn highlights(t: &Theme, ranges: &[Range<usize>], current: Option<usize>) -> Vec<TextHighlight> {
    ranges
        .iter()
        .enumerate()
        .map(|(i, r)| {
            if Some(i) == current {
                TextHighlight::new(r.clone(), t.idle).color(t.find_ink).pad(1.5).key(MARK)
            } else {
                TextHighlight::new(r.clone(), t.idle.alpha(0.38)).pad(1.5)
            }
        })
        .collect()
}

/// The marks over `text`, found in it as typed.
pub fn over(t: &Theme, text: &str, marks: &Marks) -> Vec<TextHighlight> {
    highlights(t, &plain_ranges(text, &marks.query), marks.current)
}
