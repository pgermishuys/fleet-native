//! `ErrorNote` (`components/ui/error-note`): the red box, the message in medium weight and the reason under it.
use super::rem;
use crate::icons::CIRCLE_ALERT;
use crate::theme::{RADIUS_CARD, Theme, sans};
use rune::{El, col, icon, row, text};

pub fn error_note(t: &Theme, key: &str, message: &str, reason: &str) -> El {
    row()
        .key(key.to_string())
        .role("alert")
        .w_full()
        .gap(8.0)
        .items_start()
        .px(12.0)
        .py(8.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.1))
        .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .child(text(message.to_string()).font(sans(rem(0.875), 500.0, 1.43)).color(t.error_text).wrap().w_full())
                .child(text(reason.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.error_text).wrap().w_full().mt(2.0)),
        )
}

/// `ErrorNote compact`: no tint or box, one note of 12 px words with a 14 px glyph, for under a card or a field.
pub fn compact(t: &Theme, key: &str, message: &str) -> El {
    row()
        .key(key.to_string())
        .role("alert")
        .w_full()
        .gap(8.0)
        .items_start()
        .child(icon(&CIRCLE_ALERT, 14.0).color(t.error_text).mt(1.0))
        .child(
            text(message.to_string())
                .key(format!("{key}-text"))
                .font(sans(rem(0.75), 400.0, 1.33))
                .color(t.error_text)
                .wrap()
                .grow(1.0)
                .shrink()
                .min_w(0.0),
        )
}
