//! A scene only: the Markdown an export writes, as plain text, in the pane the export dialog shows it in
//! (`ExportMarkdownDialog.vue`, the Source tab: `font-mono text-xs`, 10.5px on a 14px line). The dialog itself is piece S.
use crate::theme::{RADIUS_PANEL, Theme, mono};
use rune::{El, ScrollSpec, col, text};

pub fn view(t: &Theme, markdown: &str) -> El {
    let body = col().w_full().padding(10.5).child(text(markdown.trim_end().to_string()).font(mono(10.5, 400.0, 14.0 / 10.5)).color(t.text));
    col()
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .radius(RADIUS_PANEL)
        .child(body.scroll(ScrollSpec { stick_bottom: false }).key("export-source").grow(1.0))
}
