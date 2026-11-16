//! `PrStatePill.vue`: the state in words on a tinted pill: "Checks failing", "Ready to merge", "Merged". 22 tall, the tone's
//! ink on the tone at 14%.
use super::item_icon::{ItemKind, glyph, tone};
use crate::fleet::pr_state::PrState;
use crate::theme::{Theme, sans};
use rune::{El, icon, row, text};

pub fn pr_state_pill(t: &Theme, key: &str, kind: ItemKind, state: PrState, label: &str) -> El {
    let tone = tone(kind, state);
    let ink = t.pr_tone_text(tone);
    row()
        .key(key.to_string())
        .h(22.0)
        .px(9.0)
        .gap(5.0)
        .center_y()
        .radius(999.0)
        .fill(t.pr_tone(tone).alpha(0.14))
        .child(icon(glyph(kind, state), 13.0).color(ink))
        .child(text(label.to_string()).font(sans(12.0, 500.0, 1.2)).color(ink))
}
