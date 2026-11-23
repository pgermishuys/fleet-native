//! A machine's own page, Settings → Machines → Manage (`MachinePage.vue`, `use-machine-page.ts`): "About this machine" (the card
//! with its route line, agent work, job seats and Details), "Key and addresses" for another machine (its address list and the new
//! token, masked), and "Who can reach it" for home (how others reach it, its addresses and its access token). The trail leads back to
//! the list. "Let others reach" and the Tailscale note are the reach dialog's piece (J).
use super::machine_card::{faded, page_card, token_form};
use super::settings::kit;
use crate::fleet::machine_actions::{Access, AccessState};
use crate::fleet::machine_card::Card;
use crate::fleet::machines::reach::{ReachMode, ReachState};
use crate::icons::{CHECK, CHEVRON_LEFT, CIRCLE_ALERT, COPY, EYE, EYE_OFF, REFRESH_CW};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::page_header::{plain, trail, trail_link};
use crate::ui::rem;
use crate::{State, Tx};
use rune::{Cursor, El, Span, col, icon, rich, row, text};

const PROSE: f32 = 490.0;

/// What a muted paragraph's words are made of: plain, bold (the text colour) or `code`.
enum Bit {
    Plain(&'static str),
    Owned(String),
    Bold(String),
    Code(&'static str),
}

/// `max-w-prose text-sm text-muted`, with its bold and `<code>` runs.
fn para(t: &Theme, key: &str, bits: Vec<Bit>) -> El {
    let (mut words, mut spans) = (String::new(), Vec::new());
    for bit in bits {
        let start = words.len();
        match bit {
            Bit::Plain(w) => words.push_str(w),
            Bit::Owned(w) => words.push_str(&w),
            Bit::Bold(w) => {
                words.push_str(&w);
                spans.push(Span::new(start, words.len()).bold().color(t.text));
            }
            Bit::Code(w) => {
                words.push_str(w);
                spans.push(Span::new(start, words.len()).mono().bg(t.accent_dim, 3.0));
            }
        }
    }
    // `max-w-prose` is 65 characters of this 12.25 px text.
    rich(words, spans).key(key.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().max_w(PROSE).shrink().min_w(0.0)
}

fn note(t: &Theme, key: &str, words: &str) -> El {
    text(words.to_string()).key(key.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().max_w(PROSE)
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
        .child(text(words.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().max_w(PROSE * 0.9).mt(4.0))
}

/// The page: the trail back, the machine's name and its line, then its groups.
pub fn content(s: &State, t: &Theme, c: &Card) -> El {
    let back = row().center_y().gap(4.0).child(icon(&CHEVRON_LEFT, 14.0).color(t.muted)).child(trail_link(
        t,
        "machine-page-back",
        "Machines",
        |s: &mut State| s.close_machine_page(),
    ));
    let meta = super::settings::machine_header::title_chip(s, t, &crate::fleet::settings_nav::Page::Machines).into_iter().collect();
    let line = "Everything about this machine: how it is reached, and what it keeps.";
    let header = col().w_full().child(trail(t, "machine-page-trail", vec![back]).role("navigation").label("Back")).child(plain(
        t,
        "settings-header",
        &c.name,
        Some(line),
        meta,
    ));
    let mut page = col().key(format!("machine-page-{}", c.key)).w_full().gap(21.0).child(header).child(kit::group(
        t,
        "machine-overview",
        "About this machine",
        None,
        vec![page_card(s, t, c)],
    ));
    if c.is_home {
        page = page.child(access_group(s, t, &c.name));
    } else {
        page = page.child(keys_group(s, t, c));
    }
    page
}

/// "Key and addresses" (not home): the address list, and the key this browser holds with New token.
fn keys_group(s: &State, t: &Theme, c: &Card) -> El {
    let mut rows = Vec::new();
    if let Some(list) = super::machine_addresses::list(s, t, c) {
        rows.push(row().w_full().px(16.0).py(12.0).child(list));
    }
    if s.machine_page.token_for.as_deref() == Some(c.key.as_str()) {
        rows.push(row().w_full().px(16.0).py(12.0).child(token_form(s, t, &c.name)));
    } else {
        let key = c.key.clone();
        let new_token = super::machine_card::plain_button(t, format!("machine-token-{}", c.key), "New token")
            .on_press_tx(move |s: &mut State, tx: &mut Tx| s.start_machine_token(&key, tx));
        rows.push(
            row()
                .w_full()
                .flex_wrap()
                .center_y()
                .gap(12.0)
                .px(16.0)
                .py(12.0)
                .child(
                    note(
                        t,
                        "machine-key-note",
                        &format!("This browser holds a key for {}. If it stopped working, give it a new one.", c.name),
                    )
                    .shrink()
                    .min_w(0.0),
                )
                .child(new_token),
        );
    }
    kit::group(t, "machine-keys", "Key and addresses", Some("What this browser uses to reach the machine."), rows)
}

/// The mode's name as the page says it (`reachLabel`); None while off or when this Fleet can't say.
fn reach_label(r: &ReachState) -> Option<String> {
    if !r.available {
        return None;
    }
    match r.mode {
        ReachMode::Off => None,
        ReachMode::TailscaleAndHome => Some("Tailscale and home network".into()),
        ReachMode::Home => Some("Home network only".into()),
        ReachMode::Own => Some(r.own_address.clone().unwrap_or_else(|| "Your own address".into())),
    }
}

/// Who can reach home, then why not when it can't be, its addresses and its token.
fn access_rows(s: &State, t: &Theme, name: &str, access: &Access) -> Vec<El> {
    let mut rows = Vec::new();
    let reach = s.machine_data.reach.as_ref();
    let pad = |e: El| row().w_full().px(16.0).py(12.0).child(e);
    if reach.is_some_and(|r| r.available && r.mode == ReachMode::Off) {
        rows.push(pad(para(
            t,
            "machine-access-loopback",
            vec![
                Bit::Plain("Reachable from "),
                Bit::Bold("this machine only".into()),
                Bit::Owned(format!(". Turn it on to use {name} from your phone or another machine.")),
            ],
        )));
    } else if let Some(label) = reach.and_then(reach_label) {
        rows.push(pad(para(
            t,
            "machine-access-reach",
            vec![Bit::Plain("Reachable from other devices: "), Bit::Bold(label), Bit::Plain(". Each one needs its own key.")],
        )));
    }
    let listens = format!("{}:{}", access.host, access.port);
    if !access.remote_reachable && !access.requires_token {
        rows.push(pad(para(
            t,
            "machine-access-listens",
            vec![
                Bit::Owned(format!(
                    "Only this machine can reach it: Fleet listens on {listens}. To add it from another device, start Fleet with "
                )),
                Bit::Code("--host 0.0.0.0"),
                Bit::Plain(" (or your tailnet address), or put "),
                Bit::Code("tailscale serve"),
                Bit::Plain(" in front of it and start Fleet with "),
                Bit::Code("--require-token"),
                Bit::Plain("."),
            ],
        )));
    } else if !access.remote_reachable {
        rows.push(pad(para(
            t,
            "machine-access-proxy",
            vec![
                Bit::Owned(format!("Fleet listens on {listens} and asks every request for the token, so a proxy such as ")),
                Bit::Code("tailscale serve"),
                Bit::Plain(" can forward other devices to it. Use the proxy's address."),
            ],
        )));
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

/// "Who can reach it": the access, or why there is none to show.
fn access_group(s: &State, t: &Theme, name: &str) -> El {
    let rows =
        match &s.machine_page.access {
            AccessState::Loaded(access) => access_rows(s, t, name, access),
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
                let failed = matches!(other, AccessState::Failed(_));
                let mut line = row().w_full().px(16.0).py(12.0).gap(8.0).items_start();
                if failed {
                    line = line.child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0));
                }
                let ink = if failed { t.error_text } else { t.muted };
                vec![line.child(
                    text(words).key("machine-access-note").font(sans(rem(0.875), 400.0, 1.43)).color(ink).wrap().shrink().min_w(0.0),
                )]
            }
        };
    kit::group(t, "phones", "Who can reach it", Some("The access token, the addresses and who may reach this machine."), rows)
}
