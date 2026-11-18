//! The card that stands for a web page where a live web view can't go (`docs/parity/14-web-content.md`, decision 1): a native
//! web view can't scroll or clip inside a Rune list, so an HTML preview in a tool card, or a rendered `.html` file in the review
//! list, is this card: the title, "HTML", and an Expand that opens it as a tab. Components 11 and 12 use `html_card` as it is.
use crate::State;
use crate::icons::*;
use crate::theme::{RADIUS_BTN, Theme, sans};
use rune::{Cursor, El, col, icon, row, text};

/// A card named `key`: the page's `title`, the word after it, and the button, which runs `expand`.
pub fn html_card(t: &Theme, key: &str, title: &str, expand: impl Fn(&mut State) + 'static) -> El {
    let head = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(icon(&GLOBE, 14.0).color(t.muted))
        .child(
            text(title.to_string())
                .key(format!("{key}-title"))
                .font(sans(12.5, 600.0, 1.4))
                .color(t.text)
                .truncate()
                .min_w(0.0)
                .help_when_truncated(title.to_string()),
        )
        .child(text("HTML").key(format!("{key}-kind")).font(sans(12.0, 400.0, 1.4)).color(t.muted));
    col().key(key.to_string()).w_full().gap(8.0).child(head).child(expand_button(t, &format!("{key}-expand"), expand))
}

/// `.tool-visual__expand`: the accent's fill, `--primary-foreground`, 12 px 600, padding 4 12, 85 % on hover. It is as wide as
/// its words, not stretched across the card.
pub fn expand_button(t: &Theme, key: &str, run: impl Fn(&mut State) + 'static) -> El {
    let button = row()
        .key(key.to_string())
        .role("button")
        .label("Expand")
        .px(12.0)
        .py(4.0)
        .radius(RADIUS_BTN)
        .fill(t.accent)
        .hover(t.accent.alpha(0.85))
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent.alpha(0.5))
        .on_press(run)
        .child(text("Expand").font(sans(12.0, 600.0, 1.4)).color(t.primary_fg));
    row().w_full().child(button)
}
