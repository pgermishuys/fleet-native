//! The find bar of Changes and its marks (`components/canvas/ChangesFindBar.vue`, rows 11.1 and 11.8 of
//! docs/parity/12-changes-and-review.md). It sits under the review's header and pushes the files down, as Fleet's does. It looks like
//! the conversation's bar (`view::find`) but is not floating: a strip with a bottom rule. The logic is `fleet::changes_find`.
use crate::State;
use crate::conv::changes_find::{ChangesHit, line_marks};
use crate::conv::review::ReviewLine;
use crate::fleet::changes_find::{FIELD, MARK};
use crate::icons::*;
use crate::theme::{Theme, sans};
use crate::ui::text_menu::text_input;
use rune::{Color, Cursor, El, Key, KeyEvent, TextHighlight, Tx, el, icon, row, text};

use super::review_lines::Tints;

/// What the lines need to draw the marks: the query, which match is current, and the colours (`::highlight(changes-find-hit)`
/// and `(changes-find-current)`: `--status-waiting` at 38 %, and solid with `#1a1400` ink).
pub struct FindLook {
    query: String,
    current: Option<ChangesHit>,
    hit: Color,
    current_fill: Color,
    ink: Color,
}

/// The marks to draw, `None` while the bar is shut or has nothing typed.
pub fn look(s: &State, t: &Theme) -> Option<FindLook> {
    let f = &s.changes_find;
    if !f.open || f.query.trim().is_empty() {
        return None;
    }
    Some(FindLook {
        query: f.query.clone(),
        current: f.current_hit().cloned(),
        hit: t.idle.alpha(0.38),
        current_fill: t.idle,
        ink: t.find_ink,
    })
}

/// The marks on one drawn copy of a line. `keyed` names the copy a scroll aims at: Split draws an unchanged line twice and marks
/// both, but only one is scrolled to.
pub fn find_marks(k: &Tints, path: &str, l: &ReviewLine, keyed: bool) -> Vec<TextHighlight> {
    let Some(look) = &k.find else { return Vec::new() };
    let (ranges, at) = line_marks(path, l.index, &l.line.content, &look.query, look.current.as_ref());
    ranges
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            if Some(i) == at {
                let h = TextHighlight::new(r, look.current_fill).color(look.ink).pad(1.5);
                if keyed { h.key(MARK) } else { h }
            } else {
                TextHighlight::new(r, look.hit).pad(1.5)
            }
        })
        .collect()
}

/// `.cfind__icon`: 28 square, a 16 px glyph, a wash of the accent at 14 % on hover, 40 % when there is nothing to step to.
fn icon_button(t: &Theme, key: &'static str, glyph: &'static rune::Icon, label: &'static str, tip: &'static str, off: bool) -> El {
    let b = el()
        .key(key)
        .role("button")
        .size(28.0, 28.0)
        .center()
        .radius(6.0)
        .label(label)
        .help(tip)
        .focus_ring(2.0, 0.0, t.accent)
        .child(icon(glyph, 16.0).color(t.text));
    if off {
        return b.opacity(0.4).disabled(true);
    }
    b.hover(t.accent.alpha(0.14)).cursor(Cursor::Pointer)
}

/// The bar, `None` while it is shut.
pub fn bar(s: &State, t: &Theme) -> Option<El> {
    let find = &s.changes_find;
    if !find.open {
        return None;
    }
    let none = find.hits().is_empty();
    let field = text_input(&find.field, sans(13.0, 400.0, 1.3))
        .key(FIELD)
        .label("Find in changes")
        .placeholder("Find in changes", t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .single_line()
        .w(160.0)
        .grow(1.0)
        .min_w(80.0)
        .max_w(320.0)
        .h(28.0)
        .px(8.0)
        .py(4.5)
        .radius(6.0)
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .border_layout()
        .focus_within_ring(2.0, -1.0, t.accent)
        .on_edit(|s: &mut State| &mut s.changes_find.field)
        .on_input_change(|s: &mut State, _tx: &mut Tx| s.changes_find_typed())
        .on_input_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| match ev.key {
            Key::Escape => {
                s.changes_find_close_ui(tx);
                true
            }
            Key::Enter => {
                s.changes_find_step(if ev.mods.shift { -1 } else { 1 });
                true
            }
            _ => false,
        });
    let count = row().min_w(64.0).px(4.0).child(
        text(s.changes_find_count_label())
            .key("changes-find-count")
            .font(sans(12.0, 400.0, 1.5).features("'tnum' 1"))
            .color(t.idle_text)
            .role("status"),
    );
    let unsearched = s.changes_find_unsearched();
    let mut bar = row()
        .key("changes-find")
        .role("search")
        .label("Find in changes")
        .w_full()
        .flex_wrap()
        .items_center()
        .gap(4.0)
        .pl(14.0)
        .pr(10.0)
        .py(6.0)
        .fill(t.card_bg)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(field)
        .child(count)
        .child(
            icon_button(t, "changes-find-prev", &CHEVRON_UP, "Previous match", "Previous match (Shift+Enter)", none)
                .on_press(|s: &mut State| s.changes_find_step(-1)),
        )
        .child(
            icon_button(t, "changes-find-next", &CHEVRON_DOWN, "Next match", "Next match (Enter)", none)
                .on_press(|s: &mut State| s.changes_find_step(1)),
        )
        .child(
            icon_button(t, "changes-find-close", &X, "Close find", "Close (Esc)", false)
                .on_press_tx(|s: &mut State, tx: &mut Tx| s.changes_find_close_ui(tx)),
        );
    if unsearched > 0 {
        bar = bar.child(el().grow(1.0)).child(
            text(format!("{unsearched} collapsed {} not searched", if unsearched == 1 { "file" } else { "files" }))
                .key("changes-find-unsearched")
                .help("Lockfiles and very large diffs start collapsed and are searched once you open them")
                .font(sans(12.0, 400.0, 1.5))
                .color(t.idle_text)
                .single_line(),
        );
    }
    Some(bar)
}
