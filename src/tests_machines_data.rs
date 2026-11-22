//! Piece E of component 26: the requests and the words behind Settings → Machines' reach line, Found, On a new server, Phones and
//! browsers and the reach dialog. The cases of `machine-reach-line.test.ts`, `reach-choices.test.ts`, `phone-readiness.test.ts` and
//! `use-home-access-screens.test.ts` under Fleet's names, then each request against a fake Fleet. Every name and address is invented.
use crate::fleet::machine::Kind;
use crate::fleet::machines::data::JoinState;
use crate::fleet::machines::found::{FoundMachine, load_found, make_join_code};
use crate::fleet::machines::reach::{
    ReachAddress, ReachLine, ReachLineInput, ReachMode, Tone, WayIn, clock_of, reach_choices, reach_line, reach_lines,
};
use crate::fleet::machines::screens;
use crate::tests_machines_page::{Rig, fake};
use serde_json::json;

const ZONE: i64 = 0;

fn input<'a>(answering: bool, addresses: &'a [Kind]) -> ReachLineInput<'a> {
    ReachLineInput { answering, offline_since: None, addresses, reach_mode: None, own_address: None }
}

fn said(text: &str, tone: Tone) -> ReachLine {
    ReachLine { text: text.into(), tone }
}

// ── machine-reach-line.test.ts ─────────────────────────────────────────────────

#[test]
fn says_a_machine_on_tailscale_and_the_home_network_reaches_your_phone_both_ways() {
    let line = reach_line(&input(true, &[Kind::Tailnet, Kind::Lan]), ZONE);
    assert_eq!(line, said("Reaches your phone over Tailscale and the home network", Tone::Ok));
}

#[test]
fn says_a_machine_with_only_tailscale_addresses_is_only_on_tailscale() {
    let line = reach_line(&input(true, &[Kind::Tailnet, Kind::TailnetIp]), ZONE);
    assert_eq!(line, said("Only on Tailscale", Tone::Limited));
}

#[test]
fn says_the_home_network_alone_and_the_persons_own_address() {
    assert_eq!(reach_line(&input(true, &[Kind::Lan]), ZONE).text, "Reaches your phone on the home network");
    let own = ReachLineInput { reach_mode: Some(ReachMode::Own), own_address: Some("https://fleet.example.com"), ..input(true, &[]) };
    assert_eq!(reach_line(&own, ZONE).text, "Reaches your phone at https://fleet.example.com");
    let none = ReachLineInput { reach_mode: Some(ReachMode::Own), ..input(true, &[]) };
    assert_eq!(reach_line(&none, ZONE).text, "Reaches your phone at your own address");
}

#[test]
fn says_the_desktop_app_is_only_this_machine_until_it_lets_others_reach_it() {
    let off = ReachLineInput { reach_mode: Some(ReachMode::Off), ..input(true, &[]) };
    assert_eq!(reach_line(&off, ZONE), said("Only this machine", Tone::Limited));
}

#[test]
fn says_when_a_machine_stopped_answering_apart_from_the_tailscale_case() {
    // 19 Nov 2026, 08:40 UTC.
    let at = 1_795_077_600_000;
    let gone = ReachLineInput { offline_since: Some(at), ..input(false, &[Kind::Tailnet]) };
    assert_eq!(reach_line(&gone, ZONE), said(&format!("Off since {}", clock_of(at, ZONE)), Tone::Out));
    assert_eq!(clock_of(at, ZONE), "8:40 AM");
    assert_eq!(reach_line(&input(false, &[Kind::Lan]), ZONE).text, "Not reachable");
}

#[test]
fn says_the_machines_own_address_when_that_is_all_it_has() {
    assert_eq!(reach_line(&input(true, &[Kind::Other]), ZONE).text, "Reaches your phone by its own address");
    assert_eq!(reach_line(&input(true, &[Kind::Tailnet, Kind::Lan]), ZONE).tone, Tone::Ok);
    assert_eq!(reach_line(&input(true, &[Kind::Tailnet]), ZONE).text, "Only on Tailscale");
    assert_eq!(reach_line(&input(true, &[]), ZONE), said("Only this machine", Tone::Limited));
}

// ── reach-choices.test.ts ──────────────────────────────────────────────────────

fn found() -> Vec<ReachAddress> {
    let at = |url: &str, kind| ReachAddress { url: url.into(), kind };
    vec![
        at("http://studio.tail4f2a.ts.net:6262", Kind::Tailnet),
        at("http://100.64.90.72:6262", Kind::Tailnet),
        at("http://192.168.1.20:6262", Kind::Lan),
    ]
}

fn labels(c: &crate::fleet::machines::reach::ReachChoice) -> Vec<&str> {
    c.addresses.iter().map(|a| a.label.as_str()).collect()
}

#[test]
fn offers_tailscale_and_home_network_first_recommended_with_the_name_tailscale_gave_and_the_home_address() {
    let choices = reach_choices(&found());
    let [both, home, own] = &choices[..] else { panic!("three choices") };
    assert_eq!(both.mode, ReachMode::TailscaleAndHome);
    assert!(both.recommended);
    assert_eq!(labels(both), ["studio.tail4f2a.ts.net", "192.168.1.20"]);
    assert_eq!(labels(home), ["192.168.1.20"]);
    assert_eq!(own.mode, ReachMode::Own);
    assert!(!own.disabled);
}

#[test]
fn shows_the_tailscale_ip_when_tailscale_gave_no_name() {
    let only = [ReachAddress { url: "http://100.64.90.72:6262".into(), kind: Kind::Tailnet }];
    assert_eq!(labels(&reach_choices(&only)[0]), ["100.64.90.72"]);
}

#[test]
fn switches_off_the_choices_that_have_nothing_to_listen_on_and_says_why() {
    let choices = reach_choices(&[]);
    assert!(choices[0].disabled);
    assert!(choices[1].disabled);
    assert!(choices[1].hint("studio").contains("didn't find a home network address"));
    assert!(!choices[2].disabled);
}

#[test]
fn only_recommends_the_first_choice_when_tailscale_is_there_and_says_what_to_expect_without_it() {
    let lan = [ReachAddress { url: "http://192.168.1.20:6262".into(), kind: Kind::Lan }];
    let both = &reach_choices(&lan)[0];
    assert!(!both.recommended);
    assert!(both.hint("studio").contains("at home only"));
    assert_eq!(reach_choices(&found())[0].hint("studio"), "Your phone reaches studio at home and away.");
    assert_eq!(reach_choices(&found())[1].hint("studio"), "At home only. Away from home, the phone shows studio as out of reach.");
}

// ── phone-readiness.test.ts ────────────────────────────────────────────────────

fn way(id: WayIn, title: &'static str, hosts: &[&str]) -> crate::fleet::machines::reach::WayLine {
    crate::fleet::machines::reach::WayLine { id, title, hosts: hosts.iter().map(|h| h.to_string()).collect() }
}

#[test]
fn reach_lines_name_the_home_network_and_tailscale_each_with_its_address() {
    assert_eq!(
        reach_lines(&["http://studio.tail4f2a.ts.net:6262", "http://192.168.1.20:6262"]),
        [
            way(WayIn::Home, "At home, on your Wi-Fi", &["192.168.1.20"]),
            way(WayIn::Tailscale, "Away from home, with Tailscale on the phone", &["studio.tail4f2a.ts.net"]),
        ]
    );
}

#[test]
fn reach_lines_show_the_tailscale_address_only_when_there_is_no_name() {
    let hosts = |urls: &[&str]| reach_lines(urls).into_iter().map(|l| l.hosts).collect::<Vec<_>>();
    assert_eq!(hosts(&["http://100.64.90.72:6262"]), [["100.64.90.72"]]);
    assert_eq!(hosts(&["http://100.64.90.72:6262", "http://studio.tail4f2a.ts.net:6262"]), [["studio.tail4f2a.ts.net"]]);
}

#[test]
fn reach_lines_fall_back_to_the_host_name_for_the_home_network_when_there_is_no_private_address() {
    assert_eq!(reach_lines(&["http://studio:6262"]), [way(WayIn::Home, "At home, on your Wi-Fi", &["studio"])]);
}

#[test]
fn reach_lines_list_an_address_of_the_persons_own_and_never_a_loopback_one() {
    assert_eq!(
        reach_lines(&["https://fleet.example.com", "http://127.0.0.1:6262", "http://localhost:6262"]),
        [way(WayIn::Own, "From anywhere, at your own address", &["fleet.example.com"])]
    );
}

#[test]
fn reach_lines_have_nothing_to_say_about_a_machine_that_listens_only_on_itself() {
    assert!(reach_lines(&["http://127.0.0.1:6262"]).is_empty());
    assert!(reach_lines(&[]).is_empty());
}

// ── use-home-access-screens.test.ts, against a fake Fleet ──────────────────────

fn own_list() -> String {
    json!({ "devices": [
        { "id": "d1", "name": "Rowan's phone", "platform": "ios", "createdAt": "2026-11-01T09:00:00Z", "lastUsedAt": "2026-11-02T09:00:00Z", "pairedVia": null, "kind": "device" },
        { "id": "m1", "name": "studio", "platform": null, "createdAt": "2026-11-01T09:00:00Z", "lastUsedAt": "2026-11-02T09:00:00Z", "pairedVia": null, "kind": "machine" },
    ] })
    .to_string()
}

fn fleet_list() -> String {
    json!({
        "screens": [{ "id": "screen:abc", "name": "Rowan's tablet", "platform": "android", "throughId": "m-atlas", "throughName": "atlas",
                      "createdAt": "2026-11-01T09:00:00Z", "lastUsedAt": "2026-11-02T09:00:00Z", "machines": ["m-atlas"] }],
        "notAnswering": [{ "id": "m2", "name": "workbench" }],
    })
    .to_string()
}

#[test]
fn use_home_access_lists_the_whole_fleet_and_keeps_the_machines_that_didnt_answer() {
    let home = fake(vec![("GET", "/api/machine/screens", 200, fleet_list()), ("GET", "/api/machine/devices", 200, own_list())]);
    let list = screens::load(&home.base);
    assert_eq!(list.screens.len(), 1);
    assert_eq!(list.screens[0].id, "screen:abc");
    assert_eq!(list.screens[0].through_name.as_deref(), Some("atlas"));
    assert_eq!(list.screens[0].paired_via.as_deref(), Some("m-atlas"));
    assert!(list.screens[0].fleet_wide);
    assert_eq!((list.not_answering[0].id.as_str(), list.not_answering[0].name.as_str()), ("m2", "workbench"));
    assert!(home.seen("GET", "/api/machine/devices").is_empty(), "the own list is only for a Fleet with no Fleet-wide one");
}

#[test]
fn use_home_access_falls_back_to_this_machines_own_list_when_the_fleet_wide_list_cant_be_had() {
    let home = fake(vec![
        ("GET", "/api/machine/screens", 500, json!({ "error": "An older Fleet has no Fleet-wide list." }).to_string()),
        ("GET", "/api/machine/devices", 200, own_list()),
    ]);
    let list = screens::load(&home.base);
    assert_eq!(list.screens.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["d1"], "another machine's key is not a screen");
    assert!(list.not_answering.is_empty());
    assert!(!list.screens[0].fleet_wide);
}

// ── the requests ───────────────────────────────────────────────────────────────

#[test]
fn removes_a_screen_by_its_fleet_wide_id_and_reads_the_list_again() {
    let home =
        fake(vec![("DELETE", "/api/machine/screens/screen%3Aabc", 204, String::new()), ("GET", "/api/machine/screens", 200, fleet_list())]);
    let mut rig = Rig::new(&home);
    rig.rt.update(|s, tx| s.remove_screen("screen:abc", tx));
    rig.wait("the list after Remove", |s| s.machine_data.screens.screens.len() == 1);
    assert_eq!(home.seen("DELETE", "/api/machine/screens/screen%3Aabc").len(), 1);
}

#[test]
fn a_removal_the_server_refuses_says_why_and_a_missing_one_is_done() {
    let home = fake(vec![
        ("DELETE", "/api/machine/screens/a", 403, json!({ "error": "This browser may not remove devices." }).to_string()),
        ("DELETE", "/api/machine/screens/b", 404, "{}".into()),
    ]);
    assert_eq!(screens::remove(&home.base, "a"), Err("This browser may not remove devices.".to_string()));
    assert_eq!(screens::remove(&home.base, "b"), Ok(()));
}

fn waiting() -> serde_json::Value {
    json!({ "machineId": "m-atlas", "name": "atlas", "os": "linux", "baseUrl": "http://192.168.1.30:6262", "hint": "7K3M", "source": "Lan", "expiresAt": "2026-11-22T10:00:00Z" })
}

#[test]
fn found_lists_the_machines_waiting_and_is_empty_when_it_cant_say() {
    let home = fake(vec![("GET", "/api/machines/found", 200, json!({ "machines": [waiting()] }).to_string())]);
    let machines = load_found(&home.base);
    assert_eq!(machines.len(), 1);
    assert_eq!(machines[0].title(), "atlas · Linux");
    assert_eq!(machines[0].reach_label(), "On your home network");
    let older = fake(vec![]);
    assert!(load_found(&older.base).is_empty());
    assert!(load_found(&crate::test_http::dead_address()).is_empty());
}

fn atlas() -> FoundMachine {
    load_found(&fake(vec![("GET", "/api/machines/found", 200, json!({ "machines": [waiting()] }).to_string())]).base).remove(0)
}

#[test]
fn found_takes_the_rest_of_the_code_or_the_whole_of_it_and_nothing_else() {
    let atlas = atlas();
    assert_eq!(atlas.code_for(""), None);
    assert_eq!(atlas.code_for("q9z"), None);
    assert_eq!(atlas.code_for("q9zd").as_deref(), Some("7K3M-Q9ZD"));
    assert_eq!(atlas.code_for("1111-2222"), None, "a whole code for another machine");
    assert_eq!(atlas.code_for("7k3m-q9zd").as_deref(), Some("7K3M-Q9ZD"));
    let tailnet = FoundMachine { source: "Tailscale".into(), os: None, ..atlas };
    assert_eq!(tailnet.reach_label(), "On Tailscale");
    assert_eq!(tailnet.title(), "atlas");
}

#[test]
fn makes_a_join_code_and_says_why_when_it_cant() {
    let made = json!({ "code": "K7QM-R2XD", "expiresAt": "2026-11-22T10:30:00Z", "addresses": ["http://192.168.1.20:6262"],
                       "target": "http://192.168.1.20:6262#K7QM-R2XD", "machineName": "studio" })
    .to_string();
    let home = fake(vec![("POST", "/api/machines/join-code", 200, made)]);
    let code = make_join_code(&home.base).unwrap();
    assert_eq!(code.target, "http://192.168.1.20:6262#K7QM-R2XD");
    assert_eq!(code.addresses, ["http://192.168.1.20:6262"]);
    assert!(!format!("{code:?}").contains("K7QM"), "the code is a secret until it is used");
    let local =
        fake(vec![("POST", "/api/machines/join-code", 400, json!({ "error": "This Fleet only listens on this computer." }).to_string())]);
    assert_eq!(make_join_code(&local.base), Err("This Fleet only listens on this computer.".to_string()));
    assert_eq!(make_join_code(&fake(vec![]).base), Err("Couldn't make a join code.".to_string()));
}

#[test]
fn a_join_code_made_through_the_state_is_kept() {
    let made =
        json!({ "code": "K7QM-R2XD", "expiresAt": "x", "addresses": [], "target": "t#K7QM-R2XD", "machineName": "studio" }).to_string();
    let home = fake(vec![("POST", "/api/machines/join-code", 200, made)]);
    let mut rig = Rig::new(&home);
    rig.rt.update(|s, tx| s.make_join_code(tx));
    assert_eq!(rig.rt.state.machine_data.join_code, JoinState::Making);
    rig.wait("the join code", |s| matches!(s.machine_data.join_code, JoinState::Ready(_)));
}

fn reach_body(mode: &str, available: bool) -> String {
    json!({
        "available": available, "mode": mode, "ownAddress": null, "ownPort": 6262,
        "listening": [], "found": [{ "url": "http://192.168.1.20:6262", "kind": "lan" }],
        "tailscale": { "state": "running", "headline": "Tailscale is on", "detail": "studio.tail4f2a.ts.net", "covered": false, "canTurnOn": true },
    })
    .to_string()
}

#[test]
fn reads_and_changes_who_can_reach_the_desktop_apps_fleet() {
    let home = fake(vec![
        ("GET", "/api/machine/reach", 200, reach_body("off", true)),
        ("PUT", "/api/machine/reach", 200, reach_body("home", true)),
    ]);
    let mut rig = Rig::new(&home);
    rig.rt.update(|s, tx| s.load_reach(tx));
    rig.wait("the reach state", |s| s.machine_data.reach.is_some());
    let state = rig.rt.state.machine_data.reach.clone().unwrap();
    assert!(state.available);
    assert_eq!((state.mode, state.own_port), (ReachMode::Off, 6262));
    assert_eq!(state.found[0].kind, Kind::Lan);
    assert!(state.tailscale.as_ref().unwrap().can_turn_on);
    rig.rt.update(|s, tx| s.set_reach(ReachMode::Home, None, tx));
    rig.wait("the saved mode", |s| s.machine_data.reach.as_ref().is_some_and(|r| r.mode == ReachMode::Home));
    let (sent, _) = &home.seen("PUT", "/api/machine/reach")[0];
    assert_eq!(sent["mode"], "home");
}

#[test]
fn another_fleet_or_an_older_one_has_nothing_to_change() {
    let other = fake(vec![("GET", "/api/machine/reach", 200, reach_body("off", false))]);
    assert!(!crate::fleet::machines::reach::load(&other.base).unwrap().unwrap().available);
    assert_eq!(crate::fleet::machines::reach::load(&fake(vec![]).base), Ok(None), "404: too old to know");
    let paired = fake(vec![("GET", "/api/machine/reach", 403, "{}".into())]);
    assert_eq!(crate::fleet::machines::reach::load(&paired.base), Ok(None), "403: a paired device");
    let refused = fake(vec![("PUT", "/api/machine/reach", 409, json!({ "error": "Tailscale isn't running." }).to_string())]);
    assert_eq!(
        crate::fleet::machines::reach::save(&refused.base, ReachMode::TailscaleAndHome, None).unwrap_err(),
        "Tailscale isn't running."
    );
}

/// The requests against a real scratch Fleet (v0.74.0), for the pull request's log:
/// `FLEET_NATIVE_LIVE=http://127.0.0.1:<port> cargo test --lib live_requests -- --ignored --nocapture`. Not run by CI.
#[test]
#[ignore = "needs a scratch Fleet"]
fn live_requests_against_a_scratch_fleet() {
    let Ok(base) = std::env::var("FLEET_NATIVE_LIVE") else { return };
    println!("GET    /api/machines/found          -> {:?}", load_found(&base));
    println!("GET    /api/machine/screens         -> {:?}", screens::list_fleet(&base));
    println!("GET    /api/machine/devices         -> {:?}", screens::list_own(&base));
    println!("DELETE /api/machine/screens/screen:none -> {:?}", screens::remove(&base, "screen:none"));
    println!("POST   /api/machines/join-code      -> {:?}", make_join_code(&base));
    let state = crate::fleet::machines::reach::load(&base);
    println!("GET    /api/machine/reach           -> {state:?}");
    println!("PUT    /api/machine/reach (home)    -> {:?}", crate::fleet::machines::reach::save(&base, ReachMode::Home, None));
}
