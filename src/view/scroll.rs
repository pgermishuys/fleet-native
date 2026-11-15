//! The "Jump to latest" lane under the conversation's scroll box and the ring on a message that was jumped to.
//! Ports the `.jump-to-latest-lane` / `.jump-to-latest` and `.activity-message--shown` parts of `ActivityStream.vue`.
use crate::theme::{Theme, sans};
use crate::{State, icons::ARROW_DOWN};
use rune::{Color, Cursor, El, Tx, col, icon, row, text};

/// `.jump-to-latest-lane`: its own row under the scroll box (`flex: none`, centred, `padding: 6px 0 4px`), so no text is
/// ever under the pill. `.jump-to-latest`: 12px, a 999px pill with a 1px border, which hover tints toward the accent.
pub fn jump_lane(t: &Theme) -> El {
    // Rune has no border colour for hover, so the border is a 1px ring that takes the hover fill.
    let inner = row()
        .items_center()
        .gap(5.0)
        .px(10.0)
        .py(5.0)
        .radius(999.0)
        .fill(t.card_bg)
        .child(icon(&ARROW_DOWN, 12.0).color(t.muted).group_color(t.text))
        .child(text("Jump to latest").font(sans(12.0, 400.0, 1.0)).color(t.muted).group_color(t.text));
    let pill = row()
        .key("jump-to-latest")
        .px(1.0)
        .py(1.0)
        .radius(999.0)
        .fill(t.border)
        .hover(t.accent_border(55.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        .label("Jump to latest")
        .on_press_tx(|s: &mut State, tx: &mut Tx| s.jump_to_latest(tx))
        .child(inner);
    col().key("jump-lane").w_full().items_center().pt(6.0).pb(4.0).child(pill)
}

/// `.activity-message--shown`: `outline: 2px solid color-mix(accent 60%)`, offset 4px, on the message's rounded box.
pub fn shown_ring(t: &Theme) -> (f32, f32, Color) {
    (2.0, 4.0, t.accent.alpha(0.6))
}
