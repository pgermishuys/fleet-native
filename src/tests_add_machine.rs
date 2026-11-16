//! The Add a machine form (piece K of docs/parity/17-machines.md): the cases of `MachinesSection.test.ts` and `stores/machines.test.ts`
//! under Fleet's names. Home is a fake Fleet that records what it is asked; every token here is invented.
use crate::fleet::pairing::{CODE_FIELD, LINK_FIELD, TOKEN_FIELD};
use crate::tests::{FRAME, click};
use crate::tests_machines_page::{Rig, Rt, exists, fake, page, press, words};
use rune::Key;
use serde_json::json;

const LINK: &str = "http://100.64.90.72:2113/add-machine#K7QM-R2XD";

fn kestrel() -> String {
    json!({ "machineId": "m-kestrel", "name": "kestrel", "os": "linux", "version": "0.72.0", "baseUrl": "http://100.64.90.72:2113" })
        .to_string()
}

fn added() -> String {
    json!({ "id": "m-kestrel", "name": "kestrel", "baseUrl": "http://100.64.90.72:2113", "token": "scratch-key-0001", "addedAt": "2026-11-16T10:00:00Z" })
        .to_string()
}

fn type_in(rt: &mut Rt, field: &str, text: &str) {
    click(rt, field);
    rt.key(rune::KeyEvent { key: Key::Text(text.into()), mods: rune::Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn open(rig: &mut Rig) {
    click(&mut rig.rt, "machines-add");
    assert!(exists(&rig.rt, "machines-add-form"));
}

fn refusal(error: &str) -> String {
    json!({ "error": error }).to_string()
}

#[test]
fn opens_the_add_a_machine_form_when_something_else_asked_for_it() {
    let mut rt = page();
    assert!(!exists(&rt, "machines-add-form"));
    rt.update(|s, _| s.add_machine());
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(exists(&rt, "machines-add-form"));
    assert!(!rt.state.add_machine_asked, "asked once, opened once");
    assert!(!exists(&rt, "machines-add"), "the button is gone while the form is open");
}

#[test]
fn opens_the_form_at_once_when_it_was_asked_for_before_machines_was_shown() {
    let mut rt = crate::tests::app();
    rt.update(|s, _| s.add_machine());
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.rail, crate::Rail::Settings);
    assert!(exists(&rt, "machines-add-form"));
}

#[test]
fn opens_on_the_pairing_link_when_fleet_sees_no_machines() {
    let mut rt = page();
    click(&mut rt, "machines-add");
    assert!(exists(&rt, LINK_FIELD) && exists(&rt, "machines-add-link-help"));
    assert!(words(&rt, "machines-add-link-help").contains("fleet node pair"));
}

#[test]
fn looks_at_the_machine_behind_a_pasted_link_and_says_what_adding_it_means() {
    let home = fake(vec![("POST", "/api/machines/pair/preview", 200, kestrel())]);
    let mut rig = Rig::new(&home);
    open(&mut rig);
    assert!(!rig.rt.state.machine_page.add.can_add());
    type_in(&mut rig.rt, LINK_FIELD, LINK);
    rig.wait("the machine behind the link", |s| s.machine_page.add.found.is_some());
    assert_eq!(words(&rig.rt, "machines-add-found-line"), "kestrel · Linux · Fleet 0.72.0");
    assert_eq!(words(&rig.rt, "machines-add-found-reach"), "Reached over Tailscale at 100.64.90.72:2113");
    assert!(words(&rig.rt, "machines-add-note").contains("kestrel gives this Fleet a key of its own."));
    assert_eq!(rig.rt.state.machine_page.add.label(), "Add kestrel");
    let asked = home.seen("POST", "/api/machines/pair/preview");
    assert_eq!(asked.len(), 1, "one look, and the code is not used up");
    assert_eq!(asked[0].0, json!({ "baseUrl": "http://100.64.90.72:2113", "code": "K7QM-R2XD" }));
    assert!(home.seen("POST", "/api/machines/pair").is_empty());
}

#[test]
fn adds_the_machine_from_the_link_and_says_so() {
    let home = fake(vec![("POST", "/api/machines/pair/preview", 200, kestrel()), ("POST", "/api/machines/pair", 200, added())]);
    let mut rig = Rig::new(&home);
    open(&mut rig);
    type_in(&mut rig.rt, LINK_FIELD, LINK);
    rig.wait("the machine behind the link", |s| s.machine_page.add.found.is_some());
    click(&mut rig.rt, "machines-add-submit");
    rig.wait("the machine added", |s| s.machine_page.add.added.is_some());
    rig.rt.frame(FRAME);
    assert_eq!(home.seen("POST", "/api/machines/pair")[0].0, json!({ "baseUrl": "http://100.64.90.72:2113", "code": "K7QM-R2XD" }));
    assert!(!exists(&rig.rt, "machines-add-form"), "the form closes");
    assert!(rig.rt.state.machine_page.add.link.text.is_empty(), "and is emptied");
    assert_eq!(words(&rig.rt, "machines-added"), "Added kestrel. Its sessions are in the sessions list.");
}

#[test]
fn takes_a_bare_address_asks_for_the_code_then_looks() {
    let home = fake(vec![("POST", "/api/machines/pair/preview", 200, kestrel())]);
    let mut rig = Rig::new(&home);
    open(&mut rig);
    assert!(!exists(&rig.rt, CODE_FIELD));
    type_in(&mut rig.rt, LINK_FIELD, "100.64.90.72:2113");
    assert!(exists(&rig.rt, CODE_FIELD), "the code is asked for");
    assert!(home.seen("POST", "/api/machines/pair/preview").is_empty(), "nothing to look at without a code");
    type_in(&mut rig.rt, CODE_FIELD, "k7qm r2xd");
    rig.wait("the machine behind the address", |s| s.machine_page.add.found.is_some());
    assert_eq!(home.seen("POST", "/api/machines/pair/preview")[0].0, json!({ "baseUrl": "http://100.64.90.72:2113", "code": "K7QM-R2XD" }));
}

#[test]
fn keeps_the_master_token_under_advanced_for_an_address() {
    let home = fake(vec![("POST", "/api/machines", 200, added())]);
    let mut rig = Rig::new(&home);
    open(&mut rig);
    type_in(&mut rig.rt, LINK_FIELD, "100.64.90.72:2113");
    assert!(!exists(&rig.rt, TOKEN_FIELD), "the token is under Advanced");
    click(&mut rig.rt, "machines-add-advanced");
    assert!(exists(&rig.rt, TOKEN_FIELD));
    type_in(&mut rig.rt, TOKEN_FIELD, "  scratch-master-0002 ");
    assert!(!exists(&rig.rt, CODE_FIELD), "with a token there is no code");
    assert_eq!(rig.rt.state.machine_page.add.label(), "Add machine");
    click(&mut rig.rt, "machines-add-submit");
    rig.wait("the machine added", |s| s.machine_page.add.added.is_some());
    // The address is normalised, the token trimmed; nothing was looked at, and no code was redeemed.
    assert_eq!(home.seen("POST", "/api/machines")[0].0, json!({ "baseUrl": "http://100.64.90.72:2113", "token": "scratch-master-0002" }));
    assert!(home.seen("POST", "/api/machines/pair/preview").is_empty() && home.seen("POST", "/api/machines/pair").is_empty());
}

#[test]
fn says_why_a_link_cant_be_used_and_why_the_machine_wasnt_found() {
    let diagnosis = json!({ "code": "pairing", "headline": "Nothing answers", "try": "starting Fleet on kestrel.",
        "steps": [{ "label": "Machine answers", "state": "failed", "detail": "No route to the host." }] });
    let home = fake(vec![(
        "POST",
        "/api/machines/pair/preview",
        400,
        json!({ "error": "Couldn't reach http://100.64.90.72:2113. Check the address, that Fleet is running there, and that this machine can reach it.", "diagnosis": diagnosis }).to_string(),
    )]);
    let mut rig = Rig::new(&home);
    open(&mut rig);
    type_in(&mut rig.rt, LINK_FIELD, "kestrel#nope");
    assert!(words(&rig.rt, "machines-add-link-invalid").starts_with("That link has no valid code."));
    click(&mut rig.rt, LINK_FIELD);
    for _ in 0..12 {
        press(&mut rig.rt, Key::Backspace);
    }
    type_in(&mut rig.rt, LINK_FIELD, "###");
    assert!(
        words(&rig.rt, "machines-add-link-invalid").starts_with("That isn't a pairing link or an address."),
        "{}",
        rig.rt.state.machine_page.add.link.text
    );
    rig.rt.update(|s, _| s.machine_page.add.link = rune::TextEdit::new(LINK.to_string()));
    rig.rt.frame(FRAME);
    rig.wait("the refusal", |s| s.machine_page.add.preview_error.is_some());
    rig.rt.frame(FRAME);
    // With a diagnosis its headline is the message (`diagnosis?.headline ?? error.message`); the server's sentence stays in the state.
    assert!(words(&rig.rt, "machines-add-preview-error").starts_with("Nothing answers"));
    assert!(rig.rt.state.machine_page.add.preview_error.as_ref().unwrap().message.starts_with("Couldn't reach http://100.64.90.72:2113."));
    assert!(words(&rig.rt, "machines-add-preview-error").contains("No route to the host."), "the diagnosis's steps show");
    assert!(words(&rig.rt, "machines-add-preview-error").contains("starting Fleet on kestrel."));
    assert!(!rig.rt.state.machine_page.add.can_add());
}

#[test]
fn says_what_went_wrong_when_home_cant_pair_and_keeps_nothing() {
    let home = fake(vec![
        ("POST", "/api/machines/pair/preview", 200, kestrel()),
        (
            "POST",
            "/api/machines/pair",
            400,
            refusal("That code has expired or was already used. Run fleet node pair on the other machine for a new one."),
        ),
    ]);
    let mut rig = Rig::new(&home);
    open(&mut rig);
    type_in(&mut rig.rt, LINK_FIELD, LINK);
    rig.wait("the machine behind the link", |s| s.machine_page.add.found.is_some());
    click(&mut rig.rt, "machines-add-submit");
    rig.wait("the refusal", |s| s.machine_page.add.error.is_some());
    rig.rt.frame(FRAME);
    assert!(words(&rig.rt, "machines-add-error").starts_with("That code has expired or was already used."));
    assert!(exists(&rig.rt, "machines-add-form"), "the form stays, with what was typed");
    assert_eq!(rig.rt.state.machine_page.add.link.text, LINK);
    assert!(rig.rt.state.machine_page.add.added.is_none());
}

#[test]
fn wont_pair_from_a_fleet_that_doesnt_keep_the_list() {
    // A Fleet too old to have the route answers 404 with no words.
    let home = fake(vec![("POST", "/api/machines/pair/preview", 200, kestrel())]);
    let mut rig = Rig::new(&home);
    open(&mut rig);
    type_in(&mut rig.rt, LINK_FIELD, LINK);
    rig.wait("the machine behind the link", |s| s.machine_page.add.found.is_some());
    click(&mut rig.rt, "machines-add-submit");
    rig.wait("the refusal", |s| s.machine_page.add.error.is_some());
    assert_eq!(
        rig.rt.state.machine_page.add.error.as_ref().unwrap().message,
        "This Fleet can't pair a machine from here. Add it by address with its token instead."
    );
}

#[test]
fn says_the_servers_words_when_an_address_and_token_are_refused() {
    for why in [
        "That token wasn't accepted.",
        "Couldn't reach http://100.64.90.72:2113. Check the address, that Fleet is running there, and that this machine can reach it.",
        "That Fleet speaks machine contract 2; this one speaks 1. Update the older one.",
        "That Fleet is too old to add as a machine. Update it first.",
        "That Fleet signs people in with an identity provider, which machines don't support yet.",
        "That's this machine.",
    ] {
        let home = fake(vec![("POST", "/api/machines", 400, refusal(why))]);
        let mut rig = Rig::new(&home);
        open(&mut rig);
        type_in(&mut rig.rt, LINK_FIELD, "100.64.90.72:2113");
        click(&mut rig.rt, "machines-add-advanced");
        type_in(&mut rig.rt, TOKEN_FIELD, "scratch-0003");
        click(&mut rig.rt, "machines-add-submit");
        rig.wait("the refusal", |s| s.machine_page.add.error.is_some());
        assert_eq!(rig.rt.state.machine_page.add.error.as_ref().unwrap().message, why);
        assert!(rig.rt.state.machine_page.add.added.is_none(), "nothing is kept");
    }
}

#[test]
fn asks_for_the_token_when_it_is_empty_and_for_an_address_it_can_read() {
    let home = fake(vec![]);
    let err = crate::fleet::pairing::add_by_token(&home.base, "http://100.64.90.72:2113", "  ").unwrap_err();
    assert_eq!(err.message, "Paste the machine's access token.");
    assert!(home.seen("POST", "/api/machines").is_empty());
}

#[test]
fn closes_and_clears_with_cancel_and_with_escape() {
    let mut rt = page();
    click(&mut rt, "machines-add");
    type_in(&mut rt, LINK_FIELD, LINK);
    click(&mut rt, "machines-add-cancel");
    assert!(!exists(&rt, "machines-add-form") && exists(&rt, "machines-add"));
    assert!(rt.state.machine_page.add.link.text.is_empty());
    click(&mut rt, "machines-add");
    type_in(&mut rt, LINK_FIELD, "100.64.90.72:2113");
    press(&mut rt, Key::Escape);
    assert!(!exists(&rt, "machines-add-form"));
    assert!(rt.state.machine_page.add.link.text.is_empty() && !rt.state.machine_page.add.advanced);
}
