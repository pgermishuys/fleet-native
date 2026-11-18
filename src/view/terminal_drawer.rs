//! Ports `components/terminal/TerminalDrawer.vue` (component 13, piece D): the section under the composer that spans the
//! conversation column: the header (tabs, New terminal, the folder and size, Clear, Hide) and the body (the active tab's
//! view, "Starting a shell…", an error with Try again). The grip and the tab keys are piece E's; the view is `terminal_view`.
use rune::{Cursor, El, Tx, col, el, icon, row, spacer, text};

use super::terminal_view;
use crate::State;
use crate::fleet::terminal_slot::VIEW_KEY;
use crate::fleet::terminals::MIN_DRAWER_HEIGHT;
use crate::icons::{CHEVRON_DOWN, ERASER, PLUS, SQUARE_TERMINAL, X};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};

/// `MAX_HEIGHT_SHARE`.
const MAX_HEIGHT_SHARE: f32 = 0.6;

pub const STRIP: &str = "terminal-tabs";

pub fn tab_key(id: &str) -> String {
    format!("terminal-tab-{id}")
}

pub fn close_key(id: &str) -> String {
    format!("terminal-tab-close-{id}")
}

/// The height the drawer is drawn at: the remembered one, kept under 60 % of the column it sits in and never under 140
/// (`maxHeight`). Before the column is measured there is no cap. A scene's own height is taken as it is.
pub fn height(s: &State) -> f32 {
    if s.terminal.height > 0.0 {
        return s.terminal.height;
    }
    let wanted = s.terminals.height() as f32;
    if s.sheet_h <= 0.0 { wanted } else { wanted.min((s.sheet_h * MAX_HEIGHT_SHARE).round().max(MIN_DRAWER_HEIGHT as f32)) }
}

/// The drawer, or nothing while it is hidden or was never opened in this session (13.43).
pub fn drawer(s: &State, t: &Theme) -> Option<El> {
    if !s.terminal_open() || s.terminal.session != s.open_id() {
        return None;
    }
    let focused = s.terminal.focused;
    Some(
        col()
            .key("terminal-drawer")
            .role("region")
            .label("Terminal")
            .h(height(s))
            .min_h(0.0)
            .fill(t.terminal_bg())
            // The rule above: `--border`, or `--accent` at 60 % while a terminal has the keyboard.
            .child(el().h(1.0).fill(if focused { t.accent.alpha(0.6) } else { t.border }))
            .child(header(s, t))
            .child(body(s, t)),
    )
}

/// `.terminal-drawer__icon-btn`: 26 × 26, the glyph in `--muted`, a `--text` wash behind `--text` on hover, 40 % while off.
fn icon_button(t: &Theme, key: &str, glyph: &'static rune::Icon, size: f32, tip: &str, off: bool) -> El {
    let b = el().key(key).role("button").size(26.0, 26.0).min_w(26.0).center().radius(RADIUS_BTN).help(tip).label(tip);
    if off {
        return b.opacity(0.4).child(icon(glyph, size).color(t.muted));
    }
    b.hover(t.wash(6.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, -2.0, t.accent)
        .child(icon(glyph, size).color(t.muted).group_color(t.text))
}

fn header(s: &State, t: &Theme) -> El {
    let tabs = s.drawer_tabs();
    let active = s.drawer_active();
    let mut inner = row().gap(2.0).center_y();
    for (id, title) in &tabs {
        inner = inner.child(tab(t, id, title, active.as_deref() == Some(id)));
    }
    // `.terminal-tabs`: scrolls sideways, no scrollbar.
    let strip = el().key(STRIP).role("tablist").label("Terminals").min_w(0.0).shrink().scroll_x().child(inner);
    let new = icon_button(t, "terminal-new", &PLUS, 14.0, "New terminal", s.terminal.starting).label("New terminal");
    let new = if s.terminal.starting { new } else { new.on_press_tx(|s: &mut State, tx: &mut Tx| new_terminal(s, tx)) };
    let clear = icon_button(t, "terminal-clear", &ERASER, 13.0, "Clear", active.is_none());
    let clear = if active.is_some() { clear.on_press(|s: &mut State| s.clear_terminal()) } else { clear };
    let hide = icon_button(t, "terminal-hide", &CHEVRON_DOWN, 15.0, "Hide terminal (Ctrl J). Shells keep running.", false)
        .on_press(|s: &mut State| s.hide_terminal());
    row()
        .h(36.0)
        .min_h(36.0)
        .center_y()
        .gap(2.0)
        .pl(8.0)
        .pr(6.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(strip)
        .child(new)
        .child(spacer().min_w(8.0))
        .child(meta(s, t))
        .child(clear)
        .child(hide)
}

/// `.terminal-tab`: 26 px, the icon at 70 %, the title (at most 160), a close button shown on hover and on the active tab.
fn tab(t: &Theme, id: &str, title: &str, active: bool) -> El {
    let ink = if active { t.text } else { t.muted };
    let close = {
        let id = id.to_string();
        el().key(close_key(&id))
            .role("button")
            .label(format!("Close {title}"))
            .help("Close (ends the shell)")
            .size(18.0, 18.0)
            .min_w(18.0)
            .center()
            .radius(4.0)
            .cursor(Cursor::Pointer)
            .hover(t.wash(10.0))
            .hover_group()
            .group_opacity(if active { 1.0 } else { 0.0 }, 1.0)
            .on_press(move |s: &mut State| s.close_terminal_tab(&id))
            .child(icon(&X, 11.0).color(t.muted).group_color(t.text))
    };
    let (press, middle) = (id.to_string(), id.to_string());
    let tab = row()
        .key(tab_key(id))
        .role("tab")
        .label(title)
        .gap(6.0)
        .center_y()
        .h(26.0)
        .min_h(26.0)
        .pl(8.0)
        .pr(4.0)
        .radius(RADIUS_BTN)
        .cursor(Cursor::Pointer)
        .hover_group()
        .focus_ring(2.0, -2.0, t.accent)
        .focusable(true)
        .tab_index(if active { 0 } else { -1 })
        .child(icon(&SQUARE_TERMINAL, 13.0).color(ink.alpha(0.7)).group_color(t.text.alpha(0.7)))
        .child(
            text(title)
                .font(sans(12.5, if active { 500.0 } else { 400.0 }, 1.3))
                .color(ink)
                .group_color(t.text)
                .truncate()
                .max_w(160.0)
                .min_w(0.0),
        )
        .child(close);
    let tab = if active { tab.fill(t.wash(9.0)) } else { tab.hover(t.wash(5.0)) };
    tab.on_press_tx(move |s: &mut State, tx: &mut Tx| s.show_terminal_tab(&press, tx))
        .on_middle_press(move |s: &mut State, _| s.close_terminal_tab(&middle))
}

/// `.terminal-drawer__meta`: the session's folder (cut at its start, 80 %) and the active size "70×11" (tabular, 80 %).
fn meta(s: &State, t: &Theme) -> El {
    let mut meta = row().gap(10.0).center_y().min_w(0.0).pr(6.0);
    let dir = s.session().listing.workspace_directory.clone();
    if !dir.is_empty() {
        meta = meta.child(
            text(dir.clone())
                .key("terminal-cwd")
                .font(mono(11.0, 400.0, 1.4))
                .color(t.muted.alpha(0.8))
                .truncate_head()
                .max_w(260.0)
                .min_w(0.0)
                .help(dir),
        );
    }
    if let Some((cols, rows)) = s.terminal.live.as_ref().map(|l| l.term.size()) {
        meta = meta.child(
            text(format!("{cols}\u{d7}{rows}"))
                .key("terminal-size")
                .font(mono(11.0, 400.0, 1.4))
                .color(t.muted.alpha(0.8))
                .help(format!("{cols} columns \u{d7} {rows} rows")),
        );
    }
    meta
}

/// `.terminal-drawer__body`: the active view, and over it the error or "Starting a shell…".
fn body(s: &State, t: &Theme) -> El {
    let mut body = col().key("terminal-body").grow(1.0).min_h(0.0).min_w(0.0).child(terminal_view::area(s, t));
    if let Some(why) = &s.terminal.error {
        let retry = el()
            .key("terminal-retry")
            .role("button")
            .label("Try again")
            .h(28.0)
            .px(12.0)
            .center()
            .radius(RADIUS_BTN)
            .border(1.0, t.border)
            .border_layout()
            .fill(t.panel_bg)
            .hover(t.mix(t.panel_bg, t.text, 0.95))
            .cursor(Cursor::Pointer)
            .focus_ring(2.0, 2.0, t.accent)
            .on_press_tx(|s: &mut State, tx: &mut Tx| new_terminal(s, tx))
            .child(text("Try again").font(sans(12.5, 400.0, 1.4)).color(t.text));
        body = body.child(
            col()
                .key("terminal-message")
                .role("alert")
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .center()
                .gap(10.0)
                .px(16.0)
                .py(16.0)
                .fill(t.terminal_bg())
                .child(text(why.clone()).font(sans(12.5, 400.0, 1.4)).color(t.text).max_w(52.0 * 6.6).align(rune::Align::Center).wrap())
                .child(retry),
        );
    } else if s.terminal.starting && s.drawer_tabs().is_empty() {
        body = body.child(
            col()
                .key("terminal-message")
                .role("status")
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .center()
                .child(text("Starting a shell\u{2026}").font(sans(12.5, 400.0, 1.4)).color(t.muted)),
        );
    }
    body
}

/// `newTerminal`: asks for another shell and gives the box the keyboard when it shows.
fn new_terminal(s: &mut State, tx: &mut Tx) {
    s.open_new_terminal();
    tx.focus(VIEW_KEY, false);
}
