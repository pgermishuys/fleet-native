//! `machine-routes.test.ts`, by its names: which address a machine is talked to on, and how it is picked.
use crate::fleet::machine::Machine;
use crate::fleet::machine_routes::{self as routes, Outcome, PROBE_TIMEOUT, Probe, better_address, pick_address, race, retrying};
use crate::fleet::{auth, machine_routes::probe_address};
use serde_json::json;
use std::collections::HashMap;
use std::io::Write;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const NAME: &str = "https://studio.tail9c2e.ts.net";
const TAIL_IP: &str = "http://100.64.90.72:2113";
const LAN: &str = "http://192.168.1.20:2113";
const HOST: &str = "http://studio.local:2113";

/// Every test has a machine of its own: picks are kept per machine id for the run, so two tests never share one.
fn machine(id: &str, first: &str, addresses: &[&str]) -> Machine {
    let list: Vec<_> = addresses.iter().map(|u| json!({ "url": u, "kind": "other", "learned": false })).collect();
    Machine::from_json(&json!({ "id": id, "name": "studio", "baseUrl": first, "addresses": list, "token": "studio-token-0123456789" }))
        .unwrap()
}

fn studio(id: &str) -> Machine {
    machine(id, NAME, &[NAME, TAIL_IP, LAN, HOST])
}

/// A network where each address answers as listed (nothing for the rest), and what was asked, in order.
fn network(answers: &[(&str, Outcome)]) -> (Arc<Mutex<Vec<String>>>, Probe) {
    let asked: Arc<Mutex<Vec<String>>> = Arc::default();
    let log = asked.clone();
    let answers: HashMap<String, Outcome> = answers.iter().map(|(u, o)| ((*u).to_string(), *o)).collect();
    (
        asked,
        Arc::new(move |url| {
            log.lock().unwrap().push(url.to_string());
            answers.get(url).copied().unwrap_or(Outcome::NoAnswer)
        }),
    )
}

fn urls(list: &[&str]) -> Vec<String> {
    list.iter().map(|u| u.to_string()).collect()
}

// ─── kinds and the pick ─────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn is_home_s_until_this_run_picks_one_and_the_pick_is_remembered() {
    let m = studio("routes-remembered");
    assert_eq!(m.base(), NAME, "the address home uses, until one is picked");
    routes::use_address(&m, LAN);
    assert_eq!(m.base(), LAN);
    assert_eq!(routes::picked("routes-remembered").as_deref(), Some(LAN));
    assert_eq!(studio("routes-remembered").base(), LAN, "a record read again from home keeps the pick");
    let gone = machine("routes-remembered", NAME, &[NAME, HOST]);
    assert_eq!(gone.base(), NAME, "a pick that is no longer one of the machine's addresses is ignored");
}

#[test]
fn takes_the_first_in_order_even_when_a_later_one_answers_sooner() {
    let probe: Probe = Arc::new(|url| {
        std::thread::sleep(Duration::from_millis(if url == "a" { 120 } else { 5 }));
        Outcome::Ok
    });
    assert_eq!(race(&urls(&["a", "b"]), &probe, Duration::from_millis(10)).0.as_deref(), Some("a"));
}

#[test]
fn moves_on_as_soon_as_the_ones_above_have_failed_without_waiting_out_the_gap() {
    let (asked, probe) = network(&[("c", Outcome::Ok)]);
    let began = std::time::Instant::now();
    let (winner, outcomes) = race(&urls(&["a", "b", "c"]), &probe, Duration::from_secs(60));
    assert_eq!(winner.as_deref(), Some("c"));
    assert_eq!(*asked.lock().unwrap(), ["a", "b", "c"]);
    assert_eq!(outcomes["a"], Outcome::NoAnswer);
    assert_eq!(outcomes["c"], Outcome::Ok);
    assert!(began.elapsed() < Duration::from_secs(5));
}

#[test]
fn starts_the_next_before_a_slow_one_gives_up_and_waits_for_the_slow_one_above_it() {
    let (release, hold) = std::sync::mpsc::channel::<()>();
    let hold = Mutex::new(hold);
    let started: Arc<Mutex<Vec<String>>> = Arc::default();
    let log = started.clone();
    let probe: Probe = Arc::new(move |url| {
        log.lock().unwrap().push(url.to_string());
        if url == "slow" {
            let _ = hold.lock().unwrap().recv();
            Outcome::NoAnswer
        } else {
            Outcome::Ok
        }
    });
    let race = std::thread::spawn(move || race(&urls(&["slow", "fast"]), &probe, Duration::from_millis(10)).0);
    crate::test_wait::until_true("the fast one to start before the slow one gave up", || started.lock().unwrap().len() == 2);
    assert!(!race.is_finished(), "the fast one answered, but the slow one above it is still undecided");
    release.send(()).unwrap();
    assert_eq!(race.join().unwrap().as_deref(), Some("fast"));
}

#[test]
fn never_takes_another_machine_or_one_that_can_t_say_who_it_is() {
    let (_, probe) = network(&[("a", Outcome::OtherMachine), ("b", Outcome::NoProbe)]);
    assert_eq!(race(&urls(&["a", "b"]), &probe, Duration::from_millis(10)).0, None);
}

#[test]
fn keeps_the_address_in_use_when_it_answers_and_asks_nothing_else() {
    let (asked, probe) = network(&[(NAME, Outcome::Ok)]);
    assert_eq!(pick_address(&studio("routes-keeps"), &probe), NAME);
    assert_eq!(*asked.lock().unwrap(), [NAME]);
}

#[test]
fn falls_back_down_the_list_when_tailscale_doesn_t_answer_and_starts_there_next_time() {
    let (_, probe) = network(&[(LAN, Outcome::Ok), (HOST, Outcome::Ok)]);
    let m = studio("routes-falls-back");
    assert_eq!(pick_address(&m, &probe), LAN);
    assert_eq!(m.base(), LAN);
    let (asked, again) = network(&[(LAN, Outcome::Ok)]);
    assert_eq!(pick_address(&m, &again), LAN);
    assert_eq!(*asked.lock().unwrap(), [LAN]);
}

#[test]
fn skips_an_address_where_another_fleet_answers() {
    let (_, probe) = network(&[(TAIL_IP, Outcome::OtherMachine), (LAN, Outcome::Ok)]);
    assert_eq!(pick_address(&studio("routes-skips"), &probe), LAN);
}

#[test]
fn stays_put_when_nothing_answers_so_the_request_after_it_says_why() {
    let (_, probe) = network(&[]);
    let m = studio("routes-stays");
    assert_eq!(pick_address(&m, &probe), NAME);
    assert_eq!(routes::picked("routes-stays"), None);
}

#[test]
fn trusts_the_address_home_uses_when_the_fleet_there_is_too_old_to_say_who_it_is() {
    let (asked, probe) = network(&[(NAME, Outcome::NoProbe)]);
    assert_eq!(pick_address(&studio("routes-trusts"), &probe), NAME);
    assert_eq!(*asked.lock().unwrap(), [NAME]);
}

#[test]
fn doesn_t_probe_a_machine_with_one_address() {
    let (asked, probe) = network(&[]);
    assert_eq!(pick_address(&machine("routes-one", NAME, &[NAME]), &probe), NAME);
    assert!(asked.lock().unwrap().is_empty());
}

#[test]
fn shares_one_pick_between_callers_that_ask_at_once() {
    let asked: Arc<Mutex<Vec<String>>> = Arc::default();
    let log = asked.clone();
    let probe: Probe = Arc::new(move |url| {
        log.lock().unwrap().push(url.to_string());
        std::thread::sleep(Duration::from_millis(200));
        Outcome::Ok
    });
    let m = studio("routes-shares");
    let callers: Vec<_> = (0..2)
        .map(|_| {
            let (m, probe) = (m.clone(), probe.clone());
            std::thread::spawn(move || pick_address(&m, &probe))
        })
        .collect();
    for caller in callers {
        assert_eq!(caller.join().unwrap(), NAME);
    }
    assert_eq!(*asked.lock().unwrap(), [NAME]);
}

#[test]
fn finds_a_higher_address_that_answers_again_without_moving_to_it() {
    let m = studio("routes-better");
    routes::use_address(&m, LAN);
    let (asked, probe) = network(&[(TAIL_IP, Outcome::Ok), (LAN, Outcome::Ok)]);
    assert_eq!(better_address(&m, &probe).as_deref(), Some(TAIL_IP));
    let asked = asked.lock().unwrap();
    assert!(!asked.contains(&LAN.to_string()) && !asked.contains(&HOST.to_string()));
    assert_eq!(m.base(), LAN, "nothing moved");
}

#[test]
fn has_nothing_to_look_for_on_the_first_address() {
    let (asked, probe) = network(&[(NAME, Outcome::Ok)]);
    assert_eq!(better_address(&studio("routes-first"), &probe), None);
    assert!(asked.lock().unwrap().is_empty());
}

#[test]
fn a_token_leaves_the_address_it_came_from_and_goes_to_the_one_picked() {
    // Addresses no other test uses: `auth` is the process's, and tests run side by side.
    let (old, new) = ("http://127.0.0.41:9", "http://127.0.0.42:9");
    let m = machine("routes-token", old, &[old, new]);
    auth::set_for(old, Some("studio-token-0123456789"));
    routes::use_address(&m, new);
    assert!(auth::token_for(old).is_none(), "another Fleet taking the old address gets no token");
    assert!(auth::token_for(new).is_some());
}

// ─── probing, against real sockets ──────────────────────────────────────────────────────────────────────────────────

type Seen = Arc<Mutex<Vec<(String, Option<String>, bool)>>>;

/// A server at a loopback port: `route` answers a path with a content type and a body. Logs the path, the `Authorization`
/// header and whether a cookie came.
fn serve(route: impl Fn(&str) -> Option<(&'static str, String)> + Send + Sync + 'static) -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen: Seen = Arc::default();
    let (log, route) = (seen.clone(), Arc::new(route));
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let (log, route) = (log.clone(), route.clone());
            std::thread::spawn(move || {
                let text = crate::test_http::read(&mut stream).raw();
                let path = text.lines().next().unwrap_or("").split(' ').nth(1).unwrap_or("").split('?').next().unwrap_or("").to_string();
                let header = |name: &str| {
                    text.lines()
                        .find_map(|l| l.to_ascii_lowercase().starts_with(name).then(|| l.split_once(':').unwrap().1.trim().to_string()))
                };
                log.lock().unwrap().push((path.clone(), header("authorization:"), header("cookie:").is_some()));
                let (kind, body) = route(&path).unwrap_or(("application/json", "{}".into()));
                let reply =
                    format!("HTTP/1.1 200 X\r\ncontent-type: {kind}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
                let _ = stream.write_all(reply.as_bytes());
            });
        }
    });
    (base, seen)
}

/// A port nothing listens on.
fn nobody_home() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    format!("http://{}", listener.local_addr().unwrap())
}

fn who(id: &'static str) -> impl Fn(&str) -> Option<(&'static str, String)> + Send + Sync + 'static {
    move |path| (path == "/api/machine/id").then(|| ("application/json", json!({ "id": id, "apiVersion": 1 }).to_string()))
}

#[test]
fn asks_who_answers_without_a_token_or_cookies_and_checks_the_id() {
    let (base, seen) = serve(who("machine-studio"));
    auth::set_for(&base, Some("studio-token-0123456789"));
    assert_eq!(probe_address(&base, "machine-studio", PROBE_TIMEOUT), Outcome::Ok);
    assert_eq!(probe_address(&base, "machine-other", PROBE_TIMEOUT), Outcome::OtherMachine);
    let seen = seen.lock().unwrap();
    assert_eq!(seen[0], ("/api/machine/id".to_string(), None, false), "no token and no cookie, though one is held for this address");
}

#[test]
fn says_no_answer_when_the_address_doesn_t_answer_in_time() {
    // Accepts the call and never says a word.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let held: Vec<_> = listener.incoming().flatten().collect();
        std::thread::sleep(Duration::from_secs(5));
        drop(held);
    });
    assert_eq!(probe_address(&base, "machine-studio", Duration::from_millis(300)), Outcome::NoAnswer);
    assert_eq!(probe_address(&nobody_home(), "machine-studio", PROBE_TIMEOUT), Outcome::NoAnswer);
}

#[test]
fn says_no_probe_for_a_fleet_too_old_to_have_one() {
    let (base, _) = serve(|_| Some(("text/html", "<html></html>".into())));
    assert_eq!(probe_address(&base, "machine-studio", PROBE_TIMEOUT), Outcome::NoProbe);
}

#[test]
fn go_again_on_the_address_that_answers_and_the_token_only_goes_where_the_machine_answered() {
    let (dead, (stranger, stranger_seen), (lan, lan_seen)) = (nobody_home(), serve(who("machine-someone-else")), {
        serve(|path| match path {
            "/api/machine/id" => Some(("application/json", json!({ "id": "routes-retry", "apiVersion": 1 }).to_string())),
            "/api/sessions" => Some(("application/json", "[]".into())),
            _ => None,
        })
    });
    let m = machine("routes-retry", &dead, &[&dead, &stranger, &lan]);
    auth::set_for(&dead, Some("studio-token-0123456789"));

    let (status, body) = routes::get_on(&m, "/api/sessions").expect("the second try reaches the machine");

    assert_eq!((status, body.as_str()), (200, "[]"));
    assert_eq!(m.base(), lan);
    assert!(stranger_seen.lock().unwrap().iter().all(|(_, token, _)| token.is_none()), "the other Fleet is never sent the token");
    let on_lan = lan_seen.lock().unwrap();
    let with_token: Vec<&str> = on_lan.iter().filter(|(_, t, _)| t.is_some()).map(|(p, _, _)| p.as_str()).collect();
    assert_eq!(with_token, ["/api/sessions"], "the probe has none, the read has it");
    assert!(auth::token_for(&dead).is_none(), "and it left the address that didn't answer");
}

#[test]
fn aren_t_sent_again_when_they_change_something() {
    let calls = std::cell::Cell::new(0);
    let m = studio("routes-writes");
    let sent = retrying(
        &m,
        "POST",
        |_| {
            calls.set(calls.get() + 1);
            Err::<(), _>("Failed to fetch".to_string())
        },
        |_| panic!("a write never looks for another address"),
    );
    assert_eq!(sent, Err("Failed to fetch".to_string()));
    assert_eq!(calls.get(), 1);
}

#[test]
fn repick_says_whether_the_address_moved() {
    let (base, _) = serve(who("routes-repick"));
    let m = machine("routes-repick", &base, &[&base, &nobody_home()]);
    assert!(!routes::repick(&m), "it answers where it is");
    assert!(!routes::repick(&machine("routes-repick-one", &base, &[&base])), "one address has nothing to pick from");
}
