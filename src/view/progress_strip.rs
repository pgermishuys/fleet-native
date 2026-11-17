//! `SessionMetadataHeader.vue`: a one-line summary of the session's progress along the foot of the right panel. 36 px,
//! a 1 px rule above it, a 1 px accent bar on that rule as wide as the share done, the 14 px ring, "Todos ·" and the
//! current todo, and `done/total`. Muted until hovered; a click opens the Progress tab.
use super::progress_ring::progress_ring;
use crate::State;
use crate::fleet::progress::Plan;
use crate::fleet::progress_line as line;
use crate::fleet::row_chips::Progress;
use crate::theme::{Theme, sans};
use rune::{Cursor, El, el, row, text};

/// The strip for `progress`, or nothing while it has no items or its tab is open (`visible`).
pub fn strip(s: &State, t: &Theme, progress: &Progress, plan: Option<&Plan>) -> El {
    let tip = line::label(progress, plan);
    let fill = line::fraction(progress);
    let mut bar = row().w_full().h(1.0).absolute(0.0, -1.0);
    if fill > 0.0 {
        bar = bar.child(el().key("progress-strip-bar").w_frac(fill).h(1.0).fill(t.accent));
    }
    row()
        .key("progress-strip")
        .role("button")
        .w_full()
        .h(36.0)
        .min_h(36.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .border_sides([1.0, 0.0, 0.0, 0.0], t.border)
        .hover(t.wash(4.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, -2.0, t.accent)
        .help(tip.clone())
        .label(tip)
        .on_press(|s: &mut State| s.set_panel_tab(line::PROGRESS_TAB))
        .child(bar)
        .child(progress_ring(&s.row_rings, t, "progress-strip", progress.done, progress.total, t.panel_bg))
        .child(
            row()
                .grow(1.0)
                .min_w(0.0)
                .gap(6.0)
                .center_y()
                .child(
                    text(format!("{} ·", line::heading(progress, plan)))
                        .font(sans(12.0, 400.0, 1.3))
                        .color(t.muted)
                        .group_color(t.text)
                        .truncate(),
                )
                .child(text(line::current_line(progress, plan)).font(sans(12.0, 400.0, 1.3)).color(t.text).truncate().grow(1.0).min_w(0.0)),
        )
        .child(text(format!("{}/{}", progress.done, progress.total)).font(sans(12.0, 400.0, 1.3)).color(t.muted).group_color(t.text))
}

/// The strip for the open session, or nothing.
pub fn slot(s: &State, t: &Theme) -> Option<El> {
    let progress = line::visible(s.session().row.progress.as_ref(), s.context.tab)?;
    let plan = s.progress.of(&s.session().id).and_then(|d| d.plan.as_ref());
    Some(strip(s, t, progress, plan))
}
