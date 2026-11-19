//! `dialog/`, `alert-dialog/`, `responsive-dialog/` (desktop form) and `sheet/`, plus the
//! popover (`popover/`). All are modal-ish layers: a scrim, then the panel.
use super::overlay::{anchored, centred, scrim};
use super::rem;
use crate::State;
use crate::theme::{Theme, sans};
use rune::{Align, Color, Cursor, El, Side, Spring, Transition, Vec2, col, el, icon, row, text};

pub(crate) fn lift(t: &Theme, mut e: El) -> El {
    for s in t.menu_shadows() {
        e = e.box_shadow(s);
    }
    e
}

/// `DialogOverlay`: `bg-black/80`, fading in.
pub(crate) fn overlay(key: &str) -> El {
    el().key(format!("{key}-overlay")).role("none").absolute(0.0, 0.0).w_full().h_full().fill(Color::BLACK.alpha(0.8)).a11y_hidden()
}

#[derive(Default)]
pub struct Dialog {
    pub key: &'static str,
    pub title: String,
    /// A title with parts in other fonts (a name in mono); drawn in place of `title`, which stays the dialog's label.
    pub title_el: Option<El>,
    pub description: Option<String>,
    pub body: Vec<El>,
    pub footer: Vec<El>,
    /// `DialogContent`'s close button; an alert has none.
    pub close_button: bool,
    /// Pressing the overlay closes a dialog, never an alert.
    pub click_outside: bool,
    /// `AlertTriangle` before the title and its colour (`ConfirmDeleteProjectDialog`).
    pub title_icon: Option<(&'static rune::Icon, Color)>,
    /// A description with parts in other weights; used in place of `description`.
    pub description_el: Option<El>,
    /// What Escape does; `None` closes the open overlay (`Ui::open`). A dialog with a state of its own names it.
    pub dismiss: Option<fn(&mut State)>,
    /// The window's width when it is not the default `sm:max-w-lg` (`sm:max-w-md` is 28rem).
    pub width: Option<f32>,
    /// `max-h-[90vh] overflow-y-auto`: the panel is no taller than this and scrolls inside.
    pub max_height: Option<f32>,
}

/// `DialogContent`: `max-w-lg` (448), `p-6`, `gap-4`, a border and `shadow-lg`, no radius.
pub fn dialog(t: &Theme, d: Dialog) -> Vec<El> {
    let dismiss = d.dismiss;
    let close = move |s: &mut State| match dismiss {
        Some(f) => f(s),
        None => s.ui.open = None,
    };
    let title = d.title_el.unwrap_or_else(|| text(d.title.clone()).font(sans(rem(1.125), 600.0, 1.0)).color(t.text));
    let title = match d.title_icon {
        Some((glyph, ink)) => row().gap(rem(0.5)).center_y().child(icon(glyph, rem(1.0)).color(ink)).child(title),
        None => title,
    };
    let mut head = col().gap(rem(0.5)).child(title);
    if let Some(desc) = d.description_el {
        head = head.child(desc);
    } else if let Some(desc) = d.description {
        head = head.child(text(desc).font(sans(rem(0.875), 400.0, 1.4)).color(t.muted).wrap().w_full());
    }
    let mut panel = col()
        .key(format!("{}-dialog", d.key))
        .role(if d.close_button { "dialog" } else { "alertdialog" })
        .label(d.title)
        .w(d.width.unwrap_or(rem(32.0)))
        .max_w(d.width.unwrap_or(rem(32.0)))
        .padding(rem(1.5))
        .gap(rem(1.0))
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .child(head)
        .children(d.body);
    if !d.footer.is_empty() {
        panel = panel.child(row().w_full().justify_end().gap(rem(0.5)).children(d.footer));
    }
    if d.close_button {
        panel = panel.child(close_x(t, d.key, close));
    }
    if let Some(h) = d.max_height {
        panel = panel.max_h(h).scroll(rune::ScrollSpec { stick_bottom: false });
    }
    let panel = lift(t, panel)
        .transition(Transition::scale_anchor(0.95, Vec2::new(0.5, 0.5)))
        .token_motion(Spring::new(0.2, 0.0))
        .modal(d.click_outside, close);
    vec![overlay(d.key), centred(&format!("{}-frame", d.key), panel)]
}

/// `DialogContent`'s close button, at the top right: 16px, 70% opaque until hovered.
pub(crate) fn close_x(t: &Theme, key: &str, on_press: impl Fn(&mut State) + 'static) -> El {
    el().key(format!("{key}-close"))
        .role("button")
        .label("Close")
        .pin(None, Some(rem(1.0)), Some(rem(1.0)), None)
        .size(rem(1.0), rem(1.0))
        .center()
        .cursor(Cursor::Pointer)
        .opacity(0.7)
        .hover(Color::TRANSPARENT)
        .focus_ring(2.0, 2.0, t.accent)
        .on_press(on_press)
        .child(icon(&crate::icons::X, rem(1.0)).color(t.text))
}

/// `AlertDialogCancel` and `AlertDialogAction`: an outline and a default button.
pub fn footer_button(t: &Theme, label: &str, primary: bool) -> El {
    use super::button::{Size, Variant, button};
    button(t, if primary { Variant::Default } else { Variant::Outline }, Size::Default, None, label)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SheetSide {
    Right,
    Left,
    Bottom,
    Top,
}

/// `SheetContent`: a side panel (`w-3/4 sm:max-w-sm`, 336px at most) or a bar, `shadow-lg`,
/// sliding in.
pub fn sheet(t: &Theme, key: &'static str, side: SheetSide, title: &str, description: &str, body: Vec<El>) -> Vec<El> {
    let close = |s: &mut State| s.ui.open = None;
    let mut panel = col()
        .key(format!("{key}-sheet"))
        .role("dialog")
        .label(title.to_string())
        .padding(rem(1.0))
        .gap(rem(1.0))
        .fill(t.panel_bg)
        .child(
            col()
                .gap(rem(0.375))
                .child(text(title.to_string()).font(sans(rem(1.0), 600.0, 1.2)).color(t.text))
                .child(text(description.to_string()).font(sans(rem(0.875), 400.0, 1.4)).color(t.muted)),
        )
        .children(body);
    panel = match side {
        SheetSide::Right | SheetSide::Left => panel
            .w(rem(24.0))
            .h_full()
            .border_sides(if side == SheetSide::Right { [0.0, 0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0, 0.0] }, t.border),
        SheetSide::Top | SheetSide::Bottom => {
            panel.w_full().border_sides(if side == SheetSide::Top { [0.0, 0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0, 0.0] }, t.border)
        }
    };
    panel = panel.child(
        el().key(format!("{key}-close"))
            .role("button")
            .label("Close")
            .pin(None, Some(rem(1.0)), Some(rem(1.0)), None)
            .size(rem(1.0), rem(1.0))
            .center()
            .cursor(Cursor::Pointer)
            .opacity(0.7)
            .on_press(close)
            .child(icon(&crate::icons::X, rem(1.0)).color(t.text)),
    );
    let from = match side {
        SheetSide::Right => Vec2::new(1.0, 0.5),
        SheetSide::Left => Vec2::new(0.0, 0.5),
        SheetSide::Top => Vec2::new(0.5, 0.0),
        SheetSide::Bottom => Vec2::new(0.5, 1.0),
    };
    let panel = lift(t, panel).transition(Transition::scale_anchor(0.98, from)).token_motion(Spring::new(0.3, 0.0)).modal(true, close);
    let frame = match side {
        SheetSide::Right => row().justify_end(),
        SheetSide::Left => row().justify_start(),
        SheetSide::Top => col().justify_start(),
        SheetSide::Bottom => col().justify_end(),
    };
    vec![overlay(key), frame.key(format!("{key}-frame")).absolute(0.0, 0.0).w_full().h_full().child(panel)]
}

/// `PopoverContent`: `w-72` (252), `p-4`, a border, `shadow-md`, 4px from its trigger.
#[allow(clippy::too_many_arguments)]
pub fn popover(t: &Theme, key: &str, anchor: &str, side: Side, align: Align, width: f32, padding: f32, body: El) -> Vec<El> {
    let p = lift(
        t,
        col().key(format!("{key}-popover")).role("dialog").w(width).padding(padding).fill(t.card_bg).border(1.0, t.border).child(body),
    )
    .focus_return(anchor.to_string())
    .modal(true, |s: &mut State| s.ui.open = None);
    vec![scrim(&format!("{key}-scrim"), |s: &mut State| s.ui.open = None), anchored(p, anchor, side, align, 4.0)]
}
