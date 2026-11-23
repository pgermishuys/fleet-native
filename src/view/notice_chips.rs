//! The notice chips in the status bar (`components/notices/NoticeChips.vue`, row 7.10): where a card settles, small, and
//! stays until whatever posted it is done. A press runs the notice's own click, else opens its card again.
//!
//! The chip's fade (220 ms after 180 ms) and the ring that pulses twice around its dot are drawn from the chip's age on
//! the app clock (`Notices::chip_age`), because Rune's motion eases layout and opacity on change, not a timed keyframe.
use crate::State;
use crate::fleet::notices::Notice;
use crate::theme::{Theme, sans};
use rune::{Cursor, El, el, row, text};

const DOT: f32 = 6.0;
/// The ring, as a share of its full size: the dot's box plus 4 px each side.
const RING_BOX: f32 = DOT + 8.0;

/// The chips, in the order the notices were posted; none when there are none.
pub fn chips(s: &State, t: &Theme) -> Option<El> {
    let slots = s.notices.chip_slots();
    if slots.is_empty() {
        return None;
    }
    let mut r = row().key("notice-chips").center_y().gap(6.0);
    for (n, ghost) in slots {
        r = r.child(if ghost { ghost_chip(t, n) } else { chip(s, t, n) });
    }
    Some(r)
}

/// `ring` is how far along a pulse is, 0 to 1: it grows from half to 1.6 times the box while fading from 0.8 to nothing.
fn ring_at(age: f64) -> Option<f32> {
    // Two pulses of 1.1 s, the first after 400 ms (`animation: notice-chip-ring 1.1s 400ms ease-out 2`).
    let t = age - 0.4;
    (0.0..2.2).contains(&t).then(|| (t % 1.1 / 1.1) as f32)
}

/// ease-out (`cubic-bezier(0, 0, .58, 1)`, near enough), for the fade and the ring.
fn ease_out(x: f32) -> f32 {
    1.0 - (1.0 - x).powi(2)
}

fn body(t: &Theme, n: &Notice, ink: rune::Color, dot: rune::Color) -> El {
    row()
        .h(18.0)
        .center_y()
        .gap(6.0)
        .pl(7.0)
        .pr(8.0)
        .radius(999.0)
        .child(el().key("notice-chip-dot").size(DOT, DOT).radius(999.0).fill(dot))
        .child(text(n.chip.clone().unwrap_or_default()).font(sans(11.0, 500.0, 1.3).features("'tnum' 1")).color(ink))
        .help(n.title.clone())
        .label(n.title.clone())
        .fill(if n.quiet { rune::Color::TRANSPARENT } else { t.accent_dim })
}

fn chip(s: &State, t: &Theme, n: &Notice) -> El {
    let (ink, dot) = if n.quiet { (t.muted, t.running) } else { (t.accent_text, t.accent) };
    let age = s.notices.chip_age(&n.id, s.clock).unwrap_or(f64::MAX);
    let fade = ease_out(((age - 0.18) / 0.22).clamp(0.0, 1.0) as f32);
    let id = n.id.clone();
    let click = n.on_chip_click.clone();
    let hover = if n.quiet { t.wash(6.0) } else { t.accent.alpha(0.24) };
    let mut b = body(t, n, ink, dot)
        .key(format!("notice-chip-{id}"))
        .role("button")
        .opacity(fade)
        .hover(hover)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .on_press(move |s: &mut State| match &click {
            Some(run) => s.notices.tell(run),
            None => s.notices.reopen(&id),
        });
    // Two rings as it arrives, then still; a quiet chip has none.
    if !n.quiet
        && let Some(p) = ring_at(age)
    {
        let size = RING_BOX * (0.5 + 1.1 * ease_out(p));
        let ring = el()
            .key("notice-chip-ring")
            .absolute(7.0 + DOT / 2.0 - size / 2.0, 9.0 - size / 2.0)
            .size(size, size)
            .radius(999.0)
            .border(1.5, t.accent.alpha(0.8 * (1.0 - ease_out(p))));
        b = b.child(ring);
    }
    b
}

/// Where the open card's chip will be: laid out and unseen, so the card can fold into it.
fn ghost_chip(t: &Theme, n: &Notice) -> El {
    let (ink, dot) = if n.quiet { (t.muted, t.running) } else { (t.accent_text, t.accent) };
    // A zero-width slot with the chip hanging out of it: its place is known and the status bar's text doesn't move for it.
    let ghost = body(t, n, ink, dot)
        .key("notice-chip-ghost")
        .absolute(0.0, 0.0)
        .opacity(0.0)
        .on_layout(|s: &mut State, rect| s.place.chip = Some(rect));
    el().w(0.0).h(18.0).child(ghost)
}
