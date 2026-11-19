//! The editor spike's other way (component 11, piece P), drawn by scenes and `--bench-editor` only: the whole file in one of Rune's
//! multi-line `text_input`s with the syntax colours painted over it (`.spans`), a column of numbers beside it, in a scroll. Rune's
//! field brings the caret, selection, undo, clipboard and input method; what it costs on a long file and what wrap does to the
//! numbers is what the spike measures (docs/RUNE.md, "Component 11, piece P").
use super::code_view::{FONT, span_of};
use crate::State;
use crate::conv::file_language::language_for;
use crate::theme::{Theme, mono};
use rune::{El, ScrollSpec, col, row, text, text_input};
use rune_kit::syntax::highlight;

/// The field, for tests and scripts to focus.
pub const FIELD: &str = "code-input";

pub fn input_view(s: &State, t: &Theme, path: &str) -> El {
    let Some((_, edit)) = &s.code_edit.input else { return col() };
    let count = edit.text.split('\n').count();
    let width = count.to_string().len();
    let numbers: String = (1..=count).map(|n| format!("{n:>width$}")).collect::<Vec<_>>().join("\n");
    // Coloured as the viewer colours (the first 128 KiB); the highlighter caches by text, so every edit colours the file again.
    let spans: Vec<rune::Span> = match language_for(path) {
        Some(lang) => highlight(Some(lang), &edit.text).iter().map(|(r, p)| span_of(t, *p, r.clone())).collect(),
        None => Vec::new(),
    };
    let font = mono(FONT, 400.0, 1.6);
    let field = text_input(edit, font)
        .key(FIELD)
        .color(t.text)
        .caret(t.accent, t.accent.alpha(0.3))
        .spans(spans)
        .grow(1.0)
        .min_w(0.0)
        .pl(6.0)
        .pr(2.0)
        .py(6.0)
        .on_edit(|s: &mut State| &mut s.code_edit.input.as_mut().expect("the field is drawn only with its edit").1);
    let gutter = text(numbers).font(font).color(t.muted.alpha(0.7)).py(6.0).pl(8.0).pr(10.0);
    col()
        .key("code-input-scroll")
        .w_full()
        .grow(1.0)
        .shrink()
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(row().w_full().items_start().child(col().fill(t.panel_bg).child(gutter)).child(field))
}
