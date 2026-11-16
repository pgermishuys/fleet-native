//! The New session draft's row in the session list (`DraftSessionRow.vue`): the same box as a session's row, so the session
//! it becomes takes its place.
use super::session_row::title_ink;
use crate::State;
use crate::fleet::new_session::draft::DraftRow;
use crate::fleet::row_status::Dim;
use crate::icons::PENCIL_LINE;
use crate::theme::{RADIUS_BTN, Theme, sans};
use rune::{Color, Cursor, El, el, icon, row, text};

pub fn draft_row(s: &State, t: &Theme, draft: &DraftRow) -> El {
    let active = s.on_new_session_page();
    let empty = draft.title.is_empty();
    let title = if empty { "New session".to_string() } else { draft.title.clone() };
    let ink = if empty { t.row_meta_ink() } else { title_ink(t, active, Dim::None) };
    let meta = if draft.is_starting { "Starting…" } else { "Draft" };
    let project = draft.project_id.clone();
    // `.draft-row-shell`: 1px above and below, as a session row has.
    el().w_full().py(1.0).child(
        row()
            .key("new-session-draft-row")
            .w_full()
            .min_h(32.0)
            .center_y()
            .gap(9.0)
            .pl(6.0)
            .pr(10.0)
            .radius(RADIUS_BTN)
            .fill(if active { t.wash(9.0) } else { Color::TRANSPARENT })
            .hover(if active { Color::TRANSPARENT } else { t.wash(5.0) })
            .hover_group()
            .cursor(Cursor::Pointer)
            .focusable(true)
            .focus_ring(2.0, -2.0, t.accent)
            .on_press(move |s: &mut State| s.open_new_session_page(project.clone()))
            // The caret's empty place, then the icon in the status glyph's column, a touch larger (`.draft-row__icon`).
            .child(el().size(14.0, 14.0).mr(-4.0))
            .child(el().size(12.0, 12.0).mx(-2.0).child(icon(&PENCIL_LINE, 12.0).color(t.muted)))
            .child(
                text(title)
                    .font(sans(13.0, if active && !empty { 500.0 } else { 400.0 }, 1.3))
                    .color(ink)
                    .group_color(t.text)
                    .truncate()
                    .grow(1.0),
            )
            .child(text(meta).font(sans(12.0, 400.0, 1.3)).color(t.row_meta_ink()).group_color(t.row_meta_ink())),
    )
}
