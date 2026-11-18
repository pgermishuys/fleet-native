//! The Context tab (component 8, piece I): `SessionContextCanvas.test.ts` under its names, and the rows' words. The scenes
//! hold invented links (`mock::context_tab`); the attach form and Refresh talk to a fake Fleet.
use crate::fleet::context_tab::{Feedback, feedback, row_detail, workspace_rows};
use crate::fleet::link::{Job, Link, Post};
use crate::mock::context::SESSION;
use crate::mock::context_tab::Case;
use crate::tests::{FRAME, app, click};
use crate::tests_header_edit::has;
use crate::{State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

/// What the fake Fleet answers and what it was asked.
#[derive(Default)]
struct Fake {
    asked: Vec<(String, String)>,
    /// The status `GET .../smart-links/all` answers with; 200 sends `all`.
    all_status: u16,
    all: Vec<Value>,
    attach_status: u16,
}

type Shared = Arc<Mutex<Fake>>;

fn serve(fake: Shared) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let fake = fake.clone();
            std::thread::spawn(move || answer(stream, fake));
        }
    });
    base
}

fn answer(mut stream: std::net::TcpStream, fake: Shared) {
    let req = crate::test_http::read(&mut stream);
    if req.method.is_empty() {
        return;
    }
    let head = req.head;
    let mut first = head.lines().next().unwrap_or("").split(' ');
    let (method, path) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
    let (status, reply) = {
        let mut f = fake.lock().unwrap();
        f.asked.push((method.clone(), path.clone()));
        if method == "GET" && path.ends_with("/smart-links/all") {
            (f.all_status, if f.all_status == 200 { Value::Array(f.all.clone()).to_string() } else { "boom".into() })
        } else if method == "POST" && path.ends_with("/smart-links") {
            (f.attach_status, if f.attach_status == 200 { crate::mock::context::issue(55, "mentioned").to_string() } else { "{}".into() })
        } else if method == "GET" && path.ends_with("/origin") {
            (200, "[]".into())
        } else if method == "POST" && path.ends_with("/refresh") {
            (200, "{}".into())
        } else {
            (404, "{}".into())
        }
    };
    let out = format!(
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(out.as_bytes());
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Shared,
}

/// The Context tab of the open session on an app whose Fleet is the fake one, with no links held yet.
fn rig(setup: impl FnOnce(&mut Fake)) -> Rig {
    let fake: Shared = Default::default();
    {
        let mut f = fake.lock().unwrap();
        f.all_status = 200;
        f.attach_status = 200;
        setup(&mut f);
    }
    let base = serve(fake.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    let link = Link::start(base, post);
    rt.update(move |s, _| {
        s.link = Some(link);
        s.show_header_for_tab();
    });
    for _ in 0..3 {
        rt.frame(FRAME);
    }
    Rig { rt, queue, fake }
}

impl State {
    /// The first session with its panel open on the Context tab and nothing held: the tab asks for its links itself.
    fn show_header_for_tab(&mut self) {
        self.show_context_tab_scene(Case::Empty, true);
        self.smart_links.by_session.clear();
        self.context.loaded_for = None;
    }
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| {
            format!("loading {}, error {:?}", s.smart_links.is_loading(SESSION), s.smart_links.load_error(SESSION))
        });
    }

    fn has(&self, key: &str) -> bool {
        has(&self.rt, key)
    }

    fn asked(&self, method: &str, suffix: &str) -> usize {
        self.fake.lock().unwrap().asked.iter().filter(|(m, p)| m == method && p.ends_with(suffix)).count()
    }
}

fn shown(case: Case) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_context_tab_scene(case, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt
}

// SessionContextCanvas.test.ts: "groups links by how they relate to the session"
#[test]
fn groups_links_by_how_they_relate_to_the_session() {
    let rt = shown(Case::Links);
    let labels: Vec<&str> = ["context-started-from", "context-own", "context-pinned", "context-mentioned"]
        .into_iter()
        .filter(|k| has(&rt, &format!("{k}-label")))
        .collect();
    assert_eq!(labels, ["context-started-from", "context-own", "context-pinned", "context-mentioned"], "in Fleet's order");
    let order: Vec<f32> = ["context-started-from", "context-own", "context-pinned", "context-mentioned"]
        .into_iter()
        .map(|k| rt.find(&format!("{k}-label")).expect("a heading").1)
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "top to bottom: {order:?}");
    let own = rt.state.smart_links.visible_links(SESSION).into_iter().find(|l| l.id == "link-187").expect("the pull request").clone();
    assert!(row_detail(&own, None).contains("1 failing"), "{}", row_detail(&own, None));
    assert_eq!(crate::fleet::context_tab::own_heading(1), "This session's pull request");
    assert_eq!(crate::fleet::context_tab::own_heading(2), "This session's pull requests");
}

// SessionContextCanvas.test.ts: "asks once to connect GitHub when links can't be checked"
#[test]
fn asks_once_to_connect_github_when_links_cant_be_checked() {
    let mut rt = shown(Case::NotConnected);
    let notices = rt.elements().into_iter().filter(|e| e.key.as_deref() == Some("context-notice")).count();
    assert_eq!(notices, 1, "one notice for two links");
    click(&mut rt, "context-connect");
    assert_eq!(rt.state.rail, crate::Rail::Settings, "the button goes to Settings, until the GitHub plugin has its page");
    let detail = row_detail(&rt.state.smart_links.visible_links(SESSION)[0].clone(), None);
    assert_eq!(detail, "GitHub not connected");
}

// SessionContextCanvas.test.ts: "says it's loading, not that nothing is attached, until the links arrive"
#[test]
fn says_its_loading_not_that_nothing_is_attached_until_the_links_arrive() {
    let mut r = rig(|f| f.all = vec![crate::mock::context::failing(187, "own")]);
    assert!(r.has("context-links-loading") || r.rt.state.smart_links.by_session.contains_key(SESSION));
    r.wait("the links to arrive", |s| s.smart_links.by_session.contains_key(SESSION));
    r.rt.frame(FRAME);
    assert!(!r.has("context-links-loading") && !r.has("context-empty"), "neither the spinner nor the empty note");
    assert!(r.has("context-own-label"), "This session's pull request");
    assert_eq!(feedback(false, false, true, None), Feedback::Loading, "nothing held, still loading");
    assert_eq!(feedback(false, false, false, None), Feedback::Empty);
}

#[test]
fn the_spinner_shows_while_the_first_load_runs() {
    let rt = shown(Case::Loading);
    assert!(has(&rt, "context-links-loading"));
    assert!(!has(&rt, "context-empty"), "loading is not empty");
}

// SessionContextCanvas.test.ts: "says the links couldn't be loaded, not that nothing is attached, and tries again"
#[test]
fn says_the_links_couldnt_be_loaded_not_that_nothing_is_attached_and_tries_again() {
    let mut r = rig(|f| {
        f.all_status = 500;
        f.all = vec![crate::mock::context::failing(187, "own")];
    });
    r.wait("the failure", |s| s.smart_links.load_error(SESSION).is_some());
    r.rt.frame(FRAME);
    assert!(r.has("context-error") && !r.has("context-empty"), "an error, not 'Nothing attached yet'");
    assert_eq!(r.rt.state.smart_links.load_error(SESSION), Some("Fleet answered HTTP 500."));
    r.fake.lock().unwrap().all_status = 200;
    click(&mut r.rt, "context-retry");
    r.wait("the links after Try again", |s| s.smart_links.by_session.contains_key(SESSION));
    r.rt.frame(FRAME);
    assert!(!r.has("context-error") && r.has("context-own-label"), "the pull request shows after trying again");
    assert_eq!(r.asked("GET", "/smart-links/all"), 2, "asked twice: the first time and the retry");
}

// SessionContextCanvas.test.ts: "explains an empty session"
#[test]
fn explains_an_empty_session() {
    let rt = shown(Case::Empty);
    assert!(has(&rt, "context-empty"));
    assert!(!has(&rt, "context-footer"), "no footer when there are no links");
}

#[test]
fn an_automation_origin_is_not_empty() {
    let rt = shown(Case::Automation);
    assert!(has(&rt, "context-origin") && !has(&rt, "context-empty"));
}

#[test]
fn the_attach_form_opens_cancels_with_escape_and_says_why_a_link_was_refused() {
    let mut r = rig(|f| f.attach_status = 400);
    r.wait("the links", |s| s.smart_links.by_session.contains_key(SESSION));
    click(&mut r.rt, "context-attach-start");
    assert!(r.has("context-attach-form"));
    assert!(!r.rt.state.context.can_attach(), "blank: Attach is off");
    r.rt.update(|s, _| s.context.url = rune::TextEdit::new("https://example.test/x"));
    r.rt.frame(FRAME);
    click(&mut r.rt, "context-attach-submit");
    r.wait("the refusal", |s| s.context.attach_error.is_some());
    r.rt.frame(FRAME);
    assert_eq!(r.rt.state.context.attach_error.as_deref(), Some("Paste a link to a GitHub pull request or issue."));
    assert!(r.has("context-attach-error") && r.has("context-attach-form"), "the form stays open");
    r.fake.lock().unwrap().attach_status = 200;
    click(&mut r.rt, "context-attach-submit");
    r.wait("the link to be attached", |s| !s.context.attaching);
    assert!(r.rt.state.smart_links.by_session[SESSION].iter().any(|l| l.id == "link-55"), "the new link is held");
    assert!(r.rt.state.context.url.text.is_empty() && r.rt.state.context.attach_error.is_none(), "cleared");
    click(&mut r.rt, "context-attach-start");
    click(&mut r.rt, "context-attach-cancel");
    assert!(!r.rt.state.context.attaching, "Cancel closes it");
}

#[test]
fn other_statuses_and_a_network_failure_have_their_own_words() {
    let mut r = rig(|f| f.attach_status = 502);
    r.wait("the links", |s| s.smart_links.by_session.contains_key(SESSION));
    r.rt.update(|s, _| {
        s.context.start_attach();
        s.context.url = rune::TextEdit::new("https://github.com/example/demo/pull/1");
    });
    r.rt.update(|s, _| s.submit_attach());
    r.wait("the refusal", |s| s.context.attach_error.is_some());
    assert_eq!(r.rt.state.context.attach_error.as_deref(), Some("Couldn't attach the link (HTTP 502)."));
}

#[test]
fn refresh_asks_the_server_and_keeps_its_spinner_for_a_moment() {
    let mut rt = shown(Case::Links);
    assert!(has(&rt, "context-footer"));
    assert!(rt.state.context.refreshing_since.is_none());
    rt.update(|s, _| {
        s.clock = 10.0;
        s.refresh_context();
    });
    assert!(rt.state.context.refreshing(10.5), "the spinner shows");
    assert!(!rt.state.context.refreshing(11.3), "and ends after 1.2 s");
}

#[test]
fn the_footer_says_when_github_was_last_checked() {
    let rt = shown(Case::Links);
    assert!(rt.state.smart_links.last_checked_at(SESSION).is_some());
    let none = shown(Case::NotConnected);
    assert!(none.state.smart_links.last_checked_at(SESSION).is_none(), "Waiting for GitHub");
}

#[test]
fn a_chips_request_flashes_its_target_once_and_again_on_a_repeat() {
    let mut rt = shown(Case::Flash);
    rt.update(|s, tx| {
        s.smart_links.request_focus(SESSION, "link-187");
        s.clock = 5.0;
        s.keep_context(5.0, tx);
    });
    assert_eq!(rt.state.context.flash.as_ref().map(|(t, _)| t.as_str()), Some("link-187"));
    assert!(rt.state.context.flash_ring("link-187", 5.2).is_some_and(|a| a > 0.3), "a ring is drawn");
    assert!(rt.state.context.flash_ring("link-187", 6.5).is_none(), "faded after 1.2 s");
    assert!(rt.state.context.flash_ring("link-12", 5.2).is_none(), "only its target");
    rt.update(|s, tx| {
        s.keep_context(8.0, tx);
        s.keep_context(9.0, tx);
    });
    assert!(rt.state.context.flash.is_none(), "once");
    rt.update(|s, tx| {
        s.smart_links.request_focus(SESSION, "link-187");
        s.keep_context(10.0, tx);
    });
    assert!(rt.state.context.flash_ring("link-187", 10.1).is_some(), "a repeat request flashes again");
}

#[test]
fn a_chip_opens_the_panel_on_the_context_tab() {
    let mut rt = app();
    rt.update(|s, _| s.show_chips(crate::mock::context::Case::Chips, true));
    rt.frame(FRAME);
    assert_ne!(rt.state.active_canvas(), "context");
    rt.update(|s, _| s.show_in_context("link-187"));
    rt.frame(FRAME);
    assert!(rt.state.content_open && rt.state.active_canvas() == "context");
    assert!(has(&rt, "context"), "the tab is drawn");
}

#[test]
fn pin_unpin_and_dismiss_are_on_the_right_rows() {
    let mut rt = shown(Case::Links);
    assert!(has(&rt, "context-pin-link-7") && !has(&rt, "context-unpin-link-7"), "a mentioned link can be pinned");
    assert!(has(&rt, "context-unpin-link-9"), "a pinned one can be unpinned");
    assert!(!has(&rt, "context-pin-link-42") && !has(&rt, "context-dismiss-link-42"), "the origin has neither");
    assert!(has(&rt, "context-dismiss-link-187") && !has(&rt, "context-pin-link-187"), "an own link can only be dismissed");
    click(&mut rt, "context-dismiss-link-7");
    assert!(!has(&rt, "context-link-link-7"), "gone at once");
}

#[test]
fn a_rows_detail_line_says_what_github_said() {
    let wire = |extra: Value| {
        let mut w = crate::mock::context::issue(42, "mentioned");
        for (k, v) in extra.as_object().unwrap() {
            w[k] = v.clone();
        }
        crate::fleet::smart_links::parse_wire_link(&w)
    };
    assert_eq!(row_detail(&wire(json!({"statusLabel": "Open"})), Some("Session started here")), "Session started here · Issue #42 · Open");
    assert_eq!(row_detail(&wire(json!({"enrichmentStatus": "pending", "title": "example/demo#42"})), None), "Checking GitHub\u{2026}");
    assert_eq!(row_detail(&wire(json!({"enrichmentStatus": "not_found", "title": "example/demo#42"})), None), "Not found on GitHub");
    assert_eq!(row_detail(&wire(json!({"enrichmentStatus": "error", "title": "example/demo#42"})), None), "Couldn't check GitHub");
}

#[test]
fn the_workspace_lists_what_exists() {
    assert_eq!(
        workspace_rows(Some("fix/cache-limit"), Some("worktree"), "/srv/work/demo"),
        [("Branch", "fix/cache-limit".to_string()), ("Isolation", "worktree".to_string()), ("Directory", "/srv/work/demo".to_string())]
    );
    assert_eq!(workspace_rows(None, Some("existing"), "/srv/work/demo"), [("Directory", "/srv/work/demo".to_string())]);
    assert!(workspace_rows(None, None, "").is_empty());
}
