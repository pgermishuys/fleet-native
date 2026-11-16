//! The notice card. Piece K only draws a plain box (title and body) so the quiet moment can be seen; piece L replaces
//! this with `NoticeCard.vue`'s card (icon tile, close button, items, actions, countdown).
use crate::State;
use crate::theme::{RADIUS_CARD, Theme, sans};
use rune::{El, col, text};

/// The open card, or none while no notice is open. Fixed 12 from the right and 40 from the bottom (row 7.5).
pub fn view(s: &State, t: &Theme) -> Option<El> {
    let n = s.notices.open()?;
    let mut card = col().key("notice-card").w(264.0).gap(4.0).px(12.0).py(12.0).fill(t.card_bg).border(1.0, t.border).radius(RADIUS_CARD);
    card = card.child(text(n.title.clone()).font(sans(13.0, 600.0, 1.3)).color(t.text));
    if let Some(body) = &n.body {
        card = card.child(text(body.clone()).font(sans(12.0, 400.0, 1.45)).color(t.muted).wrap());
    }
    Some(col().key("notice-frame").absolute(0.0, 0.0).w_full().h_full().justify_end().items_end().pr(12.0).pb(40.0).child(card))
}
