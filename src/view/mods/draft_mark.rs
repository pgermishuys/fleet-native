//! `components/mods/ModDraftMark.vue`: marks what a draft mod drew (a pane's tab, its `+` menu row, later the band and the chip). The
//! mod runs only in this session until it is kept.
use crate::theme::{Theme, sans};
use rune::{El, row, text};

/// The mark's key; a test and a script find it by it.
pub const KEY: &str = "mod-draft-mark";

/// What the mark says when hovered.
pub const HELP: &str = "Drawn by a draft mod: only this session runs it until you keep it";

/// The mark. `compact` is the one on a pane's tab (`.canvas-tab :deep(.mod-draft-mark)`: `0 3px`, 9 px, line 13); the full one
/// is `0 5px`, 10 px, line 15. Both have a 1 px dashed `--accent` border at 55%, radius 4, 600, tracked 0.02em, upper case.
pub fn draft_mark(t: &Theme, compact: bool) -> El {
    let (px, size, line) = if compact { (3.0, 9.0, 13.0 / 9.0) } else { (5.0, 10.0, 15.0 / 10.0) };
    row()
        .key(KEY)
        .label("Draft")
        .help(HELP)
        .px(px)
        .min_w(0.0)
        .center_y()
        .radius(4.0)
        .border(1.0, t.accent.alpha(0.55))
        .border_dash(3.0, 3.0)
        .border_layout()
        .child(text("DRAFT").font(sans(size, 600.0, line).tracking(size * 0.02)).color(t.accent_text))
}
