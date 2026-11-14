//! What every overlay shares: where it opens relative to its trigger, how it enters and leaves,
//! and how it is dismissed. Fleet's reka-ui content animates `fade-in-0 zoom-in-[0.97]` over
//! 120ms (`menu-classes.ts`); Rune's anchored transition fades and scales from the trigger side.
use rune::{Align, El, Side, Spring, Transition, Vec2, col, el};

/// Positions `content` next to the element keyed `anchor` (reka's `side`/`align`, `sideOffset`
/// as `gap`), flipping at the window's edge, with the overlay entrance.
pub fn anchored(content: El, anchor: &str, side: Side, align: Align, gap: f32) -> El {
    let along = match align {
        Align::Start => 0.0,
        Align::Center => 0.5,
        Align::End => 1.0,
    };
    let origin = match side {
        Side::Top => Vec2::new(along, 1.0),
        Side::Bottom => Vec2::new(along, 0.0),
        Side::Left => Vec2::new(1.0, along),
        Side::Right => Vec2::new(0.0, along),
    };
    let content = if matches!(side, Side::Top | Side::Bottom) {
        content.anchored_fit(anchor, side, align, gap)
    } else {
        content.anchored(anchor, side, align, gap)
    };
    content.transition(Transition::scale_anchor(0.97, origin)).token_motion(Spring::new(0.15, 0.0))
}

/// A transparent full-window press target under an open overlay: pressing outside closes it.
pub fn scrim<S: 'static>(key: &str, close: impl Fn(&mut S) + 'static) -> El {
    el().key(key.to_string()).role("none").absolute(0.0, 0.0).w_full().h_full().on_press(close)
}

/// A column that fills the window and centres its child (a dialog's frame).
pub fn centred(key: &str, child: El) -> El {
    col().key(key.to_string()).absolute(0.0, 0.0).w_full().h_full().center().child(child)
}
