//! Settings → Machines (`MachinesSection.vue`, inside `SettingsPage.vue`'s detail pane): the machines this app knows as cards, and
//! "This machine", how other devices reach it, with its access token. `settings.rs` mounts `content` as the Machines page. The
//! pairing form is piece K's, the address list and route line piece M's, phones are out.
use super::machine_card::{card, faded};
use super::settings::kit::{self, Row};
use crate::fleet::machine_actions::{Access, AccessState};
use crate::icons::{CHECK, CIRCLE_ALERT, COPY, EYE, EYE_OFF, PLUS, REFRESH_CW};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::rem;
use crate::{State, Tx};
use rune::{Cursor, El, col, el, icon, row, text};

fn note(t: &Theme, key: &str, words: &str) -> El {
    text(words.to_string()).key(key.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().w_full()
}

/// `.machine-value`: a code box holding a token or an address.
fn value(t: &Theme, key: &str, words: String) -> El {
    row()
        .px(9.0)
        .py(5.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .shrink()
        .min_w(0.0)
        .child(text(words).key(key.to_string()).font(mono(12.0, 400.0, 1.4)).color(t.text).wrap())
}

/// `.machine-icon-button`: 28px, a border, muted until hovered.
fn icon_button(s: &State, t: &Theme, key: String, label: &str, glyph: &'static rune::Icon, ticked: bool) -> El {
    let _ = s;
    row()
        .key(key)
        .role("button")
        .label(label.to_string())
        .help(label.to_string())
        .size(28.0, 28.0)
        .center()
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .hover(t.accent.alpha(0.1))
        .hover_group()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .child(icon(if ticked { &CHECK } else { glyph }, 14.0).color(t.muted).group_color(t.text))
}

fn label(t: &Theme, words: &str) -> El {
    text(words.to_uppercase()).font(sans(rem(0.75), 500.0, 1.33).tracking(0.6)).color(t.muted)
}

fn copied(s: &State, what: &str) -> bool {
    s.machine_page.copied.as_ref().is_some_and(|(w, _)| w == what)
}

/// "Addresses", "Access token" and what to do about both, once the access has been read.
fn access_rows(s: &State, t: &Theme, access: &Access) -> Vec<El> {
    let mut rows = Vec::new();
    // Why only this machine can reach it, when that is so (`access.remoteReachable`).
    if !access.remote_reachable {
        let words = if access.requires_token {
            "Fleet asks every request for the token, so a proxy such as tailscale serve can forward other devices to it. Use the proxy's address."
        } else {
            "Only this machine can reach it. To add it from another device, start Fleet with --host 0.0.0.0 (or your tailnet address), or put tailscale serve in front of it and start Fleet with --require-token."
        };
        rows.push(col().w_full().px(16.0).py(12.0).child(note(t, "machine-access-loopback", words)));
    }
    if !access.addresses.is_empty() {
        let mut block = col().w_full().px(16.0).py(12.0).gap(4.0).child(label(t, "Addresses"));
        for (at, a) in access.addresses.iter().enumerate() {
            let url = a.url.clone();
            let tick = copied(s, &a.url);
            block = block.child(
                row()
                    .flex_wrap()
                    .center_y()
                    .gap(8.0)
                    .child(value(t, &format!("machine-address-{at}"), a.url.clone()))
                    .child(kind_chip(t, &a.kind))
                    .child(
                        icon_button(s, t, format!("machine-copy-address-{at}"), &format!("Copy {}", a.url), &COPY, tick)
                            .on_press_tx(move |s: &mut State, tx: &mut Tx| s.copy_on_page(&url.clone(), url.clone(), tx)),
                    ),
            );
        }
        rows.push(block);
    }
    rows.push(token_block(s, t, access));
    rows
}

fn kind_chip(t: &Theme, kind: &str) -> El {
    row()
        .px(5.0)
        .radius(4.0)
        .border(1.0, t.border)
        .border_layout()
        .child(text(kind.to_uppercase()).font(mono(10.0, 600.0, 1.4).tracking(0.5)).color(t.muted))
}

fn token_block(s: &State, t: &Theme, access: &Access) -> El {
    let page = &s.machine_page;
    let shown = if page.show_token { access.token.clone() } else { Access::hidden_token() };
    let token = access.token.clone();
    let mut line = row()
        .flex_wrap()
        .center_y()
        .gap(8.0)
        .child(value(t, "machine-access-token", shown))
        .child(
            icon_button(
                s,
                t,
                "machine-show-token".into(),
                if page.show_token { "Hide token" } else { "Show token" },
                if page.show_token { &EYE_OFF } else { &EYE },
                false,
            )
            .on_press(|s: &mut State| s.toggle_token()),
        )
        .child(
            icon_button(s, t, "machine-copy-token".into(), "Copy token", &COPY, copied(s, "token"))
                .on_press_tx(move |s: &mut State, tx: &mut Tx| s.copy_on_page("token", token.clone(), tx)),
        );
    // Replace token isn't offered when the environment fixes the token.
    if access.token_source != "environment" {
        let replace = row()
            .key("machine-replace-token")
            .role("button")
            .label("Replace token")
            .h(rem(2.14))
            .px(rem(0.75))
            .gap(8.0)
            .center()
            .radius(RADIUS_BTN)
            .fill(t.main_bg)
            .border(1.0, t.border)
            .border_layout()
            .hover(t.accent.alpha(0.1))
            .cursor(Cursor::Pointer)
            .focusable(true)
            .child(icon(&REFRESH_CW, 14.0).color(t.text))
            .child(text("Replace token").font(sans(rem(0.875), 500.0, 1.4)).color(t.text));
        line = line.child(if page.replacing {
            faded(replace)
        } else {
            replace.on_press_tx(|s: &mut State, tx: &mut Tx| s.ask_replace_token(tx))
        });
    }
    let words = if access.token_source == "environment" {
        "WEAVE_FLEET_AUTH_TOKEN sets it; change the variable and restart Fleet to replace it."
    } else {
        "Anyone who has it can run agents, change files and open terminals on this machine. Replacing it locks out every device that has the old one."
    };
    col()
        .key("machine-access-block")
        .w_full()
        .px(16.0)
        .py(12.0)
        .gap(4.0)
        .child(label(t, "Access token"))
        .child(line)
        .child(text(words.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().w_full().mt(4.0))
}

fn access_group(s: &State, t: &Theme) -> El {
    let name = s.machines.home.as_ref().map_or("This machine", |h| h.name.as_str());
    let rows =
        match &s.machine_page.access {
            AccessState::Loaded(access) => access_rows(s, t, access),
            other => {
                let words = match other {
                AccessState::Loading => "Loading…".to_string(),
                AccessState::Device => {
                    "This app is signed in as a paired device. Adding phones and seeing the token happen on the machine itself.".into()
                }
                AccessState::Unavailable => {
                    "This Fleet doesn't hand out access tokens: it signs people in with an identity provider, or it's too old for machines."
                        .into()
                }
                AccessState::Failed(why) => why.clone(),
                AccessState::Loaded(_) => String::new(),
            };
                let mut line = row().w_full().px(16.0).py(12.0).gap(8.0).items_start();
                if matches!(other, AccessState::Failed(_)) {
                    line = line.child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0));
                }
                let ink = if matches!(other, AccessState::Failed(_)) { t.error_text } else { t.muted };
                vec![line.child(
                    text(words).key("machine-access-note").font(sans(rem(0.875), 400.0, 1.43)).color(ink).wrap().shrink().min_w(0.0),
                )]
            }
        };
    kit::group(t, "phones", &format!("Let other devices reach {name}"), Some("Phones and devices, and the access token."), rows)
}

/// The page's content: the title and its line, then the groups. The Settings shell (`settings.rs`) gives it the sheet and the scroll.
pub fn content(s: &State, t: &Theme) -> El {
    let add = button(t, Variant::Default, Size::Default, Some(&PLUS), "Add a machine")
        .key("machines-add")
        .on_press_tx(|s: &mut State, tx: &mut Tx| s.open_add_form(tx));
    let form = &s.machine_page.add;
    // The button is gone while the form is open (`v-if="!showAddForm"`).
    let control = if form.open { el().size(0.0, 0.0) } else { add };
    let mut rows = vec![Row::new("add-machine", "Add a machine")
        .summary(
        "Every machine runs its own Fleet and keeps its own sessions, repositories and sign-ins. A session stays on the machine it started on. The sessions list shows them all; the one you're working in is live.",
        )
        .control(control)
        .build(s, t)];
    if form.open {
        rows.push(super::add_machine::form(s, t));
    }
    if let Some(name) = &form.added {
        rows.push(super::add_machine::added(t, name));
    }
    rows.extend(s.machine_cards().iter().map(|c| card(s, t, c)));
    let header = super::settings::page_header(s, t, &crate::fleet::settings_nav::Page::Machines);
    col()
        .key("machines-page-body")
        .w_full()
        .gap(21.0)
        .child(header)
        .child(kit::group(t, "machines", "Machines", None, rows))
        .child(access_group(s, t))
}
