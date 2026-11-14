//! Fleet's toast (`sessions/ArchiveUndoToast.vue`): an inverted bar at the bottom centre,
//! 420px wide, a message, one pill action, and a 2px bar that drains while the action works.
use crate::State;
use crate::theme::{RADIUS_CARD, Theme, sans};
use rune::{Color, Cursor, El, Shadow, col, el, icon, row, text};

pub struct Toast {
    pub message: String,
    pub action: Option<String>,
    /// Seconds on the clock at which it goes, and how long it lasts in all.
    pub until: f64,
    pub total: f64,
}

/// `now` is the app clock, for the drain.
pub fn view(t: &Theme, toast: &Toast, now: f64) -> El {
    let ink = t.main_bg;
    let left = ((toast.until - now) / toast.total).clamp(0.0, 1.0) as f32;
    let mut r = row()
        .w_full()
        .gap(12.0)
        .center_y()
        .pl(14.0)
        .pr(8.0)
        .py(9.0)
        .child(text(toast.message.clone()).font(sans(13.0, 400.0, 1.4)).color(ink).truncate().grow(1.0).shrink());
    if let Some(a) = &toast.action {
        r = r.child(
            row()
                .key("toast-action")
                .role("button")
                .label(a.clone())
                .gap(5.0)
                .center_y()
                .px(10.0)
                .py(3.0)
                .radius(999.0)
                .fill(ink.alpha(0.16))
                .hover(ink.alpha(0.1))
                .cursor(Cursor::Pointer)
                .focus_ring(2.0, 2.0, t.accent)
                .on_press(|s: &mut State| s.ui.toast = None)
                .child(icon(&crate::icons::UNDO_2, 13.0).color(ink))
                .child(text(a.clone()).font(sans(13.0, 600.0, 1.4)).color(ink)),
        );
    }
    col()
        .key("toast")
        .role("status")
        .w(420.0)
        .radius(RADIUS_CARD)
        .fill(t.text)
        .clip_rounded(RADIUS_CARD)
        .box_shadow(Shadow { x: 0.0, y: 12.0, blur: 32.0, spread: -12.0, color: Color::BLACK.alpha(0.45), inset: false })
        .child(r)
        .child(el().w(420.0 * left).h(2.0).fill(ink.alpha(0.45)))
}
