//! The search panel of a file tab and its marks (CodeMirror's `search({ top: true })` panel, as `theme.ts` dresses it: rows 12.1 and
//! 12.2 of docs/parity/11-files.md). It is a strip at the top of the code with a rule under it: the field, **next**, **previous**,
//! **all**, the three boxes, and a close button at the right, wrapping as the browser's inline layout does. A file that can be edited
//! has a second row under them (`<br>` in CodeMirror's panel): the replace field, **replace** and **replace all** (row 12.3). The state
//! is `fleet::file_find`.
use crate::State;
use crate::conv::file_find::Options;
use crate::fleet::file_find::{FIELD, MARK};
use crate::icons::CHECK;
use crate::theme::{Theme, sans};
use crate::ui::text_menu::text_input;
use rune::{Cursor, El, Key, KeyEvent, TextHighlight, Tx, col, el, icon, row, text};
use std::ops::Range;

/// A button of the panel (`.cm-button`): 70 % of the panel's 12 px type, `--text` at 6 % for a fill, a 1 px rule, radius 5.
fn button(t: &Theme, key: &'static str, label: &'static str, help: &'static str, off: bool) -> El {
    let b = row()
        .key(key)
        .role("button")
        .label(label)
        .help(help)
        .px(8.4)
        .py(1.68)
        .items_center()
        .radius(5.0)
        .fill(t.wash(6.0))
        .border(1.0, t.border)
        .border_layout()
        .focus_ring(2.0, 0.0, t.accent)
        .child(text(label).font(sans(8.4, 400.0, 1.4)).color(t.text).single_line());
    if off {
        return b.opacity(0.4).disabled(true);
    }
    b.hover(t.wash(12.0)).cursor(Cursor::Pointer)
}

/// A browser checkbox, 13 px.
fn mark(t: &Theme, on: bool) -> El {
    let b = el().size(13.0, 13.0).border(1.0, if on { t.accent } else { t.muted }).border_layout().radius(2.0);
    if on { b.fill(t.accent).center().child(icon(&CHECK, 10.0).color(t.primary_fg)) } else { b }
}

/// `<label><input type=checkbox> match case</label>`: 80 % of the panel's type, in `--muted`.
fn checkbox(t: &Theme, key: &'static str, label: &'static str, on: bool, flip: fn(&mut Options) -> &mut bool) -> El {
    row()
        .key(key)
        .role("checkbox")
        .label(label)
        .checked(on)
        .gap(1.92)
        .items_center()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, t.accent)
        .child(mark(t, on))
        .child(text(label).font(sans(9.6, 400.0, 1.15)).color(t.muted).single_line())
        .on_press(move |s: &mut State| s.file_find_toggle(flip))
}

/// The panel, `None` while it is shut or not over the file in view.
pub fn panel(s: &State, t: &Theme) -> Option<El> {
    if !s.file_find_shows() {
        return None;
    }
    let find = &s.file_find;
    let none = find.matches().is_empty();
    let field = text_input(&find.field, sans(12.0, 400.0, 1.3))
        .key(FIELD)
        .label("Find")
        .placeholder("Find", t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .single_line()
        .w(187.0)
        .h(24.8)
        .px(6.0)
        .py(2.4)
        .radius(5.0)
        .border(1.0, t.border)
        .border_layout()
        .focus_within_ring(1.0, -1.0, t.accent)
        .on_edit(|s: &mut State| &mut s.file_find.field)
        .on_input_change(|s: &mut State, _tx: &mut Tx| s.file_find_typed())
        .on_input_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| match ev.key {
            Key::Escape => {
                s.file_find_close_ui(tx);
                true
            }
            Key::Enter => {
                s.file_find_step(if ev.mods.shift { -1 } else { 1 }, tx);
                true
            }
            _ => false,
        });
    let o = find.options;
    let can_replace = s.file_find_can_replace();
    let first = row()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .flex_wrap()
        .items_center()
        .gaps(2.4, 7.2)
        .child(field)
        .child(
            row().gap(5.04).items_center().children([
                button(t, "file-find-next", "next", "Next match (Enter)", none)
                    .on_press_tx(|s: &mut State, tx: &mut Tx| s.file_find_step(1, tx)),
                button(t, "file-find-prev", "previous", "Previous match (Shift+Enter)", none)
                    .on_press_tx(|s: &mut State, tx: &mut Tx| s.file_find_step(-1, tx)),
                // CodeMirror's "all" selects every match as one multi-selection; there is no selection in the code yet (row 12.1).
                button(t, "file-find-all", "all", "Selecting every match comes with editing", true),
            ]),
        )
        .child(checkbox(t, "file-find-case", "match case", o.case, |o| &mut o.case))
        .child(checkbox(t, "file-find-regexp", "regexp", o.regexp, |o| &mut o.regexp))
        .child(checkbox(t, "file-find-word", "by word", o.word, |o| &mut o.word));
    let items = if can_replace {
        let replace_field = text_input(&find.replace, sans(12.0, 400.0, 1.3))
            .key("file-find-replace-field")
            .label("Replace")
            .placeholder("Replace", t.muted)
            .color(t.text)
            .caret(t.text, t.accent.alpha(0.35))
            .single_line()
            .w(187.0)
            .h(24.8)
            .px(6.0)
            .py(2.4)
            .radius(5.0)
            .border(1.0, t.border)
            .border_layout()
            .focus_within_ring(1.0, -1.0, t.accent)
            .on_edit(|s: &mut State| &mut s.file_find.replace)
            .on_input_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| match ev.key {
                Key::Escape => {
                    s.file_find_close_ui(tx);
                    true
                }
                Key::Enter if ev.mods.alt && (ev.mods.ctrl || ev.mods.cmd) => {
                    s.file_find_replace_all();
                    true
                }
                Key::Enter => {
                    s.file_find_replace(tx);
                    true
                }
                _ => false,
            });
        let second = row().items_center().gaps(2.4, 7.2).child(replace_field).child(
            row().gap(5.04).items_center().children([
                button(t, "file-find-replace", "replace", "Replace the current match (Enter)", false)
                    .on_press_tx(|s: &mut State, tx: &mut Tx| s.file_find_replace(tx)),
                button(t, "file-find-replace-all", "replace all", "Replace every match (Ctrl+Alt+Enter)", none)
                    .on_press(|s: &mut State| s.file_find_replace_all()),
            ]),
        );
        col().grow(1.0).shrink().min_w(0.0).gap(7.2).child(first).child(second)
    } else {
        first
    };
    let close = row()
        .key("file-find-close")
        .role("button")
        .label("close")
        .help("Close (Esc)")
        .w(14.0)
        .h(18.0)
        .center()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 0.0, t.accent)
        .child(text("\u{d7}").font(sans(12.0, 400.0, 1.5)).color(t.text).single_line())
        .on_press_tx(|s: &mut State, tx: &mut Tx| s.file_find_close_ui(tx));
    Some(
        row()
            .key("file-find")
            .role("search")
            .label("Find in file")
            .w_full()
            .items_start()
            .pl(6.0)
            .pr(6.0)
            .pt(2.0)
            .pb(4.0)
            .fill(t.card_bg)
            .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
            .child(items)
            .child(close),
    )
}

/// The matches on line `line` (0-based), as ranges of the line as drawn (`shown` maps the line's bytes: tabs become spaces), with
/// whether each is the current one.
pub fn finds(s: &State, session: &str, path: &str, line: usize, shown: &dyn Fn(Range<usize>) -> Range<usize>) -> Vec<(Range<usize>, bool)> {
    let f = &s.file_find;
    if !f.open || f.target() != (session, path) {
        return Vec::new();
    }
    let (on, first) = f.on_line(line);
    on.iter().enumerate().map(|(n, m)| (shown(m.range.clone()), f.current() == Some(first + n))).collect()
}

/// `.cm-searchMatch` (`--idle` at 25 % and a 1 px ring of it at 45 %) and, on the current match, `.cm-searchMatch-selected` (`--idle`
/// at 45 %).
pub fn highlight(t: &Theme, range: Range<usize>, current: bool) -> TextHighlight {
    let h = TextHighlight::new(range, t.idle.alpha(if current { 0.45 } else { 0.25 })).ring(t.idle.alpha(0.45), 1.0);
    if current { h.key(MARK) } else { h }
}
