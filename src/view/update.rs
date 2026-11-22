//! The updater's two parts: the status bar's "Update available · build N" item, and the dialog it opens (build, date, "Update
//! and restart", progress). Fleet's desktop app updates through Electron's own dialog, so there is no Vue component to port:
//! the item is styled as the status bar's other items are (`view::status`) and the dialog is the kit's (`ui::dialog`).
use crate::State;
use crate::fleet::settings::updates_page::about_size;
use crate::theme::{Theme, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::controls::progress;
use crate::ui::dialog::{Dialog, dialog as kit_dialog};
use crate::ui::rem;
use crate::update::Phase;
use rune::{Cursor, El, col, row, text};

/// The status bar's item; `None` when no newer build is known.
pub fn item(s: &State, t: &Theme) -> Option<El> {
    let offer = s.update.available.as_ref()?;
    Some(
        row()
            .key("update-item")
            .role("button")
            .label(format!("Update available, build {}", offer.build))
            .center_y()
            .px(5.0)
            .py(2.0)
            .radius(5.0)
            .hover(t.wash(5.0))
            .cursor(Cursor::Pointer)
            .child(text(format!("Update available · build {}", offer.build)).font(sans(11.0, 500.0, 1.3)).color(t.accent_text))
            .on_press(|s: &mut State| s.update_open()),
    )
}

/// Megabytes with one decimal: "12.4".
fn mb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_000_000.0)
}

/// The dialog and its scrim; `None` while it is closed.
pub fn dialog(s: &State, t: &Theme) -> Option<Vec<El>> {
    if !s.update.dialog {
        return None;
    }
    let offer = s.update.available.as_ref()?;
    let small = sans(rem(0.875), 400.0, 1.4);
    let busy = s.update.busy();
    let mut body = Vec::new();
    match &s.update.phase {
        Phase::Idle => {
            let size = about_size(offer.asset.size);
            let words = if offer.link.is_some() {
                "This copy was installed from a .deb package, which can't replace itself. Download the new package and install it; your drafts are kept."
            } else {
                "Fleet Native downloads the new build, checks it, replaces itself and its server, and starts again. Your drafts are kept."
            };
            body.push(
                col()
                    .gap(rem(0.5))
                    .child(text(words).font(small).color(t.text).wrap())
                    .child(text(format!("Download: {size}.")).key("update-size").font(small).color(t.muted)),
            )
        }
        Phase::Downloading(done, total) => body.push(
            col()
                .key("update-progress")
                .gap(rem(0.5))
                .child(progress(t, 100.0 * *done as f32 / (*total).max(1) as f32, rem(26.0)))
                .child(text(format!("Downloading · {} of {} MB", mb(*done), mb(*total))).font(small).color(t.muted)),
        ),
        Phase::Installing => body.push(text("Checked. Installing and restarting…").font(small).color(t.muted)),
        Phase::Failed(why) => body.push(
            col()
                .key("update-failed")
                .gap(rem(0.25))
                .child(text(why.clone()).font(small).color(t.error_text).wrap())
                .child(text("The app you're running was not changed.").font(small).color(t.muted).wrap()),
        ),
        Phase::Manual { why } => body.push(text(why.clone()).font(small).color(t.text).wrap().key("update-manual")),
    }
    let retry = matches!(s.update.phase, Phase::Failed(_));
    let notify = offer.link.is_some();
    let go_words = if notify {
        "Download the new .deb"
    } else if retry {
        "Try again"
    } else {
        "Update and restart"
    };
    let go = button(t, Variant::Default, Size::Default, None, go_words).key("update-go");
    let go = if busy || matches!(s.update.phase, Phase::Manual { .. }) {
        disabled(go)
    } else if notify {
        go.on_press_tx(|s: &mut State, tx: &mut rune::Tx| s.update_download_link(tx))
    } else {
        go.on_press(|s: &mut State| s.update_start())
    };
    let later =
        button(t, Variant::Outline, Size::Default, None, if matches!(s.update.phase, Phase::Manual { .. }) { "Close" } else { "Not now" })
            .key("update-later");
    let later = if busy { disabled(later) } else { later.on_press(|s: &mut State| s.update_close()) };
    let date = if offer.date.is_empty() { String::new() } else { format!(" · {}", offer.date) };
    Some(kit_dialog(
        t,
        Dialog {
            width: None,
            key: "update",
            title: "Update available".into(),
            description: Some(format!("Build {}{date}", offer.build)),
            body,
            footer: vec![later, go],
            close_button: !busy,
            click_outside: !busy,
            title_icon: None,
            title_el: None,
            description_el: None,
            dismiss: Some(|s: &mut State| s.update_close()),
            max_height: None,
            radius: None,
        },
    ))
}
