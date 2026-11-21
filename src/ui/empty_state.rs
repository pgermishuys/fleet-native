//! `EmptyState` (`components/ui/empty-state`), the compact size: what a panel shows when it has nothing in it.
use super::rem;
use crate::theme::{Theme, sans};
use rune::{Align, El, Icon, col, icon, row, text};

/// An optional 20 px icon, a medium 14 px title, centred, padding `py-4 px-3`, gap 4.
pub fn compact(t: &Theme, key: &str, glyph: &'static Icon, title: &str) -> El {
    col()
        .key(key.to_string())
        .w_full()
        .items_center()
        .gap(rem(0.25))
        .px(rem(0.75))
        .py(rem(1.0))
        .child(icon(glyph, 20.0).color(t.muted).mb(rem(0.5)))
        .child(text(title.to_string()).font(sans(rem(0.875), 500.0, 1.43)).color(t.text).wrap().align(Align::Center))
}

/// The compact `EmptyState` with its one line of text (`text-xs`, `--muted`) under the title, and the icon optional.
pub fn compact_described(t: &Theme, key: &str, glyph: Option<&'static Icon>, title: &str, description: &str) -> El {
    let mut c = col().key(key.to_string()).w_full().items_center().gap(rem(0.25)).px(rem(0.75)).py(rem(1.0));
    if let Some(glyph) = glyph {
        c = c.child(icon(glyph, 20.0).color(t.muted).mb(rem(0.5)));
    }
    c.child(text(title.to_string()).font(sans(rem(0.875), 500.0, 1.43)).color(t.text).wrap().align(Align::Center))
        .child(text(description.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().align(Align::Center))
}

/// The compact `EmptyState` with a title only and no icon.
pub fn compact_bare(t: &Theme, key: &str, title: &str) -> El {
    col()
        .key(key.to_string())
        .w_full()
        .items_center()
        .px(rem(0.75))
        .py(rem(1.0))
        .child(text(title.to_string()).font(sans(rem(0.875), 500.0, 1.43)).color(t.text).wrap().align(Align::Center))
}

/// The full `EmptyState` (not `compact`): a 28 px icon, a 14 px 600 title, a 12.25 px line under it and an optional action,
/// centred, padding 21, gap 4; the action is 12 under the text.
pub fn full(t: &Theme, key: &str, glyph: &'static Icon, title: &str, description: &str, action: Option<El>) -> El {
    let mut c = col()
        .key(key.to_string())
        .items_center()
        .gap(rem(0.25))
        .padding(rem(1.5))
        .child(icon(glyph, 28.0).color(t.muted).mb(rem(0.5)))
        .child(text(title.to_string()).font(sans(rem(1.0), 600.0, 1.5)).color(t.text).wrap().align(Align::Center))
        .child(text(description.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().max_w(513.0).align(Align::Center));
    if let Some(action) = action {
        c = c.child(row().mt(rem(0.75)).center().gap(rem(0.5)).child(action));
    }
    c
}

/// The compact-less `EmptyState bordered` inside a panel (`p-6`, title only): a dashed card, at least `min_h` tall,
/// its words in the middle (`rounded-card border border-dashed border-border`, `text-base` 600).
pub fn bordered(t: &Theme, key: &str, title: &str, min_h: f32) -> El {
    col()
        .key(key.to_string())
        .w_full()
        .min_h(min_h)
        .center()
        .gap(rem(0.25))
        .padding(rem(1.5))
        .radius(crate::theme::RADIUS_CARD)
        .border(1.0, t.border)
        .border_dash(4.0, 3.0)
        .border_layout()
        .child(text(title.to_string()).font(sans(rem(1.0), 600.0, 1.5)).color(t.text).wrap().align(Align::Center))
}

/// `EmptyState bordered compact`: the dashed card at the compact size (`px-3 py-4`, a 14 px medium title).
pub fn bordered_compact(t: &Theme, key: &str, title: &str) -> El {
    col()
        .key(key.to_string())
        .w_full()
        .center()
        .px(rem(0.75))
        .py(rem(1.0))
        .radius(crate::theme::RADIUS_CARD)
        .border(1.0, t.border)
        .border_dash(4.0, 3.0)
        .border_layout()
        .child(text(title.to_string()).font(sans(rem(0.875), 500.0, 1.43)).color(t.text).wrap().align(Align::Center))
}
