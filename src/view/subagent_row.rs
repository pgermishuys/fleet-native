//! A running sub-agent under the session that started it (`SubagentSessionRow.vue`): the pulsing glyph, its task and a
//! SUBAGENT label. Pressing it opens the sub-agent's own session under its parent. It goes when the sub-agent finishes.
use super::parts::status_glyph;
use crate::fleet::work::RunningWorkItem;
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::{State, Tx};
use rune::{Color, Cursor, El, el, row, text};

/// The row. `indent` is where its content starts (`--lineage-indent`).
pub fn subagent_row(s: &State, t: &Theme, work: &RunningWorkItem, indent: f32) -> El {
    let active = s.open_id().is_some_and(|open| work.child_session_id.as_deref() == Some(open.as_str()));
    let title = work.label.clone().unwrap_or_else(|| work.title.clone());
    let (parent, child) = (work.session_id.clone(), work.child_session_id.clone().unwrap_or_default());
    let name = title.clone();
    // The sheet's `.subagent-row` has no `py` of its own: the shell around it gives the 1px.
    let line = row()
        .key(format!("subagent-row-{}", work.id))
        .w_full()
        .min_h(28.0)
        .center_y()
        .gap(9.0)
        .pl(indent)
        .pr(10.0)
        .radius(RADIUS_BTN)
        .fill(if active { t.card_bg } else { Color::TRANSPARENT })
        .hover(if active { t.card_bg } else { t.wash(5.0) })
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        .help(format!("{}: {title}", work.title))
        .on_press_tx(move |s: &mut State, tx: &mut Tx| {
            // No session of its own: nothing to open.
            if !child.is_empty() {
                s.open_subagent_session(&parent, &child, &name, tx);
            }
        })
        .child(status_glyph(t, "running", false))
        .child(
            text(title)
                .key(format!("subagent-title-{}", work.id))
                .font(sans(13.0, 400.0, 1.3))
                .color(if active { t.text } else { t.text.alpha(0.86) })
                .group_color(t.text)
                .truncate()
                .grow(1.0),
        )
        // Always `--muted`, and its own colour, so the row's hover colour doesn't reach it (decided 17 October 2026).
        .child(
            text("SUBAGENT").key(format!("subagent-kind-{}", work.id)).font(sans(10.5, 600.0, 1.2).tracking(0.04 * 10.5)).color(t.muted),
        );
    el().w_full().py(1.0).hover_group().child(line)
}
