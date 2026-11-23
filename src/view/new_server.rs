//! Add a machine → On a new server (`NewServerCard.vue`, with `ErrorNote`): one line to paste on a server that has no Fleet yet.
//! It installs Fleet, starts it as a machine with no screen and joins this Fleet with a one-time code (30 minutes, one use) made
//! here. The code carries every address of this machine, so nobody types one; the new machine shows up in the list by itself.
use super::add_machine::spinner;
use super::machine_card::faded;
use crate::icons::{CHECK, CIRCLE_ALERT, COPY, TRIANGLE_ALERT};
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::rem;
use crate::{State, Tx};
use rune::{El, col, icon, row, text};

fn xs(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap()
}

/// `ErrorNote` with a reason and Try again: "Couldn't make a join code." over the server's words.
fn failure(s: &State, t: &Theme, why: &str) -> El {
    let making = s.machine_page.add.server.making;
    let again = button(t, Variant::Outline, Size::Sm, None, "Try again").key("machines-new-server-retry");
    let again = if making { faded(again) } else { again.on_press(|s: &mut State| s.make_new_server_code()) };
    let words = |w: &str, weight: f32| text(w.to_string()).font(sans(rem(0.875), weight, 1.43)).color(t.error_text).wrap().w_full();
    row()
        .key("machines-new-server-error")
        .role("alert")
        .w_full()
        .gap(8.0)
        .items_start()
        .px(12.0)
        .py(8.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.1))
        .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0))
        .child(
            col()
                .grow(1.0)
                .shrink()
                .min_w(0.0)
                .child(words("Couldn't make a join code.", 500.0))
                .child(words(why, 400.0).mt(rem(0.125)))
                .child(row().mt(rem(0.5)).child(again)),
        )
}

/// The On a new server tab.
pub fn card(s: &State, t: &Theme) -> El {
    let server = &s.machine_page.add.server;
    let e = col().key("machines-new-server").w_full().gap(12.0).child(xs(
        t,
        "Paste this on the new machine, in a terminal or a cloud server's start-up script. It installs Fleet, keeps it running, and \
         adds it here.",
    ));
    let Some(code) = &server.code else {
        if let Some(why) = &server.failure {
            return e.child(failure(s, t, why));
        }
        if !server.making {
            return e;
        }
        return e.child(
            row().key("machines-new-server-making").gap(8.0).center_y().child(spinner(t.muted, 14.0)).child(xs(t, "Making a join code…")),
        );
    };
    let copied = server.copied_until.is_some();
    let copy = button(t, Variant::Outline, Size::Sm, Some(if copied { &CHECK } else { &COPY }), if copied { "Copied" } else { "Copy" })
        .key("machines-new-server-copy");
    let copy = if server.expired { faded(copy) } else { copy.on_press_tx(|s: &mut State, tx: &mut Tx| s.copy_new_server_line(tx)) };
    let mut line = row()
        .w_full()
        .gap(8.0)
        .items_start()
        .padding(12.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(
            text(server.line())
                .key("machines-new-server-line")
                .font(mono(rem(0.75), 400.0, 1.33))
                .color(t.text)
                .wrap_anywhere()
                .grow(1.0)
                .shrink()
                .min_w(0.0),
        )
        .child(copy);
    if server.expired {
        line = line.opacity(0.6);
    }
    let warning = row()
        .key("machines-new-server-warning")
        .w_full()
        .gap(8.0)
        .items_start()
        .child(icon(&TRIANGLE_ALERT, 14.0).color(t.muted).mt(rem(0.125)))
        .child(
            xs(
                t,
                &format!(
                    "Works once, for 30 minutes. Anyone who has the line can add a machine to {}, so treat it like a password.",
                    code.machine_name
                ),
            )
            .shrink()
            .min_w(0.0),
        );
    let after = if server.expired {
        let renew = button(t, Variant::Outline, Size::Sm, None, "Make a new code").key("machines-new-server-renew");
        let renew = if server.making { faded(renew) } else { renew.on_press(|s: &mut State| s.make_new_server_code()) };
        row()
            .key("machines-new-server-expired")
            .flex_wrap()
            .gap(8.0)
            .center_y()
            .child(text("This code has expired.").font(sans(rem(0.875), 400.0, 1.43)).color(t.text))
            .child(renew)
    } else {
        row()
            .key("machines-new-server-waiting")
            .gap(8.0)
            .center_y()
            .child(spinner(t.muted, 12.0))
            .child(xs(t, &format!("Waiting for a machine to join… The code expires {}.", server.countdown)))
    };
    e.child(line).child(warning).child(after)
}
