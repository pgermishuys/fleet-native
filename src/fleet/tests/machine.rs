//! Machines (`fleet::machine`): weave-fleet's `machine-routes.test.ts` ("kinds and order"), `pairing-link.test.ts`,
//! `pairing.test.ts` ("normalizes typed codes") and the address rules of `machines.ts`, under the same names.
use crate::fleet::machine::*;
use serde_json::json;

const NAME: &str = "https://studio.tail9c2e.ts.net";
const TAIL_IP: &str = "http://100.64.90.72:2113";
const LAN: &str = "http://192.168.1.20:2113";
const HOST: &str = "http://studio:2113";

#[test]
fn kinds() {
    let cases = [
        (NAME, Kind::Tailnet),
        (TAIL_IP, Kind::TailnetIp),
        (LAN, Kind::Lan),
        ("http://10.0.0.5:2113", Kind::Lan),
        ("http://172.20.0.3:2113", Kind::Lan),
        ("http://172.32.0.3:2113", Kind::Other),
        ("http://[fd12:3456::1]:2113", Kind::Lan),
        ("http://[fd7a:115c:a1e0::1]:2113", Kind::TailnetIp),
        (HOST, Kind::Hostname),
        ("http://studio.local:2113", Kind::Hostname),
        ("https://fleet.example.com", Kind::Other),
        ("http://203.0.113.9:2113", Kind::Other),
        ("http://localhost:40123", Kind::Other),
        ("http://100.128.0.1:2113", Kind::Other),
        ("http://nas.home.arpa", Kind::Hostname),
    ];
    for (url, kind) in cases {
        assert_eq!(kind_of(url), kind, "{url}");
    }
    assert_eq!(Kind::TailnetIp.label(), "Tailscale IP");
    assert!(Kind::Tailnet.is_tailscale() && !Kind::Lan.is_tailscale());
}

#[test]
fn moves_an_address_up_and_down_the_list() {
    assert_eq!(move_address(&["a", "b", "c", "d"], 2, 0), ["c", "a", "b", "d"]);
    assert_eq!(move_address(&["a", "b", "c", "d"], 0, 3), ["b", "c", "d", "a"]);
    assert_eq!(move_address(&["a", "b"], 1, -4), ["b", "a"]);
    assert_eq!(move_address(&["a", "b"], 5, 0), ["a", "b"]);
}

#[test]
fn an_address_is_normalized_like_a_browser_would() {
    let ok = |typed: &str, want: &str| assert_eq!(normalize_base_url(typed).as_deref(), Ok(want), "{typed}");
    ok("100.64.90.72:2113", "http://100.64.90.72:2113");
    ok("  http://Falcon:2113// ", "http://falcon:2113");
    ok("https://falcon.tail9c2e.ts.net/", "https://falcon.tail9c2e.ts.net");
    ok("https://falcon.tail9c2e.ts.net:443/fleet/", "https://falcon.tail9c2e.ts.net/fleet");
    ok("HTTP://falcon.example.test:80", "http://falcon.example.test");
    ok("[fd12:3456::1]:2113", "http://[fd12:3456::1]:2113");
    for bad in ["http://", "not a link at all", "", "http://host:99999", "http://host:abc"] {
        assert_eq!(normalize_base_url(bad).unwrap_err(), "That isn't an address. Try something like http://100.64.90.72:6262.", "{bad}");
    }
}

#[test]
fn normalizes_typed_codes() {
    assert_eq!(normalize_manual_code("abcd efgh").as_deref(), Some("ABCD-EFGH"));
    assert_eq!(normalize_manual_code("oil0-1234").as_deref(), Some("0110-1234"));
    assert_eq!(normalize_manual_code("ABCD-EFG"), None);
    assert_eq!(normalize_manual_code("ABCD-EFGU"), None);
}

fn link(base: &str, code: &str) -> Option<PairingInput> {
    Some(PairingInput::Link { base_url: base.into(), code: code.into() })
}

fn address(base: &str) -> Option<PairingInput> {
    Some(PairingInput::Address { base_url: base.into() })
}

fn invalid(typed: &str) -> bool {
    matches!(parse_pairing_input(typed), Some(PairingInput::Invalid(_)))
}

#[test]
fn reads_a_link_into_its_address_and_code() {
    assert_eq!(parse_pairing_input("http://100.64.90.72:2113/add-machine#K3MQ-9ZDT"), link("http://100.64.90.72:2113", "K3MQ-9ZDT"));
}

#[test]
fn keeps_the_scheme_port_and_path_of_the_address_and_ignores_spaces_and_a_lower_case_code() {
    assert_eq!(
        parse_pairing_input("  https://falcon.tail9c2e.ts.net/fleet/add-machine#k3mq9zdt \n"),
        link("https://falcon.tail9c2e.ts.net/fleet", "K3MQ-9ZDT")
    );
}

#[test]
fn takes_a_bare_address_and_asks_for_the_code_next() {
    assert_eq!(parse_pairing_input("100.64.90.72:2113"), address("http://100.64.90.72:2113"));
    assert_eq!(parse_pairing_input("https://falcon.example.test/"), address("https://falcon.example.test"));
}

#[test]
fn reads_an_address_that_still_has_add_machine_but_no_code() {
    assert_eq!(parse_pairing_input("http://falcon:2113/add-machine"), address("http://falcon:2113"));
}

#[test]
fn refuses_a_link_whose_code_is_missing_or_wrong() {
    assert!(invalid("http://falcon:2113/add-machine#"));
    assert!(invalid("http://falcon:2113/add-machine#NOPE"));
}

#[test]
fn refuses_what_isnt_an_address() {
    assert!(invalid("http://"));
    assert!(invalid("not a link at all"));
}

#[test]
fn says_nothing_while_the_field_is_empty() {
    assert_eq!(parse_pairing_input(""), None);
    assert_eq!(parse_pairing_input("   "), None);
}

#[test]
fn describes_how_an_address_is_reached() {
    let cases = [
        ("http://100.64.90.72:2113", "over Tailscale at 100.64.90.72:2113"),
        ("https://falcon.tail9c2e.ts.net", "over Tailscale at falcon.tail9c2e.ts.net"),
        ("http://192.168.1.20:2113", "on your network at 192.168.1.20:2113"),
        ("http://falcon.local:2113", "on your network at falcon.local:2113"),
        ("http://localhost:2113", "on this machine at localhost:2113"),
        ("http://falcon.example.test:2113", "at falcon.example.test:2113"),
    ];
    for (url, said) in cases {
        assert_eq!(describe_reach(url), said, "{url}");
    }
}

#[test]
fn a_machine_reads_from_what_home_lists_and_uses_the_address_it_picked() {
    let m = Machine::from_json(&json!({
        "id": "machine-studio", "name": "studio", "baseUrl": "https://studio.tail9c2e.ts.net/", "token": "t0k", "os": "linux",
        "addedAt": "2026-11-06T09:00:00Z", "sshDestination": null, "tunnelUp": null,
        "addresses": [
            {"url": "https://studio.tail9c2e.ts.net", "kind": "tailnet", "learned": false, "lastResult": "ok", "lastTriedAt": "2026-11-15T08:00:00Z"},
            {"url": LAN, "kind": "lan", "learned": true, "lastResult": "no-answer"},
            {"url": "http://old:1", "learned": true},
        ],
    }))
    .unwrap();
    assert_eq!((m.id.as_str(), m.name.as_str(), m.base()), ("machine-studio", "studio", "https://studio.tail9c2e.ts.net"));
    assert_eq!(m.addresses.len(), 3);
    assert_eq!(m.addresses[1].kind, Kind::Lan);
    assert_eq!(m.addresses[1].last_result, Some(AddressResult::NoAnswer));
    assert_eq!(m.addresses[2].kind, Kind::Hostname);
    assert_eq!(m.token.as_ref().map(|t| t.expose()), Some("t0k"));
    // Home's address until a pick, and home's again once the pick is no longer one of the machine's.
    assert_eq!(m.base_url_with(Some(LAN)), LAN);
    assert_eq!(m.base_url_with(Some("http://192.168.1.99:2113")), m.base());
    // A null token (a sign-in that can't reach other machines) is no token.
    let none = Machine::from_json(&json!({"id": "x", "baseUrl": "http://x:1", "token": null})).unwrap();
    assert!(none.token.is_none() && none.addresses.is_empty());
    assert!(Machine::from_json(&json!({"name": "no id", "baseUrl": "http://x:1"})).is_none());
}

#[test]
fn a_target_is_home_or_the_machines_id() {
    let home = Target::home("http://127.0.0.1:6262");
    assert!(home.is_home() && home.key == "home");
    let m = Machine::from_json(&json!({"id": "machine-studio", "baseUrl": TAIL_IP})).unwrap();
    assert_eq!(m.target(), Target { key: "machine-studio".into(), base: TAIL_IP.into() });
    assert!(!m.target().is_home());
}
