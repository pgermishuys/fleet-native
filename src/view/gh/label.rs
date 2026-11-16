//! `GitHubLabel.vue`: a GitHub label in its own colour, tinted so it reads on any theme. 18 tall, a 1 px edge at 35%, a
//! fill at 14%, the text its colour lifted 70% toward `--text`.
use crate::theme::{Theme, sans};
use rune::{Color, El, row, text};

/// `#rrggbb` as GitHub sends it (no `#`); anything else is grey (`888888`).
pub fn label_color(hex: &str) -> Color {
    let ok = hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit());
    rune::color::hex(&format!("#{}", if ok { hex } else { "888888" })).expect("a six digit colour")
}

pub fn label(t: &Theme, key: &str, name: &str, color: &str) -> El {
    let c = label_color(color);
    row()
        .key(key.to_string())
        .h(18.0)
        .px(7.0)
        .center_y()
        .radius(999.0)
        .border(1.0, c.alpha(0.35))
        .fill(c.alpha(0.14))
        .child(text(name.to_string()).font(sans(11.0, 500.0, 1.0)).color(t.text_mix(c, 0.7)))
}
