//! Where the notice card sits, and where its chip is (`NoticeCard.vue`, "Placement" and "Settling"). The card is 12 from
//! the right and 40 from the bottom; a composer that would sit under it lifts it to 8 above the composer's top. When it
//! settles it folds into the corner of its chip. Both need rectangles the layout reports (window points, `[x, y, w, h]`),
//! kept here and handed to plain functions.

/// Where the card sits, from the bottom-right corner (`CARD_RIGHT_PX`, `CARD_BOTTOM_PX`).
pub const RIGHT: f32 = 12.0;
pub const BOTTOM: f32 = 40.0;
/// The gap left above a composer the card lifts over (`CARD_LIFT_GAP_PX`).
pub const LIFT_GAP: f32 = 8.0;

/// The card's settle shrinks it to its chip's width, but never below this share of itself (`Math.max(0.2, …)`).
pub const MIN_SETTLE_SCALE: f32 = 0.2;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Place {
    /// The window's size, from the card's own frame.
    pub window: [f32; 2],
    /// The open card, as last laid out.
    pub card: [f32; 4],
    /// The composer (`data-notice-avoid`), while one is drawn.
    pub composer: Option<[f32; 4]>,
    /// The chip the open card settles into: a hidden one is laid out in the status bar while the card is open.
    pub chip: Option<[f32; 4]>,
}

impl Place {
    /// How far from the bottom the card sits: [`BOTTOM`], or just above a composer it would cover (`place`).
    pub fn bottom(&self) -> f32 {
        let [win_w, win_h] = self.window;
        let [_, _, w, h] = self.card;
        let Some([x, y, cw, ch]) = self.composer.filter(|c| c[2] > 0.0 && c[3] > 0.0) else { return BOTTOM };
        let right = win_w - RIGHT;
        let left = right - w;
        let card_bottom = win_h - BOTTOM;
        let overlaps = x < right && x + cw > left && y < card_bottom && y + ch > card_bottom - h;
        if overlaps { BOTTOM.max(win_h - y + LIFT_GAP) } else { BOTTOM }
    }

    /// Where the card's bottom-right corner lands when it settles, as a move from where it is, and how much it shrinks:
    /// both corners sit on the right, so the corner itself is the anchor. `None` before both have been laid out.
    pub fn settle(&self) -> Option<([f32; 2], f32)> {
        let [cx, cy, cw, ch] = self.card;
        let [x, y, w, h] = self.chip?;
        if cw <= 0.0 || ch <= 0.0 {
            return None;
        }
        let scale = (w / cw.max(1.0)).max(MIN_SETTLE_SCALE);
        Some(([(x + w) - (cx + cw), (y + h) - (cy + ch)], scale))
    }
}
