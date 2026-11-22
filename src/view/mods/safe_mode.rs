//! `components/mods/ModsSafeModeBanner.vue` (component 22, piece K): says mods are stopped while safe mode is on ("Start without
//! mods"), across the top of the app above the rail, and turns them back on. Nothing at all otherwise, so it takes no room.
use crate::State;
use crate::icons::POWER_OFF;
use crate::theme::{Theme, sans};
use crate::ui::button::{Size, Variant, button};
use rune::{El, icon, row, text};

/// The banner's key; the Turn mods back on button is `mods-safe-mode-turn-on`, a refusal `mods-safe-mode-error`.
pub const KEY: &str = "mods-safe-mode-banner";

/// `.px-3 .py-1.5 .gap-2.5 .text-[0.8125rem]` at the 14 px root.
pub fn banner(s: &State, t: &Theme) -> Option<El> {
    if !s.mods_banner_shown() {
        return None;
    }
    let size = 0.8125 * 14.0;
    let font = sans(size, 400.0, 1.5);
    let mut words = row()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .flex_wrap()
        .gaps(1.75, 7.0)
        .center_y()
        .child(text("Mods are stopped for now").font(sans(size, 700.0, 1.5)).color(t.text));
    words = words.child(
        text("Fleet started without mods. They start again when Fleet restarts or you turn them back on.").font(font).color(t.muted).wrap(),
    );
    if let Some(error) = &s.mod_sites.error {
        words = words.child(text(error.clone()).key("mods-safe-mode-error").role("alert").font(font).color(t.error_text).wrap());
    }
    let pending = s.mod_sites.pending;
    let label = if pending { "Turning on\u{2026}" } else { "Turn mods back on" };
    let mut turn_on = button(t, Variant::Outline, Size::Sm, None, label).key("mods-safe-mode-turn-on");
    turn_on =
        if pending { crate::ui::button::disabled(turn_on) } else { turn_on.on_press(|s: &mut State| s.set_mods_safe_mode(false, true)) };
    Some(
        row()
            .key(KEY)
            .role("status")
            .label("Mods are stopped for now")
            .w_full()
            .center_y()
            .gap(8.75)
            .px(10.5)
            .py(5.25)
            .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
            .border_layout()
            .fill(t.panel_bg)
            .child(icon(&POWER_OFF, 12.25).color(t.accent_text))
            .child(words)
            .child(turn_on),
    )
}
