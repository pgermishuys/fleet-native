//! Updates & about › Fleet › the app's own row (`DesktopAppUpdates.vue`). Fleet's is Electron's updater; this app's is build
//! numbers from the rolling release with one dialog (decision 4 of docs/parity/16-settings.md), so the row says "Fleet app",
//! the build, one status line, "Check for updates" (the checker looks now) and "Update and restart" (opens that dialog).
use super::updates::{bar, check_button, ink, note, pill, status_icon};
use crate::State;
use crate::fleet::settings::updates_page::app_row;
use crate::theme::{Theme, mono, sans};
use crate::ui::rem;
use rune::{El, col, text};

pub fn view(s: &State, t: &Theme) -> El {
    let now = crate::view::message::now_ms(s);
    let r = app_row(&s.update, now, crate::conv::time::local_offset_secs());
    let mut buttons = rune::row().center_y().gap(7.0);
    if r.can_update {
        buttons = buttons.child(pill(t, "app-update-go", "Update and restart", None, true).on_press(|s: &mut State| s.update_open()));
    }
    if r.can_check {
        buttons = buttons.child(check_button(t, "app-update-check", r.checking, |s: &mut State| s.update_check_now()));
    }
    let title = col()
        .child(text("Fleet app").font(sans(rem(0.875), 500.0, 1.43)).color(t.text))
        .child(rune::row().mt(2.0).child(text(r.build.clone()).key("app-update-build").font(mono(rem(0.75), 400.0, 1.33)).color(t.muted)));
    let mut status = rune::row().center_y().gap(7.0);
    if let Some(i) = status_icon(t, r.tone, r.busy) {
        status = status.child(i);
    }
    status = status.child(text(r.label.clone()).key("app-update-status").font(sans(rem(0.875), 400.0, 1.43)).color(ink(t, r.tone)));
    let mut body = col()
        .key("app-update-row")
        .w_full()
        .px(14.0)
        .py(10.5)
        .gap(7.0)
        .child(rune::row().w_full().items_center().justify_between().gap(10.5).child(title).child(buttons))
        .child(status);
    if let Some(percent) = r.percent {
        body = body.child(bar(t, "app-update-bar", Some(percent)));
    }
    if let Some(words) = &r.note {
        body = body.child(note(t, "app-update-note", words));
    }
    if let Some(why) = &r.error {
        body = body.child(note(t, "app-update-error", why).color(t.text));
    }
    body
}
