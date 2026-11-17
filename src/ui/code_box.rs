//! The code box (`ProfileConfigEditor.vue`, CodeMirror): a mono text field with a column of line numbers, a minimum height of
//! 160 and a maximum of 360 (then it scrolls), a border that turns accent while the box has focus, Tab that indents and
//! Ctrl/⌘ S that saves. Profiles use it now; the Weave config dialog and the skill editor use it too, so it takes the text
//! as a `TextEdit` the caller keeps and says nothing about what the text is.
//!
//! What CodeMirror has and this doesn't (Rune has no code editor, `docs/RUNE.md`): colours while typing, bracket matching and
//! closing, a line picked by its number. A line that is longer than the box wraps, and the numbers go by lines of the text, so
//! the ones below a wrapped line sit a row too high.
use crate::State;
use crate::theme::{RADIUS_BTN, Theme, mono};
use rune::{El, Key, KeyEvent, ScrollSpec, TextEdit, col, el, row, text, text_input};

/// `font-size: 12px` is the box's; the theme's 12.5px mono at line-height 1.6.
const SIZE: f32 = 12.5;
pub const MIN_HEIGHT: f32 = 160.0;
pub const MAX_HEIGHT: f32 = 360.0;
/// What Tab types: two spaces (`indentWithTab` with CodeMirror's default indent unit).
pub const INDENT: &str = "  ";

/// How many lines the text has: an empty text has one, and a trailing newline starts another.
pub fn line_count(text: &str) -> usize {
    text.split('\n').count()
}

/// The line (from 0) the caret is on.
pub fn caret_line(edit: &TextEdit) -> usize {
    let at = edit.focus.min(edit.text.len());
    edit.text.as_bytes()[..at].iter().filter(|b| **b == b'\n').count()
}

/// Ctrl/⌘ S (`Mod-s`).
pub fn is_save(ev: &KeyEvent) -> bool {
    (ev.mods.cmd || ev.mods.ctrl) && matches!(&ev.key, Key::Text(t) if t.eq_ignore_ascii_case("s"))
}

/// A code box named `key` (the text field takes `{key}`; the box is `{key}-box`, the numbers `{key}-lines`).
/// `access` picks the text out of the state; `save` runs on Ctrl/⌘ S.
pub fn code_box(
    t: &Theme,
    key: &str,
    label: &str,
    edit: &TextEdit,
    access: for<'a> fn(&'a mut State) -> &'a mut TextEdit,
    save: impl Fn(&mut State) + 'static,
) -> El {
    let active = caret_line(edit);
    let mut numbers = col().key(format!("{key}-lines")).min_w(34.0).items_end().py(6.0).px(8.0).fill(t.panel_bg);
    for n in 0..line_count(&edit.text) {
        let ink = if edit.focused && n == active { t.text } else { t.muted.alpha(t.muted.a * 0.7) };
        numbers = numbers.child(text((n + 1).to_string()).font(mono(SIZE, 400.0, 1.6)).color(ink));
    }
    let field = text_input(edit, mono(SIZE, 400.0, 1.6))
        .key(key.to_string())
        .label(label.to_string())
        .color(t.text)
        .caret(t.accent, t.accent.alpha(0.3))
        .grow(1.0)
        .min_w(0.0)
        .px(6.0)
        .py(6.0)
        .on_edit(access)
        .on_input_key(move |s: &mut State, ev: &KeyEvent, _| {
            if is_save(ev) {
                save(s);
                return true;
            }
            // Tab indents (`indentWithTab`); Shift-Tab leaves the field, as it does for a form's other boxes.
            if ev.key == Key::Tab && !ev.mods.shift && !ev.mods.ctrl && !ev.mods.cmd && !ev.mods.alt {
                access(s).insert(INDENT);
                return true;
            }
            false
        })
        .animate_edit_layout();
    let scroll = col()
        .key(format!("{key}-scroll"))
        .w_full()
        .shrink()
        .min_h(MIN_HEIGHT - 2.0)
        .max_h(MAX_HEIGHT - 2.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(row().w_full().min_h(MIN_HEIGHT - 2.0).child(numbers).child(field));
    let border = if edit.focused { t.accent_border(55.0) } else { t.border };
    el().key(format!("{key}-box"))
        .w_full()
        .radius(RADIUS_BTN)
        .border(1.0, border)
        .border_layout()
        .fill(t.main_bg)
        .clip_rounded(RADIUS_BTN)
        .child(scroll)
}
