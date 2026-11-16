//! The workspace sheet: the session header, the conversation, the composer, and the content
//! panel on the right (collapsed to a 48px rail by default).
use super::conversation::{Scroll, stream};
use crate::State;
use crate::fleet::context_tab::PanelTab;
use crate::icons::*;
use crate::mock::Session;
use crate::theme::{Theme, sans};
use rune::{Color, Cursor, El, Shadow, col, icon, row, spacer, text};

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
        let over = content(s, t, session, true)
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
    sheet.child(right_rail::gutter(t)).child(content(s, t, session, false).w(s.right_shown_w()).min_w(rp::MIN_W))
}

/// The collapsed content panel (`CollapsedRightRail.vue`), counting the session's plan.
fn collapsed_rail(s: &State, t: &Theme, session: &Session) -> El {
    let (steps, pending) = plan(session);
    right_rail::rail(s, t, steps.len() - pending, steps.len())
}

/// The open content panel, showing the session's plan. Fleet shows files, pages and more here;
/// the prototype only shows that the third column opens and closes.
fn content(s: &State, t: &Theme, session: &Session, over: bool) -> El {
    if s.context.tab == PanelTab::Context {
        return col()
            .key("content")
            .h_full()
            .border_sides([0.0, 0.0, 0.0, 1.0], t.border)
            .child(panel_head(t, PanelTab::Context, None, over))
            .child(super::context_tab::context_tab(s, t));
    }
    let (steps, pending) = plan(session);
    let mut list = col().w_full().px(10.0).py(10.0).gap(2.0);
    for (done, step) in &steps {
        let (glyph, ink) = if *done { (&CIRCLE_CHECK, t.muted) } else { (&CIRCLE, t.text) };
        list = list.child(
            row()
                .w_full()
                .min_h(30.0)
                .center_y()
                .gap(9.0)
                .px(8.0)
                .radius(6.0)
                .hover(t.wash(5.0))
                .child(icon(glyph, 14.0).color(if *done { t.running } else { t.muted }))
                .child(text(*step).font(sans(13.0, 400.0, 1.3)).color(ink).truncate().grow(1.0)),
        );
    }
    if steps.is_empty() {
        list = list.child(text("This session has no plan.").font(sans(13.0, 400.0, 1.4)).color(t.muted).px(8.0).py(6.0));
    }
    let head = panel_head(t, PanelTab::Plan, Some(pending), over);
    col().key("content").h_full().border_sides([0.0, 0.0, 0.0, 1.0], t.border).child(head).child(list)
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

/// The strip at the top of the panel. Component 10 builds Fleet's real one (`CanvasHost`); until then it holds the two tabs
/// there are, and the button that closes the panel.
fn panel_head(t: &Theme, active: PanelTab, pending: Option<usize>, over: bool) -> El {
    let tab = |key: &'static str, glyph: &'static rune::Icon, label: &str, which: PanelTab| {
        let on = active == which;
        row()
            .key(key)
            .role("tab")
            .label(label.to_string())
            .gap(6.0)
            .center_y()
            .h_full()
            .px(2.0)
            .cursor(Cursor::Pointer)
            .border_sides([0.0, 0.0, 2.0, 0.0], if on { t.accent } else { Color::TRANSPARENT })
            .on_press(move |s: &mut State| s.set_panel_tab(which))
            .child(icon(glyph, 14.0).color(if on { t.text } else { t.muted }))
            .child(text(label.to_string()).font(sans(14.0, 600.0, 1.3)).color(if on { t.text } else { t.muted }))
    };
    let mut head = row()
        .w_full()
        .h(56.0)
        .center_y()
        .gap(14.0)
        .pl(17.5)
        .pr(12.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(tab("content-tab-plan", &LIST_TODO, "Plan", PanelTab::Plan))
        .child(tab("content-tab-context", &PAPERCLIP, "Context", PanelTab::Context));
    if let Some(pending) = pending {
        head = head.child(text(format!("{pending} to do")).font(sans(12.0, 400.0, 1.4)).color(t.muted));
    }
    head.child(spacer()).child(right_rail::collapse_button(t, over))
}
