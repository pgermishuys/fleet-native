//! `ProgressRing.vue`: a small ring that fills as work gets done, green once everything is. 14 px, 2 px stroke, starting
//! at twelve o'clock. The arc's end eases over 150 ms (`transition: stroke-dashoffset var(--transition)`); a ring
//! drawn for the first time lands at once.
use super::ring::srgb_mix;
use crate::fleet::row_chips::ring_state;
use crate::theme::Theme;
use rune::{Color, El, LineCap, Live, Shape, Spring, canvas, el};
use std::cell::RefCell;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, TAU};

pub const SIZE: f32 = 14.0;
const STROKE: f32 = 2.0;
const RADIUS: f32 = (SIZE - STROKE) / 2.0;
const SEGMENTS: usize = 48;

/// The arc ends of the rings on screen, by owner: each moves from where it was, so a change of count sweeps.
#[derive(Default)]
pub struct Trims(RefCell<HashMap<String, Live<[f32; 2]>>>);

impl Trims {
    /// The animated end for `owner`'s ring, aimed at `end`; a ring seen the first time snaps to it.
    pub(super) fn aim(&self, owner: &str, end: f32) -> rune::live::Binding<[f32; 2]> {
        let mut map = self.0.borrow_mut();
        let live = match map.get(owner) {
            Some(live) => {
                live.set([0.0, end]);
                live
            }
            None => {
                map.insert(owner.to_string(), Live::new([0.0, end]));
                &map[owner]
            }
        };
        live.smooth(Spring::new(0.15, 0.0))
    }
}

/// Points on the circle from `from` to `to` (fractions of the turn) starting at twelve o'clock, clockwise.
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

/// The ring for `done` of `total`. `under` is what it sits on: the track is `--text` at 14% of it, mixed here because
/// Rune blends in linear light and would draw a translucent ink brighter than a browser does.
pub fn progress_ring(trims: &Trims, t: &Theme, owner: &str, done: u32, total: u32, under: Color) -> El {
    let (ratio, complete) = ring_state(done, total);
    let track = Shape::new().stroke_with(circle(), true, STROKE, LineCap::Butt, srgb_mix(t.text, under, 0.14));
    let ink = if complete { t.running } else { t.accent };
    let mut face = el().size(SIZE, SIZE).child(canvas(track).size(SIZE, SIZE).absolute(0.0, 0.0));
    // Aimed even at zero, so the first item ticked off sweeps from nothing instead of landing.
    let end = trims.aim(owner, ratio);
    if ratio > 0.0 {
        face = face.child(
            canvas(Shape::new().stroke_with(circle(), true, STROKE, LineCap::Round, ink))
                .size(SIZE, SIZE)
                .absolute(0.0, 0.0)
                .live_trim(end),
        );
    }
    face
}
