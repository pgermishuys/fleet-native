//! The right panel's two edges that belong to the shell, not to a tab: the collapsed rail (`CollapsedRightRail.vue`) and the
//! button that collapses the panel (`SessionsV2RightPanel.vue`, `right-panel__collapse`), and the gutter on its left
//! (`AppShell.vue`, `.resize-gutter`).
use super::parts::tool_button;
use super::ring::srgb_mix;
use crate::State;
use crate::fleet::right_panel as rp;
use crate::icons::*;
use crate::theme::{Theme, sans};
use rune::{Cursor, Drag, El, LineCap, Phase, Shape, canvas, col, el, icon, text};
use std::f32::consts::{FRAC_PI_2, TAU};

const RING: f32 = 32.0;
const RADIUS: f32 = 12.0;
const STROKE: f32 = 3.0;

fn circle(from: f32, to: f32) -> Vec<[f32; 2]> {
    let n = (((to - from) * 64.0).ceil() as usize).max(2);
    (0..=n)
        .map(|i| {
            let a = (from + (to - from) * i as f32 / n as f32) * TAU - FRAC_PI_2;
            [RING / 2.0 + RADIUS * a.cos(), RING / 2.0 + RADIUS * a.sin()]
        })
        .collect()
}

/// The plan's progress as a ring with the total inside: the track is `--border` at 75%, the value `--accent` with a round
/// cap, and the arc eases over 0.2 s when a step is ticked.
fn ring(s: &State, t: &Theme, done: usize, total: usize) -> El {
    let ratio = if total == 0 { 0.0 } else { done.min(total) as f32 / total as f32 };
    let mut full = circle(0.0, 1.0);
    full.pop();
    let track = Shape::new().stroke_with(full, true, STROKE, LineCap::Butt, srgb_mix(t.border, t.panel_bg, 0.75));
    let mut face = el().key("content-rail-ring").size(RING, RING).child(canvas(track).size(RING, RING).absolute(0.0, 0.0));
    let end = s.row_rings.aim("right-rail", ratio);
    if ratio > 0.0 {
        face = face.child(
            canvas(Shape::new().stroke_with(circle(0.0, 1.0), true, STROKE, LineCap::Round, t.accent))
                .size(RING, RING)
                .absolute(0.0, 0.0)
                .live_trim(end),
        );
    }
    face.child(el().size(RING, RING).absolute(0.0, 0.0).center().child(text(total.to_string()).font(sans(10.0, 700.0, 1.0)).color(t.text)))
}

/// The collapsed panel: one button the width of the rail, chevron on top, then the progress ring and the pending count.
pub fn rail(s: &State, t: &Theme, done: usize, total: usize) -> El {
    let pending = total.saturating_sub(done);
    let mut body = col().items_center().gap(12.0).mt(4.0);
    if total > 0 {
        body = body.child(ring(s, t, done, total));
    }
    body = body.child(
        col()
            .key("content-rail-pending")
            .items_center()
            .gap(2.0)
            .child(icon(&LIST_TODO, 14.0).color(t.muted).group_color(t.text))
            .child(text(pending.to_string()).font(sans(10.0, 700.0, 1.3)).color(t.muted)),
    );
    col()
        .key("content-rail")
        .w(rp::RAIL_W)
        .min_w(rp::RAIL_W)
        .h_full()
        .items_center()
        .gap(18.0)
        .px(8.0)
        .py(12.0)
        .border_sides([0.0, 0.0, 0.0, 1.0], t.border)
        .hover(t.wash(6.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        .help("Expand right panel")
        .label("Expand right panel")
        .on_press(|s: &mut State| s.set_content_open(true))
        .child(icon(&CHEVRON_LEFT, 16.0).color(t.muted))
        .child(body)
}

/// The toolbar button at the end of the tab strip (`size-7` is 24.5px at the 14px root). Over the conversation it closes the
/// sheet instead of collapsing a column.
pub fn collapse_button(t: &Theme, over: bool) -> El {
    let (glyph, tip) = if over { (&X, "Close right panel") } else { (&PANEL_RIGHT_CLOSE, "Collapse right panel") };
    tool_button(t, "close-content", glyph, 14.0, tip).on_press(|s: &mut State| s.set_content_open(false))
}

/// The resize handle between the conversation and the panel: 8 wide and straddling the seam by 4 each side.
pub fn gutter(t: &Theme) -> El {
    el().key("right-gutter")
        .w(8.0)
        .h_full()
        .mx(-4.0)
        .hover(t.accent.alpha(0.15))
        .press(t.accent.alpha(0.15))
        .cursor(Cursor::ResizeColumn)
        .on_drag(|s: &mut State, d: &mut Drag, tx| {
            tx.instant();
            if d.phase == Phase::Down {
                d.data = s.right.begin_drag(s.sheet_w);
            }
            if d.cancelled() {
                s.right.width = d.data;
            } else {
                // `.main` is the window inside the 8px frame.
                let app = (s.ui.sel.window.0 - 16.0).max(0.0);
                s.right.drag(d.data, d.dx, app, s.sheet_w);
            }
        })
}
