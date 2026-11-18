//! The strip's two bits of motion (`CanvasHost.vue`): a tab the agent opens slides in once, and a tab whose page updated itself
//! pulses. Both are drawn from the app's clock (Rune animates opacity, not a fading background), and `State::keep_tab_motion`
//! keeps the frames coming until they have run.
use crate::{State, Tx};
use std::time::Duration;

/// `canvas-tab-enter`: 240 ms; Fleet drops the class after 320 ms.
pub const ENTER_SECS: f64 = 0.24;
const ENTER_CLASS_SECS: f64 = 0.32;
/// `canvas-tab-pulse`: 1.2 s; the class drops after 1.4 s.
pub const PULSE_SECS: f64 = 1.2;
const PULSE_CLASS_SECS: f64 = 1.4;
/// The icon keeps `--accent-text` for the first 40% of the pulse.
pub const PULSE_ICON_SHARE: f32 = 0.4;

/// `cubic-bezier(x1, y1, x2, y2)` at time `x` (0..1), solved by bisection.
pub fn bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let at = |t: f32, a: f32, b: f32| 3.0 * (1.0 - t) * (1.0 - t) * t * a + 3.0 * (1.0 - t) * t * t * b + t * t * t;
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if at(mid, x1, x2) < x { lo = mid } else { hi = mid }
    }
    at((lo + hi) / 2.0, y1, y2)
}

/// `ease-out`, which CSS makes `cubic-bezier(0, 0, .58, 1)`.
pub fn ease_out(x: f32) -> f32 {
    bezier(0.0, 0.0, 0.58, 1.0, x.clamp(0.0, 1.0))
}

/// How far the slide-in has come (0..1 through the 240 ms), or `None` when the tab isn't the one that just arrived.
pub fn enter_progress(started: f64, now: f64) -> Option<f32> {
    let age = now - started;
    (0.0..ENTER_CLASS_SECS).contains(&age).then(|| (age / ENTER_SECS).min(1.0) as f32)
}

/// How far the pulse has come (0..1 through the 1.2 s), or `None` once its class has dropped.
pub fn pulse_progress(started: f64, now: f64) -> Option<f32> {
    let age = now - started;
    (0.0..PULSE_CLASS_SECS).contains(&age).then(|| (age / PULSE_SECS).min(1.0) as f32)
}

impl State {
    /// The slide-in's progress for a tab, if it is the one that just arrived.
    pub fn tab_enter(&self, id: &str) -> Option<f32> {
        let (tab, started) = self.canvases.entering.as_ref()?;
        (tab == id).then(|| enter_progress(*started, self.clock)).flatten()
    }

    /// The pulse's progress for an open tab whose page updated itself.
    pub fn tab_pulse(&self, id: &str) -> Option<f32> {
        pulse_progress(self.canvases.updated_at(id)?, self.clock)
    }

    /// Keeps the strip drawing while a slide-in or a pulse runs; false once both are over.
    pub fn keep_tab_motion(&mut self, tx: &mut Tx) -> bool {
        let tabs = self.open_canvases();
        let pulsing = tabs.canvases.iter().any(|c| self.tab_pulse(&c.id).is_some());
        let entering = self.canvases.entering.as_ref().is_some_and(|(_, at)| enter_progress(*at, self.clock).is_some());
        if !entering {
            self.canvases.entering = None;
        }
        if pulsing || entering {
            tx.wake_after(Duration::from_millis(16));
        }
        pulsing || entering
    }
}
