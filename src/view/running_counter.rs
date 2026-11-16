//! The status bar's count of work left running in the background, and the list it opens (ports `RunningWorkCounter.vue`;
//! docs/parity/09-app-frame.md, Part 5, piece H). What the numbers are is `fleet::list_work`; each row is the background
//! strip's own (`view::work::work_row`), so Output, Open, Details, Events and Stop work here as they do there.
use super::message::now_ms;
use super::work::work_row;
use crate::State;
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::Open;
use crate::ui::overlay::{anchored, scrim};
use rune::{Align, Cursor, El, ScrollSpec, Side, col, el, row, text};

/// The chip the popover is placed over.
pub const TRIGGER: &str = "running-work-counter";
const NAME: &str = "running";
const WIDTH: f32 = 460.0;
/// `max-height: min(480px, 70vh)`.
const MAX_H: f32 = 480.0;

fn is_open(s: &State) -> bool {
    s.ui.open == Some(Open::Popover(NAME))
}

fn close(s: &mut State) {
    s.ui.open = None;
}

impl State {
    /// Opens the list, or closes it. Fleet's popover shows only while something runs, so with nothing running it stays shut.
    pub fn toggle_running_popover(&mut self) {
        self.ui.open = if is_open(self) || self.list_work.is_empty() { None } else { Some(Open::Popover(NAME)) };
    }

    /// Opens a session from its heading in the list, and closes the list (`goTo`). A session this list doesn't hold opens nothing.
    pub fn open_running_session(&mut self, id: &str, tx: &mut rune::Tx) {
        self.ui.open = None;
        if let Some((p, i)) = self.row_of(id) {
            self.open_session(p, i, tx);
        }
    }
}

/// The chip, or nothing while nothing runs (no loading and no error state).
pub fn chip(s: &State, t: &Theme) -> Option<El> {
    if s.list_work.is_empty() {
        return None;
    }
    let label = s.list_work.label();
    let lit = t.running.alpha(0.24);
    let mut b = row()
        .key(TRIGGER)
        .role("button")
        .label(format!("{label}: show what runs in the background"))
        .help(format!("{label}: show what runs in the background"))
        .h(18.0)
        .center_y()
        .gap(6.0)
        .pl(7.0)
        .pr(8.0)
        .radius(999.0)
        .fill(if is_open(s) { lit } else { t.running.alpha(0.14) })
        .hover(lit)
        .cursor(Cursor::Pointer)
        .on_press(|s: &mut State| s.toggle_running_popover())
        .child(dot(t));
    b = b.child(text(label).key("running-work-label").font(sans(11.0, 500.0, 1.3).features("'tnum' 1")).color(t.running_text));
    Some(b)
}

fn dot(t: &Theme) -> El {
    el().size(6.0, 6.0).radius(3.0).fill(t.running)
}

/// The scrim and the card (`side="top" align="end" :side-offset="6"`).
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let (w, h) = s.ui.sel.window;
    let h = if h > 0.0 { h } else { 760.0 };
    let width = if w > 0.0 { WIDTH.min(w - 16.0) } else { WIDTH };
    let (items, now) = (s.list_work.is_empty(), now_ms(s));
    let mut list = col().key("running-work-list").w_full().items_stretch().gap(6.0).padding(6.0);
    if !items {
        for g in s.placed_work_groups() {
            list = list.child(group(s, t, &g, now));
        }
    }
    let card = col()
        .key("running-work-popover")
        .role("dialog")
        .label("Running in the background")
        .w(width)
        .items_stretch()
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .border_layout()
        .radius(RADIUS_CARD)
        .clip_rounded(RADIUS_CARD)
        .child(col().w_full().max_h(MAX_H.min(h * 0.7) - 2.0).min_h(0.0).child(list).scroll(ScrollSpec { stick_bottom: false }));
    let card =
        t.menu_shadows().into_iter().fold(card, |e, shadow| e.box_shadow(shadow)).focus_return(TRIGGER.to_string()).modal(true, close);
    vec![scrim("running-work-scrim", close), anchored(card, TRIGGER, Side::Top, Align::End, 6.0)]
}

/// One session's heading and its rows (`.running-popover__group`).
fn group(s: &State, t: &Theme, g: &crate::fleet::list_work::PlacedGroup, now: i64) -> El {
    let id = g.group.session_id.clone();
    let mut head = row()
        .key(format!("running-work-session-{id}"))
        .role("link")
        .label(format!("Open {}", g.title))
        .w_full()
        .center_y()
        .gap(8.0)
        .min_w(0.0)
        .pl(8.0)
        .pr(8.0)
        .pt(6.0)
        .pb(4.0)
        .radius(7.0)
        .hover(t.wash(5.0))
        .cursor(Cursor::Pointer)
        .child(dot(t))
        .child(
            text(g.title.clone())
                .key(format!("running-work-title-{id}"))
                .font(sans(12.5, 600.0, 1.3))
                .color(t.text)
                .truncate()
                .shrink()
                .min_w(0.0),
        );
    if let Some(place) = &g.place {
        head =
            head.child(text(format!("\u{b7} {place}")).key(format!("running-work-place-{id}")).font(sans(12.5, 400.0, 1.3)).color(t.muted));
    }
    let head = head.on_press_tx(move |s: &mut State, tx: &mut rune::Tx| s.open_running_session(&id, tx));
    let rows = g.group.items.iter().map(|item| work_row(s, t, item, now, false));
    col()
        .key(format!("running-work-group-{}", g.group.session_id))
        .w_full()
        .items_stretch()
        .child(head)
        .child(col().w_full().gap(2.0).children(rows))
}
