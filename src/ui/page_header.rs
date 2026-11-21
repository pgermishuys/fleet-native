//! `PageHeader` (`components/ui/page-header`) with `bar`: the title of a page whose body scrolls under it, full width with its
//! own padding and a rule under it (Automations, Jobs, Fleet, Board).
use super::rem;
use crate::theme::{Theme, sans};
use rune::{El, col, row, text};

/// The bar: the title (`text-2xl` 600, tight) at padding 14 / 1.25rem, and a 1 px rule under it. `actions` sit at the end.
pub fn bar(t: &Theme, key: &str, title: &str, actions: Option<El>) -> El {
    bar_with(t, key, title, Vec::new(), actions.into_iter().collect())
}

/// `bar` with `description`, the line of words under the title (14 px muted, 1.5, 4 px down): Fleet's counts.
pub fn bar_described(t: &Theme, key: &str, title: &str, description: &str) -> El {
    let heading = text(title.to_string()).font(sans(rem(1.5), 600.0, 1.25).tracking(-0.525)).color(t.text);
    let line = text(description.to_string()).key(format!("{key}-description")).font(sans(rem(0.875), 400.0, 1.5)).color(t.muted);
    row()
        .key(key.to_string())
        .w_full()
        .center_y()
        .gap(8.0)
        .px(rem(1.25))
        .py(14.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .border_layout()
        .child(col().grow(1.0).shrink().min_w(0.0).child(heading).child(row().mt(4.0).child(line)))
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

/// `PageHeader` without `bar`: for a page that already has its own padding (Settings, Analytics). The title (`text-2xl` 600,
/// tight) with the `meta` slot beside it, 10 px apart, and the line under it (14 px muted, 1.5, 4 px down) when there is one.
pub fn plain(t: &Theme, key: &str, title: &str, line: Option<&str>, meta: Vec<El>) -> El {
    let heading = text(title.to_string()).font(sans(rem(1.5), 600.0, 1.25).tracking(-0.525)).color(t.text);
    let head = col().key(key.to_string()).w_full().child(row().wrap().center_y().gap(10.0).child(heading).children(meta));
    match line {
        Some(line) => head.child(row().mt(4.0).child(text(line.to_string()).font(sans(rem(0.875), 400.0, 1.5)).color(t.muted))),
        None => head,
    }
}

/// `plain` with the `actions` slot: the buttons sit at the end of the title row, 8 px apart, centred on the title and its line.
pub fn plain_actions(t: &Theme, key: &str, title: &str, line: Option<&str>, actions: Vec<El>) -> El {
    row()
        .key(key.to_string())
        .w_full()
        .center_y()
        .gap(16.0)
        .child(plain(t, &format!("{key}-lead"), title, line, Vec::new()).grow(1.0).shrink().min_w(0.0))
        .child(row().gap(8.0).center_y().children(actions))
}

/// `plain_actions` with a heading made of several parts (a muted owner and the name), for a title that isn't one run of words.
pub fn heading_actions(key: &str, heading: El, actions: Vec<El>) -> El {
    row()
        .key(key.to_string())
        .w_full()
        .center_y()
        .gap(16.0)
        .child(row().grow(1.0).shrink().min_w(0.0).center_y().child(heading))
        .child(row().gap(8.0).center_y().children(actions))
}
