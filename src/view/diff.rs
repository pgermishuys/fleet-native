//! The diff in a tool card's body. Ports `components/session/DiffView.vue`: a 1px box, a gutter of two line numbers per
//! row, a marker, and the code. Fleet draws it as one grid with sticky number columns; here the numbers are one fixed
//! column and the code another that scrolls sideways on its own, both made of rows of the same fixed height so they stay
//! level. The rows and their collapsing are `conv::diff`; this file only draws them.
//!
//! Rune's own `rune_kit::diff::DiffView` is a review tool (file headers, split view, word marks, comments, a virtual
//! list over many files) and does not look like Fleet's, so it isn't used (docs/RUNE.md).
use crate::State;
use crate::conv::diff::{DiffLine, Kind, Row, expand_label, rows};
use crate::theme::{RADIUS_CARD, Theme, mono};
use rune::{Color, Cursor, El, col, el, row, text};

use super::toolcard::mix;

/// Line height 12 x 1.6 and 1px of padding above and below.
const LINE_H: f32 = 12.0 * 1.6 + 2.0;
/// The expander row: a line and 3px of padding either side, with a 1px border above and below.
const EXPAND_H: f32 = 12.0 * 1.6 + 6.0 + 2.0;
/// The two number columns (42 each) and the 2px stripe at the left.
const GUTTER_W: f32 = 86.0;
const MARKER_W: f32 = 22.0;
/// A monospace character at 12px: used to give every code row the width of the longest, so a tint reaches the end.
const CHAR_W: f32 = 7.2;

/// `.diff-line__marker`.
pub fn marker(kind: Kind) -> &'static str {
    match kind {
        Kind::Add => "+",
        Kind::Remove => "-",
        Kind::Context => "",
    }
}

struct Tints {
    add: Color,
    remove: Color,
    gutter: Color,
}

fn tints(t: &Theme) -> Tints {
    Tints { add: mix(t.panel_bg, t.running, 0.13), remove: mix(t.panel_bg, t.error, 0.12), gutter: mix(t.panel_bg, t.text, 0.03) }
}

fn number(t: &Theme, tint: Color, n: Option<u32>) -> El {
    // The cell's right edge is the column rule; the number sits 8px from it.
    row()
        .w(42.0)
        .h_full()
        .fill(tint)
        .justify_end()
        .center_y()
        .pr(8.0)
        .child(
            text(n.map(|n| n.to_string()).unwrap_or_default()).font(mono(12.0, 400.0, 1.6).features("'tnum' 1")).color(t.muted.alpha(0.8)),
        )
        .child(el().w(1.0).h_full().fill(t.border))
}

/// The id a context run's expansion is kept under: it holds the card's key and how many lines there are, so a diff that
/// changes goes back to collapsed (`watch(() => props.lines)`).
fn run_key(key: &str, lines: usize, id: &str) -> String {
    format!("{key}#{lines}:{id}")
}

/// `.diff-view`. `key` names the box (and its expanders) for tests and for what is expanded.
pub fn diff_view(t: &Theme, s: &State, key: &str, lines: &[DiffLine]) -> El {
    boxed_diff(t, s, key, lines, true)
}

/// The same rows with no border or rounded corners of their own, for a box that draws its own (the skill diff).
pub fn bare_diff(t: &Theme, s: &State, key: &str, lines: &[DiffLine]) -> El {
    boxed_diff(t, s, key, lines, false)
}

fn boxed_diff(t: &Theme, s: &State, key: &str, lines: &[DiffLine], boxed: bool) -> El {
    let tint = tints(t);
    let open = |id: &str| s.diff_open.contains(&run_key(key, lines.len(), id));
    let expanded: std::collections::HashSet<String> = rows(lines, &Default::default())
        .iter()
        .filter_map(|r| if let Row::Expand { id, .. } = r { Some(id.clone()) } else { None })
        .filter(|id| open(id))
        .collect();
    let made = rows(lines, &expanded);
    let widest = lines.iter().map(|l| l.content.chars().count()).max().unwrap_or(0);
    let code_w = MARKER_W + widest as f32 * CHAR_W + 16.0;

    let mut numbers = col().key(format!("{key}-numbers")).w(GUTTER_W);
    let mut code = col().w_full();
    for r in &made {
        match r {
            Row::Line { index } => {
                let line = &lines[*index];
                let (fill, stripe, ink) = match line.kind {
                    Kind::Add => (tint.add, Some(t.running), t.running),
                    Kind::Remove => (tint.remove, Some(t.error), t.error),
                    Kind::Context => (Color::TRANSPARENT, None, t.muted),
                };
                let cell = if line.kind == Kind::Context { tint.gutter } else { fill };
                numbers = numbers.child(
                    row()
                        .key(format!("{key}-line-{index}"))
                        .w_full()
                        .h(LINE_H)
                        .child(el().w(2.0).h_full().fill(stripe.unwrap_or(Color::TRANSPARENT)))
                        .child(number(t, cell, line.old))
                        .child(number(t, cell, line.new)),
                );
                code = code.child(
                    row()
                        .w_full()
                        .min_w(code_w)
                        .h(LINE_H)
                        .fill(fill)
                        .center_y()
                        .child(text(marker(line.kind)).font(mono(12.0, 400.0, 1.6)).color(ink).w(MARKER_W).px(6.0))
                        .child(text(line.content.clone()).font(mono(12.0, 400.0, 1.6)).color(t.text.alpha(0.88)).pr(16.0)),
                );
            }
            Row::Expand { id, hidden } => {
                let label = expand_label(*hidden);
                let fill = mix(t.panel_bg, t.accent, 0.06);
                let run = run_key(key, lines.len(), id);
                numbers = numbers.child(
                    row().w_full().h(EXPAND_H).child(el().w_full().h_full().fill(tint.gutter).border(1.0, t.border).border_layout()),
                );
                code = code.child(
                    row().w_full().min_w(code_w).h(EXPAND_H).fill(fill).border(1.0, t.border).border_layout().center_y().child(
                        row()
                            .key(format!("{key}-expand-{id}"))
                            .px(12.0)
                            .cursor(Cursor::Pointer)
                            .label(label.clone())
                            .on_press(move |s: &mut State| {
                                s.diff_open.insert(run.clone());
                            })
                            .child(text(label).font(mono(12.0, 400.0, 1.6)).color(t.accent_text)),
                    ),
                );
            }
        }
    }
    let frame = row().key(key.to_string()).w_full().fill(t.panel_bg).label("Diff view");
    let frame = if boxed { frame.radius(RADIUS_CARD).border(1.0, t.border).border_layout().clip_rounded(RADIUS_CARD) } else { frame };
    frame.child(numbers).child(el().key(format!("{key}-code")).grow(1.0).min_w(0.0).scroll_x().child(code))
}
