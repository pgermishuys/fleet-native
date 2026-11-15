//! What the permission card and the question card share: the amber frame, "Needs you", and the 18px key chip.
//! Both are `.pcard` / `.qcard` in weave-fleet with the same CSS; the two components keep their own copies there.
use super::parts::diamond;
use super::toolcard::mix;
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use rune::{El, col, el, row, text};

/// The card's box: a grid with a 12px gap and 12px padding, tinted with `--status-waiting` (`.pcard`, `.qcard`).
pub fn frame(t: &Theme, key: String) -> El {
    col()
        .key(key)
        .w_full()
        .mt(10.0)
        .padding(12.0)
        .gap(12.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border_mix(t.status_waiting(), 40.0))
        .border_layout()
        .fill(mix(t.card_bg, t.status_waiting(), 0.05))
}

/// The waiting diamond and "Needs you", at the right end of a card's head.
pub fn needs_you(t: &Theme, key: &str) -> El {
    row()
        .key(key.to_string())
        .gap(6.0)
        .center_y()
        .child(diamond(t.status_waiting()))
        .child(text("Needs you").font(sans(12.0, 600.0, 1.3)).color(t.status_waiting_text))
}

/// The 18px chip a choice carries (`.pcard__key`, `.qcard__key`); the caller puts the number or the check in it.
pub fn chip_box(t: &Theme) -> El {
    el().size(18.0, 18.0).center().radius(5.0).border(1.0, t.border).border_layout()
}

/// The chip with its number in it.
pub fn chip(t: &Theme, number: String) -> El {
    chip_box(t).child(text(number).font(mono(11.0, 400.0, 1.0)).color(t.muted))
}
