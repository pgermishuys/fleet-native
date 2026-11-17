//! The sign-in page (piece S of docs/parity/09-app-frame.md, rows 11.1-11.4). Fleet has no tests for `LoginPage.vue`, so these are written
//! from reading it and keep its words: a wrong token says "Invalid token", a right one connects. Home is a fake Fleet that records what
//! it is asked, and every token here is invented.
use crate::fleet::auth;
use crate::fleet::link::{Connection, Job, Link, Post};
use crate::fleet::sign_in::{INVALID, Options, TOKEN_FIELD};
use crate::tests::{FRAME, app, click};
use crate::tests_machines_page::{Fake, Rt, exists, fake, words};
use crate::{State, test_wait};
use rune::{Key, KeyEvent, Mods};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};

const TOKEN: &str = "invented-token-0000";

struct Rig {
    rt: Rt,
    jobs: Receiver<Job>,
    home: Fake,
}

fn status(extra: &str) -> (&'static str, &'static str, u16, String) {
    (
        "GET",
        "/api/auth/status",
        200,
        format!(r#"{{"authEnabled":false,"tokenAuthEnabled":true,"authenticated":false,"isLocalhost":false{extra}}}"#),
    )
}

impl Rig {
    /// The app on a Fleet that has just turned it away.
    fn refused(routes: Vec<(&'static str, &'static str, u16, String)>) -> Rig {
        let home = fake(routes);
        let mut rt = app();
        let (tx, jobs) = channel::<Job>();
        let tx = Mutex::new(tx);
        let link = Link::start(home.base.clone(), Post::new(move |job| drop(tx.lock().unwrap().send(job))));
        rt.update(move |s, _| {
            s.link = Some(link);
            s.set_connection("home", Connection::Refused);
        });
        rt.frame(FRAME);
        rt.frame(FRAME);
        Rig { rt, jobs, home }
    }

    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.jobs, what, done, |s| format!("sign-in {:?}, error {:?}", s.sign_in, s.sign_in.error));
    }

    fn offered(&mut self) {
        self.wait("the sign-in options", |s| s.sign_in.open && s.sign_in.options != Options::Loading);
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    fn type_token(&mut self, text: &str) {
        click(&mut self.rt, TOKEN_FIELD);
        self.rt.key(KeyEvent { key: Key::Text(text.into()), mods: Mods::default() });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    fn everything_drawn(&self) -> String {
        self.rt.elements().iter().filter_map(|e| e.text.clone().into_iter().chain(e.label.clone()).reduce(|a, b| a + &b)).collect()
    }
}

fn login(status: u16) -> (&'static str, &'static str, u16, String) {
    ("POST", "/auth/token-login", status, "{}".into())
}

#[test]
fn opens_the_page_when_the_fleet_refuses_and_asks_what_it_offers() {
    let mut rig = Rig::refused(vec![status("")]);
    assert!(rig.rt.state.sign_in.open);
    assert!(exists(&rig.rt, "sign-in-loading"), "Loading sign-in options… while the status is read");
    assert_eq!(words(&rig.rt, "sign-in-loading"), "Loading sign-in options…");
    rig.offered();
    assert_eq!(rig.rt.state.sign_in.options, Options::Token);
    assert_eq!(words(&rig.rt, "sign-in-title"), "Weave");
    assert_eq!(words(&rig.rt, "sign-in-subtitle"), "Agent Fleet");
    assert_eq!(words(&rig.rt, "sign-in-lede"), "Orchestrate AI coding agents across your projects.");
    assert_eq!(words(&rig.rt, "sign-in-label"), "Access token");
    assert!(words(&rig.rt, "sign-in-hint").starts_with("It's in the sign-in link Fleet prints when it starts"));
    assert_eq!(words(&rig.rt, "sign-in-continue"), "Continue");
    assert!(!exists(&rig.rt, "sidebar-gutter"), "a page of its own: no session list round it");
    assert_eq!(rig.home.seen("GET", "/api/auth/status").len(), 1);
}

#[test]
fn the_page_opens_once_for_one_refusal() {
    let mut rig = Rig::refused(vec![status("")]);
    rig.offered();
    rig.rt.update(|s, _| s.sign_in.error = Some("kept".into()));
    rig.rt.update(|s, _| s.set_connection("home", Connection::Refused));
    assert_eq!(rig.rt.state.sign_in.error.as_deref(), Some("kept"), "the same refusal again changes nothing");
    assert_eq!(rig.home.seen("GET", "/api/auth/status").len(), 1);
}

#[test]
fn a_cloud_fleet_is_not_offered_a_token_form() {
    let mut rig = Rig::refused(vec![("GET", "/api/auth/status", 200, r#"{"authEnabled":true,"tokenAuthEnabled":false}"#.into())]);
    rig.offered();
    assert_eq!(rig.rt.state.sign_in.options, Options::Oidc);
    assert!(exists(&rig.rt, "sign-in-oidc") && !exists(&rig.rt, TOKEN_FIELD));
}

#[test]
fn an_unreadable_status_still_offers_the_token_form() {
    let mut rig = Rig::refused(vec![]);
    rig.offered();
    assert_eq!(rig.rt.state.sign_in.options, Options::Unknown);
    assert!(exists(&rig.rt, TOKEN_FIELD) && exists(&rig.rt, "sign-in-continue"));
}

#[test]
fn a_blank_token_says_invalid_token_without_asking_the_fleet() {
    let mut rig = Rig::refused(vec![status(""), login(200)]);
    rig.offered();
    rig.type_token("   ");
    click(&mut rig.rt, "sign-in-continue");
    assert_eq!(words(&rig.rt, "sign-in-error"), INVALID);
    assert_eq!(INVALID, "Invalid token");
    assert!(rig.home.seen("POST", "/auth/token-login").is_empty());
}

#[test]
fn a_wrong_token_says_invalid_token_and_keeps_the_page() {
    let mut rig = Rig::refused(vec![status(""), login(401)]);
    rig.offered();
    rig.type_token("not-the-token");
    click(&mut rig.rt, "sign-in-continue");
    rig.wait("the refusal", |s| s.sign_in.error.is_some());
    rig.rt.frame(FRAME);
    assert_eq!(words(&rig.rt, "sign-in-error"), "Invalid token");
    assert_eq!(rig.home.seen("POST", "/auth/token-login").len(), 1);
    assert!(rig.rt.state.sign_in.open && !rig.rt.state.sign_in.submitting);
    assert!(auth::token_for(&rig.home.base).is_none(), "a token the Fleet turned away is not kept");
    assert_eq!(rig.rt.state.sign_in.token.text, "not-the-token", "the field keeps what was typed, to be fixed");
}

#[test]
fn a_right_token_connects() {
    let mut rig = Rig::refused(vec![status(""), login(200)]);
    rig.offered();
    rig.type_token(TOKEN);
    click(&mut rig.rt, "sign-in-continue");
    let base = rig.home.base.clone();
    let Rig { rt, jobs, home } = &mut rig;
    test_wait::drive(
        rt,
        jobs,
        "the Fleet to be asked again with the token",
        |s| !s.sign_in.open && home.seen("GET", "/api/harnesses").iter().any(|(_, bearer)| bearer.as_deref() == Some(TOKEN)),
        |s| format!("sign-in {:?}", s.sign_in),
    );
    assert_eq!(rig.home.seen("POST", "/auth/token-login")[0].0["token"], TOKEN, "the token is checked the way Fleet's page does");
    assert_eq!(auth::token_for(&base).map(|t| t.expose().to_string()).as_deref(), Some(TOKEN));
    assert_ne!(rig.rt.state.connection, Connection::Refused);
    assert!(rig.rt.state.sign_in.token.text.is_empty(), "nothing typed is left behind");
    rig.rt.frame(FRAME);
    assert!(!exists(&rig.rt, "sign-in-page"), "the app is back");
}

#[test]
fn enter_in_the_field_continues_and_the_token_is_trimmed() {
    let mut rig = Rig::refused(vec![status(""), login(401)]);
    rig.offered();
    rig.type_token("  padded-token  ");
    rig.rt.key(KeyEvent { key: Key::Enter, mods: Mods::default() });
    rig.wait("the answer", |s| s.sign_in.error.is_some());
    assert_eq!(rig.home.seen("POST", "/auth/token-login")[0].0["token"], "padded-token");
}

#[test]
fn says_signing_in_and_holds_the_button_while_it_asks() {
    let mut rt = app();
    rt.update(|s, _| s.show_sign_in(true, crate::mock::sign_in::Case::Signing));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(words(&rt, "sign-in-continue"), "Signing in…");
}

#[test]
fn the_token_is_never_drawn_or_logged() {
    let mut rig = Rig::refused(vec![status(""), login(401)]);
    rig.offered();
    rig.type_token(TOKEN);
    assert_eq!(rig.rt.state.sign_in.token.text, TOKEN, "typed");
    assert!(!rig.everything_drawn().contains(TOKEN), "the field draws bullets, not the token");
    let printed = format!("{:?} {:?}", rig.rt.state.sign_in, rig.rt.state.sign_in.token);
    assert!(!printed.contains(TOKEN), "{printed}");
    click(&mut rig.rt, "sign-in-continue");
    rig.wait("the refusal", |s| s.sign_in.error.is_some());
    rig.rt.frame(FRAME);
    assert!(!rig.everything_drawn().contains(TOKEN));
    assert!(!rig.rt.state.connection_note().unwrap_or_default().contains(TOKEN));
}
