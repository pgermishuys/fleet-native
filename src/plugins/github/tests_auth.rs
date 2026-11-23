//! Component 16, piece AF: GitHub's connection page. Fleet has no tests for it, so these are native: the status, the device flow
//! through each result, the countdown, cancelling, the token's messages, Disconnect. A fake Fleet answers `/api/integrations/github/
//! auth/*` and a fake GitHub answers `/user`; no real GitHub, account or token is ever reached. Every name and token is invented.
use super::{auth, auth_ref};
use crate::fleet::link::{Job, Link, Post};
use crate::plugins::github::auth::{Device, Pat};
use crate::tests::{FRAME, app, click};
use crate::{Rail, State, test_http, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fake {
    connected: bool,
    /// The status answers this instead of 200 when set.
    status_fails: Option<u16>,
    /// What the device-code request answers; `None` is 500 with a reason.
    device: Option<Value>,
    /// What each poll answers, in order; pending when it runs out.
    polls: VecDeque<(u16, Value)>,
    /// What `POST .../auth/token` answers.
    token: Option<(u16, Value)>,
    delete_fails: bool,
    /// A completed authorization that Fleet's own status doesn't show.
    never_connects: bool,
    /// Every request to Fleet's GitHub routes, and the one GitHub itself got.
    log: Vec<(String, String, Value)>,
    user_calls: Vec<(String, Option<String>)>,
}

type Shared = Arc<Mutex<Fake>>;

fn serve(handler: impl Fn(test_http::Request, &mut std::net::TcpStream) + Send + Sync + 'static) -> String {
    let listener = crate::test_http::bind().expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let handler = Arc::new(handler);
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let handler = handler.clone();
            std::thread::spawn(move || {
                let req = test_http::read(&mut stream);
                if !req.method.is_empty() {
                    handler(req, &mut stream);
                }
            });
        }
    });
    base
}

fn fleet(fake: Shared) -> String {
    serve(move |req, stream| {
        let route = req.route().to_string();
        if route == "/api/preferences" {
            return test_http::respond_json(stream, "200 OK", "{}");
        }
        if !route.starts_with("/api/integrations/github/") {
            return test_http::respond_json(stream, "200 OK", "{}");
        }
        let mut f = fake.lock().unwrap();
        f.log.push((req.method.clone(), route.clone(), req.json()));
        let (status, body): (u16, Value) = match (req.method.as_str(), route.as_str()) {
            ("GET", "/api/integrations/github/auth/status") => match f.status_fails {
                Some(code) => (code, json!({"error": "nope"})),
                None => (200, json!({"connected": f.connected})),
            },
            ("POST", "/api/integrations/github/auth/device-code") => match f.device.clone() {
                Some(d) => (200, d),
                None => (500, json!({"error": "GitHub is not set up on this Fleet."})),
            },
            ("POST", "/api/integrations/github/auth/poll") => {
                let (code, body) = f.polls.pop_front().unwrap_or((200, json!({"status": "pending"})));
                if body["status"] == "complete" && !f.never_connects {
                    f.connected = true;
                }
                (code, body)
            }
            ("POST", "/api/integrations/github/auth/token") => {
                let (code, body) = f.token.clone().unwrap_or((200, json!({})));
                if code == 200 {
                    f.connected = true;
                }
                (code, body)
            }
            ("DELETE", "/api/integrations/github/auth") if f.delete_fails => (500, json!({"error": "The token store is locked."})),
            ("DELETE", "/api/integrations/github/auth") => {
                f.connected = false;
                (204, Value::Null)
            }
            _ => (404, json!({})),
        };
        drop(f);
        test_http::respond_json(stream, format!("{status} X"), &body.to_string());
    })
}

/// A GitHub that knows one token.
fn github(fake: Shared) -> String {
    serve(move |req, stream| {
        let auth = req.header("authorization");
        let agent = req.header("user-agent");
        fake.lock().unwrap().user_calls.push((auth.clone().unwrap_or_default(), agent));
        if req.route() == "/user" && auth.as_deref() == Some("Bearer ghp_invented_good") {
            test_http::respond_json(stream, "200 OK", r#"{"login":"octo-test"}"#);
        } else {
            test_http::respond_json(stream, "401 Unauthorized", r#"{"message":"Bad credentials"}"#);
        }
    })
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Shared,
    gh: Shared,
    /// The clock when the code arrived: `at` counts from it.
    t0: f64,
}

fn rig(fake: Fake) -> Rig {
    let fake: Shared = Arc::new(Mutex::new(fake));
    let gh: Shared = Arc::new(Mutex::new(Fake::default()));
    let base = fleet(fake.clone());
    let api = github(gh.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(
        base,
        Post::new(move |job| {
            let _ = jobs.lock().unwrap().send(job);
        }),
    );
    let mut rt = app();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.rail = Rail::Settings;
        auth(&mut s.places).api = api;
        s.open_settings("plugin:github", None);
    });
    rt.frame(FRAME);
    Rig { rt, queue, fake, gh, t0: 0.0 }
}

fn code(interval: u64, expires_in: u64) -> Value {
    json!({"userCode": "WXYZ-1234", "verificationUri": "https://github.test/login/device", "deviceCode": "dev_invented", "expiresIn": expires_in, "interval": interval})
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("{:?}", auth_ref(&s.places)));
        self.rt.frame(FRAME);
    }
    /// The page has read the status.
    fn read(&mut self) {
        self.wait("the status", |s| auth_ref(&s.places).asked && !auth_ref(&s.places).checking);
    }
    fn has(&self, key: &str) -> bool {
        self.rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }
    fn words(&self) -> String {
        self.rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
    }
    fn text(&self, key: &str) -> String {
        self.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text).unwrap_or_default()
    }
    fn requests(&self, method: &str, route: &str) -> usize {
        self.fake.lock().unwrap().log.iter().filter(|(m, r, _)| m == method && r == route).count()
    }
    fn set(&mut self, f: impl FnOnce(&mut State) + 'static) {
        self.rt.update(move |s, _| f(s));
        self.rt.frame(FRAME);
    }
    /// Time passes until `secs` after the code arrived (the runtime's own clock; a frame this long runs the page's timers).
    fn at(&mut self, secs: f64) {
        let dt = (self.t0 + secs - self.rt.state.clock).max(0.001);
        self.rt.frame(dt);
    }
    fn start_device(&mut self) {
        click(&mut self.rt, "github-connect");
        self.wait("the code", |s| matches!(auth_ref(&s.places).device, Device::Awaiting { .. }));
        self.t0 = self.rt.state.clock;
    }
}

const POLL: &str = "/api/integrations/github/auth/poll";
const STATUS: &str = "/api/integrations/github/auth/status";

#[test]
fn a_status_that_fails_means_not_connected() {
    let mut r = rig(Fake { status_fails: Some(500), ..Fake::default() });
    r.read();
    assert!(!auth_ref(&r.rt.state.places).connected);
    assert!(r.has("github-connect"), "the device card is offered");
    assert!(r.words().contains("Connect with device flow"));
}

#[test]
fn says_it_is_checking_until_the_status_answers_then_shows_the_connected_rows() {
    let mut r = rig(Fake { connected: true, ..Fake::default() });
    assert!(r.words().contains("Checking GitHub connection\u{2026}"));
    r.read();
    assert!(auth_ref(&r.rt.state.places).connected);
    let words = r.words();
    for w in [
        "GitHub is connected",
        "Fleet can list your repositories, show linked pull requests and issues, and start sessions from them.",
        "Repository cache",
        "Disconnect",
        "Forgets the token. Sessions keep their links, but they stop updating.",
    ] {
        assert!(words.contains(w), "{w}");
    }
    assert!(!r.has("github-connect"));
    assert_eq!(r.requests("GET", STATUS), 1, "read once on opening");
}

#[test]
fn the_device_flow_waits_for_the_code_counts_it_down_and_connects_on_complete() {
    let mut r = rig(Fake { device: Some(code(5, 900)), ..Fake::default() });
    r.read();
    r.start_device();
    assert!(!auth_ref(&r.rt.state.places).connected);
    assert_eq!(r.text("github-user-code"), "WXYZ-1234");
    assert!(r.words().contains("Polling GitHub every 5s. Code expires in 15:00."));
    r.at(1.0);
    assert!(r.words().contains("Code expires in 14:5"), "{}", r.words());
    r.at(61.0);
    let words = r.words();
    assert!(words.contains("Polling GitHub every 5s. Code expires in 14:0") || words.contains("13:5"), "{words}");
    r.fake.lock().unwrap().polls.push_back((200, json!({"status": "complete"})));
    r.at(66.0);
    r.wait("GitHub to be connected", |s| auth_ref(&s.places).connected);
    assert_eq!(auth_ref(&r.rt.state.places).device, Device::Complete);
    assert!(r.requests("POST", POLL) >= 1);
    assert_eq!(r.requests("GET", STATUS), 2, "the status is read again to confirm");
    assert!(r.has("github-connected"));
}

#[test]
fn pending_asks_again_after_the_interval_the_server_names() {
    let mut r = rig(Fake { device: Some(code(5, 900)), ..Fake::default() });
    r.read();
    r.start_device();
    r.fake.lock().unwrap().polls.push_back((200, json!({"status": "pending", "interval": 10})));
    r.at(6.0);
    r.wait("its answer", |s| auth_ref(&s.places).poll_every == 10);
    assert_eq!(r.requests("POST", POLL), 1);
    r.at(14.0);
    assert_eq!(r.requests("POST", POLL), 1, "slowed to 10 s: not yet");
    r.at(17.0);
    let fake = r.fake.clone();
    test_wait::until_true("the second poll to reach Fleet", move || {
        fake.lock().unwrap().log.iter().filter(|(_, r, _)| r == POLL).count() == 2
    });
    assert!(r.words().contains("Polling GitHub every 5s."), "the label keeps the first interval, as Fleet's does");
}

#[test]
fn each_way_it_can_end_without_connecting_says_so_and_try_again_goes_back_to_the_button() {
    let cases = [
        (json!({"status": "expired"}), 200, "The GitHub code expired. Please try again."),
        (json!({"status": "denied"}), 200, "GitHub authorization was denied."),
        (json!({"status": "error", "message": "Slow down."}), 200, "Slow down."),
        (json!({"status": "error"}), 200, "GitHub authorization failed."),
        (json!({"error": "Fleet lost its token store."}), 500, "Fleet lost its token store."),
        (json!({}), 502, "HTTP 502"),
    ];
    for (answer, status, words) in cases {
        let mut r = rig(Fake { device: Some(code(5, 900)), ..Fake::default() });
        r.read();
        r.start_device();
        r.fake.lock().unwrap().polls.push_back((status, answer));
        r.at(6.0);
        r.wait("the end", |s| !matches!(auth_ref(&s.places).device, Device::Awaiting { .. }));
        assert!(r.words().contains(words), "{words}");
        assert!(!r.has("github-cancel"));
        click(&mut r.rt, "github-try-again");
        r.rt.frame(FRAME);
        assert_eq!(auth_ref(&r.rt.state.places).device, Device::Idle);
        assert!(r.has("github-connect"));
    }
}

#[test]
fn says_when_the_saved_connection_was_not_confirmed() {
    let mut r = rig(Fake { device: Some(code(5, 900)), never_connects: true, ..Fake::default() });
    r.read();
    r.start_device();
    r.fake.lock().unwrap().polls.push_back((200, json!({"status": "complete"})));
    r.at(6.0);
    r.wait("the end", |s| matches!(auth_ref(&s.places).device, Device::Error(_)));
    assert!(r.words().contains("GitHub authorization completed, but the saved connection was not confirmed."));
    assert!(!auth_ref(&r.rt.state.places).connected);
}

#[test]
fn a_device_code_that_cannot_be_had_says_why() {
    let mut r = rig(Fake::default());
    r.read();
    click(&mut r.rt, "github-connect");
    r.wait("the refusal", |s| matches!(auth_ref(&s.places).device, Device::Error(_)));
    assert!(r.words().contains("GitHub is not set up on this Fleet."));
}

#[test]
fn cancel_stops_polling() {
    let mut r = rig(Fake { device: Some(code(5, 900)), ..Fake::default() });
    r.read();
    r.start_device();
    click(&mut r.rt, "github-cancel");
    r.rt.frame(FRAME);
    assert_eq!(auth_ref(&r.rt.state.places).device, Device::Idle);
    r.at(6.0);
    r.at(500.0);
    r.rt.frame(FRAME);
    assert_eq!(r.requests("POST", POLL), 0, "nothing asked after Cancel");
    assert!(r.has("github-connect"));
}

#[test]
fn leaving_the_page_stops_polling_and_forgets_the_token_that_was_typed() {
    let mut r = rig(Fake { device: Some(code(5, 900)), ..Fake::default() });
    r.read();
    r.start_device();
    r.set(|s| auth(&mut s.places).pat.replace_text("ghp_invented_typed"));
    r.set(|s| s.open_settings("plugins", None));
    r.at(10.0);
    assert_eq!(r.requests("POST", POLL), 0);
    let g = auth_ref(&r.rt.state.places);
    assert_eq!(g.device, Device::Idle);
    assert!(g.pat.text.is_empty());
}

#[test]
fn the_token_box_is_masked_and_its_buttons_wait_for_a_token() {
    let mut r = rig(Fake::default());
    r.read();
    assert!(!r.has("github-token"), "the card is folded");
    click(&mut r.rt, "github-pat-toggle");
    r.rt.frame(FRAME);
    assert!(r.words().contains("Use a PAT when device flow is unavailable. The token is validated before it is stored server-side."));
    let disabled = |r: &Rig, key: &str| r.rt.elements().iter().find(|e| e.key.as_deref() == Some(key)).is_some_and(|e| e.opacity < 1.0);
    assert!(disabled(&r, "github-pat-test") && disabled(&r, "github-pat-connect"));
    r.set(|s| auth(&mut s.places).pat.replace_text("   "));
    assert!(disabled(&r, "github-pat-test"), "spaces are not a token");
    r.set(|s| auth(&mut s.places).pat.replace_text("ghp_invented_good"));
    r.rt.frame(1.0);
    assert!(!disabled(&r, "github-pat-test") && !disabled(&r, "github-pat-connect"));
    assert!(!r.words().contains("ghp_invented_good"), "the token is never drawn");
    assert!(!format!("{:?}", auth_ref(&r.rt.state.places)).contains("ghp_invented_good"), "nor logged");
}

#[test]
fn testing_a_token_names_its_owner_or_says_why_not_and_typing_clears_the_message() {
    let mut r = rig(Fake::default());
    r.read();
    click(&mut r.rt, "github-pat-toggle");
    r.set(|s| s.github_test_pat());
    assert!(r.words().contains("Enter a personal access token."));

    r.set(|s| auth(&mut s.places).pat.replace_text(" ghp_invented_bad "));
    click(&mut r.rt, "github-pat-test");
    r.wait("the refusal", |s| matches!(auth_ref(&s.places).pat_state, Pat::Error(_)));
    assert!(r.words().contains("Bad credentials"));
    r.set(|s| auth(&mut s.places).pat.replace_text("ghp_invented_other"));
    r.at(7.0);
    assert!(!r.has("github-pat-message"), "typing again clears it");

    r.set(|s| auth(&mut s.places).pat.replace_text("ghp_invented_good"));
    click(&mut r.rt, "github-pat-test");
    r.wait("the owner", |s| matches!(auth_ref(&s.places).pat_state, Pat::Success(_)));
    assert!(r.words().contains("Connected as @octo-test"));
    let calls = r.gh.lock().unwrap().user_calls.clone();
    assert_eq!(calls.last().unwrap().0, "Bearer ghp_invented_good", "trimmed, and sent as a bearer token");
    assert!(r.fake.lock().unwrap().log.iter().all(|(_, route, _)| !route.ends_with("/token")), "testing stores nothing");
}

#[test]
fn a_token_the_network_cannot_check_says_it_could_not() {
    let mut r = rig(Fake::default());
    r.read();
    click(&mut r.rt, "github-pat-toggle");
    r.set(|s| {
        auth(&mut s.places).api = "http://127.0.0.1:1".into();
        auth(&mut s.places).pat.replace_text("ghp_invented_good");
    });
    click(&mut r.rt, "github-pat-test");
    r.wait("the failure", |s| matches!(auth_ref(&s.places).pat_state, Pat::Error(_)));
    assert!(r.words().contains("Unable to validate GitHub token."));
}

#[test]
fn connecting_with_a_token_stores_it_wipes_the_box_and_shows_the_connected_rows() {
    let mut r = rig(Fake::default());
    r.read();
    click(&mut r.rt, "github-pat-toggle");
    r.set(|s| auth(&mut s.places).pat.replace_text("ghp_invented_good"));
    click(&mut r.rt, "github-pat-connect");
    r.wait("GitHub to be connected", |s| auth_ref(&s.places).connected);
    let sent = r.fake.lock().unwrap().log.iter().find(|(_, route, _)| route.ends_with("/auth/token")).cloned().expect("the request");
    assert_eq!(sent.2, json!({"token": "ghp_invented_good"}));
    let g = auth_ref(&r.rt.state.places);
    assert!(g.pat.text.is_empty());
    assert_eq!(g.device, Device::Complete);
    assert!(r.has("github-connected"));
}

#[test]
fn a_token_fleet_refuses_keeps_the_box_and_says_what_fleet_said() {
    let mut r = rig(Fake { token: Some((400, json!({"error": "That token has no repo scope."}))), ..Fake::default() });
    r.read();
    click(&mut r.rt, "github-pat-toggle");
    r.set(|s| auth(&mut s.places).pat.replace_text("ghp_invented_good"));
    click(&mut r.rt, "github-pat-connect");
    r.wait("the refusal", |s| matches!(auth_ref(&s.places).pat_state, Pat::Error(_)));
    assert!(r.words().contains("That token has no repo scope."));
    assert_eq!(auth_ref(&r.rt.state.places).pat.text, "ghp_invented_good", "kept to try again");
    assert!(!auth_ref(&r.rt.state.places).connected);
}

#[test]
fn disconnect_forgets_the_token_and_goes_back_to_the_cards() {
    let mut r = rig(Fake { connected: true, ..Fake::default() });
    r.read();
    click(&mut r.rt, "github-disconnect");
    r.wait("the connection to go", |s| !auth_ref(&s.places).connected && !auth_ref(&s.places).disconnecting);
    assert_eq!(r.requests("DELETE", "/api/integrations/github/auth"), 1);
    assert!(r.has("github-connect"));
    assert_eq!(auth_ref(&r.rt.state.places).device, Device::Idle);
}

#[test]
fn a_disconnect_that_fails_keeps_the_connection() {
    let mut r = rig(Fake { connected: true, delete_fails: true, ..Fake::default() });
    r.read();
    click(&mut r.rt, "github-disconnect");
    r.wait("the failure", |s| matches!(auth_ref(&s.places).device, Device::Error(_)));
    assert!(auth_ref(&r.rt.state.places).connected);
    assert_eq!(auth_ref(&r.rt.state.places).device, Device::Error("The token store is locked.".into()));
    assert!(r.has("github-disconnect"), "still connected");
}

#[test]
fn nothing_of_the_page_shows_while_the_plugin_is_off() {
    let mut r = rig(Fake::default());
    r.read();
    r.set(|s| {
        s.pickers.prefs.set(crate::fleet::rails::DISABLED_PLUGINS_KEY, "[\"github\"]");
    });
    assert!(!r.has("plugin-config-github"));
    assert!(r.words().contains("GitHub is off."));
}
