//! `lib/__tests__/browser-target.test.ts` ("browserTarget"), the steps half of `stores/__tests__/agent-browser.test.ts`
//! and the counts of `BrowserSteps.vue`'s head, under Fleet's names.
use crate::conv::browser::{BrowserStep, BrowserTarget, Steps, browser_target, heading};
use serde_json::json;

fn address(url: &str) -> Option<BrowserTarget> {
    Some(BrowserTarget::Address(url.into()))
}

// describe "browserTarget": it.each "reads %s as an address"
#[test]
fn reads_each_as_an_address() {
    for (input, url) in [
        ("5173", "http://localhost:5173/"),
        ("localhost:5173", "http://localhost:5173/"),
        ("localhost:5173/cart", "http://localhost:5173/cart"),
        ("127.0.0.1:8080", "http://127.0.0.1:8080/"),
        ("shop.localhost:3000", "http://shop.localhost:3000/"),
        ("https://localhost:7043/swagger", "https://localhost:7043/swagger"),
        (" http://[::1]:4000 ", "http://[::1]:4000/"),
    ] {
        assert_eq!(browser_target(input), address(url), "{input}");
    }
}

// it.each "reads %s as a command"
#[test]
fn reads_each_as_a_command() {
    for input in ["npm run dev", "bun --hot server.ts", "dotnet watch -- --urls http://localhost:$PORT", "python3 -m http.server 8000"] {
        assert_eq!(browser_target(input), Some(BrowserTarget::Command(input.into())), "{input}");
    }
}

#[test]
fn has_nothing_for_an_empty_field() {
    assert_eq!(browser_target("   "), None);
}

// Beyond Fleet's cases: what the regular expressions decide.
#[test]
fn keeps_to_the_shapes_of_fleets_expressions() {
    // Six digits is too long for a port, so it is a command; a port over five digits is no address either.
    assert_eq!(browser_target("123456"), Some(BrowserTarget::Command("123456".into())));
    assert_eq!(browser_target("localhost:123456"), Some(BrowserTarget::Command("localhost:123456".into())));
    // An address is case-insensitive, and its scheme's own port is left out as `new URL` does.
    assert_eq!(browser_target("HTTP://LocalHost:80/A"), address("http://localhost/A"));
    assert_eq!(browser_target("LOCALHOST"), address("http://localhost/"));
    assert_eq!(browser_target("0.0.0.0:9000"), address("http://0.0.0.0:9000/"));
    // A host that merely starts like a local one is not one; a url with a space in it is a command.
    assert_eq!(browser_target("localhost.example.com"), Some(BrowserTarget::Command("localhost.example.com".into())));
    assert_eq!(browser_target("http://a b"), Some(BrowserTarget::Command("http://a b".into())));
    assert_eq!(browser_target("localhost:5173/a b"), Some(BrowserTarget::Command("localhost:5173/a b".into())));
}

fn step(seq: i64, call: &str) -> BrowserStep {
    BrowserStep::read(&json!({"seq": seq, "callId": call, "ok": true, "summary": format!("Step {seq}"), "detail": "click @e2 · 170 ms"}))
        .unwrap()
}

// describe "agent browser store"
#[test]
fn lists_a_calls_steps_in_order_once_each() {
    let mut steps = Steps::default();
    steps.apply("s1", step(3, "call_2"));
    steps.apply("s1", step(2, "call_1"));
    steps.apply("s1", step(1, "call_1"));
    steps.apply("s1", step(2, "call_1"));
    let seqs = |call: &str| steps.of("s1", call).iter().map(|s| s.seq).collect::<Vec<_>>();
    assert_eq!(seqs("call_1"), [1, 2]);
    assert_eq!(seqs("call_2"), [3]);
    assert!(steps.of("s1", "").is_empty());
    assert!(steps.of("another", "call_1").is_empty());
}

#[test]
fn keeps_steps_that_arrived_while_the_session_was_loading() {
    let mut steps = Steps::default();
    steps.apply("s1", step(3, "call_1"));
    steps.set_from_server("s1", vec![step(2, "call_1"), step(1, "call_1")]);
    let seqs: Vec<i64> = steps.of("s1", "call_1").iter().map(|s| s.seq).collect();
    assert_eq!(seqs, [1, 2, 3]);
    // A step the load also has is the server's, not a second one.
    steps.set_from_server("s1", vec![step(3, "call_1")]);
    assert_eq!(steps.count("s1", "call_1"), 3);
}

// The head's wording (`BrowserSteps.vue`): the tense follows the call, the plural follows the count.
#[test]
fn the_head_counts_steps_and_says_singular_only_for_one() {
    assert_eq!(heading(true, 1), "Using its browser · 1 step");
    assert_eq!(heading(false, 1), "Used its browser · 1 step");
    assert_eq!(heading(true, 3), "Using its browser · 3 steps");
    assert_eq!(heading(false, 12), "Used its browser · 12 steps");
}

#[test]
fn reads_a_step_as_the_server_sends_it() {
    let read = BrowserStep::read(&json!({
        "sessionId": "s1", "seq": 4, "at": "2026-10-02T10:00:00Z", "kind": "click", "summary": "Couldn't click “Save”",
        "detail": "click @e9 · 5.0 s", "ok": false, "error": "No element.", "callId": "call_1", "tabId": "tab_1",
        "screenshot": {"id": "shot_1", "width": 1280, "height": 800}
    }))
    .unwrap();
    assert!(!read.ok);
    assert_eq!(read.error.as_deref(), Some("No element."));
    assert_eq!(read.screenshot, Some(("shot_1".into(), 1280, 800)));
    // No place in the list, no step; a screenshot without a size is no screenshot.
    assert!(BrowserStep::read(&json!({"summary": "x"})).is_none());
    assert!(BrowserStep::read(&json!({"seq": 1, "screenshot": {"id": "a", "width": 0, "height": 9}})).unwrap().screenshot.is_none());
}
