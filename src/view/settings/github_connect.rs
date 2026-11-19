//! GitHub's own page under Settings → Plugins → GitHub (`plugins/builtin/github/GitHubSettings.vue`): the connected rows, or the
//! device-flow card and the personal-access-token card. Sizes are the component's Tailwind classes and scoped styles at the 14 px
//! root. The repository cache row (11.38) belongs to component 21: it says what the cache says, and its Refresh waits for it.
use crate::State;
use crate::Tx;
use crate::fleet::settings::github_auth::{Device, Github, Pat};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::confirm::{error_note, spinner};
use crate::ui::rem;
use rune::{Cursor, El, col, icon, row, text, text_input};

#[derive(Clone, Copy, PartialEq)]
enum Look {
    /// `buttonPrimaryClass`.
    Primary,
    /// `buttonSecondaryClass`.
    Secondary,
    /// `buttonDangerClass`.
    Danger,
}

/// The three button classes of the component: the primary is the 14 px accent one, the others the 28 px 13 px ones.
fn button(t: &Theme, key: &str, look: Look, lead: Option<El>, words: &str, on: bool) -> El {
    let (fill, edge, ink, hover) = match look {
        Look::Primary => (t.accent, None, t.primary_fg, t.accent.alpha(0.9)),
        Look::Secondary => (rune::Color::TRANSPARENT, Some(t.border), t.text, t.text.alpha(0.06)),
        Look::Danger => (rune::Color::TRANSPARENT, Some(t.error.alpha(0.4)), t.error_text, t.error.alpha(0.1)),
    };
    let primary = look == Look::Primary;
    let mut b = row()
        .key(key.to_string())
        .role("button")
        .label(words.to_string())
        .items_center()
        .justify_center()
        .gap(if primary { rem(0.5) } else { rem(0.375) })
        .px(rem(0.75))
        .radius(RADIUS_BTN)
        .fill(fill)
        .hover(hover)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent);
    b = if primary { b.py(rem(0.5)) } else { b.h(rem(2.0)) };
    if let Some(edge) = edge {
        b = b.border(1.0, edge).border_layout();
    }
    if let Some(lead) = lead {
        b = b.child(lead);
    }
    let font = if primary { sans(rem(0.875), 500.0, 1.43) } else { sans(13.0, 500.0, 1.5) };
    let b = b.child(text(words.to_string()).font(font).color(ink));
    if on { b } else { b.opacity(0.6).disabled(true).cursor(Cursor::Default) }
}

fn card(t: &Theme, key: &str) -> El {
    col().key(key.to_string()).w_full().padding(rem(1.0)).radius(RADIUS_CARD).border(1.0, t.border).border_layout().fill(t.card_bg)
}

/// The green box of a result (`border-running/30 bg-running/10 px-3 py-2 text-sm text-running-text`).
fn success(t: &Theme, key: &str, words: &str) -> El {
    row()
        .key(key.to_string())
        .w_full()
        .items_start()
        .gap(rem(0.5))
        .px(rem(0.75))
        .py(rem(0.5))
        .radius(RADIUS_CARD)
        .border(1.0, t.running.alpha(0.3))
        .border_layout()
        .fill(t.running.alpha(0.1))
        .child(icon(&CIRCLE_CHECK, 16.0).color(t.running_text).mt(1.75))
        .child(text(words.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.running_text).wrap().shrink().min_w(0.0))
}

fn checking(t: &Theme) -> El {
    row()
        .key("github-checking")
        .items_center()
        .gap(rem(0.5))
        .child(spinner(t.muted))
        .child(text("Checking GitHub connection\u{2026}").font(sans(rem(0.875), 400.0, 1.43)).color(t.muted))
}

/// One of the settings rows: what it is on the left, the action on the right.
fn settings_row(t: &Theme, key: &str, first: bool, last: bool, lead: Option<El>, copy: El, action: Option<El>) -> El {
    let mut r = row().key(key.to_string()).w_full().flex_wrap().items_center().gap(12.0).py(14.0);
    if first {
        r = r.pt(0.0);
    }
    if last {
        r = r.pb(0.0);
    }
    if !first {
        r = r.border_sides([1.0, 0.0, 0.0, 0.0], t.border).border_layout();
    }
    if let Some(lead) = lead {
        r = r.child(lead);
    }
    r = r.child(copy);
    match action {
        Some(a) => r.child(a),
        None => r,
    }
}

fn copy(t: &Theme, title: &str, hint: &str, extra: Option<El>) -> El {
    let mut c = col()
        .grow(1.0)
        .shrink()
        .min_w(220.0)
        .child(text(title.to_string()).font(sans(13.0, 600.0, 1.5)).color(t.text))
        .child(text(hint.to_string()).font(sans(12.5, 400.0, 1.5)).color(t.muted).wrap().mt(2.0));
    if let Some(extra) = extra {
        c = c.child(extra);
    }
    c
}

fn connected(t: &Theme, g: &Github) -> El {
    let ok = icon(&CIRCLE_CHECK, 18.0).color(t.running_text).mt(1.0).self_start();
    let first = settings_row(
        t,
        "github-row-connected",
        true,
        false,
        Some(ok),
        copy(
            t,
            "GitHub is connected",
            "Fleet can list your repositories, show linked pull requests and issues, and start sessions from them.",
            None,
        ),
        None,
    );
    // Component 21 fills the cache; until then it has nothing cached.
    let refresh = button(t, "github-refresh", Look::Secondary, Some(icon(&REFRESH_CW, 14.0).color(t.text)), "Refresh", !g.disconnecting);
    let second = settings_row(
        t,
        "github-row-cache",
        false,
        false,
        None,
        copy(t, "Repository cache", "No repositories cached yet.", None),
        Some(refresh),
    );
    let lead = if g.disconnecting { spinner(t.error_text) } else { icon(&UNPLUG, 14.0).color(t.error_text) };
    let disconnect = button(
        t,
        "github-disconnect",
        Look::Danger,
        Some(lead),
        if g.disconnecting { "Disconnecting\u{2026}" } else { "Disconnect" },
        !g.disconnecting,
    )
    .on_press(|s: &mut State| s.github_disconnect());
    let third = settings_row(
        t,
        "github-row-disconnect",
        false,
        true,
        None,
        copy(t, "Disconnect", "Forgets the token. Sessions keep their links, but they stop updating.", None),
        Some(disconnect),
    );
    col().key("github-connected").w_full().child(first).child(second).child(third)
}

fn waiting(s: &State, t: &Theme, g: &Github, code: &str, uri: &str) -> El {
    let code_box =
        row().px(rem(0.75)).py(rem(0.5)).radius(RADIUS_BTN).border(1.0, t.border).border_layout().fill(t.card_bg).child(
            text(code.to_string()).key("github-user-code").font(mono(rem(0.875), 600.0, 1.43).tracking(0.2 * rem(0.875))).color(t.text),
        );
    let copy_btn = button(t, "github-copy-code", Look::Secondary, Some(icon(&COPY, 16.0).color(t.text)), "Copy code", true)
        .on_press_tx(|s: &mut State, tx: &mut Tx| s.github_copy_code(tx));
    let go = uri.to_string();
    let link = row()
        .key("github-open")
        .role("link")
        .label("Open GitHub")
        .items_center()
        .gap(rem(0.5))
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 2.0, t.accent)
        .on_press_tx(move |_: &mut State, tx: &mut Tx| {
            let _ = tx.open_url(&go);
        })
        .child(icon(&EXTERNAL_LINK, 16.0).color(t.text))
        .child(text("Open GitHub").font(sans(rem(0.875), 500.0, 1.43)).color(t.text));
    col()
        .key("github-waiting")
        .w_full()
        .gap(rem(0.75))
        .padding(rem(1.0))
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(
            col()
                .gap(rem(0.5))
                .child(
                    text("DEVICE CODE").label("Device code").font(sans(rem(0.75), 500.0, 1.33).tracking(0.025 * rem(0.75))).color(t.muted),
                )
                .child(row().flex_wrap().items_center().gap(rem(0.5)).child(code_box).child(copy_btn)),
        )
        .child(link)
        .child(
            row().items_center().gap(rem(0.5)).child(spinner(t.muted)).child(
                text(g.poll_label(s.clock).unwrap_or_default())
                    .key("github-poll-label")
                    .font(sans(rem(0.875), 400.0, 1.43))
                    .color(t.muted)
                    .wrap()
                    .shrink()
                    .min_w(0.0),
            ),
        )
        .child(
            button(t, "github-cancel", Look::Secondary, None, "Cancel", true)
                .self_start()
                .on_press(|s: &mut State| s.github_reset_device()),
        )
}

fn device_card(s: &State, t: &Theme, g: &Github) -> El {
    let head = row().items_start().gap(rem(0.75)).child(icon(&GITHUB, 18.0).color(t.text).mt(1.75)).child(
        col().shrink().min_w(0.0).child(text("Connect with device flow").font(sans(rem(0.875), 600.0, 1.43)).color(t.text)).child(
            text("The recommended option. Authorize in your browser and Weave finishes the connection automatically.")
                .font(sans(rem(0.75), 400.0, 1.33))
                .color(t.muted)
                .wrap()
                .mt(rem(0.25)),
        ),
    );
    let body = match &g.device {
        Device::Idle => {
            button(t, "github-connect", Look::Primary, Some(icon(&PLUG_ZAP, 16.0).color(t.primary_fg)), "Connect with GitHub", true)
                .self_start()
                .on_press(|s: &mut State| s.github_connect_device())
        }
        Device::Initiating => {
            button(t, "github-connect", Look::Primary, Some(spinner(t.primary_fg)), "Starting authorization\u{2026}", false).self_start()
        }
        Device::Awaiting { user_code, verification_uri, .. } => waiting(s, t, g, user_code, verification_uri),
        Device::Complete => success(t, "github-complete", "GitHub connected successfully."),
        Device::Expired | Device::Denied | Device::Error(_) => {
            let message = match &g.device {
                Device::Expired => "The GitHub code expired. Please try again.",
                Device::Denied => "GitHub authorization was denied.",
                Device::Error(m) => m.as_str(),
                _ => "",
            };
            col().w_full().gap(rem(0.75)).child(error_note(t, "github-device-error", message, None)).child(
                button(t, "github-try-again", Look::Secondary, None, "Try again", true)
                    .self_start()
                    .on_press(|s: &mut State| s.github_reset_device()),
            )
        }
    };
    card(t, "github-device").child(head).child(col().w_full().mt(rem(1.0)).gap(rem(0.75)).child(body))
}

/// `inputClass` over the secure box: bullets, never the token.
fn token_input(t: &Theme, g: &Github) -> El {
    let input = text_input(&g.pat, sans(rem(0.875), 400.0, 1.43))
        .key("github-token")
        .label("Personal access token".to_string())
        .single_line()
        .placeholder("ghp_xxxxxxxxxxxx".to_string(), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .on_edit(|s: &mut State| s.github_pat_edit())
        .on_submit(|s: &mut State, _: &mut Tx| s.github_connect_pat());
    row()
        .w_full()
        .child(
            input
                .grow(1.0)
                .min_w(0.0)
                .px(rem(0.75))
                .py(rem(0.5))
                .radius(RADIUS_BTN)
                .border(1.0, if g.pat.focused { t.accent } else { t.border })
                .border_layout()
                .fill(t.main_bg),
        )
        .opacity(if g.connecting_pat { 0.6 } else { 1.0 })
}

fn pat_card(t: &Theme, g: &Github) -> El {
    let head = row()
        .key("github-pat-toggle")
        .role("button")
        .label("Advanced: personal access token")
        .w_full()
        .items_center()
        .justify_between()
        .gap(rem(0.75))
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 2.0, t.accent)
        .on_press(|s: &mut State| s.settings.github.pat_open = !s.settings.github.pat_open)
        .child(text("Advanced: personal access token").font(sans(rem(0.875), 600.0, 1.43)).color(t.text))
        .child(icon(if g.pat_open { &CHEVRON_UP } else { &CHEVRON_DOWN }, 16.0).color(t.muted));
    let mut c = card(t, "github-pat").child(head);
    if !g.pat_open {
        return c;
    }
    let mut body = col().w_full().mt(rem(1.0)).gap(rem(0.75)).child(
        text("Use a PAT when device flow is unavailable. The token is validated before it is stored server-side.")
            .font(sans(rem(0.75), 400.0, 1.33))
            .color(t.muted)
            .wrap(),
    );
    body = body.child(token_input(t, g));
    if let Some((message, is_error)) = g.pat_state.message() {
        body = body.child(if is_error {
            error_note(t, "github-pat-message", &message, None)
        } else {
            success(t, "github-pat-message", &message)
        });
    }
    let testing = g.pat_state == Pat::Testing;
    let test = button(
        t,
        "github-pat-test",
        Look::Secondary,
        testing.then(|| spinner(t.muted)),
        if testing { "Testing\u{2026}" } else { "Test connection" },
        g.has_token() && !testing && !g.connecting_pat,
    )
    .on_press(|s: &mut State| s.github_test_pat());
    let connect = button(
        t,
        "github-pat-connect",
        Look::Primary,
        g.connecting_pat.then(|| spinner(t.primary_fg)),
        if g.connecting_pat { "Connecting\u{2026}" } else { "Connect" },
        g.has_token() && !g.connecting_pat,
    )
    .on_press(|s: &mut State| s.github_connect_pat());
    c = c.child(body.child(row().flex_wrap().gap(rem(0.5)).child(test).child(connect)));
    c
}

/// The page's body: what `GitHubSettings.vue` renders.
pub fn content(s: &State, t: &Theme) -> El {
    let g = &s.settings.github;
    let body = if g.checking {
        checking(t)
    } else if g.connected {
        connected(t, g)
    } else {
        col().w_full().gap(rem(1.0)).child(device_card(s, t, g)).child(pat_card(t, g))
    };
    col().key("plugin-config-github").w_full().child(body)
}
