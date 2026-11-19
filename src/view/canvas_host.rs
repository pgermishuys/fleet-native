//! Ports `components/canvas/CanvasHost.vue` (the header: the tab strip, the `+` button and its menu, Widen, the slot for the
//! collapse button; the body: the active tab's component) and the frame of `SessionsV2RightPanel.vue`. A tab whose body is
//! another component's draws a stand-in that names that component, until it is built.
use super::right_rail;
use crate::State;
use crate::fleet::canvas_host::{Badge, STRIP, close_key, pin_key, tab_key};
use crate::fleet::canvases::{CanvasInstance, CanvasKind, VisualType};
use crate::fleet::tab_motion::{PULSE_ICON_SHARE, bezier, ease_out};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, RADIUS_PANEL, Theme, sans};
use rune::{Cursor, El, Key, KeyEvent, Phase, col, el, icon, row, spacer, text};
use std::cell::Cell;

/// The `+` button's key, which the menu hangs from.
pub const ADD: &str = "canvas-add";

thread_local! {
    /// How far the strip is scrolled, as its scroll box last said; a drag starts from it.
    static STRIP_X: Cell<f32> = const { Cell::new(0.0) };
}

/// `useDragScroll`: the strip follows the pointer, so a tab pulled sideways moves the tabs under it.
fn drag_strip(d: &mut rune::Drag, tx: &mut crate::Tx) {
    match d.phase {
        Phase::Down => d.data = STRIP_X.with(Cell::get),
        Phase::Move if !d.cancelled() => tx.scroll_to_x(STRIP, d.data - d.dx),
        _ => {}
    }
}

/// The open panel: header, body, then the progress strip (hidden while Progress is the tab).
pub fn host(s: &State, t: &Theme, over: bool) -> El {
    col()
        .key("content")
        .h_full()
        .border_sides([0.0, 0.0, 0.0, 1.0], t.border)
        .child(header(s, t, over))
        .child(body(s, t))
        .children(super::progress_strip::slot(s, t))
}

fn header(s: &State, t: &Theme, over: bool) -> El {
    let tabs = s.open_canvases();
    let mut inner = row().gap(2.0).center_y().on_key(|s: &mut State, ev: &KeyEvent, tx| strip_key(s, ev, tx));
    for canvas in &tabs.canvases {
        inner = inner.child(tab(s, t, canvas, canvas.id == tabs.active_id));
    }
    // `useDragScroll`: the strip follows a drag; the offset it starts from is read off the scroll box as it moves.
    let strip = el()
        .key(STRIP)
        .role("tablist")
        .label("Canvases")
        .min_w(0.0)
        .shrink()
        .scroll_x()
        .scroll_edge_fade(20.0)
        .on_scroll_offset(|at| STRIP_X.with(|x| x.set(at.offset[0])))
        .on_drag_any(drag_strip)
        .child(inner);
    let mut head = row().w_full().min_h(56.0).center_y().gap(2.0).pl(10.0).pr(8.0).border_sides([0.0, 0.0, 1.0, 0.0], t.border);
    head = head.child(strip).child(
        icon_button(t, ADD, &PLUS, 15.0, "Open a canvas", s.ui.open == Some(crate::ui::Open::Menu("canvas")))
            .on_press(|s: &mut State| s.ui.open = Some(crate::ui::Open::Menu("canvas"))),
    );
    head = head.child(spacer().min_w(8.0));
    if !over {
        let (glyph, tip) = if s.right.widened { (&MINIMIZE_2, "Narrow canvas") } else { (&MAXIMIZE_2, "Widen canvas") };
        head = head
            .child(icon_button(t, "canvas-widen", glyph, 14.0, tip, s.right.widened).on_press_tx(|s: &mut State, tx| s.toggle_widened(tx)));
    }
    head.child(right_rail::collapse_button(t, over))
}

/// `canvas-host__icon-btn`: 28 square, tinted while its menu is open or it is pressed in.
fn icon_button(t: &Theme, key: &str, glyph: &'static rune::Icon, size: f32, tip: &str, on: bool) -> El {
    let mut b = el()
        .key(key)
        .role("button")
        .size(28.0, 28.0)
        .min_w(28.0)
        .center()
        .radius(RADIUS_BTN)
        .hover(t.wash(6.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, -2.0, t.accent)
        .help(tip)
        .label(tip);
    if on {
        b = b.fill(t.wash(6.0));
    }
    b.child(icon(glyph, size).color(if on { t.text } else { t.muted }).group_color(t.text))
}

/// A tab (`.canvas-tab`): the kind's icon, the title, a badge, and a close button that shows on hover and on the active tab.
fn tab(s: &State, t: &Theme, canvas: &CanvasInstance, active: bool) -> El {
    let id = canvas.id.clone();
    let title = canvas.title();
    let ink = if active { t.text } else { t.muted };
    let weight = if active { 500.0 } else { 400.0 };
    let mut tab = row()
        .key(tab_key(&id))
        .role("tab")
        .label(title.clone())
        .help(canvas.file.as_ref().map_or_else(|| title.clone(), |f| f.path.clone()))
        .gap(6.0)
        .center_y()
        .h(28.0)
        .min_h(28.0)
        .max_w(200.0)
        .px(9.0)
        .radius(RADIUS_BTN)
        .cursor(Cursor::Pointer)
        .hover_group()
        .focus_ring(2.0, -2.0, t.accent)
        .focusable(true)
        .on_drag_any(drag_strip)
        .tab_index(if active { 0 } else { -1 });
    tab = if active { tab.fill(t.wash(9.0)) } else { tab.hover(t.wash(5.0)) };
    // A page that updated itself: `--accent` at 22% easing out to nothing, the icon in `--accent-text` for the first 40%.
    let mut icon_ink = ink;
    if let Some(p) = s.tab_pulse(&canvas.id) {
        tab = tab.fill(t.accent.alpha(0.22 * (1.0 - ease_out(p))));
        if p < PULSE_ICON_SHARE {
            icon_ink = t.accent_text;
        }
    }
    // A tab that just arrived: 240 ms from opacity 0, 8 px to the right and 96% size.
    if let Some(p) = s.tab_enter(&canvas.id) {
        let e = bezier(0.2, 0.8, 0.2, 1.0, p);
        tab = tab.opacity(e).offset(8.0 * (1.0 - e), 0.0).scale(0.96 + 0.04 * e);
    }
    // A preview tab's name is italic: the next file you open takes its place.
    let preview = name_italic(canvas);
    let font = if preview { sans(13.0, weight, 1.3).italic() } else { sans(13.0, weight, 1.3) };
    tab = tab
        .child(icon(canvas.icon(), 14.0).color(icon_ink).group_color(t.text))
        .child(text(title).font(font).color(ink).group_color(t.text).truncate().min_w(0.0));
    if let Some(badge) = badge(s, t, canvas) {
        tab = tab.child(badge);
    }
    if let Some(file) = &canvas.file {
        tab = tab.child(pin_button(t, &id, &canvas.title(), file.preview, active));
    }
    if canvas.closable() {
        tab = tab.child(close_button(t, &id, &canvas.title(), active));
    }
    let (press, middle) = (id.clone(), id);
    let keep = press.clone();
    let tab = tab.on_press_tx(move |s: &mut State, tx| s.activate_tab(&press, tx));
    let tab = if canvas.file.is_some() { tab.on_double_press(move |s: &mut State| s.keep_preview_tab(&keep)) } else { tab };
    if canvas.closable() { tab.on_middle_press(move |s: &mut State, tx| s.close_tab(&middle, tx)) } else { tab }
}

/// A preview tab's name is italic (`.canvas-tab--preview .canvas-tab__label`).
pub fn name_italic(canvas: &CanvasInstance) -> bool {
    canvas.file.as_ref().is_some_and(|f| f.preview)
}

/// The badge: Changes' count (once the diffs are read), else an attention dot, else a count.
fn badge(s: &State, t: &Theme, canvas: &CanvasInstance) -> Option<El> {
    let counted = |n: String| text(n).font(sans(12.0, 500.0, 1.3)).color(t.muted).group_color(t.text);
    if canvas.kind == CanvasKind::Changes {
        let n = s.diffs.files().len();
        return (n > 0).then(|| counted(n.to_string()).key("canvas-tab-changes-count"));
    }
    Some(match s.tab_badge(&canvas.id)? {
        Badge::Attention(label) => el().role("img").label(label).size(7.0, 7.0).min_w(7.0).radius(3.5).fill(t.error),
        Badge::Count(n, label) => counted(n).label(label),
    })
}

/// `canvas-tab__pin`: like the close button (16 square, hidden until the tab is hovered or active), filled while the tab is kept.
fn pin_button(t: &Theme, id: &str, title: &str, preview: bool, active: bool) -> El {
    let id = id.to_string();
    let (verb, tip) = if preview {
        ("Pin", "Pin: keep this tab open when you open another file")
    } else {
        ("Unpin", "Unpin: the next file you open replaces this tab")
    };
    el().key(pin_key(&id))
        .role("button")
        .label(format!("{verb} {title}"))
        .help(tip)
        .size(16.0, 16.0)
        .min_w(16.0)
        .mr(-4.0)
        .center()
        .radius(4.0)
        .cursor(Cursor::Pointer)
        .hover(t.wash(10.0))
        .hover_group()
        .group_opacity(if active { 1.0 } else { 0.0 }, 1.0)
        .on_press_tx(move |s: &mut State, tx| s.toggle_file_pin(&id, tx))
        .child(icon(if preview { &PIN } else { &PIN_FILLED }, 12.0).color(t.muted).group_color(t.text))
}

fn close_button(t: &Theme, id: &str, title: &str, active: bool) -> El {
    let id = id.to_string();
    el().key(close_key(&id))
        .role("button")
        .label(format!("Close {title}"))
        .size(16.0, 16.0)
        .min_w(16.0)
        .mr(-4.0)
        .center()
        .radius(4.0)
        .cursor(Cursor::Pointer)
        .hover(t.wash(10.0))
        .hover_group()
        .group_opacity(if active { 1.0 } else { 0.0 }, 1.0)
        .on_press_tx(move |s: &mut State, tx| s.close_tab(&id, tx))
        .child(icon(&X, 12.0).color(t.muted).group_color(t.text))
}

/// `onTabKeydown`: the arrows move to the next or previous tab and wrap, Home and End go to the ends, Delete closes the
/// active tab if it can be closed. The tab reached is activated and focused.
fn strip_key(s: &mut State, ev: &KeyEvent, tx: &mut crate::Tx) -> bool {
    if ev.mods.cmd || ev.mods.alt || ev.mods.ctrl {
        return false;
    }
    let target = match ev.key {
        Key::Right => s.neighbour_tab(1),
        Key::Left => s.neighbour_tab(-1),
        Key::Home => s.edge_tab(false),
        Key::End => s.edge_tab(true),
        Key::Delete => {
            let tabs = s.open_canvases();
            let Some(active) = tabs.active().filter(|c| c.closable()) else { return true };
            s.close_tab(&active.id.clone(), tx);
            tx.focus(tab_key(&s.active_canvas()), true);
            return true;
        }
        _ => return false,
    };
    if let Some(id) = target {
        s.activate_tab(&id, tx);
        tx.focus(tab_key(&id), true);
    }
    true
}

/// The active tab's body (`canvas-host__body`, `role=tabpanel`). A kind another component builds draws its stand-in.
fn body(s: &State, t: &Theme) -> El {
    let tabs = s.open_canvases();
    let inner = match tabs.active().map(|c| c.kind) {
        Some(CanvasKind::Context) => super::context_tab::context_tab(s, t),
        Some(CanvasKind::Progress) => super::progress_tab::progress_tab(s, t),
        Some(CanvasKind::Changes) => super::changes::changes_body(s, t),
        Some(CanvasKind::Files) => super::files_tab::files_tab(s, t),
        Some(CanvasKind::File) => super::file_tab::file_tab(s, t),
        Some(CanvasKind::Agents) => super::agents::agents_tab(s, t),
        Some(CanvasKind::Turns) => super::turns::turns_tab(s, t),
        // Component 14's page tab, from the server canvas the tab carries.
        Some(CanvasKind::Page) => match crate::web::wanted::page_tab(s) {
            Some((tab, page)) => super::page_tab::page_tab(s, t, &tab, &page),
            None => stand_in(t, CanvasKind::Page),
        },
        // A browser canvas hosts Agent's view itself (piece J of component 14); a scene may still watch a tab with none.
        Some(CanvasKind::Browser) => match (&s.agent_view.target, s.web.agent_owned) {
            (Some((session, tab)), false) => super::agent_tab::agent_tab(s, t, session, tab),
            _ => match tabs.active().and_then(|c| Some((c.id.clone(), c.browser()?))) {
                Some((tab, page)) => super::browser_tab::browser_tab(s, t, &tab, &page),
                None => stand_in(t, CanvasKind::Browser),
            },
        },
        Some(CanvasKind::Visual) => match tabs.active().and_then(|c| c.payload.as_ref()) {
            Some(payload) => super::document_tab::document_tab(s, t, payload),
            None => stand_in(t, CanvasKind::Visual),
        },
        Some(kind) => stand_in(t, kind),
        None => el(),
    };
    col().key("canvas-body").role("tabpanel").w_full().grow(1.0).min_h(0.0).child(inner)
}

/// What a tab that isn't built yet says, in the muted 13 px, centred; Fleet has no such state.
fn stand_in(t: &Theme, kind: CanvasKind) -> El {
    let owner = match kind {
        CanvasKind::Changes => "Component 12",
        CanvasKind::Files | CanvasKind::File => "Component 11",
        CanvasKind::Progress | CanvasKind::Turns | CanvasKind::Agents | CanvasKind::Context => "Component 10",
        CanvasKind::Browser | CanvasKind::Page | CanvasKind::Visual => "Component 14",
        CanvasKind::Mod => "Component 22",
    };
    col().key("canvas-stand-in").w_full().h_full().center().child(
        text(format!("{} is drawn by {}.", kind.label(), owner.to_lowercase().replacen('c', "C", 1)))
            .font(sans(13.0, 400.0, 1.4))
            .color(t.muted),
    )
}

/// The `+` menu (`DropdownMenu`, end-aligned under the button, `w-80`, radius `--radius-panel`, padding 6).
pub fn picker(s: &State, t: &Theme) -> Vec<El> {
    let mut list = col().key("rows").items_stretch().child(group_label(t, "Built in"));
    for (kind, open) in s.picker_rows() {
        let hint = open.then_some("Open");
        list = list.child(picker_row(
            t,
            format!("canvas-picker-{}", kind.slug()),
            kind.icon(),
            kind.label(),
            hint,
            move |s: &mut State, tx| s.open_built_in(kind, tx),
        ));
    }
    list = list.child(picker_row(t, "canvas-picker-browser".into(), &GLOBE, "Browser\u{2026}", Some("Run or open a page"), |s, tx| {
        s.open_browser_dialog(tx)
    }));
    let visuals = s.conversation_visuals();
    if !visuals.is_empty() {
        list = list.child(crate::ui::controls::separator(t).my(4.0).mx(2.0)).child(group_label(t, "From this conversation"));
        for (at, (payload, title, open)) in visuals.into_iter().enumerate() {
            let glyph = if payload.kind == VisualType::Html { &GLOBE } else { &FILE_TEXT };
            list = list.child(picker_row(t, format!("canvas-picker-visual-{at}"), glyph, &title, open.then_some("Open"), move |s, tx| {
                s.open_conversation_visual(payload.clone(), tx)
            }));
        }
    }
    let panel = col()
        .key("canvas-picker-menu")
        .role("menu")
        .focus_group()
        .w(280.0)
        .padding(6.0)
        .radius(RADIUS_PANEL)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .items_stretch()
        .child(list)
        .focus_return(ADD.to_string())
        .modal(true, |s: &mut State| s.ui.open = None);
    let panel = t.menu_shadows().into_iter().fold(panel, |p, shadow| p.box_shadow(shadow));
    vec![
        crate::ui::overlay::scrim("canvas-picker-scrim", |s: &mut State| s.ui.open = None),
        crate::ui::overlay::anchored(panel, ADD, rune::Side::Bottom, rune::Align::End, 4.0),
    ]
}

/// `canvas-picker__group` on the menu's label (`menuLabelClass`: 11 px, 600, uppercase, tracked).
fn group_label(t: &Theme, label: &str) -> El {
    row().px(8.0).pt(8.0).pb(4.0).child(text(label.to_uppercase()).font(sans(11.0, 600.0, 1.2).tracking(0.55)).color(t.muted))
}

/// `canvas-picker__item`: 34 high, the icon, the label, and a muted hint at the right.
fn picker_row(
    t: &Theme,
    key: String,
    glyph: &'static rune::Icon,
    label: &str,
    hint: Option<&str>,
    run: impl Fn(&mut State, &mut crate::Tx) + 'static,
) -> El {
    let mut r = row()
        .key(key)
        .role("menuitem")
        .label(label.to_string())
        .h(34.0)
        .px(8.0)
        .gap(8.0)
        .center_y()
        .radius(RADIUS_BTN)
        .cursor(Cursor::Default)
        .hover_group()
        .focusable(true)
        .focus_highlight(t.wash(7.0))
        .hover(t.wash(7.0))
        .child(icon(glyph, 14.0).color(t.muted).group_color(t.text))
        .child(text(label.to_string()).font(sans(13.0, 400.0, 1.3)).color(t.text).truncate().min_w(0.0));
    r = r.child(spacer());
    if let Some(hint) = hint {
        r = r.child(text(hint.to_string()).font(sans(11.5, 400.0, 1.3)).color(t.muted).pl(12.0));
    }
    r.on_press_tx(move |s: &mut State, tx| {
        s.ui.open = None;
        run(s, tx);
    })
}
