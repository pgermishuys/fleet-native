//! Component 16, piece L: `HarnessSignInPanel.test.ts`'s cases marked "F" under their names, and the flow's own rules, against the
//! fake Fleet of `tests_settings_harness_sign_in`. Every key and code in here is invented.
use crate::tests_settings_harness_sign_in::{Fake, Rig, rig};
use rune::Tx;
use serde_json::{Value, json};

/// OpenCode 2's providers cut down: Azure asks for a resource name with its key (or reads a variable), and OpenAI takes a key or a
/// sign-in in the browser.
fn providers() -> Vec<Value> {
    vec![
        json!({"id": "azure", "name": "Azure", "connections": [], "methods": [
            {"type": "key", "label": "API key", "fields": [
                {"key": "resourceName", "type": "string", "title": "Enter Azure Resource Name", "required": true, "hidden": false}]},
            {"type": "env", "label": "Environment variable", "fields": [], "environmentVariables": ["AZURE_API_KEY"]}]}),
        json!({"id": "openai", "name": "OpenAI", "connections": [], "methods": [
            {"type": "key", "label": "API key", "fields": []},
            {"type": "oauth", "id": "chatgpt-browser", "label": "ChatGPT Pro/Plus (browser)", "fields": []}]}),
        json!({"id": "other", "name": "Other", "connections": [], "methods": [
            {"type": "command", "label": "Terminal", "fields": [], "command": ["other", "login"]}]}),
    ]
}

fn attempt() -> Value {
    json!({
        "id": "con_1",
        "url": "https://auth.example.test/oauth/authorize?redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback",
        "instructions": "Complete authorization in your browser. This window will close automatically.",
        "needsCode": false,
        "expiresAt": "2026-09-22T12:10:00Z",
        "callbackAddress": "http://localhost:1455/auth/callback"
    })
}

fn flow_rig(fake: Fake) -> Rig {
    let started = fake.attempt.clone().or_else(|| Some(attempt()));
    let mut r = rig(Fake { providers: providers(), attempt: started, ..fake });
    r.listed();
    r
}

fn bodies(r: &Rig) -> Vec<Value> {
    r.fake.lock().unwrap().bodies.iter().filter(|b| !b.is_null()).cloned().collect()
}

fn set(r: &mut Rig, f: impl FnOnce(&mut crate::State) + 'static) {
    r.rt.update(move |s, _| f(s));
    r.frames();
}

fn type_field(r: &mut Rig, key: &'static str, words: &'static str) {
    set(r, move |s| {
        let flow = s.settings.harnesses.sign_in.flow.as_mut().expect("a flow");
        let i = flow.edit_index(key).expect("a box for the field");
        flow.edits[i].1.replace_text(words);
    });
}

/// The providers to sign in to scroll in a list 22rem tall: a long form is reached by scrolling it, as in Fleet.
fn scroll_to(r: &mut Rig, key: &str) {
    let (x, y) = r.rt.find("harness-sign-in-list").expect("the list");
    for _ in 0..4 {
        r.rt.wheel(x, y, 400.0);
        r.rt.frame(crate::tests::FRAME);
    }
    let _ = key;
}

fn submit(r: &mut Rig) {
    scroll_to(r, "sign-in-submit");
    r.click("sign-in-submit");
}

fn browser_sign_in(r: &mut Rig) {
    r.click("harness-sign-in-open-openai");
    r.click("sign-in-method-oauth:chatgpt-browser");
    submit(r);
    r.wait("the attempt", |s| s.sign_in_flow("opencode2").is_some_and(|f| f.attempt.is_some()));
}

#[test]
fn signs_in_with_a_key_and_the_provider_s_own_fields() {
    let mut r = flow_rig(Fake::default());
    r.click("harness-sign-in-open-azure");
    submit(&mut r);
    assert_eq!(r.text_of("sign-in-error"), "Fill in \u{201c}Enter Azure Resource Name\u{201d}.");
    assert!(r.fake.lock().unwrap().writes.is_empty(), "nothing is sent for a form that isn't filled in");

    type_field(&mut r, "resourceName", "my-models");
    submit(&mut r);
    assert_eq!(r.text_of("sign-in-error"), "Enter the key.");

    set(&mut r, |s| s.settings.harnesses.sign_in.flow.as_mut().unwrap().key.replace_text("sk-dummy-0000"));
    submit(&mut r);
    r.wait("the notice", |s| s.settings.harnesses.sign_in.notice.is_some());
    r.wait("the list again", |s| {
        s.sign_ins("opencode2").is_some_and(|l| l.providers.iter().any(|p| p.id == "azure" && !p.connections.is_empty()))
    });
    assert_eq!(bodies(&r), [json!({"key": "sk-dummy-0000", "answers": {"resourceName": "my-models"}})]);
    assert_eq!(r.fake.lock().unwrap().writes[0], ("POST".to_string(), "/api/harnesses/opencode2/sign-in/azure/key".to_string()));
    assert_eq!(r.text_of("harness-sign-in-notice"), "Signed in to Azure. New sessions can use its models.");
    // Listed again, Azure has moved to the signed-in providers, and its form is gone with the key.
    assert!(r.text_of("harness-sign-in-summary-azure").contains("Signed in"));
    assert!(!r.has("sign-in-key"));
    assert!(r.rt.state.sign_in_flow("opencode2").is_none());
}

#[test]
fn shows_the_harness_s_reason_when_it_refuses() {
    let mut r = flow_rig(Fake { refuse_key: Some("Missing required form field: resourceName"), ..Fake::default() });
    r.click("harness-sign-in-open-openai");
    set(&mut r, |s| s.settings.harnesses.sign_in.flow.as_mut().unwrap().key.replace_text("sk-dummy-0000"));
    submit(&mut r);
    r.wait("the refusal", |s| s.sign_in_flow("opencode2").is_some_and(|f| f.error.is_some()));
    assert_eq!(r.text_of("sign-in-error"), "Missing required form field: resourceName");
    assert!(r.has("sign-in-key"), "the form stays, and so does what was typed, to try again");
    assert!(!r.rt.state.sign_in_flow("opencode2").unwrap().busy);
}

#[test]
fn describes_the_environment_method_instead_of_running_anything() {
    let mut r = flow_rig(Fake::default());
    r.click("harness-sign-in-open-azure");
    r.click("sign-in-method-env:");
    assert!(r.words().contains("AZURE_API_KEY"));
    assert!(r.words().contains("Set it where Fleet starts, then restart Fleet."));
    assert!(!r.has("sign-in-submit"), "nothing runs from here");
    assert!(r.fake.lock().unwrap().writes.is_empty());
}

#[test]
fn describes_the_command_method_and_runs_nothing() {
    let mut r = flow_rig(Fake::default());
    r.click("harness-sign-in-open-other");
    let words = r.words();
    assert!(words.contains("other login"), "{words}");
    assert!(words.contains("Fleet doesn't run commands for you"));
    assert!(!r.has("sign-in-submit"));
    assert!(r.fake.lock().unwrap().writes.is_empty());
}

#[test]
fn opens_the_provider_s_page_and_follows_the_browser_sign_in_until_it_s_done() {
    let mut r = flow_rig(Fake { statuses: vec![json!({"status": "pending"}), json!({"status": "complete"})], ..Fake::default() });
    browser_sign_in(&mut r);
    assert_eq!(bodies(&r)[0], json!({"methodId": "chatgpt-browser", "answers": {}}));
    assert!(r.has("sign-in-attempt-url"));
    assert!(r.words().contains("Waiting for you to finish on OpenAI's page. It runs out at "), "{}", r.words());
    // On this computer the page comes back by itself; pasting its address is only offered.
    assert!(!r.has("sign-in-remote-note"));
    assert!(r.has("sign-in-paste-open"));
    assert!(!r.has("sign-in-landed-on"));

    // Two looks, two seconds apart: the first says pending, the second complete.
    let mut tx = Tx::default();
    r.rt.update(|s, _| s.clock = 10.0);
    r.rt.update(|s, _| s.settings.harnesses.sign_in.flow.as_mut().unwrap().poll_due = Some(12.0));
    r.rt.update(|s, tx2| assert!(!s.keep_sign_in_flow(11.0, tx2), "not yet"));
    r.rt.update(|s, _| {
        s.keep_sign_in_flow(12.0, &mut tx);
    });
    r.wait("the first look", |s| s.sign_in_flow("opencode2").is_some_and(|f| !f.checking && f.poll_due.is_some()));
    r.rt.update(|s, _| {
        s.keep_sign_in_flow(f64::MAX, &mut Tx::default());
    });
    r.wait("the notice", |s| s.settings.harnesses.sign_in.notice.is_some());
    assert_eq!(r.text_of("harness-sign-in-notice"), "Signed in to OpenAI. New sessions can use its models.");
    let looks = r.fake.lock().unwrap().writes.iter().filter(|(m, _)| m == "GET").count();
    assert_eq!(looks, 2);
    assert!(r.rt.state.sign_in_flow("opencode2").is_none(), "the flow closes");
}

#[test]
fn from_another_device_says_the_page_won_t_load_and_takes_its_address() {
    use crate::fleet::settings::harness_sign_in::flow::is_remote;
    // A machine other than this one is what Fleet calls another device (decided in docs/parity/16-settings.md).
    assert!(is_remote("falcon") && !is_remote(crate::fleet::settings::HOME_MACHINE_KEY));
    let mut r = flow_rig(Fake::default());
    browser_sign_in(&mut r);
    assert!(r.has("sign-in-paste-open") && !r.has("sign-in-remote-note"), "this computer: only offered");
    // The flow was opened on another machine: the note is there and so is the paste box, with nothing to press first.
    set(&mut r, |s| s.settings.harnesses.sign_in.flow.as_mut().unwrap().remote = true);
    assert!(r.text_of("sign-in-remote-note").contains("http://localhost:1455/auth/callback"), "{}", r.words());
    assert!(r.words().contains("You're using Fleet from another device."));
    assert!(r.has("sign-in-landed-on") && !r.has("sign-in-paste-open"));
    scroll_to(&mut r, "x");
    r.click("sign-in-landed-finish");
    assert!(r.fake.lock().unwrap().writes.iter().all(|(_, p)| !p.ends_with("/callback")), "nothing to send while it is empty");
    set(&mut r, |s| {
        s.settings.harnesses.sign_in.flow.as_mut().unwrap().landed.replace_text("http://localhost:1455/auth/callback?code=abc&state=xyz")
    });
    scroll_to(&mut r, "x");
    r.click("sign-in-landed-finish");
    let fake = r.fake.clone();
    crate::test_wait::until_true("the callback", move || fake.lock().unwrap().writes.iter().any(|(_, p)| p.ends_with("/callback")));
    assert_eq!(bodies(&r).last().unwrap(), &json!({"address": "http://localhost:1455/auth/callback?code=abc&state=xyz"}));
}

#[test]
fn says_why_a_browser_sign_in_failed() {
    let mut r = flow_rig(Fake { statuses: vec![json!({"status": "failed", "message": "access_denied"})], ..Fake::default() });
    browser_sign_in(&mut r);
    r.rt.update(|s, _| {
        s.keep_sign_in_flow(f64::MAX, &mut Tx::default());
    });
    r.wait("the failure", |s| s.sign_in_flow("opencode2").is_some_and(|f| f.error.is_some()));
    assert_eq!(r.text_of("sign-in-error"), "The sign-in didn't work: access_denied.");
    assert!(!r.has("sign-in-attempt"), "the attempt is over, the form is back");
}

#[test]
fn says_a_browser_sign_in_ran_out_of_time() {
    let mut r = flow_rig(Fake { statuses: vec![json!({"status": "expired"})], ..Fake::default() });
    browser_sign_in(&mut r);
    r.rt.update(|s, _| {
        s.keep_sign_in_flow(f64::MAX, &mut Tx::default());
    });
    r.wait("the end", |s| s.sign_in_flow("opencode2").is_some_and(|f| f.error.is_some()));
    assert_eq!(r.text_of("sign-in-error"), "The sign-in ran out of time. Start it again.");
}

#[test]
fn cancels_a_browser_sign_in_left_open() {
    let mut r = flow_rig(Fake::default());
    browser_sign_in(&mut r);
    // Leaving the page ends it the same way Cancel does.
    r.rt.update(|s, _| s.rail = crate::Rail::Sessions);
    r.rt.update(|s, tx| {
        s.keep_harnesses_page(tx);
    });
    let fake = r.fake.clone();
    crate::test_wait::until_true("the cancel", move || {
        fake.lock().unwrap().writes.contains(&("DELETE".to_string(), "/api/harnesses/opencode2/sign-in/openai/attempts/con_1".to_string()))
    });
    assert!(r.rt.state.sign_in_flow("opencode2").is_none());
}

#[test]
fn cancel_ends_the_attempt_and_closes_the_provider() {
    let mut r = flow_rig(Fake::default());
    browser_sign_in(&mut r);
    r.click("sign-in-cancel");
    let fake = r.fake.clone();
    crate::test_wait::until_true("the cancel", move || fake.lock().unwrap().writes.iter().any(|(m, _)| m == "DELETE"));
    assert!(!r.has("sign-in-attempt"));
    assert_eq!(r.rt.state.settings.harnesses.sign_in.open, None);
}

#[test]
fn takes_a_code_and_says_when_it_is_missing() {
    let device = json!({
        "id": "con_2", "url": "https://auth.example.test/device", "instructions": "Open the page and enter code: ABCD-1234",
        "needsCode": true, "expiresAt": "2026-09-22T12:10:00Z", "callbackAddress": null
    });
    let mut r = flow_rig(Fake { attempt: Some(device), ..Fake::default() });
    browser_sign_in(&mut r);
    scroll_to(&mut r, "sign-in-copy-code");
    assert!(r.has("sign-in-copy-code"), "the instructions hold a code to copy");
    assert!(r.has("sign-in-code"));
    r.click("sign-in-code-finish");
    assert_eq!(r.text_of("sign-in-error"), "Paste the code the provider showed you.");
    set(&mut r, |s| s.settings.harnesses.sign_in.flow.as_mut().unwrap().code.replace_text("123456"));
    r.click("sign-in-code-finish");
    let fake = r.fake.clone();
    crate::test_wait::until_true("the code", move || fake.lock().unwrap().writes.iter().any(|(_, p)| p.ends_with("/code")));
    assert_eq!(bodies(&r).last().unwrap(), &json!({"code": "123456"}));
}

#[test]
fn another_provider_s_sign_in_replaces_the_open_one_and_cancels_its_attempt() {
    let mut r = flow_rig(Fake::default());
    browser_sign_in(&mut r);
    r.click("harness-sign-in-open-azure");
    let fake = r.fake.clone();
    crate::test_wait::until_true("the cancel", move || fake.lock().unwrap().writes.iter().any(|(m, _)| m == "DELETE"));
    assert!(r.has("sign-in-flow-azure") && !r.has("sign-in-flow-openai"));
}

#[test]
fn the_key_box_draws_dots_and_never_the_key() {
    let mut r = flow_rig(Fake::default());
    r.click("harness-sign-in-open-openai");
    set(&mut r, |s| s.settings.harnesses.sign_in.flow.as_mut().unwrap().key.replace_text("sk-dummy-0000"));
    assert!(!r.words().contains("sk-dummy-0000"));
    assert!(!format!("{:?}", r.rt.state.settings.harnesses.sign_in).contains("sk-dummy-0000"));
}
