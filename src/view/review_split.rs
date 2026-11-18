//! Split in the Changes review (`ChangesReview.vue` `.rsplit`, rows 7.7 and 7.8 of docs/parity/12-changes-and-review.md): a hunk
//! or an opened unchanged run drawn as old and new side by side. Fleet makes one grid per hunk; here each split row is a row of
//! two equal cells, and a hunk's rows are one column inside the same sideways-scrolling region as Unified, so old and new scroll
//! as one. A cell with no partner (the shorter side of a change) is padding.
use super::changes_find::find_marks;
use super::review_lines::{LINE_H, Tints};
use crate::conv::diff::Kind;
use crate::conv::review::{ReviewLine, SplitRow, split_rows};
use crate::theme::{Theme, mono};
use rune::{Color, El, TextHighlight, col, row, text};

/// A column's number cell (34 wide in Split) and marker cell (20).
const NO_W: f32 = 34.0;
const MARK_W: f32 = 20.0;
const TEXT_PAD: f32 = 16.0;
/// A 12 px mono character is 0.6 em wide: how far a cell must reach so its longest line fits before the region scrolls.
const CHAR_W: f32 = 7.2;

/// One side of a row: its number, marker and text on the line's tint, or the padding cell.
#[allow(clippy::too_many_arguments)]
fn cell(t: &Theme, k: &Tints, key: String, path: &str, l: Option<&ReviewLine>, new_side: bool, keyed: bool, wrap: bool, min_w: f32) -> El {
    let font = mono(12.0, 400.0, LINE_H / 12.0);
    let base = row().key(key).items_stretch().min_w(min_w);
    // Wrapped, the two share the region exactly; unwrapped they start at their minimum and share what is left.
    let base = if wrap { base.w_frac(0.5) } else { base.grow(1.0).w(0.0) };
    let base = if new_side { base.border_sides([0.0, 1.0, 0.0, 0.0], t.border) } else { base };
    let Some(l) = l else {
        // `.rline--pad` is hatched (135 degrees, 6 % lines every 5 px); Rune has no pattern fill, so it is the lines' average.
        return base.fill(t.mix(t.text, t.panel_bg, 0.015));
    };
    let (fill, mark, ink, word) = match l.line.kind {
        Kind::Add => (k.add, "+", t.diff_add_text(), k.add_word),
        Kind::Remove => (k.del, "\u{2212}", t.diff_del_text(), k.del_word),
        Kind::Context => (t.panel_bg, "", t.muted, Color::TRANSPARENT),
    };
    let number = if new_side { l.line.new } else { l.line.old };
    let no = row().w(NO_W).justify_end().pr(8.0).fill(fill).child(
        text(number.map(|n| n.to_string()).unwrap_or_default())
            .font(mono(11.5, 400.0, LINE_H / 11.5))
            .color(t.muted.alpha(0.65))
            .single_line(),
    );
    let code = text(l.line.content.clone())
        .font(font)
        .color(t.text)
        .text_highlights(l.marks.iter().map(|&(a, b)| TextHighlight::new(a..b, word).pad(0.0)).chain(find_marks(k, path, l, keyed)));
    let code = if wrap { code.wrap_anywhere().shrink().min_w(0.0).grow(1.0) } else { code.single_line() };
    let mut c = base
        .fill(if l.line.kind == Kind::Context { Color::TRANSPARENT } else { fill })
        .child(no)
        .child(row().w(MARK_W).justify_center().child(text(mark).font(font).color(ink).single_line()))
        .child(code.pr(TEXT_PAD));
    if !wrap {
        c = c.h(LINE_H);
    }
    c
}

/// A run's lines as split rows, in one column. Each side is at least as wide as its longest line needs (Fleet's
/// `minmax(max-content, 1fr)`: flex basis 0 and a minimum) and they share what is left equally, so a side with nothing long in it
/// stays narrow until the region is wider than both. Wrapped, the two share the region.
pub fn split_lines(t: &Theme, k: &Tints, path: &str, lines: &[ReviewLine], wrap: bool) -> El {
    let rows: Vec<SplitRow> = split_rows(lines);
    let need = |old: bool| {
        if wrap {
            return 0.0;
        }
        let longest =
            rows.iter().filter_map(|r| if old { r.left } else { r.right }).map(|l| l.line.content.chars().count()).max().unwrap_or(0)
                as f32;
        NO_W + MARK_W + TEXT_PAD + longest * CHAR_W
    };
    let (old_w, new_w) = (need(true), need(false));
    let mut c = col().w_full();
    if !wrap {
        c = c.min_w(old_w + new_w);
    }
    for r in rows {
        let (old, new) = (r.left, r.right);
        let id = old.or(new).map_or(0, |l| l.index);
        let mut line = row().w_full().items_stretch();
        if !wrap {
            line = line.h(LINE_H);
        }
        c = c.child(line.child(cell(t, k, format!("review-old-{path}-{id}"), path, old, false, true, wrap, old_w)).child(cell(
            t,
            k,
            format!("review-new-{path}-{id}"),
            path,
            new,
            true,
            // An unchanged line is both cells' (the same line twice): only the old copy is scrolled to.
            old.is_none_or(|o| new.is_none_or(|n| o.index != n.index)),
            wrap,
            new_w,
        )));
    }
    c
}
