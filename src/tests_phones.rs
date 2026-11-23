//! Settings → Machines → "Phones and browsers" (piece I of docs/parity/26-parity-audit.md): `DevicesList.test.ts`, the cases of
//! `AddPhonePanel.test.ts` before the code, and MachinesSection's two phone cases, under Fleet's names. The page is the invented
//! one of `mock::machines_page` in a tall window; the cases that write go to a fake home that records each request.
use crate::Rail;
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::machine_actions::{Access, AccessAddress, AccessState};
use crate::fleet::machines::screens::{NotAnswering, Screen, Screens, remove_words};
use crate::tests::{FRAME, app_tall, click};
use crate::tests_machines_page::{Fake, Rig, Rt, exists, fake, words};
use crate::view::devices_list::{line, not_answering_words};
use rune::{Key, KeyEvent, Mods};
use serde_json::json;
use std::sync::Mutex;
use std::sync::mpsc::channel;

fn page() -> Rt {
    let mut rt = app_tall();
    rt.update(|s, _| {
        s.show_machines_page(true, false);
        s.machines.list.retain(|m| m.id == "m-falcon");
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

/// The page with its home at `home`.
fn rig(home: &Fake) -> Rig {
    let mut rt = page();
    let (tx, jobs) = channel::<Job>();
    let tx = Mutex::new(tx);
    let link = Link::start_machine("home".to_string(), home.base.clone(), Post::new(move |job| drop(tx.lock().unwrap().send(job))));
    rt.update(move |s, _| s.link = Some(link));
    rt.frame(FRAME);
    Rig { rt, jobs }
}

/// `phone` and `studio` of `DevicesList.test.ts`: a phone of this machine's own list, and another machine paired with a code.
fn phone() -> Screen {
    Screen {
        id: "d1".into(),
        name: "Robin's phone".into(),
        platform: Some("ios".into()),
        created_at: "2026-11-01T09:00:00.000Z".into(),
        last_used_at: "2026-11-14T09:00:00.000Z".into(),
        paired_via: None,
        is_machine: false,
        through_name: None,
        fleet_wide: false,
    }
}

fn studio() -> Screen {
    Screen { id: "m1".into(), name: "studio".into(), platform: Some("linux".into()), is_machine: true, ..phone() }
}

fn listing(rt: &mut Rt, devices: Vec<Screen>, not_answering: &[&str]) {
    let not_answering = not_answering.iter().map(|n| NotAnswering { id: format!("m-{n}"), name: n.to_string() }).collect();
    rt.update(move |s, _| s.machine_data.screens = Screens { screens: devices, not_answering });
    rt.frame(FRAME);
}

fn asked(rt: &Rt) -> (String, String, String, bool) {
    let c = rt.state.ui.confirm.as_ref().expect("asked");
    (c.title.clone(), c.body.clone(), c.confirm_label.clone(), c.destructive)
}

// ── DevicesList.test.ts ────────────────────────────────────────────────────────

#[test]
fn asks_first_saying_what_the_device_loses_then_removes_it() {
    let home = fake(vec![
        ("DELETE", "/api/machine/devices/d1", 204, String::new()),
        ("GET", "/api/machine/devices", 200, json!({ "devices": [] }).to_string()),
    ]);
    let mut rig = rig(&home);
    listing(&mut rig.rt, vec![phone(), studio()], &[]);

    click(&mut rig.rt, "remove-device-d1");
    let (title, body, label, destructive) = asked(&rig.rt);
    assert_eq!(title, "Remove Robin's phone on every machine in your Fleet?");
    assert!(body.contains("loses its keys on every machine"), "{body}");
    assert_eq!((label.as_str(), destructive), ("Remove Robin's phone", true));

    click(&mut rig.rt, "confirm-action");
    assert_eq!(rig.rt.state.machine_data.removing.as_deref(), Some("d1"), "its Remove waits");
    rig.wait("the list read again after Remove", |s| s.machine_data.removing.is_none());
    assert_eq!(home.seen("DELETE", "/api/machine/devices/d1").len(), 1, "removed from this machine's own list");
    assert_eq!(home.seen("GET", "/api/machine/devices").len(), 1, "then the list is read again (`removed`)");
    assert!(rig.rt.state.machine_data.screens.screens.is_empty());
}

#[test]
fn says_it_reaches_every_machine_for_a_phone_but_not_for_a_machines_key() {
    let mut rt = page();
    listing(&mut rt, vec![phone(), studio()], &[]);

    click(&mut rt, "remove-device-d1");
    let (title, _, label, destructive) = asked(&rt);
    assert_eq!(
        (title.as_str(), label.as_str(), destructive),
        ("Remove Robin's phone on every machine in your Fleet?", "Remove Robin's phone", true)
    );
    click(&mut rt, "confirm-cancel");

    click(&mut rt, "remove-device-m1");
    assert_eq!(asked(&rt).2, "Remove machine");
    assert_eq!(remove_words(&studio()).0, "Remove studio?");
}

#[test]
fn names_a_paired_machine_as_a_machine() {
    let mut rt = page();
    listing(&mut rt, vec![phone(), studio()], &[]);
    click(&mut rt, "remove-device-m1");
    assert_eq!(asked(&rt).2, "Remove machine");
    assert!(words(&rt, "device-row-m1").contains("Machine, paired Nov 1"));
}

#[test]
fn removes_nothing_when_the_confirm_is_cancelled() {
    let home = fake(vec![("DELETE", "/api/machine/devices/d1", 204, String::new())]);
    let mut rig = rig(&home);
    listing(&mut rig.rt, vec![phone(), studio()], &[]);

    click(&mut rig.rt, "remove-device-d1");
    click(&mut rig.rt, "confirm-cancel");
    for _ in 0..10 {
        rig.rt.frame(FRAME);
    }
    assert!(rig.rt.state.ui.confirm.is_none());
    assert!(home.seen("DELETE", "/api/machine/devices/d1").is_empty());
    assert!(rig.rt.state.machine_data.removing.is_none());
    assert_eq!(rig.rt.state.machine_data.screens.screens.len(), 2);
}

#[test]
fn lists_a_phone_of_the_whole_fleet_with_the_machine_it_connected_through_and_removes_it_by_its_fleet_wide_id() {
    let iphone =
        Screen { id: "screen:abc".into(), name: "Alex's iPhone".into(), through_name: Some("atlas".into()), fleet_wide: true, ..phone() };
    let home = fake(vec![
        ("DELETE", "/api/machine/screens/screen%3Aabc", 204, String::new()),
        ("GET", "/api/machine/screens", 200, json!({ "screens": [], "notAnswering": [] }).to_string()),
    ]);
    let mut rig = rig(&home);
    listing(&mut rig.rt, vec![iphone], &[]);

    assert!(words(&rig.rt, "device-row-screen:abc").contains("Connected through atlas, added"));

    click(&mut rig.rt, "remove-device-screen:abc");
    click(&mut rig.rt, "confirm-action");
    rig.wait("the list read again after Remove", |s| s.machine_data.removing.is_none());
    assert_eq!(home.seen("DELETE", "/api/machine/screens/screen%3Aabc").len(), 1);
    assert!(home.seen("DELETE", "/api/machine/devices/screen%3Aabc").is_empty());
    assert!(rig.rt.state.machine_data.screens.screens.is_empty());
}

#[test]
fn says_which_machines_arent_answering() {
    assert_eq!(not_answering_words(&["atlas".into()]), "atlas isn't answering, so what it holds isn't shown.");
    assert_eq!(
        not_answering_words(&["atlas".into(), "workbench".into()]),
        "atlas, workbench aren't answering, so what they hold isn't shown."
    );

    let mut rt = page();
    listing(&mut rt, vec![], &["atlas"]);
    assert_eq!(words(&rt, "devices-not-answering"), "atlas isn't answering, so what it holds isn't shown.");
    listing(&mut rt, vec![], &["atlas", "workbench"]);
    assert_eq!(words(&rt, "devices-not-answering"), "atlas, workbench aren't answering, so what they hold isn't shown.");
    listing(&mut rt, vec![], &[]);
    assert!(!exists(&rt, "devices-not-answering"));
}

/// The row's second line, on a fixed clock: the date it was added, and how long ago it was last seen (`formatRelative`).
#[test]
fn a_rows_line_says_when_it_was_added_and_last_seen() {
    let now = crate::fleet::work::parse_iso_ms("2026-11-15T09:00:00Z").unwrap();
    assert_eq!(line(&phone(), now, 0), "Added Nov 1 \u{b7} last seen yesterday");
    let through = Screen { through_name: Some("atlas".into()), fleet_wide: true, ..phone() };
    assert_eq!(line(&through, now, 0), "Connected through atlas, added Nov 1 \u{b7} last seen yesterday");
    let via = Screen { paired_via: Some("m-falcon".into()), ..phone() };
    assert_eq!(line(&via, now, 0), "Added Nov 1 from another machine \u{b7} last seen yesterday");
}

// ── AddPhonePanel.test.ts, before the code ─────────────────────────────────────

const URLS: [&str; 2] = ["http://studio.tail4f2a.ts.net:6262", "http://192.168.1.20:6262"];

/// `mountPanel`: home is "studio", listening on Tailscale and the home network unless `addresses` says otherwise.
fn studio_home(rt: &mut Rt, addresses: &[(&str, &str)]) {
    let addresses: Vec<AccessAddress> =
        addresses.iter().map(|(url, kind)| AccessAddress { url: url.to_string(), kind: kind.to_string() }).collect();
    rt.update(move |s, _| {
        if let Some(home) = s.machines.home.as_mut() {
            home.name = "studio".into();
            home.public_url = None;
        }
        if let AccessState::Loaded(a) = &mut s.machine_page.access {
            a.addresses = addresses;
        }
    });
    rt.frame(FRAME);
}

fn both_ways() -> Vec<(&'static str, &'static str)> {
    vec![(URLS[0], "tailnet"), (URLS[1], "lan")]
}

#[test]
fn shows_where_the_phone_will_reach_the_machine_before_there_is_a_code_and_no_address_field() {
    let mut rt = page();
    studio_home(&mut rt, &both_ways());

    assert!(words(&rt, "add-phone-readiness").contains("Your phone will reach studio"));
    assert!(words(&rt, "add-phone-reach-home").contains("192.168.1.20"));
    assert!(words(&rt, "add-phone-reach-tailscale").contains("studio.tail4f2a.ts.net"));
    assert!(!exists(&rt, "add-phone-url"));
    assert!(!words(&rt, "add-phone").contains("Which address will the phone open?"));
}

/// The first half of "asks for a code without naming an address, and shows what the code carries": the card that shows the code
/// waits for the owner's phones decision, so the panel says it can't show it yet.
#[test]
fn asks_for_a_code_without_naming_an_address() {
    let home = fake(vec![("POST", "/api/machine/pairing", 200, json!({ "manualCode": "H4KD-92QX" }).to_string())]);
    let mut rig = rig(&home);
    studio_home(&mut rig.rt, &both_ways());

    click(&mut rig.rt, "add-phone-start");
    rig.wait("the code", |s| s.machine_data.phone.made);
    assert_eq!(home.seen("POST", "/api/machine/pairing")[0].0, json!({}));
    assert!(words(&rig.rt, "add-phone-made").contains("can't show it yet"));
    assert!(!exists(&rig.rt, "add-phone-error"));
}

#[test]
fn keeps_the_address_field_under_use_my_own_address_and_sends_that_address_first() {
    let home = fake(vec![
        ("PUT", "/api/machine", 200, "{}".into()),
        ("POST", "/api/machine/pairing", 200, json!({ "manualCode": "H4KD-92QX" }).to_string()),
    ]);
    let mut rig = rig(&home);
    studio_home(&mut rig.rt, &both_ways());

    click(&mut rig.rt, "add-phone-set-address");
    click(&mut rig.rt, "add-phone-url");
    rig.rt.key(KeyEvent { key: Key::Text("https://fleet.example.com".into()), mods: Mods::default() });
    rig.rt.frame(FRAME);
    click(&mut rig.rt, "add-phone-show-code");
    rig.wait("the code", |s| s.machine_data.phone.made);

    assert_eq!(home.seen("PUT", "/api/machine")[0].0, json!({ "publicUrl": "https://fleet.example.com" }));
    assert_eq!(home.seen("POST", "/api/machine/pairing")[0].0, json!({ "baseUrl": "https://fleet.example.com" }));
    assert!(words(&rig.rt, "add-phone-reach-own").contains("fleet.example.com"));
    assert!(!exists(&rig.rt, "add-phone-url"), "the field closes once the address is saved");
}

#[test]
fn opens_the_address_field_when_fleet_only_listens_on_this_machine() {
    let refused = json!({ "error": "Fleet only listens on this machine, so no phone can reach it." }).to_string();
    let home = fake(vec![("POST", "/api/machine/pairing", 400, refused)]);
    let mut rig = rig(&home);
    studio_home(&mut rig.rt, &[]);

    click(&mut rig.rt, "add-phone-start");
    rig.wait("the refusal", |s| s.machine_data.phone.error.is_some());
    assert!(exists(&rig.rt, "add-phone-url"));
    assert!(words(&rig.rt, "add-phone").contains("only listens on this machine"));
    assert!(!rig.rt.state.machine_data.phone.made);
}

// ── MachinesSection.test.ts ────────────────────────────────────────────────────

#[test]
fn lists_the_phones_and_browsers_with_keys_and_leaves_out_another_machines_key() {
    let devices = json!({ "devices": [
        { "id": "dev-1", "name": "Maya's iPhone", "platform": "ios", "createdAt": "2026-11-01T00:00:00.000Z", "lastUsedAt": "2026-11-18T00:00:00.000Z", "pairedVia": null, "kind": "device" },
        { "id": "m-studio", "name": "studio", "platform": "linux", "createdAt": "2026-11-01T00:00:00.000Z", "lastUsedAt": "2026-11-18T00:00:00.000Z", "pairedVia": null, "kind": "machine" },
    ] });
    let access = json!({ "token": "made-up-token", "tokenSource": "saved", "host": "127.0.0.1", "port": 6262, "remoteReachable": false, "requiresToken": true, "addresses": [] });
    let home =
        fake(vec![("GET", "/api/machine/access", 200, access.to_string()), ("GET", "/api/machine/devices", 200, devices.to_string())]);
    let mut rig = rig(&home);
    // A fresh visit: the access is read, then the phones and browsers.
    rig.rt.update(|s, _| {
        s.machine_page.opened = false;
        s.machine_page.access = AccessState::Loading;
        s.machine_data.screens = Screens::default();
        s.machine_data.screens_loaded = false;
    });
    rig.rt.frame(FRAME);
    rig.wait("the phones and browsers", |s| s.machine_data.screens_loaded);
    rig.rt.frame(FRAME);

    let rows: Vec<String> = rig.rt.elements().iter().filter_map(|e| e.key.clone()).filter(|k| k.starts_with("device-row-")).collect();
    assert_eq!(rows, ["device-row-dev-1"]);
    assert!(words(&rig.rt, "device-row-dev-1").contains("Maya's iPhone"));
    assert!(matches!(rig.rt.state.machine_page.access, AccessState::Loaded(Access { requires_token: true, .. })));
}

#[test]
fn says_removing_a_phone_or_browser_reaches_every_machine() {
    let rt = page();
    assert!(words(&rt, "machine-phones-group").contains("Remove one and it loses them on every machine."));
    assert_eq!(rt.state.rail, Rail::Settings);
}
