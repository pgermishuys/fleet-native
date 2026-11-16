//! `DiffStat.vue`: lines added and removed, "+412 −88".
use crate::theme::{Theme, mono};
use rune::{El, row, text};

pub fn diff_stat(t: &Theme, key: &str, additions: i64, deletions: i64) -> El {
    let style = || mono(11.5, 400.0, 1.2).features("'tnum' 1");
    row()
        .key(key.to_string())
        .gap(4.0)
        .help(format!("{additions} lines added, {deletions} removed"))
        .child(text(format!("+{additions}")).font(style()).color(t.diff_add_text()))
        .child(text(format!("\u{2212}{deletions}")).font(style()).color(t.diff_del_text()))
}
