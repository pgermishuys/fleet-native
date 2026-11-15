//! The context ring: how full the session's window is, by Send. Ports the button and the arc of
//! `ContextRing.vue` (docs/parity/05-composer.md, piece G); its popover, the details and Compact now are
//! piece H. Nothing shows until the session's harness has reported a call (the session has a `context`).
use crate::State;
use crate::composer::ring::{self, Tone};
use crate::fleet::context::ContextUsage;
use crate::theme::{RADIUS_BTN, Theme, sans};
use rune::{Color, Cursor, El, LineCap, Live, Shape, Spring, canvas, el, row, text};
use std::cell::RefCell;
use std::f32::consts::{FRAC_PI_2, TAU};

/// The ring's own state: the arc's animated end, and (for scenes, which have no server) a context to draw.
pub struct Ring {
    /// `[start, end]` of the fill's trim; it moves with a 300 ms spring (`transition: stroke-dasharray 300ms ease-out`).
    pub trim: Live<[f32; 2]>,
    /// The session the arc was last drawn for: another session's ring snaps to its value, it doesn't sweep to it.
    drawn_for: RefCell<Option<String>>,
    /// A context a scene shows when there is no stream.
    pub mock: Option<ContextUsage>,
}

impl Default for Ring {
    fn default() -> Self {
        Ring { trim: Live::new([0.0, 0.0]), drawn_for: RefCell::new(None), mock: None }
    }
}

impl Ring {
    /// Publishes the arc's end for `owner`; the first time (or after a switch of session) it lands at once.
    fn aim(&self, owner: &str, end: f32) {
        let same = self.drawn_for.borrow().as_deref() == Some(owner);
        if same {
            self.trim.set([0.0, end]);
        } else {
            self.trim.snap([0.0, end]);
            *self.drawn_for.borrow_mut() = Some(owner.to_string());
        }
    }
}

/// The circle's geometry, in points: the SVG's 16-unit box drawn at 18 (`stroke-width: 2` is 2.25).
const SIZE: f32 = 18.0;
const SCALE: f32 = SIZE / 16.0;
const RADIUS: f32 = 6.0 * SCALE;
const STROKE: f32 = 2.0 * SCALE;
const SEGMENTS: usize = 96;

/// Points on the circle from `from` to `to` (fractions of the turn), starting at twelve o'clock and going clockwise
/// (the SVG is rotated -90 degrees).
fn arc(from: f32, to: f32) -> Vec<[f32; 2]> {
    let n = (((to - from) * SEGMENTS as f32).ceil() as usize).max(2);
    (0..=n)
        .map(|i| {
            let a = (from + (to - from) * i as f32 / n as f32) * TAU - FRAC_PI_2;
            [SIZE / 2.0 + RADIUS * a.cos(), SIZE / 2.0 + RADIUS * a.sin()]
        })
        .collect()
}

fn circle() -> Vec<[f32; 2]> {
    let mut points = arc(0.0, 1.0);
    points.pop();
    points
}

/// `stroke-dasharray: 2 2` (the window is unknown): dashes of 2 box units with 2 between, from the top.
fn dashes(color: Color) -> Shape {
    let unit = 2.0 * SCALE / (TAU * RADIUS);
    let mut shape = Shape::new();
    let mut at = 0.0;
    while at < 1.0 {
        shape = shape.stroke_with(arc(at, (at + unit).min(1.0)), false, STROKE, LineCap::Butt, color);
        at += 2.0 * unit;
    }
    shape
}

/// `color-mix(in srgb, <ink> P%, <under>)` as an opaque colour. Rune blends in linear light, which draws a translucent
/// ink brighter than a browser does; a solid card sits under the ring, so the mix is done in sRGB numbers here.
fn srgb_mix(ink: Color, under: Color, p: f32) -> Color {
    let to = |x: f32| if x <= 0.003_130_8 { x * 12.92 } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
    let from = |x: f32| if x <= 0.040_45 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) };
    let m = |a: f32, b: f32| from(to(a) * p + to(b) * (1.0 - p));
    Color { r: m(ink.r, under.r), g: m(ink.g, under.g), b: m(ink.b, under.b), a: 1.0 }
}

/// The ring and, once the percent is known, "38%". `None` until the harness has reported a call.
pub fn button(s: &State, t: &Theme) -> Option<El> {
    let context = s.context()?;
    let compacting = false;
    let percent = ring::percent(context);
    let tone = ring::tone(percent);
    let amber = ring::amber(percent) && tone != Tone::Danger;
    let ink = match (tone, amber) {
        (Tone::Danger, _) => Some((t.error, t.error_text)),
        (_, true) => Some((t.idle, t.idle_text)),
        _ => None,
    };
    let track_color = srgb_mix(t.text, t.card_bg, 0.14);
    let track = if context.limit.is_none() && context.used.is_some() {
        dashes(track_color)
    } else {
        Shape::new().stroke_with(circle(), true, STROKE, LineCap::Butt, track_color)
    };
    let mut face = el().size(SIZE, SIZE).child(canvas(track).size(SIZE, SIZE).absolute(0.0, 0.0));
    if context.used.is_some() && context.limit.is_some() {
        let owner = s.draft_owner().unwrap_or_default();
        // At least 2% of the circle, so a nearly empty window still shows a dot.
        s.ring.aim(&owner, ring::fill(context).max(0.02));
        let color = ink.map_or(srgb_mix(t.text, t.card_bg, 0.62), |(stroke, _)| stroke);
        face = face.child(
            canvas(Shape::new().stroke_with(circle(), true, STROKE, LineCap::Round, color))
                .size(SIZE, SIZE)
                .absolute(0.0, 0.0)
                .live_trim(s.ring.trim.smooth(Spring::new(0.3, 0.0))),
        );
    }
    let mut b = row()
        .key("context-ring")
        .role("button")
        .label(ring::label(context, compacting))
        .help(ring::title(context, compacting))
        .h(28.0)
        .min_w(28.0)
        .gap(5.0)
        .center()
        .radius(RADIUS_BTN)
        .hover(t.wash(6.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, t.accent)
        .on_press(|_: &mut State| {}) // the popover is piece H
        .child(face);
    if let Some(p) = percent {
        b = b.pl(6.0).pr(7.0);
        let label = text(format!("{p}%")).font(sans(11.5, 500.0, 1.2));
        b = b.child(match ink {
            Some((_, ink)) => label.color(ink),
            None => label.color(t.muted).group_color(t.text),
        });
    }
    Some(b.mr(2.0))
}
