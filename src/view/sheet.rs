//! The workspace sheet: the session header, the conversation, the composer, and the content
//! panel on the right (collapsed to a 48px rail by default).
use super::conversation::{Scroll, stream};
use super::parts::tool_button;
use crate::icons::*;
use crate::mock::{Project, Session, Status};
use crate::theme::{Theme, mono, sans};
use crate::{State, interrupt};
use rune::{Color, Cursor, El, Shadow, col, el, icon, row, spacer, text};

/// `AppShell.vue`: the content panel is a column only while the conversation keeps 480px beside
/// the panel at its narrowest (280px). In a narrower sheet it opens over the conversation.
const MIN_CONVERSATION: f32 = 480.0;
const CONTENT_W: f32 = 360.0;
const CONTENT_MIN: f32 = 280.0;

pub fn sheet(s: &State, t: &Theme) -> El {
    let session = s.session();
    let project = s.project();
    let centre = col()
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .min_h(0.0)
        .child(header(t, project, session, s.link.is_some()))
        .child(stream(
            t,
            session,
            &s.fold_plan,
            s.is_partial(),
            s.older(),
            Scroll { jump: s.pin.show_jump(), probe: s.scroll_probe.clone() },
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
    // Unmeasured (the first frame) counts as roomy, as it does in Fleet.
    let room = if s.sheet_w > 0.0 { s.sheet_w - MIN_CONVERSATION } else { f32::INFINITY };
    sheet = sheet.child(centre);
    if !s.content_open {
        sheet.child(content_rail(t, session))
    } else if room >= CONTENT_MIN {
        sheet.child(content(t, session).w(CONTENT_W.min(room)))
    } else {
        // Too narrow for a third column: the panel lies over the conversation, on the right.
        let over = content(t, session)
            .w(CONTENT_W.min(s.sheet_w - 48.0))
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
        sheet.child(content_rail(t, session)).child(over)
    }
}

/// `on_server`: the agent and model aren't known from the list yet, so none is invented.
fn header(t: &Theme, project: &Project, session: &Session, on_server: bool) -> El {
    let meta_font = sans(12.0, 400.0, 1.4);
    let path_font = mono(11.5, 400.0, 1.4);
    let dot = || text("·").font(meta_font).color(t.muted.alpha(0.6));
    let plain = |s: &str| text(s).font(meta_font).color(t.muted);
    // Cut off at the column's edge when the sheet is narrow (CSS `overflow: hidden`).
    let meta = row()
        .w_full()
        .min_w(0.0)
        .center_y()
        .gap(6.0)
        .clip_rounded(0.0)
        .child(plain(&project.name))
        .children((!on_server).then(dot))
        .children((!on_server).then(|| plain("OpenCode")))
        .children((!on_server).then(dot))
        .children((!on_server).then(|| plain("Sonnet 5.5")))
        .child(dot())
        .child(
            row()
                .center_y()
                .gap(4.0)
                .child(icon(&GIT_BRANCH, 12.0).color(t.muted))
                .child(text(session.branch.clone()).font(path_font).color(t.text)),
        )
        .child(dot())
        // The path is what gives way when the header runs out of room.
        .child(text(project.path.clone()).font(path_font).color(t.muted.alpha(0.8)).truncate().shrink())
        // Add a tag.
        .child(
            el().key("add-tag")
                .ml(4.0)
                .size(20.0, 20.0)
                .center()
                .radius(6.0)
                .border(1.0, t.muted.alpha(0.45))
                .border_dash(3.0, 2.0)
                .hover_group()
                .cursor(Cursor::Pointer)
                .help("Add a tag")
                .label("Add a tag")
                .on_press(|_: &mut State| {})
                .child(icon(&PLUS, 12.0).color(t.muted.alpha(0.7)).group_color(t.text)),
        );
    let main = col()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .gap(3.0)
        .child(text(session.title.clone()).font(sans(14.0, 600.0, 1.3).tracking(-0.07)).color(t.text).truncate())
        .child(meta);

    let mut actions = row().center_y().gap(2.0).ml(10.5).child(tool_button(t, "terminal", &SQUARE_TERMINAL, 14.0, "Terminal"));
    if session.status == Status::Working {
        actions = actions.child(tool_button(t, "abort", &OCTAGON_X, 14.0, "Abort").on_press(interrupt));
    }
    let actions = actions
        .child(el().size(1.0, 16.0).mx(2.0).fill(t.border))
        .child(tool_button(t, "fork", &GIT_FORK, 14.0, "Fork"))
        .child(
            tool_button(t, "more", &ELLIPSIS, 14.0, "More actions")
                .on_press(|s: &mut State| s.ui.open = Some(crate::ui::Open::Menu("more"))),
        )
        .child(tool_button(t, "analytics", &CHART_NO_AXES_COLUMN, 14.0, "Session analytics"));

    row()
        .key("session-header")
        .w_full()
        .min_h(56.0)
        .center_y()
        .px(17.5)
        .py(8.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(main)
        .child(actions)
}

/// The collapsed content panel (`CollapsedRightRail.vue`): the plan icon and how many steps
/// are still to do. A click opens the panel.
fn content_rail(t: &Theme, session: &Session) -> El {
    let (_, pending) = plan(session);
    col()
        .key("content-rail")
        .w(48.0)
        .h_full()
        .items_center()
        .pt(16.0)
        .border_sides([0.0, 0.0, 0.0, 1.0], t.border)
        .hover(t.wash(3.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        .help("Open the panel")
        .label("Open the panel")
        .on_press(|s: &mut State| s.content_open = true)
        .child(icon(&LIST_TODO, 14.0).color(t.muted).group_color(t.text))
        .child(text(pending.to_string()).font(sans(10.0, 700.0, 1.3)).color(t.muted).mt(3.0))
}

/// The open content panel, showing the session's plan. Fleet shows files, pages and more here;
/// the prototype only shows that the third column opens and closes.
fn content(t: &Theme, session: &Session) -> El {
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
    let head = row()
        .w_full()
        .min_h(56.0)
        .center_y()
        .gap(8.0)
        .pl(17.5)
        .pr(12.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(icon(&LIST_TODO, 14.0).color(t.muted))
        .child(text("Plan").font(sans(14.0, 600.0, 1.3)).color(t.text))
        .child(text(format!("{pending} to do")).font(sans(12.0, 400.0, 1.4)).color(t.muted))
        .child(spacer())
        .child(tool_button(t, "close-content", &CHEVRON_RIGHT, 14.0, "Close the panel").on_press(|s: &mut State| s.content_open = false));
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
