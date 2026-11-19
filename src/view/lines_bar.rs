//! The bar docked under a diff while lines are selected (`LinesBar.vue`, rows 12.3 of docs/parity/12-changes-and-review.md): the
//! file, which lines and how many, then Copy link, Quote, Add to message and Clear. Its buttons keep the selection, so they act
//! on what is selected. Component 11's file tab uses it too.
use crate::State;
use crate::icons::{LINK, MESSAGE_SQUARE_PLUS, QUOTE, X};
use crate::theme::{Theme, mono, sans};
use rune::{Color, Cursor, El, Icon, Shadow, Tx, el, icon, row, text};

/// What the bar says and offers; the handlers are the caller's.
pub struct Bar<'a> {
    /// The file the lines are in, when the bar can be about more than one; it gives way first on a narrow bar.
    pub file: Option<&'a str>,
    /// "L18–27", or "Old L4".
    pub label: &'a str,
    /// "10 lines".
    pub count: &'a str,
    /// What Copy link copies; no Copy link when none (old lines have no place in the file to link to).
    pub link: Option<&'a str>,
    /// Under 404 px wide: Copy link and Quote are icons, and the count goes (`@container (max-width: 404px)`).
    pub narrow: bool,
}

#[allow(clippy::too_many_arguments)]
fn button(t: &Theme, key: &str, label: &str, tip: &str, glyph: &'static Icon, words: Option<&str>, primary: bool, quiet: bool) -> El {
    let (ink, fill, line) = if primary {
        (t.primary_fg, t.accent, t.accent)
    } else if !quiet {
        (t.text, Color::TRANSPARENT, t.border)
    } else {
        (t.muted, Color::TRANSPARENT, Color::TRANSPARENT)
    };
    let mut b = row()
        .key(key.to_string())
        .role("button")
        .label(label.to_string())
        .help(tip.to_string())
        .h(26.0)
        .px(if words.is_some() && !quiet { 8.0 } else { 6.0 })
        .gap(5.0)
        .items_center()
        .radius(7.0)
        .fill(fill)
        .border(1.0, line)
        .border_layout()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        // A press leaves the selection and the focus as they are, so the handler can still read what is selected.
        .keeps_text_selection()
        .child(icon(glyph, 13.0).color(ink));
    if let Some(w) = words {
        b = b.child(text(w.to_string()).font(sans(12.0, 500.0, 1.4)).color(ink).single_line());
    }
    if primary { b.hover(t.accent.alpha(0.88)) } else { b.hover(t.wash(7.0)) }
}

/// The toast of a panel with a lines bar (`.review__toast`, `.file-canvas__toast`, row 4.9 of docs/parity/11-files.md): centred 18 px
/// above the bottom (58 with the bar up), at most the width less 32, padding `7 12`, radius 9, `--text` on `--panel-bg`, 12.5/500,
/// one line with an ellipsis. It appears at once; Fleet's 160 ms rise isn't drawn.
pub fn toast(t: &Theme, key: &str, words: &str, lifted: bool) -> El {
    row()
        .key(key.to_string())
        .role("status")
        .pin(Some(0.0), None, Some(0.0), Some(if lifted { 58.0 } else { 18.0 }))
        .px(16.0)
        .justify_center()
        .child(
            row()
                .shrink()
                .min_w(0.0)
                .px(12.0)
                .py(7.0)
                .radius(9.0)
                .fill(t.text)
                .child(text(words.to_string()).font(sans(12.5, 500.0, 1.4)).color(t.panel_bg).truncate().shrink().min_w(0.0)),
        )
}

/// `.lines-bar`: margin `0 8 8`, padding `5 6 5 12`, a 1px border, radius 10, `--card-bg`, a soft shadow, one line.
pub fn lines_bar(
    t: &Theme,
    bar: &Bar,
    copy_link: fn(&mut State, &mut Tx),
    quote: fn(&mut State, &mut Tx),
    add: fn(&mut State, &mut Tx),
    clear: fn(&mut State, &mut Tx),
) -> El {
    let mut label = row().gap(6.0).items_center().shrink().min_w(0.0);
    if let Some(file) = bar.file {
        label = label.child(
            text(file.to_string())
                .key("line-bar-file")
                .font(mono(12.0, 400.0, 1.4))
                .color(t.text)
                .truncate()
                .shrink()
                .help(file.to_string()),
        );
    }
    label =
        label.child(text(bar.label.to_string()).key("line-bar-label").font(mono(12.0, 600.0, 1.4)).color(t.text).single_line()).children(
            (!(bar.narrow && bar.file.is_some()))
                .then(|| text(bar.count.to_string()).key("line-bar-count").font(sans(11.0, 400.0, 1.4)).color(t.muted).single_line()),
        );
    let mut r = row()
        .key("lines-bar")
        .role("toolbar")
        .label("Picked lines")
        .mx(8.0)
        .mb(8.0)
        .pl(12.0)
        .pr(6.0)
        .py(5.0)
        .gap(4.0)
        .items_center()
        .radius(10.0)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .box_shadow(Shadow { x: 0.0, y: 16.0, blur: 40.0, spread: -12.0, color: Color::BLACK.alpha(0.45), inset: false })
        .child(label)
        .child(el().grow(1.0));
    if let Some(link) = bar.link {
        r = r.child(
            button(t, "copy-link", "Copy link", &format!("Copy {link}"), &LINK, (!bar.narrow).then_some("Copy link"), false, false)
                .on_press_tx(copy_link),
        );
    }
    r.child(
        button(t, "copy-quote", "Quote", "Copy the lines as a quote", &QUOTE, (!bar.narrow).then_some("Quote"), false, false)
            .on_press_tx(quote),
    )
    .child(
        button(t, "add-to-message", "Add to message", "Add to message", &MESSAGE_SQUARE_PLUS, Some("Add to message"), true, false)
            .on_press_tx(add),
    )
    .child(button(t, "line-bar-clear", "Clear", "Clear (Esc)", &X, None, false, true).on_press_tx(clear))
}
