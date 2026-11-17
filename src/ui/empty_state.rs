//! `EmptyState` (`components/ui/empty-state`), the compact size: what a panel shows when it has nothing in it.
use super::rem;
use crate::theme::{Theme, sans};
use rune::{Align, El, Icon, col, icon, text};

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
