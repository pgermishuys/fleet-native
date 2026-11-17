//! The sign-in page, drawn over the whole window when the Fleet turns the app away for want of a token. Ports
//! `components/auth/LoginPage.vue` (the token form; the OIDC buttons are cloud mode only) and the "Checking your session…" line of
//! `AuthGate.vue`. Sizes are the Tailwind classes at the 14px root: card `max-w-sm` with `px-8 py-10`, `gap-6`.
use super::pages::{logo_tile, version_text};
use crate::fleet::sign_in::{Options, TOKEN_FIELD};
use crate::icons::CIRCLE_ALERT;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::rem;
use crate::{State, Tx};
use rune::{Align, Color, El, Key, KeyEvent, Shadow, col, icon, row, text, text_input};

const HINT: &str = "It's in the sign-in link Fleet prints when it starts, and in Settings → Machines on a device that's already signed in.";

pub fn page(s: &State, t: &Theme) -> El {
    let card = col()
        .key("sign-in-card")
        .w(rem(24.0))
        .items_center()
        .gap(rem(1.5))
        .px(rem(2.0))
        .py(rem(2.5))
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .box_shadow(Shadow { x: 0.0, y: 12.0, blur: 32.0, spread: 0.0, color: Color::BLACK.alpha(0.2), inset: false })
        .child(logo_tile(s, t, "sign-in-logo"))
        .child(
            col()
                .items_center()
                .child(text("Weave").key("sign-in-title").font(mono(rem(1.875), 700.0, 1.2).tracking(-0.025 * rem(1.875))).color(t.text))
                .child(text("Agent Fleet").key("sign-in-subtitle").font(mono(rem(0.875), 400.0, 1.43)).color(t.muted).mt(rem(0.25))),
        )
        .child(
            text("Orchestrate AI coding agents across your projects.")
                .key("sign-in-lede")
                .font(sans(rem(0.875), 400.0, 1.625))
                .color(t.muted)
                .wrap()
                .w_full()
                .align(Align::Center),
        )
        .child(options(s, t))
        .child(text(version_text()).key("sign-in-version").font(sans(10.0, 400.0, 1.5)).color(t.muted));
    // `min-h-screen items-center justify-center bg-main-bg px-4 py-12`
    col().key("sign-in-page").w_full().h_full().fill(t.main_bg).center().px(rem(1.0)).py(rem(3.0)).child(card)
}

fn options(s: &State, t: &Theme) -> El {
    let page = &s.sign_in;
    if page.is_loading() {
        return col()
            .key("sign-in-loading")
            .role("status")
            .w_full()
            .px(rem(1.0))
            .py(rem(0.75))
            .radius(RADIUS_BTN)
            .border(1.0, t.border)
            .border_layout()
            .fill(t.main_bg)
            .child(text("Loading sign-in options…").font(sans(rem(0.875), 400.0, 1.43)).color(t.muted));
    }
    if page.options == Options::Oidc {
        // Cloud mode signs in through the browser, which the app can't do (docs/parity/09-app-frame.md, row 11.5).
        return text("This Fleet signs in through a browser. Use a Fleet that takes an access token with this app.")
            .key("sign-in-oidc")
            .font(sans(rem(0.875), 400.0, 1.43))
            .color(t.muted)
            .wrap()
            .w_full()
            .align(Align::Center);
    }
    form(s, t)
}

fn form(s: &State, t: &Theme) -> El {
    let page = &s.sign_in;
    let field = text_input(&page.token, sans(rem(0.875), 400.0, 1.43))
        .key(TOKEN_FIELD)
        .label("Access token")
        .single_line()
        .placeholder("Paste your token", t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .px(rem(0.75))
        .py(rem(0.625))
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        // `focus:border-accent/60`
        .focus_ring(1.0, 0.0, t.accent.alpha(0.6))
        .on_input_key(|s: &mut State, ev: &KeyEvent, _: &mut Tx| match ev.key {
            Key::Enter => {
                s.sign_in_submit();
                true
            }
            _ => false,
        })
        .on_edit(|s: &mut State| &mut s.sign_in.token);
    let field = if page.submitting { disabled(field) } else { field };
    let mut form = col()
        .key("sign-in-form")
        .w_full()
        .gap(rem(0.75))
        .child(text("Access token").key("sign-in-label").font(sans(rem(0.875), 500.0, 1.43)).color(t.text))
        .child(field)
        .child(
            text(format!("{HINT} This browser stays signed in for 30 days after your last visit."))
                .key("sign-in-hint")
                .font(sans(rem(0.75), 400.0, 1.625))
                .color(t.muted)
                .wrap()
                .w_full(),
        );
    if let Some(error) = &page.error {
        form = form.child(error_note(t, error));
    }
    let go = button(t, Variant::Default, Size::Lg, None, if page.submitting { "Signing in…" } else { "Continue" })
        .key("sign-in-continue")
        .w_full()
        .h(rem(2.5));
    form.child(if page.submitting { disabled(go).opacity(0.6) } else { go.on_press(|s: &mut State| s.sign_in_submit()) })
}

/// `ErrorNote`: a red box, an alert glyph and the words.
fn error_note(t: &Theme, words: &str) -> El {
    row()
        .key("sign-in-error")
        .role("alert")
        .w_full()
        .gap(rem(0.5))
        .items_start()
        .px(rem(0.75))
        .py(rem(0.5))
        .radius(RADIUS_CARD)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.1))
        .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0))
        .child(text(words.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.error_text).wrap().shrink().min_w(0.0))
}
