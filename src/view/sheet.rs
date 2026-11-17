//! The workspace sheet: the session header, the conversation, the composer, and the content
//! panel on the right (collapsed to a 48px rail by default).
use super::conversation::{Scroll, stream};
use crate::State;
use crate::mock::Session;
use crate::theme::Theme;
use rune::{Color, El, Shadow, col, row};

use super::right_rail;
use crate::fleet::right_panel as rp;

/// How wide the conversation column is, which the header's sides and its pill's half are measured against (Fleet's
/// `session-detail-header` container). Unmeasured counts as roomy.
fn centre_w(s: &State) -> f32 {
    if s.sheet_w <= 0.0 {
        return f32::INFINITY;
    }
    let panel = if !s.content_open || !rp::fits(s.sheet_w) { rp::RAIL_W } else { s.right_shown_w() };
    s.sheet_w - panel
}

pub fn sheet(s: &State, t: &Theme) -> El {
    let session = s.session();
    let centre = col()
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .min_h(0.0)
        .child(super::header::header(s, t, centre_w(s)))
        .children(super::header_banners::banners(s, t, &s.header_model()))
        .child(stream(
            t,
            session,
            &s.fold_plan,
            s.is_partial(),
            s.older(),
            Scroll {
                jump: s.pin.show_jump(),
                probe: s.scroll_probe.clone(),
                find: super::find::bar(s, t),
                asks: s
                    .open_asks()
                    .iter()
                    .enumerate()
                    .map(|(n, ask)| super::permission::card(t, s, ask, n == 0))
                    .chain(super::pagepick::card(t, s))
                    .collect(),
            },
        ))
        .children(super::recap::recap_line(t, s.recap()))
        .child(super::composer::composer(s, t, session));
    let mut sheet = row()
        .key("workspace-sheet")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .min_h(0.0)
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .border_layout()
        .radius(12.0)
        .clip_rounded(12.0)
        .on_layout(|s: &mut State, [_, _, w, _]| s.sheet_w = w);
    for shadow in t.sheet_shadows() {
        sheet = sheet.box_shadow(shadow);
    }
    sheet = sheet.child(centre);
    // Too narrow for a third column (`rightPanelFits`): the panel lies over the conversation, on the right, closed until asked.
    if !rp::fits(s.sheet_w) {
        let over = super::canvas_host::host(s, t, true)
            .w(360.0_f32.min(s.sheet_w - 48.0))
            .fill(t.panel_bg)
            .box_shadow(Shadow {
                x: -12.0,
                y: 0.0,
                blur: 32.0,
                spread: -8.0,
                color: Color::BLACK.alpha(if t.dark { 0.55 } else { 0.18 }),
                inset: false,
            })
            .pin(None, Some(0.0), Some(0.0), Some(0.0));
        let sheet = sheet.child(collapsed_rail(s, t, session));
        return if s.content_open { sheet.child(over) } else { sheet };
    }
    if !s.content_open {
        return sheet.child(collapsed_rail(s, t, session));
    }
    sheet.child(right_rail::gutter(t)).child(super::canvas_host::host(s, t, false).w(s.right_shown_w()).min_w(rp::MIN_W))
}

/// The collapsed content panel (`CollapsedRightRail.vue`), counting the session's progress (`progress.done` of
/// `progress.total`). With no server there is no progress to read, so the invented session keeps its invented plan.
fn collapsed_rail(s: &State, t: &Theme, session: &Session) -> El {
    if let Some(p) = session.row.progress.as_ref() {
        return right_rail::rail(s, t, p.done as usize, p.total as usize);
    }
    let (steps, pending) = if s.open_id().is_none() { plan(session) } else { (Vec::new(), 0) };
    right_rail::rail(s, t, steps.len() - pending, steps.len())
}

/// The plan steps of a session (done?, step) and how many are left.
fn plan(session: &Session) -> (Vec<(bool, &'static str)>, usize) {
    let steps = if session.title == "Move the image cache to disk" {
        vec![
            (true, "Read how the cache is used"),
            (true, "Write the disk store"),
            (true, "Move the tests to a temporary folder"),
            (false, "Write files under a temporary name, then rename"),
            (false, "Ignore half-written files on start"),
            (false, "Run the cache tests 20 times"),
        ]
    } else {
        Vec::new()
    };
    let pending = steps.iter().filter(|(done, _)| !done).count();
    (steps, pending)
}
