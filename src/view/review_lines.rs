//! A file's hunks, folds and lines in the Changes review (`ChangesReview.vue`, rows 7.1 to 7.6, 7.9 and 7.10 of
//! docs/parity/12-changes-and-review.md): a header per hunk, a button for each run of unchanged lines, the lines with their two
//! numbers, a marker and word marks, sideways scrolling per hunk or wrapping, and Show more past 600 changed lines. The Split
//! layout is piece G's: until it lands both layouts draw Unified.
use crate::State;
use crate::conv::diff::Kind;
use crate::conv::review::{ReviewHunk, ReviewItem, ReviewLine};
use crate::fleet::diffs::{FileDiff, FileStatus};
use crate::fleet::review::{hidden_changes, visible};
use crate::icons::{ELLIPSIS, UNDO_2};
use crate::theme::{Theme, mono, sans};
use rune::{Color, Cursor, El, TextHighlight, col, el, icon, row, text};

/// A line, 12 px at 20 px.
pub const LINE_H: f32 = 20.0;
/// The right edge of an unwrapped hunk fades over this much.
const FADE: f32 = 28.0;

/// `.rfold`: a full-width button between runs of unchanged lines, and "Show N more lines".
fn fold_button(t: &Theme, key: String, words: String, glyph: bool, press: impl Fn(&mut State) + 'static) -> El {
    let mut b = row()
        .key(key)
        .role("button")
        .label(words.clone())
        .w_full()
        .h(26.0)
        .px(12.0)
        .gap(8.0)
        .items_center()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .fill(t.mix(t.text, t.panel_bg, 0.03))
        .hover(t.mix(t.text, t.panel_bg, 0.07))
        .hover_group()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent);
    if glyph {
        b = b.child(icon(&ELLIPSIS, 14.0).color(t.muted).group_color(t.text));
    }
    b.child(text(words).font(sans(12.0, 400.0, 1.4)).color(t.muted).group_color(t.text)).on_press(press)
}

/// `.rhunk`: its name, or "line N", and Revert hunk (drawn disabled until piece I).
fn hunk_header(t: &Theme, path: &str, hunk: &ReviewHunk, file: &FileDiff) -> El {
    let name = if hunk.name.is_empty() { format!("line {}", hunk.start_line) } else { hunk.name.clone() };
    let modified = file.status == FileStatus::Modified;
    let tip = if modified { "Revert hunk" } else { "Revert the whole file instead" };
    let revert = row()
        .key(format!("review-revert-hunk-{path}-{}", hunk.id))
        .role("button")
        .label("Revert hunk")
        .help(tip)
        .h(22.0)
        .px(8.0)
        .gap(5.0)
        .items_center()
        .radius(6.0)
        .disabled(true)
        .opacity(0.45)
        .child(icon(&UNDO_2, 12.0).color(t.muted))
        .child(text("Revert hunk").font(sans(11.5, 500.0, 1.4)).color(t.muted));
    row()
        .key(format!("review-hunk-{path}-{}", hunk.id))
        .w_full()
        .h(28.0)
        .pl(12.0)
        .pr(6.0)
        .gap(8.0)
        .items_center()
        .fill(t.accent_dim)
        .child(text("@@").font(mono(11.5, 500.0, 1.4)).color(t.accent_text))
        .child(text(name).font(mono(11.5, 400.0, 1.4)).color(t.muted).truncate().shrink())
        .child(el().grow(1.0))
        .child(revert)
}

struct Tints {
    add: Color,
    del: Color,
    add_word: Color,
    del_word: Color,
}

fn tints(t: &Theme) -> Tints {
    // `--add-bg` and `--del-bg` over the card's fill, `--add-word` and `--del-word` over those.
    Tints {
        add: t.mix(t.diff_add(), t.panel_bg, 0.10),
        del: t.mix(t.diff_del(), t.panel_bg, 0.09),
        add_word: t.diff_add().alpha(0.26),
        del_word: t.diff_del().alpha(0.24),
    }
}

/// One line: `.rline`. The numbers hold at the left of an unwrapped hunk scrolled sideways (sticky), and sit on the line's tint.
fn line(t: &Theme, k: &Tints, key: String, l: &ReviewLine, wrap: bool) -> El {
    let (fill, mark, ink, word) = match l.line.kind {
        Kind::Add => (k.add, "+", t.diff_add_text(), k.add_word),
        Kind::Remove => (k.del, "\u{2212}", t.diff_del_text(), k.del_word),
        Kind::Context => (t.panel_bg, "", t.muted, Color::TRANSPARENT),
    };
    let no = |n: Option<u32>, rule: bool| {
        let mut c = row().w(36.0).justify_end().pr(8.0).fill(fill);
        if rule {
            c = c.border_sides([0.0, 1.0, 0.0, 0.0], t.border);
        }
        c.child(
            text(n.map(|n| n.to_string()).unwrap_or_default())
                .font(mono(11.5, 400.0, LINE_H / 11.5))
                .color(t.muted.alpha(0.65))
                .single_line(),
        )
    };
    let numbers = row().items_stretch().child(no(l.line.old, false)).child(no(l.line.new, true));
    let numbers = if wrap { numbers } else { numbers.sticky_left(0.0) };
    let font = mono(12.0, 400.0, LINE_H / 12.0);
    let code = text(l.line.content.clone())
        .font(font)
        .color(t.text)
        .text_highlights(l.marks.iter().map(|&(a, b)| TextHighlight::new(a..b, word).pad(0.0)));
    let code = if wrap { code.wrap_anywhere().shrink().min_w(0.0).grow(1.0) } else { code.single_line() };
    let mut r = row()
        .key(key)
        .w_full()
        .min_h(LINE_H)
        .fill(if l.line.kind == Kind::Context { Color::TRANSPARENT } else { fill })
        .child(numbers)
        .child(row().w(20.0).justify_center().child(text(mark).font(font).color(ink).single_line()))
        .child(code.pr(16.0));
    if !wrap {
        r = r.h(LINE_H);
    }
    r
}

/// A run of lines in one sideways-scrolling region (or plain rows when they wrap), labelled for a screen reader (7.10).
fn region(t: &Theme, key: String, label: String, lines: El, count: usize, wrap: bool) -> El {
    if wrap {
        return lines.key(key).role("region").label(label);
    }
    // The viewport holds one column of rows (a viewport's own children lay out side by side, like a filmstrip). Every row is one
    // line high, so the height is known; left to the layout the viewport shrinks to nothing inside the card.
    let h = count as f32 * LINE_H;
    col()
        .child(lines)
        .scroll_x()
        .scroll_edge_fade(FADE)
        .key(key)
        .role("region")
        .label(label)
        .w_full()
        .h(h)
        .min_h(h)
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
}

fn rows<'a>(t: &Theme, k: &Tints, path: &str, lines: impl Iterator<Item = &'a ReviewLine>, wrap: bool) -> El {
    let mut c = col().w_full();
    for l in lines {
        c = c.child(line(t, k, format!("review-line-{path}-{}", l.index), l, wrap));
    }
    c
}

/// A file's hunks, its folds, and the Show more button.
pub fn lines(s: &State, t: &Theme, file: &FileDiff, items: &[ReviewItem]) -> El {
    let path = file.file.as_str();
    let wrap = s.sidebar.changes_wrap();
    let all = s.review.is_drawn_all(path);
    let k = tints(t);
    let mut out = col().key(format!("review-lines-{path}")).w_full();
    for shown in visible(items, all) {
        match shown.item {
            ReviewItem::Fold(f) if !s.review.fold_open(path, &f.id) => {
                let (p, id) = (path.to_string(), f.id.clone());
                let words = format!("{} unchanged {}", f.hidden, if f.hidden == 1 { "line" } else { "lines" });
                out = out.child(fold_button(t, format!("review-fold-{path}-{}", f.id), words, true, move |s: &mut State| {
                    s.review.open_fold(&p, &id)
                }));
            }
            ReviewItem::Fold(f) => {
                let label = format!("Unchanged lines of {path}");
                out = out.child(region(
                    t,
                    format!("review-region-{path}-{}", f.id),
                    label,
                    rows(t, &k, path, f.lines.iter(), wrap),
                    f.lines.len(),
                    wrap,
                ));
            }
            ReviewItem::Hunk(h) => {
                let at = if h.name.is_empty() { format!("line {}", h.start_line) } else { h.name.clone() };
                let label = format!("Changes to {path} at {at}");
                out = out.child(hunk_header(t, path, h, file)).child(region(
                    t,
                    format!("review-region-{path}-{}", h.id),
                    label,
                    rows(t, &k, path, h.lines.iter().take(shown.take), wrap),
                    shown.take,
                    wrap,
                ));
            }
        }
    }
    let more = hidden_changes(items, all);
    if more > 0 {
        let p = path.to_string();
        out =
            out.child(fold_button(t, format!("review-show-all-{path}"), format!("Show {more} more lines"), false, move |s: &mut State| {
                s.review.show_all(&p)
            }));
    }
    out
}
