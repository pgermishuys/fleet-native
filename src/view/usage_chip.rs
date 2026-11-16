//! The status bar's usage chip (ports the chip half of `UsageLimitsPopover.vue`; docs/parity/09-app-frame.md, Part 6, piece J):
//! the fullest limit of any provider Fleet can read, quiet grey until 80%, amber from there and red when used up. A press
//! opens the popover (`view::usage_popover`).
use crate::State;
use crate::fleet::usage::Tone;
use crate::icons::GAUGE;
use crate::theme::{Theme, sans};
use rune::{Cursor, El, el, icon, row, text};

/// The chip the popover is placed over.
pub const TRIGGER: &str = "usage-limits-chip";

/// The tone's ink: `--muted`, `--status-waiting-text`, `--error-text`.
pub fn ink(t: &Theme, tone: Tone) -> rune::Color {
    match tone {
        Tone::Quiet => t.muted,
        Tone::Warn => t.status_waiting_text,
        Tone::Used => t.error_text,
    }
}

/// A bar that fills `percent` of `w` by `h`, its track the ink at 22% (`.usage-chip__bar`, `.usage-pop__bar`).
pub fn bar(ink: rune::Color, w: f32, h: f32, percent: i64) -> El {
    let fill = el().w(w * percent.clamp(0, 100) as f32 / 100.0).h(h).radius(999.0).fill(ink);
    row().w(w).h(h).radius(999.0).clip_rounded(999.0).fill(ink.alpha(0.22)).child(fill)
}

/// The chip, or nothing: with no provider, or with the Settings switch off (`StatusBar.vue`'s `v-if`).
pub fn chip(s: &State, t: &Theme) -> Option<El> {
    if !s.usage_chip_shown() {
        return None;
    }
    let c = s.usage_chip()?;
    let ink = ink(t, c.tone);
    let fill = match c.tone {
        Tone::Quiet => None,
        Tone::Warn => Some(t.status_waiting().alpha(0.14)),
        Tone::Used => Some(t.error.alpha(0.14)),
    };
    let mut b = row()
        .key(TRIGGER)
        .role("button")
        .label(c.title.clone())
        .help(c.title.clone())
        .h(18.0)
        .center_y()
        .gap(6.0)
        .pl(7.0)
        .pr(8.0)
        .radius(999.0)
        .cursor(Cursor::Pointer)
        .on_press(|s: &mut State| s.toggle_usage_popover())
        .child(icon(&GAUGE, 12.0).color(ink))
        .child(text(c.name).key("usage-chip-name").font(sans(11.0, 500.0, 1.3).features("'tnum' 1")).color(ink));
    if let Some(fill) = fill {
        b = b.fill(fill);
    }
    if let Some(p) = c.percent {
        b = b
            .child(bar(ink, 30.0, 4.0, p).key("usage-chip-bar"))
            .child(text(format!("{p}%")).key("usage-chip-percent").font(sans(11.0, 500.0, 1.3).features("'tnum' 1")).color(ink));
    }
    Some(b)
}
