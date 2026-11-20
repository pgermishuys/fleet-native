//! `PageHeader` (`components/ui/page-header`) with `bar`: the title of a page whose body scrolls under it, full width with its
//! own padding and a rule under it (Automations, Jobs, Fleet, Board).
use super::rem;
use crate::theme::{Theme, sans};
use rune::{El, col, row, text};

/// The bar: the title (`text-2xl` 600, tight) at padding 14 / 1.25rem, and a 1 px rule under it. `actions` sit at the end.
pub fn bar(t: &Theme, key: &str, title: &str, actions: Option<El>) -> El {
    row()
        .key(key.to_string())
        .w_full()
        .center_y()
        .gap(8.0)
        .px(rem(1.25))
        .py(14.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .border_layout()
        .child(col().grow(1.0).min_w(0.0).child(text(title.to_string()).font(sans(rem(1.5), 600.0, 1.25).tracking(-0.525)).color(t.text)))
        .children(actions)
}
