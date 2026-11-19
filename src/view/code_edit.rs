//! What the editor spike draws on a line of the code view (component 11, piece P; CodeMirror's `drawSelection()` and its caret in
//! `theme.ts`, row 13.1): the caret, 2 px of `--accent`, as an element of its own over the text with `.ime_cursor()` so the input
//! method follows it; the selection as a highlight of the drawn text; an input method's composition drawn in the line, underlined.
//! Rune lays out the text, so where a byte sits is asked of Rune's own measure (`rune::text::advance`), not counted in columns.
use super::code_view::{FONT, LINE_H, hang_of, shown_range};
use crate::State;
use crate::fleet::code_edit::{Editor, Pos};
use crate::theme::{Theme, mono};
use rune::text::{RichLayoutOptions, advance, measure_with};
use rune::{Drag, El, Phase, Span, TextHighlight, el};
use std::ops::Range;

/// The caret's height: the text's own, inside the 20 px line (CodeMirror draws it from the character's top to its bottom).
const CARET_H: f32 = 17.0;
/// `.cm-line`'s left padding, where the text starts in its box.
pub const TEXT_X: f32 = 6.0;

/// What the editor marks on one line.
#[derive(Clone, Debug, Default, Hash)]
pub struct LineEdit {
    /// The caret's byte in the line, when it is on this line.
    pub caret: Option<usize>,
    /// The selected bytes of the line.
    pub selected: Option<Range<usize>>,
    /// An input method's composition at the caret, and its cursor.
    pub preedit: Option<(String, Option<(usize, usize)>)>,
}

/// What `editor` marks on line `i`.
pub fn line_edit(editor: &Editor, lines: &[String], i: usize) -> LineEdit {
    let on = editor.caret.line == i;
    LineEdit {
        caret: on.then_some(editor.caret.byte),
        selected: editor.selected_on(lines, i).map(|(r, _)| r),
        preedit: if on { editor.preedit.clone() } else { None },
    }
}

fn style() -> rune::TextStyle {
    mono(FONT, 400.0, 1.6)
}

/// Where byte `shown` of a drawn line sits from the text's left edge.
pub fn x_of(drawn: &str, shown: usize) -> f32 {
    advance(&drawn[..shown], style())
}

/// The byte of the drawn text nearest `x` points from its left edge (a press lands between the two characters it falls between).
pub fn shown_at_x(drawn: &str, x: f32) -> usize {
    let bounds: Vec<usize> = drawn.char_indices().map(|(i, _)| i).chain([drawn.len()]).collect();
    // The last boundary not past `x`, then whichever of it and the next is nearer.
    let (mut lo, mut hi) = (0, bounds.len() - 1);
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if x_of(drawn, bounds[mid]) <= x { lo = mid } else { hi = mid - 1 }
    }
    match bounds.get(lo + 1) {
        Some(&next) if x - x_of(drawn, bounds[lo]) > x_of(drawn, next) - x => next,
        _ => bounds[lo],
    }
}

/// The byte of `line` that byte `shown` of its drawn text came from (tabs drawn as spaces map back to the tab).
pub fn line_byte(line: &str, shown: usize) -> usize {
    let mut best = 0;
    for (i, _) in line.char_indices().chain([(line.len(), ' ')]) {
        if shown_range(line, i..i).start <= shown {
            best = i;
        } else {
            break;
        }
    }
    best
}

/// How many rows `text` wraps to at `width`, as the line's text lays it out (`wrap_anywhere`).
fn rows_of(text: &str, width: f32) -> usize {
    let (_, h) = measure_with(text, &[], style(), Some(width), RichLayoutOptions::new().wrap_anywhere());
    ((h / LINE_H).round() as usize).max(1)
}

/// Where row `row` of `text` wrapped at `width` starts. A row breaks before a word, so the word a byte is in decides its row: the
/// text up to the end of that word is measured. A word longer than the row, which breaks inside itself, puts the caret on its last row.
fn row_start(text: &str, row: usize, width: f32) -> usize {
    if row == 0 {
        return 0;
    }
    let bounds: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    let rows_to = |k: usize| {
        let end = k + text[k..].find(char::is_whitespace).unwrap_or(text.len() - k);
        rows_of(&text[..end], width)
    };
    let (mut lo, mut hi) = (0, bounds.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if rows_to(bounds[mid]) > row { hi = mid } else { lo = mid + 1 }
    }
    bounds.get(lo).copied().unwrap_or(text.len())
}

/// The row and x of byte `at` in `text` wrapped at `width`.
pub fn wrapped_at(text: &str, at: usize, width: f32) -> (usize, f32) {
    let end = at + text[at..].find(char::is_whitespace).unwrap_or(text.len() - at);
    let row = rows_of(&text[..end], width) - 1;
    let from = row_start(text, row, width).min(at);
    (row, advance(&text[from..at], style()))
}

/// The byte of `text` wrapped at `width` nearest (`row`, `x`).
pub fn wrapped_byte(text: &str, row: usize, x: f32, width: f32) -> usize {
    let from = row_start(text, row, width);
    let to = row_start(text, row + 1, width).max(from);
    // The row's end is before the space a wrap hangs at it.
    let piece = text[from..to].trim_end_matches(' ');
    let piece = if to == text.len() { &text[from..] } else { piece };
    from + shown_at_x(piece, x)
}

/// The drawn text and spans with an input method's composition put in at byte `at` of it, underlined, and where the caret then is.
pub fn with_preedit(
    t: &Theme,
    shown: String,
    spans: Vec<Span>,
    at: usize,
    preedit: &(String, Option<(usize, usize)>),
) -> (String, Vec<Span>, usize) {
    let (text, cursor) = preedit;
    let n = text.len() as u32;
    let a = at as u32;
    let mut out: Vec<Span> = Vec::with_capacity(spans.len() + 1);
    for mut s in spans {
        if s.end <= a {
            out.push(s);
        } else if s.start >= a {
            s.start += n;
            s.end += n;
            out.push(s);
        } else {
            let mut after = s.clone();
            s.end = a;
            after.start = a + n;
            after.end += n;
            out.push(s);
            out.push(after);
        }
    }
    let mut mark = Span::new(at, at + text.len());
    mark.underline = Some(t.text);
    out.push(mark);
    out.sort_by_key(|s| s.start);
    let caret = at + cursor.map_or(text.len(), |(_, end)| end.min(text.len()));
    (format!("{}{text}{}", &shown[..at], &shown[at..]), out, caret)
}

/// The selection over the drawn text, as the read-only view's selection is coloured.
pub fn selection_highlight(t: &Theme, line: &str, selected: &Range<usize>, from: usize) -> Option<TextHighlight> {
    let r = shown_range(line, selected.clone());
    let (a, b) = (r.start.max(from), r.end);
    (a < b).then(|| TextHighlight::new(a - from..b - from, t.accent.alpha(0.3)).pad(0.0))
}

/// The caret, at `x` from the left of its box and on row `row`; it carries the input method.
pub fn caret(t: &Theme, x: f32, row: usize) -> El {
    el().key("code-caret").absolute(x, row as f32 * LINE_H + (LINE_H - CARET_H) / 2.0).w(2.0).h(CARET_H).fill(t.accent).ime_cursor()
}

/// The press and drag on line `i`'s code: the caret goes where the pointer is, Shift extends, a drag selects. `x` and `y` are in the
/// pressed box's points; the line under a drag is counted from the pressed one in rows of 20 (unwrapped lines are all one row).
pub fn on_press(el: El, path: String, i: usize, wrap: bool) -> El {
    el.on_drag(move |s: &mut State, d: &mut Drag, tx| {
        let (extend, y) = match d.phase {
            Phase::Down => (tx.mods().shift, d.y),
            Phase::Move => (true, d.y),
            Phase::Up => return,
        };
        let Some(at) = s.code_point(&path, i, d.x - TEXT_X, y, wrap) else { return };
        s.code_edit_press(at, extend, tx);
    })
}

impl State {
    /// The place in the file under a point of line `i`'s code box (`x` from the text's left edge).
    fn code_point(&self, path: &str, i: usize, x: f32, y: f32, wrap: bool) -> Option<Pos> {
        let (m, s, p) = self.code_edit.file.clone().or_else(|| self.code_edit_target())?;
        if p != path {
            return None;
        }
        let lines = self.file_buffers.buffer(&m, &s, &p)?.lines();
        if wrap {
            let line = lines.get(i)?;
            let shown = super::code_view::draw_line(&Theme::of(true), line, Vec::new()).0;
            let hang = hang_of(&shown);
            let x = x - hang as f32 * super::code_view::DIGIT_W;
            if x < 0.0 && y < LINE_H {
                return Some(Pos::new(i, line_byte(line, shown_at_x(&shown[..hang], x + hang as f32 * super::code_view::DIGIT_W))));
            }
            let width = self.code_edit.text_w.max(1.0);
            let row = (y.max(0.0) / LINE_H) as usize;
            let b = hang + wrapped_byte(&shown[hang..], row, x.max(0.0), width);
            return Some(Pos::new(i, line_byte(line, b)));
        }
        let at = (i as f32 + (y / LINE_H).floor()).clamp(0.0, lines.len().saturating_sub(1) as f32) as usize;
        let line = lines.get(at)?;
        let shown = super::code_view::draw_line(&Theme::of(true), line, Vec::new()).0;
        Some(Pos::new(at, line_byte(line, shown_at_x(&shown, x.max(0.0)))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tab_drawn_as_spaces_maps_back_to_the_tab() {
        assert_eq!(line_byte("\tx", 0), 0);
        assert_eq!(line_byte("\tx", 2), 0);
        assert_eq!(line_byte("\tx", 4), 1);
        assert_eq!(line_byte("ab", 1), 1);
    }

    #[test]
    fn a_press_lands_between_the_characters_it_falls_between() {
        let w = x_of("ab", 1);
        assert!(w > 5.0 && w < 10.0, "a mono digit is about 7.5 px, got {w}");
        assert_eq!(shown_at_x("abcd", w * 1.4), 1);
        assert_eq!(shown_at_x("abcd", w * 1.6), 2);
        assert_eq!(shown_at_x("abcd", 999.0), 4);
    }

    #[test]
    fn a_wrapped_line_puts_the_caret_on_the_row_its_word_went_to() {
        let text = "alpha beta gamma delta";
        let w = x_of("alpha beta ", 11) + 2.0;
        assert_eq!(wrapped_at(text, 0, w).0, 0);
        let (row, x) = wrapped_at(text, text.find("gamma").unwrap() + 2, w);
        assert_eq!(row, 1);
        assert!((x - x_of("ga", 2)).abs() < 0.5);
        assert_eq!(wrapped_byte(text, 1, 0.0, w), text.find("gamma").unwrap());
    }

    #[test]
    fn a_composition_goes_in_at_the_caret_underlined_and_moves_the_colours_after_it() {
        let t = Theme::of(true);
        let mut kw = Span::new(0, 6);
        kw.color = Some(t.accent);
        let (text, spans, caret) = with_preedit(&t, "return x".into(), vec![kw], 3, &("にほ".into(), None));
        assert_eq!(text, "retにほurn x");
        assert_eq!(caret, 3 + "にほ".len());
        assert!(spans.iter().any(|s| s.underline.is_some() && s.start == 3));
        assert!(spans.iter().any(|s| s.color.is_some() && s.start == 9));
    }
}
