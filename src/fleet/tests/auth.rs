//! Access tokens (`fleet::auth`): the bearer header on HTTP, `access_token` on the hub, a refusal that says what is
//! needed instead of reconnecting for ever, and a token that never reaches the screen.
use crate::fleet::auth::{self, bearer, explicit_token, hub_query, is_refusal, split_token};
use crate::fleet::link::{Connection, Link, Post};
use crate::tests::app;
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SECRET: &str = "s3cret-token-value-1234";

/// The home token is the process's: every test that sets it takes this first.
pub(super) static HOME_TOKEN: Mutex<()> = Mutex::new(());

fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |k| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
}

#[test]
fn the_token_comes_from_the_argument_then_the_environment_and_leaves_the_arguments() {
    let args = ["--snap", "a.png", "--token", SECRET, "--server=http://x:1"].map(String::from);
    let (token, rest) = split_token(args.into_iter());
    assert_eq!(token.as_deref(), Some(SECRET));
    assert_eq!(rest, ["--snap", "a.png", "--server=http://x:1"].map(String::from));
    assert_eq!(split_token(["--token=abc".to_string()].into_iter()).0.as_deref(), Some("abc"));
    let vars = env(&[("FLEET_NATIVE_TOKEN", " from-env ")]);
    assert_eq!(explicit_token(Some("arg".into()), &vars).as_deref(), Some("arg"));
    assert_eq!(explicit_token(None, &vars).as_deref(), Some("from-env"));
    assert_eq!(explicit_token(None, &env(&[("FLEET_NATIVE_TOKEN", "")])), None);
}

#[test]
fn the_header_and_the_hub_query_carry_the_token() {
    assert_eq!(bearer(Some("abc")).as_deref(), Some("Bearer abc"));
    assert_eq!(bearer(None), None);
    assert_eq!(hub_query(Some("abc")), "&access_token=abc");
    // Characters that mean something in a URL are escaped.
    assert_eq!(hub_query(Some("a+b/c=d&e")), "&access_token=a%2Bb%2Fc%3Dd%26e");
    assert_eq!(hub_query(None), "");
    assert!(is_refusal("http status: 401") && is_refusal("http status: 403"));
    assert!(!is_refusal("http status: 500") && !is_refusal("Connection refused (os error 61)"));
}

/// A server that wants `SECRET`: 401 for a request without it, a hub token for one with it. Records every request's
/// first line and whether it carried the header.
type Seen = Arc<Mutex<Vec<(String, bool)>>>;

fn fake_fleet() -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            // The whole request, body included: closing with the POST's body unread makes Windows reset the connection under
            // the client, which then retries, and the retry is a second knock on the hub.
            let text = crate::test_http::read(&mut stream).raw();
            let line = text.lines().next().unwrap_or("").to_string();
            let with = text.to_lowercase().contains(&format!("authorization: bearer {SECRET}").to_lowercase());
            // The WebSocket asks with the token in the URL instead of a header.
            let in_url = line.contains(&format!("access_token={SECRET}"));
            log.lock().unwrap().push((line.clone(), with || in_url));
            let reply = if with || in_url {
                if line.starts_with("POST /hubs/session-events/negotiate") {
                    let body = r#"{"connectionToken":"abc","negotiateVersion":1}"#;
                    format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    )
                } else {
                    "HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_string()
                }
            } else {
                "HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_string()
            };
            let _ = stream.write_all(reply.as_bytes());
        }
    });
    (base, seen)
}

fn start(base: &str) -> (rune::Runtime<crate::State>, std::sync::mpsc::Receiver<crate::fleet::link::Job>) {
    let (jobs, queue) = channel::<crate::fleet::link::Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    auth::bind_home(base);
    let link = Link::start(base.to_string(), post);
    rt.update(move |s, _| s.link = Some(link));
    (rt, queue)
}

fn pump(
    rt: &mut rune::Runtime<crate::State>,
    queue: &std::sync::mpsc::Receiver<crate::fleet::link::Job>,
    seconds: f64,
    until: impl Fn(&crate::State) -> bool,
) -> bool {
    // A wait for something to happen gets the shared bound; only a "nothing more happens" window keeps its own length.
    let seconds = if seconds >= 10.0 || seconds == 3.0 { crate::test_wait::BOUND.as_secs_f64() } else { seconds };
    let end = Instant::now() + Duration::from_secs_f64(seconds);
    while Instant::now() < end {
        while let Ok(job) = queue.try_recv() {
            rt.update(job);
        }
        rt.frame(1.0 / 60.0);
        if until(&rt.state) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

// One test, because the token is the process's: two tests would take turns badly.
#[test]
fn a_fleet_that_needs_a_token_is_refused_once_and_then_reached_with_one() {
    let _held = HOME_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    // No token: refused, said so, and not knocked on again.
    auth::set(None);
    let (base, seen) = fake_fleet();
    let (mut rt, queue) = start(&base);
    assert!(pump(&mut rt, &queue, 10.0, |s| s.is_refused()), "never saw the refusal: {:?}", rt.state.connection);
    let note = rt.state.connection_note().unwrap();
    assert!(note.contains("needs an access token") && note.contains("--token") && note.contains(&base), "{note}");
    assert!(!note.contains(SECRET));
    // The sign-in page asks what the Fleet offers, once; that is not a reconnect.
    assert!(pump(&mut rt, &queue, 10.0, |s| s.sign_in.open && s.sign_in.options != crate::fleet::sign_in::Options::Loading));
    // A reconnect loop is the hub knocking again. The machine and plugin polls start beside the hub and make their first
    // call whenever their thread gets to run, so on a slow machine it can land after the refusal; they aren't counted.
    let hub_knocks =
        |seen: &Seen| seen.lock().unwrap().iter().filter(|(l, _)| l.starts_with("POST /hubs/session-events/negotiate")).count();
    let knocks = hub_knocks(&seen);
    assert_eq!(knocks, 1, "one knock on the hub");
    pump(&mut rt, &queue, 2.5, |_| false);
    assert_eq!(hub_knocks(&seen), knocks, "no reconnect loop: {:?}", seen.lock().unwrap());
    assert!(!rt.state.list_is_loading(), "the list isn't left loading");

    // The wrong token: its own wording.
    auth::set(Some("not-it".into()));
    let (base, _) = fake_fleet();
    let (mut rt, queue) = start(&base);
    assert!(pump(&mut rt, &queue, 10.0, |s| s.is_refused()));
    assert!(rt.state.connection_note().unwrap().contains("refused the access token"));
    assert!(!rt.state.connection_note().unwrap().contains("not-it"));

    // The right one: HTTP carries the header, the hub's URL the query.
    auth::set(Some(SECRET.into()));
    let (base, seen) = fake_fleet();
    let (mut rt, queue) = start(&base);
    pump(&mut rt, &queue, 3.0, |s| !matches!(s.connection, Connection::Connecting) && seen_hub(&seen));
    assert!(!rt.state.is_refused(), "{:?}", rt.state.connection);
    let log = seen.lock().unwrap().clone();
    let negotiate = log.iter().find(|(l, _)| l.starts_with("POST /hubs/session-events/negotiate")).expect("negotiated");
    assert!(negotiate.1, "the negotiate call carried the bearer header");
    let hub = log.iter().find(|(l, _)| l.starts_with("GET /hubs/session-events?id=abc")).expect("opened the hub");
    assert!(hub.0.contains(&format!("&access_token={SECRET}")) && hub.1);
    // Whatever the window says, the token isn't in it.
    assert!(!rt.state.connection_note().unwrap_or_default().contains(SECRET));
    assert!(!rt.state.link.as_ref().unwrap().base.contains(SECRET));
    auth::set(None);
}

fn seen_hub(seen: &Seen) -> bool {
    seen.lock().unwrap().iter().any(|(l, _)| l.starts_with("GET /hubs/session-events?id=abc"))
}

// ─── one token per machine (component 17, piece A) ──────────────────────────

/// A server that records each request's head (lower case) and answers `{}`.
fn recording_server() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            log.lock().unwrap().push(crate::test_http::read(&mut stream).raw().to_lowercase());
            let _ =
                stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}");
        }
    });
    (base, seen)
}

#[test]
fn a_token_is_sent_only_to_its_own_base_never_to_another() {
    let (falcon, falcon_seen) = recording_server();
    let (studio, studio_seen) = recording_server();
    let (other, other_seen) = recording_server();
    auth::set_for(&falcon, Some("falcon-token-1"));
    auth::set_for(&studio, Some("studio-token-2"));
    for base in [&falcon, &studio, &other] {
        crate::fleet::http::get(base, "/api/sessions").unwrap();
    }
    let head = |s: &Seen2| s.lock().unwrap().first().cloned().unwrap();
    assert!(head(&falcon_seen).contains("authorization: bearer falcon-token-1"));
    assert!(head(&studio_seen).contains("authorization: bearer studio-token-2"));
    // Two machines, two tokens: neither reached the other, and a base with none sent no header at all.
    assert!(!head(&falcon_seen).contains("studio-token-2") && !head(&studio_seen).contains("falcon-token-1"));
    assert!(!head(&other_seen).contains("authorization"), "{}", head(&other_seen));
    // No cookie or CSRF header goes along with a machine's token.
    assert!(!head(&falcon_seen).contains("cookie") && !head(&falcon_seen).contains("x-csrf-token"));
    // Forgetting a machine's token stops the header.
    auth::set_for(&falcon, None);
    crate::fleet::http::get(&falcon, "/api/sessions").unwrap();
    assert!(!falcon_seen.lock().unwrap()[1].contains("authorization"));
    auth::set_for(&studio, None);
}

type Seen2 = Arc<Mutex<Vec<String>>>;

#[test]
fn the_hub_url_takes_the_token_of_its_own_machine() {
    let (a, b) = ("http://100.64.90.72:2113", "https://falcon.tail9c2e.ts.net");
    auth::set_for(a, Some("a+b/c"));
    auth::set_for(b, Some("second"));
    assert_eq!(auth::hub_query_for(a), "&access_token=a%2Bb%2Fc");
    assert_eq!(auth::hub_query_for(b), "&access_token=second");
    assert_eq!(auth::hub_query_for("http://100.64.90.73:2113"), "");
    // The same place is the same place: default port, case and path don't matter.
    assert_eq!(auth::hub_query_for("HTTPS://Falcon.Tail9c2e.ts.net:443/fleet"), "&access_token=second");
    // Not the same place: another port.
    assert_eq!(auth::hub_query_for("http://100.64.90.72:2114"), "");
    auth::set_for(a, None);
    auth::set_for(b, Some("  "));
    assert_eq!(auth::hub_query_for(a), "");
    assert_eq!(auth::hub_query_for(b), "");
}

#[test]
fn the_token_given_for_home_goes_to_home_alone() {
    // Own addresses, so the process-wide home of the other test is left alone.
    let machine = crate::fleet::machine::Machine::from_json(
        &serde_json::json!({"id": "m1", "name": "falcon", "baseUrl": "http://10.9.8.7:2113", "token": "machine-secret"}),
    )
    .unwrap();
    machine.hold_token();
    assert_eq!(auth::token_for("http://10.9.8.7:2113/api/x").unwrap().expose(), "machine-secret");
    assert!(auth::token_for("http://10.9.8.8:2113").is_none());
    auth::set_for(machine.base(), None);
}

#[test]
fn a_token_never_appears_in_a_debug_an_error_or_a_shown_address() {
    let machine = crate::fleet::machine::Machine::from_json(
        &serde_json::json!({"id": "m2", "name": "studio", "baseUrl": "http://10.9.8.9:2113", "token": SECRET}),
    )
    .unwrap();
    assert!(!format!("{machine:?}").contains(SECRET), "{machine:?}");
    assert!(!format!("{:?}", machine.target()).contains(SECRET));
    assert!(!machine.base().contains(SECRET));
    assert!(!auth::refused_note(machine.base()).contains(SECRET));
    machine.hold_token();
    let err = crate::fleet::http::get("http://10.9.8.9:1", "/x").unwrap_err();
    assert!(!err.contains(SECRET), "{err}");
    auth::set_for(machine.base(), None);
}
