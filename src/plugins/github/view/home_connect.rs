//! The connect section of Your work (`GitHubBrowserPage.vue`'s `.gh-connect`, rows 4.x of docs/parity/21-github.md): what connecting gets
//! you, Connect with GitHub, the device code while it is waited for, how a try ended, and the way to a token instead. The flow is the
//! plugin's `auth` (the same as Settings' page); this only draws it.
use super::home::{Look, button, link};
use crate::icons::{COPY, EXTERNAL_LINK, GITHUB, TRIANGLE_ALERT};
use crate::plugins::github::auth::Device;
use crate::plugins::github::home::{expires_in_minutes, failure_words, waiting_words};
use crate::plugins::github::state_ref;
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use crate::ui::confirm::spinner;
use crate::{State, Tx};
use rune::{Align, Cursor, El, col, icon, row, text};

const LEDE: &str = "See which pull requests need you, watch each session's checks and reviews, and start a session from any issue.";

/// The address without its scheme, as the box writes it.
fn bare(uri: &str) -> &str {
    uri.trim_start_matches("https://").trim_start_matches("http://")
}

fn hint(t: &Theme, words: String) -> El {
    text(words).font(sans(12.5, 400.0, 1.4)).color(t.muted).wrap().align(Align::Center)
}

/// `.gh-connect__code`: the address, the code, Copy and Open, the countdown, Cancel.
fn waiting(s: &State, t: &Theme, code: &str, uri: &str, expires_at: f64) -> El {
    let open = uri.to_string();
    let open_too = uri.to_string();
    let address = row()
        .key("github-connect-address")
        .role("link")
        .label(bare(uri).to_string())
        .cursor(Cursor::Pointer)
        .child(text(bare(uri).to_string()).font(mono(12.5, 400.0, 1.4)).color(t.text))
        .on_press_tx(move |_: &mut State, tx: &mut Tx| {
            let _ = tx.open_url(&open);
        });
    let actions = row()
        .gap(6.0)
        .child(
            button(t, "github-connect-copy", Look::Plain, Some(icon(&COPY, 13.0).color(t.text)), "Copy code", true)
                .on_press_tx(|s: &mut State, tx: &mut Tx| s.github_copy_code(tx)),
        )
        .child(
            button(t, "github-connect-open", Look::Plain, Some(icon(&EXTERNAL_LINK, 13.0).color(t.text)), "Open GitHub", true).on_press_tx(
                move |_: &mut State, tx: &mut Tx| {
                    let _ = tx.open_url(&open_too);
                },
            ),
        );
    let minutes = expires_in_minutes(expires_at, s.clock);
    col()
        .key("github-connect-code")
        .w_full()
        .items_center()
        .gap(10.0)
        .padding(16.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(row().gap(6.0).center_y().child(hint(t, "Enter this code at".into())).child(address))
        .child(text(code.to_string()).key("github-connect-digits").font(mono(24.0, 500.0, 1.2).tracking(4.8)).color(t.text))
        .child(actions)
        .child(row().gap(6.0).center_y().child(spinner(t.muted)).child(hint(t, waiting_words(minutes))))
        .child(link(t, "github-connect-cancel", "Cancel").on_press(|s: &mut State| s.github_reset_device()))
}

/// The connect section.
pub fn connect(s: &State, t: &Theme) -> El {
    let g = state_ref(&s.places);
    let device = &g.auth.device;
    let top = (s.ui.sel.window.1 * 0.12).max(0.0);
    let mut c = col()
        .key("github-connect")
        .w_full()
        .max_w(440.0)
        .self_center()
        .items_center()
        .gap(12.0)
        .mt(top)
        .child(icon(&GITHUB, 34.0).color(t.muted))
        .child(text("Connect GitHub").key("github-connect-title").font(sans(20.0, 600.0, 1.5)).color(t.text))
        .child(text(LEDE).key("github-connect-lede").font(sans(14.0, 400.0, 1.5)).color(t.muted).wrap().align(Align::Center).mb(6.0));
    if let Device::Awaiting { user_code, verification_uri, expires_at, .. } = device {
        return c.child(waiting(s, t, user_code, verification_uri, *expires_at));
    }
    if let Some(why) = failure_words(device) {
        c = c.child(
            row()
                .key("github-connect-error")
                .role("alert")
                .gap(8.0)
                .center_y()
                .child(icon(&TRIANGLE_ALERT, 14.0).color(t.error_text))
                .child(text(why).font(sans(12.5, 400.0, 1.4)).color(t.error_text).wrap().shrink().min_w(0.0)),
        );
    }
    let starting = matches!(device, Device::Initiating);
    let lead = if starting { spinner(t.primary_fg) } else { icon(&GITHUB, 14.0).color(t.primary_fg) };
    c.child(
        button(t, "github-connect-start", Look::Primary, Some(lead), "Connect with GitHub", !starting)
            .on_press(|s: &mut State| s.github_connect_device()),
    )
    .child(
        link(t, "github-connect-token", "Use a personal access token instead")
            .on_press(|s: &mut State| s.open_settings("plugin:github", None)),
    )
}
