//! A finished turn as one line: "Worked for 34 s · read 4 files · edited 2 · ran 2 commands" and a tick. Ports
//! `components/session/TurnFoldRow.vue`. It opens to the turn's own rows; this is the one tick the rows no longer repeat.
use crate::State;
use crate::conv::fold::{summary_parts, worked_text};
use crate::conv::message_list::{FoldedTurn, Outcome};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, Theme, sans};
use rune::{Cursor, El, el, icon, row, text};
use std::f32::consts::FRAC_PI_2;

/// `.turn-fold`: a full-width button, 30px tall at least, 2px above and below, 13px; the chevron turns when it is open.
pub fn fold_row(t: &Theme, turn: &FoldedTurn, open: bool) -> El {
    let id = turn.id.clone();
    let font = sans(13.0, 400.0, 1.3);
    let mut line = row()
        .key(format!("turn-fold-{id}"))
        .w_full()
        .min_h(30.0)
        .my(2.0)
        .px(8.0)
        .gap(8.0)
        .center_y()
        .radius(RADIUS_BTN - 2.0)
        .hover(t.wash(5.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, -2.0, t.accent)
        .label(worked_text(turn.summary.duration_ms))
        .on_press(move |s: &mut State| s.toggle_turn(&id))
        .child(icon(&CHEVRON_RIGHT, 12.0).color(t.muted).group_color(t.text).rotate(if open { FRAC_PI_2 } else { 0.0 }))
        .child(text(worked_text(turn.summary.duration_ms)).font(sans(13.0, 500.0, 1.3)).color(t.text));
    let details = summary_parts(&turn.summary).join(" \u{b7} ");
    if !details.is_empty() {
        line = line.child(text(format!("\u{b7} {details}")).font(font).color(t.muted).group_color(t.text).truncate().shrink().min_w(0.0));
    }
    line = line.child(el().grow(1.0));
    // `--danger` isn't defined in Fleet's stylesheet, so the CSS falls back to `--muted`: the mark reads in grey.
    match turn.summary.outcome {
        Outcome::Done => line.child(icon(&CHECK, 13.0).color(t.running_text)),
        ended => {
            let word = if ended == Outcome::Failed { "Failed" } else { "Stopped" };
            line.child(
                row()
                    .key(format!("turn-fold-ended-{}", turn.id))
                    .gap(4.0)
                    .center_y()
                    .child(icon(&CIRCLE_ALERT, 13.0).color(t.muted))
                    .child(text(word).font(sans(12.0, 400.0, 1.3)).color(t.muted)),
            )
        }
    }
}
