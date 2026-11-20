//! `PageHeader` (`components/ui/page-header`) with `bar`: the title of a page whose body scrolls under it, full width with its
//! own padding and a rule under it (Automations, Jobs, Fleet, Board).
use super::rem;
use crate::theme::{Theme, sans};
use rune::{El, row, text};

/// The bar: the title (`text-2xl` 600, tight) at padding 14 / 1.25rem, and a 1 px rule under it. `actions` sit at the end.
pub fn bar(t: &Theme, key: &str, title: &str, actions: Option<El>) -> El {
    bar_with(t, key, title, Vec::new(), actions.into_iter().collect())
}

/// `bar` with the `meta` slot: what sits beside the title on its row (a switch, a line of words), and the `actions`' buttons.
pub fn bar_with(t: &Theme, key: &str, title: &str, meta: Vec<El>, actions: Vec<El>) -> El {
    let heading = text(title.to_string()).font(sans(rem(1.5), 600.0, 1.25).tracking(-0.525)).color(t.text);
    row()
        .key(key.to_string())
        .w_full()
        .center_y()
        .gap(8.0)
        .px(rem(1.25))
        .py(14.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .border_layout()
        // The title row: the heading, then the meta, 10 px apart.
        .child(row().grow(1.0).shrink().min_w(0.0).center_y().gap(10.0).child(heading).children(meta))
        .children(actions)
}
