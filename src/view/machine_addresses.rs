//! A card's "how it is reached" lines and its address list (`MachinesSection.vue` `routeLine`, `MachineAddressList.vue`): the
//! address in use and the others in the order Fleet tries them. Drag a row by its handle, or focus the handle and press
//! Alt+Up / Alt+Down (plain Up / Down work too); with one address there is nothing to order, so it is shown plainly.
use crate::fleet::machine_addresses::{handle_key, row_key};
use crate::fleet::machine_card::{AddressRow, Card, Route};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::{State, Tx};
use rune::{Cursor, Drag, El, Key, KeyEvent, Phase, col, el, row, text};

/// "Connected on your network" and, when Fleet fell back off Tailscale, why.
pub fn route(t: &Theme, key: &str, r: &Route) -> El {
    let small = mono(11.5, 400.0, 1.4);
    let mut line =
        row().key(format!("machine-route-{key}")).flex_wrap().w_full().child(text(r.text.clone()).font(small).color(t.running_text));
    if let Some(note) = &r.note {
        line = line.child(text(format!(" · {note}")).key(format!("machine-route-note-{key}")).font(small).color(t.muted));
    }
    line
}

/// The six dots of lucide's `grip-vertical`, 14 px.
fn grip(t: &Theme) -> El {
    let dots = || (0..3).fold(col().gap(2.6), |c, _| c.child(el().size(2.4, 2.4).radius(1.2).fill(t.muted)));
    row().size(22.0, 22.0).center().gap(3.0).child(dots()).child(dots())
}

fn handle(s: &State, t: &Theme, card: &str, at: usize, count: usize, a: &AddressRow) -> El {
    let saving = s.machine_page.addresses.saving.is_some();
    let (up, from) = (card.to_string(), card.to_string());
    let e = row()
        .key(handle_key(card, at))
        .role("button")
        .label(format!("{}, position {} of {count}. Move up or down with Alt and the arrow keys", a.kind, at + 1))
        .help(format!("Drag to reorder, or press Alt+Up or Alt+Down to move {}", a.kind))
        .radius(RADIUS_BTN)
        .cursor(Cursor::Grab)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(grip(t));
    if saving {
        return e.disabled(true).opacity(0.5);
    }
    e.on_key(move |s: &mut State, ev: &KeyEvent, tx: &mut Tx| match ev.key {
        Key::Up | Key::Down => {
            s.address_key(&up, at, ev.key == Key::Up, tx);
            true
        }
        _ => false,
    })
    .on_drag(move |s: &mut State, d: &mut Drag, tx: &mut Tx| match d.phase {
        Phase::Down => {}
        Phase::Move => s.address_drag_move(&from, at, d.window_point(), tx),
        Phase::Up => s.address_drag_end(d.cancelled(), tx),
    })
}

fn line(s: &State, t: &Theme, card: &str, at: usize, count: usize, a: &AddressRow) -> El {
    let drag = s.machine_page.addresses.drag.as_ref().filter(|d| d.key == card);
    let dragging = drag.is_some_and(|d| d.from == at);
    let over = drag.is_some_and(|d| d.over == Some(at) && d.from != at);
    let mut e = row()
        .key(row_key(card, at))
        .w_full()
        .flex_wrap()
        .center_y()
        .gap(6.0)
        .pl(4.0)
        .pr(8.0)
        .py(5.0)
        .radius(RADIUS_BTN)
        .border(
            1.0,
            if over {
                t.accent
            } else if a.in_use {
                t.border
            } else {
                rune::Color::TRANSPARENT
            },
        )
        .border_layout()
        .opacity(if dragging { 0.5 } else { 1.0 });
    if a.in_use {
        e = e.fill(t.accent_dim);
    }
    let origin = if a.learned { "learned" } else { "added" };
    let status_ink = if a.in_use { t.running_text } else { t.muted };
    e.child(handle(s, t, card, at, count, a))
        .child(text((at + 1).to_string()).font(mono(12.0, 400.0, 1.4)).color(t.muted).w(12.0))
        .child(text(a.kind).font(sans(12.0, 600.0, 1.4)).color(t.text))
        .child(text(a.url.clone()).font(mono(11.5, 400.0, 1.4)).color(t.muted).wrap().grow(1.0).shrink().min_w(0.0))
        .child(text(origin).font(sans(10.5, 400.0, 1.4)).color(t.muted).help(if a.learned {
            "The machine told Fleet about this address"
        } else {
            "The address this machine was added with"
        }))
        .child(
            row().min_w(78.0).justify_end().child(
                text(a.status)
                    .key(format!("machine-address-status-{card}-{at}"))
                    .font(sans(11.5, if a.in_use { 600.0 } else { 400.0 }, 1.4))
                    .color(status_ink),
            ),
        )
}

/// The list under a card's state lines; nothing for a machine with no addresses.
pub fn list(s: &State, t: &Theme, c: &Card) -> Option<El> {
    if c.addresses.is_empty() {
        return None;
    }
    let small = sans(11.5, 400.0, 1.4);
    let mut e = col().key(format!("machine-addresses-{}", c.key)).w_full().gap(6.0).mt(6.0);
    if let [only] = c.addresses.as_slice() {
        return Some(
            e.child(text("Address").font(small).color(t.muted)).child(
                text(only.url.clone())
                    .key(format!("machine-address-single-{}", c.key))
                    .font(mono(11.5, 400.0, 1.4))
                    .color(t.text)
                    .wrap()
                    .w_full(),
            ),
        );
    }
    let mut rows = col().role("list").label("Addresses, in the order Fleet tries them").w_full().gap(2.0);
    for (at, a) in c.addresses.iter().enumerate() {
        rows = rows.child(line(s, t, &c.key, at, c.addresses.len(), a));
    }
    e = e
        .child(text("Fleet tries these in order and goes back to a higher one as soon as it answers again.").font(small).color(t.muted).wrap().w_full())
        .child(rows)
        .child(
            text("\"Learned\" means the machine reported the address itself, the last time Fleet reached it. If the router gives it a new one, Fleet updates the address on its own.")
                .font(small)
                .color(t.muted)
                .wrap()
                .w_full(),
        );
    Some(e)
}
