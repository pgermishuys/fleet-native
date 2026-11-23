//! How a machine is reached (piece M of docs/parity/17-machines.md): the cases of `MachinesSection.test.ts`, "Settings → Machines:
//! addresses", and of "says how an SSH machine is reached", under Fleet's names, driven with no window. The machines are the
//! invented ones of `mock::machines_page::show_machines_page_addresses`; the cases that save use a fake home that records the request.
use crate::tests::{FRAME, click};
use crate::tests_machines_page::{Fake, Rig, Rt, card_of, exists, fake, page, words};
use rune::{Key, KeyEvent, Mods};
use serde_json::json;

const KEY: &str = "m-falcon";
const URLS: [&str; 4] = ["http://falcon.tail4f2a.ts.net:2113", "http://100.88.14.3:2113", "http://192.168.1.42:2113", "http://falcon:2113"];

fn addresses_page() -> Rt {
    let mut rt = page();
    rt.update(|s, _| {
        s.show_machines_page_addresses(true);
        s.manage_machine(KEY);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

/// Only falcon beside home, so its list is on the screen to be clicked, with every write going to `home`.
fn rig(home: &Fake) -> Rig {
    let mut rig = Rig::new(home);
    rig.rt.update(|s, _| {
        s.show_machines_page_addresses(true);
        s.machines.list.retain(|m| m.id == KEY);
        s.manage_machine(KEY);
    });
    rig.rt.frame(FRAME);
    rig.rt.frame(FRAME);
    rig
}

/// Another machine's own page (`openPage`).
fn manage(rt: &mut Rt, key: &str) {
    rt.update(|s, _| s.manage_machine(key));
    rt.frame(FRAME);
}

fn kinds(rt: &Rt) -> Vec<&'static str> {
    card_of(rt, KEY).addresses.iter().map(|a| a.kind).collect()
}

fn alt(rt: &mut Rt, key: Key) {
    rt.key(KeyEvent { key, mods: Mods { alt: true, ..Mods::default() } });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

#[test]
fn lists_the_addresses_in_order_and_marks_the_one_in_use() {
    let rt = addresses_page();
    let card = card_of(&rt, KEY);
    assert_eq!(kinds(&rt), ["Tailscale", "Tailscale IP", "Local network", "Host name"]);
    let status: Vec<String> = (0..4).map(|i| words(&rt, &format!("machine-address-status-{KEY}-{i}"))).collect();
    assert_eq!(status, ["No answer", "No answer", "In use", "Not tried"]);
    assert_eq!(card.addresses.iter().map(|a| a.in_use).collect::<Vec<_>>(), [false, false, true, false]);
    assert_eq!(card.addresses.iter().map(|a| a.url.as_str()).collect::<Vec<_>>(), URLS);
    assert_eq!(card.addresses.iter().map(|a| a.learned).collect::<Vec<_>>(), [false, true, true, true], "added, then learned");
}

#[test]
fn says_connected_on_your_network_for_a_local_address_and_why_not_tailscale() {
    let rt = addresses_page();
    assert_eq!(words(&rt, &format!("machine-route-{KEY}")), "Connected on your network · Tailscale isn't answering");
    assert_eq!(words(&rt, &format!("machine-route-note-{KEY}")), " · Tailscale isn't answering");
}

#[test]
fn says_connected_over_tailscale_on_its_ip_without_saying_tailscale_isnt_answering() {
    let mut rt = addresses_page();
    rt.update(|s, _| s.machines.list.iter_mut().find(|m| m.id == KEY).unwrap().base_url = URLS[1].into());
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(words(&rt, &format!("machine-route-{KEY}")), "Connected over Tailscale");
    assert_eq!(card_of(&rt, KEY).route.map(|r| r.note), Some(None));
    assert_eq!(card_of(&rt, KEY).addresses[1].status, "In use");
}

#[test]
fn says_how_the_machine_is_reached_only_while_it_answers_and_shows_one_address_plainly() {
    let mut rt = addresses_page();
    manage(&mut rt, "m-harbor");
    assert!(!exists(&rt, "machine-route-m-harbor"), "an SSH machine has no addresses to order");
    manage(&mut rt, "m-lantern");
    assert!(!exists(&rt, "machine-route-m-lantern"), "one still being read says nothing yet");
    assert_eq!(words(&rt, "machine-address-single-m-lantern"), "http://lantern.example.test:2113");
    manage(&mut rt, "home");
    assert!(!exists(&rt, "machine-addresses-home"), "home has none");
}

#[test]
fn says_how_an_ssh_machine_is_reached_and_when_its_tunnel_is_down() {
    let mut rt = addresses_page();
    manage(&mut rt, "m-harbor");
    assert_eq!(words(&rt, "machine-location-m-harbor"), "Over SSH as alice@harbor");
    assert_eq!(words(&rt, "machine-state-m-harbor"), "The SSH tunnel to harbor is down");
    assert!(!exists(&rt, "machine-addresses-m-harbor"));
}

#[test]
fn moves_an_address_up_with_alt_arrowup_and_saves_the_new_order() {
    let home = fake(vec![("PUT", "/api/machines/m-falcon", 200, "{}".into())]);
    let mut rig = rig(&home);
    click(&mut rig.rt, &format!("machine-address-handle-{KEY}-2"));
    alt(&mut rig.rt, Key::Up);
    assert_eq!(kinds(&rig.rt), ["Tailscale", "Local network", "Tailscale IP", "Host name"], "the new order shows at once");
    assert_eq!(
        rig.rt.state.machine_page.addresses.announcement.get(KEY).map(String::as_str),
        Some("Local network moved to position 2 of 4")
    );
    rig.wait("the order saved", |s| s.machine_page.addresses.saving.is_none() && s.machine_page.addresses.order.is_empty());
    let sent = home.seen("PUT", "/api/machines/m-falcon");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, json!({ "addresses": [URLS[0], URLS[2], URLS[1], URLS[3]] }), "every address, in the new order");
    assert_eq!(kinds(&rig.rt), ["Tailscale", "Local network", "Tailscale IP", "Host name"], "and it stays");
}

#[test]
fn puts_the_order_back_and_shows_the_error_when_saving_fails() {
    let home = fake(vec![("PUT", "/api/machines/m-falcon", 409, json!({ "error": "The list changed meanwhile." }).to_string())]);
    let mut rig = rig(&home);
    click(&mut rig.rt, &format!("machine-address-handle-{KEY}-3"));
    alt(&mut rig.rt, Key::Up);
    assert_eq!(kinds(&rig.rt), ["Tailscale", "Tailscale IP", "Host name", "Local network"], "moved at once");
    rig.wait("the refusal", |s| s.machine_page.errors.contains_key(KEY));
    assert_eq!(kinds(&rig.rt), ["Tailscale", "Tailscale IP", "Local network", "Host name"]);
    assert!(words(&rig.rt, &format!("machine-error-{KEY}")).contains("changed meanwhile"));
    assert_eq!(rig.rt.state.machine_page.addresses.announcement.get(KEY).map(String::as_str), Some("The order wasn't saved"));
}

#[test]
fn plain_arrow_keys_move_a_row_too_and_the_ends_stay_put() {
    let mut rt = addresses_page();
    click(&mut rt, &format!("machine-address-handle-{KEY}-0"));
    alt(&mut rt, Key::Up);
    assert_eq!(kinds(&rt), ["Tailscale", "Tailscale IP", "Local network", "Host name"], "the first can't go up");
    rt.key(KeyEvent { key: Key::Down, mods: Mods::default() });
    rt.frame(FRAME);
    assert_eq!(kinds(&rt), ["Tailscale IP", "Tailscale", "Local network", "Host name"]);
}

#[test]
fn a_dragged_address_takes_the_place_of_the_row_it_is_dropped_on_and_escape_drops_nothing() {
    let mut rt = addresses_page();
    let grab = |rt: &mut Rt| {
        let (x, y) = rt.find(&format!("machine-address-handle-{KEY}-3")).expect("the handle");
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_move(x + 12.0, y);
        rt.frame(FRAME);
        let (tx, ty) = rt.find(&format!("machine-address-{KEY}-0")).expect("the first row");
        rt.pointer_move(tx + 60.0, ty);
        rt.frame(FRAME);
        rt.frame(FRAME);
        (tx + 60.0, ty)
    };
    let end = grab(&mut rt);
    assert_eq!(rt.state.machine_page.addresses.drag.as_ref().and_then(|d| d.over), Some(0), "the row under the pointer");
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.pointer_up(end.0, end.1);
    rt.frame(FRAME);
    assert_eq!(kinds(&rt), ["Tailscale", "Tailscale IP", "Local network", "Host name"], "Escape drops nothing");
    let end = grab(&mut rt);
    rt.pointer_up(end.0, end.1);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(kinds(&rt), ["Host name", "Tailscale", "Tailscale IP", "Local network"]);
    assert!(rt.state.machine_page.addresses.drag.is_none());
}

#[test]
fn nothing_is_shown_for_a_machine_with_no_addresses() {
    let mut rt = page();
    rt.update(|s, _| s.manage_machine(KEY));
    rt.frame(FRAME);
    assert!(card_of(&rt, KEY).addresses.is_empty());
    assert!(!exists(&rt, &format!("machine-addresses-{KEY}")) && card_of(&rt, KEY).route.is_none());
}
