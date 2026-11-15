//! A group's header in the session list: the header button of `ProjectGroup.vue` (`.project-header`). The Pinned
//! group's has a pin before its name; the Ungrouped group's is a plain project header (no +, no menu: piece G adds those
//! to the real projects only). The caller adds `.on_press`, so one header serves every group.
use crate::icons::*;
use crate::theme::{Theme, sans};
use rune::{Cursor, El, icon, row, text};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Header {
    Project,
    /// Sessions with no project: always last; the same header as a project's.
    Ungrouped,
    /// `pinned-header`: `Pin` 11x11, 5 before the name.
    Pinned,
}

pub fn project_header(t: &Theme, key: String, kind: Header, name: &str, count: usize, open: bool) -> El {
    let turn = if open { 0.0 } else { -std::f32::consts::FRAC_PI_2 };
    let label = text(name.to_string()).font(sans(12.0, 600.0, 1.3)).color(t.muted).group_color(t.text).truncate();
    let title = match kind {
        Header::Pinned => row()
            .key("pinned-header")
            .center_y()
            .grow(1.0)
            .min_w(0.0)
            .child(icon(&PIN, 11.0).color(t.muted).group_color(t.text).mr(5.0))
            .child(label.grow(1.0)),
        _ => row().grow(1.0).min_w(0.0).child(label.grow(1.0)),
    };
    row()
        .key(key)
        .w_full()
        .min_h(26.0)
        .center_y()
        .gap(6.0)
        .pl(6.0)
        .pr(10.0)
        .radius(8.0)
        .hover(t.wash(5.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        // `.project-header:focus-visible`: 2px of the accent, drawn inside.
        .focus_ring(2.0, -2.0, t.accent)
        .child(icon(&CHEVRON_DOWN, 13.0).color(t.muted.alpha(0.7)).rotate(turn))
        .child(title)
        .child(text(count.to_string()).font(sans(12.0, 500.0, 1.3).features("'tnum' 1")).color(t.muted.alpha(0.7)))
}
