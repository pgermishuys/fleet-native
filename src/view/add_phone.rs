//! Add your phone, before the code (`AddPhonePanel.vue`'s first card): "Use Fleet from your phone", where the phone will reach
//! home (`phone-readiness.ts` `reachLines`), **Add your phone** and **Use my own address** with its card. The card that shows the
//! code (the QR, the typed code, the countdown, "<name> is connected.") waits for the owner's phones decision (26, row 9.1); when
//! home has made a code the panel says it can't show it yet. Asking to let others reach home first is the reach dialog's (J).
use crate::fleet::machines::reach::{WayIn, WayLine};
use crate::icons::{GLOBE, SMARTPHONE, WIFI};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, button_lead, disabled};
use crate::ui::confirm::{error_note, spinner};
use crate::ui::rem;
use crate::ui::text_menu::text_input;
use crate::{State, Tx};
use rune::{El, Key, KeyEvent, col, icon, row, text};

/// `max-w-prose` of `text-sm`.
const PROSE: f32 = 490.0;

/// `.add-phone__card`: padding 16, a border, the card colour.
fn card(t: &Theme, key: &str) -> El {
    col().key(key.to_string()).w_full().px(16.0).py(16.0).radius(RADIUS_CARD).border(1.0, t.border).border_layout().fill(t.card_bg)
}

fn heading(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(rem(0.875), 600.0, 1.43)).color(t.text)
}

fn prose(t: &Theme, words: String) -> El {
    text(words).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().max_w(PROSE).mt(rem(0.25))
}

/// `.add-phone__line`: the way's glyph, what the phone needs, and the hosts in mono.
fn way(t: &Theme, line: &WayLine) -> El {
    let (id, glyph) = match line.id {
        WayIn::Home => ("home", &WIFI),
        WayIn::Tailscale => ("tailscale", &GLOBE),
        WayIn::Own => ("own", &GLOBE),
    };
    row()
        .key(format!("add-phone-reach-{id}"))
        .flex_wrap()
        .center_y()
        .gap(8.0)
        .child(icon(glyph, 14.0).color(t.running_text))
        .child(text(line.title.to_string()).font(sans(13.0, 400.0, 1.43)).color(t.text))
        .child(text(line.hosts.join(", ")).font(mono(rem(0.75), 400.0, 1.33)).color(t.muted))
}

/// `.add-phone__reach`: "Your phone will reach <name>" and a line per way in, under a rule.
fn readiness(t: &Theme, name: &str, ways: &[WayLine]) -> El {
    let mut lines = col().gap(rem(0.25)).mt(rem(0.375));
    for line in ways {
        lines = lines.child(way(t, line));
    }
    col()
        .key("add-phone-readiness")
        .w_full()
        .mt(16.0)
        .pt(12.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], t.border)
        .child(text(format!("Your phone will reach {name}")).font(sans(rem(0.875), 500.0, 1.43)).color(t.text))
        .child(lines)
}

fn own_address_link(t: &Theme) -> El {
    button(t, Variant::Link, Size::Sm, None, "Use my own address")
        .key("add-phone-set-address")
        .on_press(|s: &mut State| s.toggle_phone_address())
}

/// The first card: what a phone does, where it will reach home, Add your phone and Use my own address.
fn first_card(s: &State, t: &Theme, name: &str) -> El {
    let phone = &s.machine_data.phone;
    let ways = s.phone_ways();
    let mut c = card(t, "add-phone-card").child(heading(t, "Use Fleet from your phone")).child(prose(
        t,
        format!("Answer agents, start sessions and get notifications on your phone. It connects straight to {name}, and nothing goes through Weave."),
    ));
    if !ways.is_empty() {
        c = c.child(readiness(t, name, &ways));
    }
    let lead = if phone.busy { spinner(t.text) } else { icon(&SMARTPHONE, rem(1.0)).color(t.text) };
    let start = button_lead(t, Variant::Outline, Size::Sm, Some(lead), "Add your phone").key("add-phone-start");
    let start = if phone.busy { disabled(start) } else { start.on_press_tx(|s: &mut State, tx: &mut Tx| s.add_phone(tx)) };
    c = c.child(row().flex_wrap().center_y().gap(rem(0.75)).mt(rem(1.0)).child(start).child(own_address_link(t)));
    if phone.made {
        // The code card (its QR) isn't in the native app yet.
        c = c.child(
            text(format!("{name} made a code, but the native app can't show it yet. Open Fleet in a browser to add your phone."))
                .key("add-phone-made")
                .font(sans(rem(0.875), 400.0, 1.43))
                .color(t.muted)
                .wrap()
                .max_w(PROSE)
                .mt(rem(0.5)),
        );
    }
    c
}

/// The field and its keys: Enter shows the code, Escape cancels.
fn field(s: &State, t: &Theme) -> El {
    text_input(&s.machine_data.phone.url, sans(rem(0.875), 400.0, 1.43))
        .key("add-phone-url")
        .label("Your own address")
        .single_line()
        .placeholder("https://fleet.example.com".to_string(), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .grow(1.0)
        .max_w(rem(28.0))
        .min_w(0.0)
        .px(rem(0.75))
        .py(rem(0.5))
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .focus_ring(1.0, 0.0, t.accent)
        .on_edit(|s: &mut State| &mut s.machine_data.phone.url)
        .on_input_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| match ev.key {
            Key::Enter => {
                s.save_phone_address(tx);
                true
            }
            Key::Escape => {
                s.cancel_phone_address();
                true
            }
            _ => false,
        })
}

/// "Use my own address": an address of the person's own, which the code carries first.
fn own_address_card(s: &State, t: &Theme, name: &str) -> El {
    let phone = &s.machine_data.phone;
    let show = button(t, Variant::Default, Size::Sm, None, "Show the code").key("add-phone-show-code");
    let show = if phone.busy || phone.url.text.trim().is_empty() {
        disabled(show)
    } else {
        show.on_press_tx(|s: &mut State, tx: &mut Tx| s.save_phone_address(tx))
    };
    let cancel =
        button(t, Variant::Ghost, Size::Sm, None, "Cancel").key("add-phone-own-cancel").on_press(|s: &mut State| s.cancel_phone_address());
    card(t, "add-phone-own-address")
        .mt(rem(0.75))
        .child(heading(t, "Use my own address"))
        .child(prose(
            t,
            format!("An address you run yourself that reaches {name}: a tunnel, or a domain such as https://fleet.example.com. The code carries it first, ahead of the ones Fleet found."),
        ))
        .child(row().w_full().flex_wrap().center_y().gap(rem(0.5)).mt(rem(0.75)).child(field(s, t)).child(show).child(cancel))
}

/// The panel: the first card, the own-address card when open, and what went wrong.
pub fn panel(s: &State, t: &Theme, name: &str) -> El {
    let phone = &s.machine_data.phone;
    let mut e = col().key("add-phone").w_full().child(first_card(s, t, name));
    if phone.editing_url {
        e = e.child(own_address_card(s, t, name));
    }
    if let Some(why) = &phone.error {
        e = e.child(error_note(t, "add-phone-error", why, None).mt(rem(0.5)));
    }
    e
}
