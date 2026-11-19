//! Mods' own page (`ModsPluginPage.vue`): a box that says what mods are and links to where they are kept. The switch and the Bun
//! it needs are drawn above it by the plugin page (`mods_panel`); the mods themselves are in Skills & tools › Mods.
use crate::State;
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::rem;
use rune::{Cursor, El, col, row, text};

pub fn page(t: &Theme) -> El {
    let font = sans(rem(0.875), 400.0, 1.43);
    let link = row()
        .key("mods-plugin-open-mods")
        .role("button")
        .label("Skills & tools \u{203a} Mods")
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .child(text("Skills & tools \u{203a} Mods").font(font).color(t.accent_text))
        .on_press(|s: &mut State| s.open_settings("skills", Some("mods")));
    col()
        .key("plugin-config-mods")
        .w_full()
        .padding(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(
            text(concat!(
                "Agents can write small add-ons for Fleet: what a tool row shows, buttons, panes you open from the + menu, and hooks on ",
                "prompts, turns and permission asks. You review each one before it's kept. Your mods, and Fleet's samples to add, are in"
            ))
            .font(font)
            .color(t.muted)
            .wrap(),
        )
        .child(row().items_center().child(link).child(text(".").font(font).color(t.muted)))
}
