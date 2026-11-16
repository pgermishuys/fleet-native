//! What a dragged row shows (`ProjectGroup.vue`, `SessionsPanel.vue`): the line where a pin will go, the empty Pinned group's
//! box and the Archive box. The group's own outline is drawn by `sessions.rs` on its header.
use crate::State;
use crate::fleet::row_drag::{ARCHIVE_KEY, Over};
use crate::icons::*;
use crate::theme::{Theme, sans};
use rune::{El, col, el, icon, row, text};

/// `.project-row--drop-before::before`: 2px of the accent, 1px radius, inset 10 either side.
pub fn pin_line(t: &Theme, w: f32) -> El {
    el().w(w.max(0.0)).h(2.0).radius(1.0).fill(t.accent)
}

/// `.project-content--drop-end::after`: the same line after the last pinned row. It takes no room.
pub fn pin_end_line(t: &Theme, w: f32) -> El {
    el().w_full().h(0.0).child(pin_line(t, w).absolute(10.0, -2.0))
}

/// `.pinned-empty`: 36 high, a dashed 1.5px muted border at 45 %; over it the accent and a 7 % fill.
pub fn pinned_empty(t: &Theme, over: bool) -> El {
    let ink = if over { t.accent_text } else { t.muted };
    row()
        .key("pinned-empty")
        .w_full()
        .h(36.0)
        .my(2.0)
        .gap(6.0)
        .center()
        .radius(8.0)
        .border(1.5, if over { t.accent } else { t.muted.alpha(0.45) })
        .border_dash(4.0, 3.0)
        .fill(if over { t.accent.alpha(0.07) } else { rune::Color::TRANSPARENT })
        .child(icon(&PIN, 12.0).color(ink))
        .child(text("Drop here to pin").font(sans(12.0, 400.0, 1.3)).color(ink))
}

/// `.complete-drop-zone`: a dashed 2px box at the end of the list while a row is dragged; over it the accent and an 8 % fill.
pub fn archive_zone(s: &State, t: &Theme) -> El {
    let over = s.drag_over() == Over::Archive;
    let ink = if over { t.accent_text } else { t.muted };
    col().w_full().mt(8.0).child(
        row()
            .key(ARCHIVE_KEY)
            .label("Drop session here to archive it")
            .w_full()
            .px(16.0)
            .py(16.0)
            .gap(8.0)
            .center()
            .radius(8.0)
            .border(2.0, if over { t.accent } else { t.muted })
            .border_dash(5.0, 4.0)
            .fill(if over { t.accent.alpha(0.08) } else { rune::Color::TRANSPARENT })
            .child(icon(&ARCHIVE, 16.0).color(ink))
            .child(text("Archive").font(sans(12.0, 500.0, 1.3)).color(ink)),
    )
}
