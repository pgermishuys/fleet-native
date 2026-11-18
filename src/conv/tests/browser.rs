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

// ─── the agent's tabs (stores/__tests__/agent-browser.test.ts, rows 5.53 to 5.57) ───────────────────────────────────────

const TAB: &str = "tab_00000000-0000-0000-0000-000000000001";

fn tab_step(seq: i64, extra: serde_json::Value) -> BrowserStep {
    let mut v = json!({"sessionId": "s1", "seq": seq, "at": "2026-10-02T10:00:00Z", "kind": "click", "summary": format!("Step {seq}"),
        "detail": "click @e2 · 170 ms", "ok": true, "callId": "call_1", "tabId": TAB, "url": "http://127.0.0.1:5173/", "title": "Signup demo"});
    v.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap());
    BrowserStep::read(&v).unwrap()
}

#[test]
fn keeps_the_agents_tabs_from_its_steps_opened_moved_and_closed() {
    let mut steps = Steps::default();
    steps.apply("s1", tab_step(1, json!({"kind": "tabs.open"})));
    let tabs = steps.tabs("s1").unwrap();
    assert_eq!(tabs.tabs.iter().map(|t| t.url.as_str()).collect::<Vec<_>>(), ["http://127.0.0.1:5173/"]);
    assert_eq!(tabs.focused.as_deref(), Some(TAB));

    steps.apply("s1", tab_step(2, json!({"kind": "navigate", "url": "http://127.0.0.1:5173/settings", "title": "Settings"})));
    let tab = &steps.tabs("s1").unwrap().tabs[0];
    assert_eq!((tab.url.as_str(), tab.title.as_str()), ("http://127.0.0.1:5173/settings", "Settings"));

    steps.apply("s1", tab_step(3, json!({"kind": "tabs.close"})));
    assert!(steps.tabs("s1").unwrap().tabs.is_empty());
    assert_eq!(steps.tabs("s1").unwrap().focused, None);
}

#[test]
fn closing_the_focused_tab_hands_focus_to_the_last_one_left() {
    let mut steps = Steps::default();
    for (n, id) in [(1, "a"), (2, "b"), (3, "c")] {
        steps.apply("s1", tab_step(n, json!({"kind": "tabs.open", "tabId": id})));
    }
    steps.apply("s1", tab_step(4, json!({"kind": "tabs.close", "tabId": "c"})));
    assert_eq!(steps.tabs("s1").unwrap().focused.as_deref(), Some("b"));
    steps.apply("s1", tab_step(5, json!({"kind": "tabs.close", "tabId": "a"})));
    assert_eq!(steps.tabs("s1").unwrap().focused.as_deref(), Some("b"), "another tab closing leaves focus alone");
}

#[test]
fn a_failed_step_doesnt_move_the_tab() {
    let mut steps = Steps::default();
    steps.apply("s1", tab_step(1, json!({"kind": "tabs.open"})));
    steps.apply("s1", tab_step(2, json!({"kind": "navigate", "ok": false, "url": "https://example.com/", "error": "Not allowed."})));
    assert_eq!(steps.tabs("s1").unwrap().tabs[0].url, "http://127.0.0.1:5173/");
}

#[test]
fn a_step_seen_twice_changes_nothing() {
    let mut steps = Steps::default();
    steps.apply("s1", tab_step(1, json!({"kind": "tabs.open"})));
    steps.apply("s1", tab_step(1, json!({"kind": "tabs.close"})));
    assert_eq!(steps.tabs("s1").unwrap().tabs.len(), 1);
}

#[test]
fn finds_the_agents_tab_for_the_page_a_canvas_shows_whatever_loopback_name_either_uses() {
    let mut steps = Steps::default();
    steps.apply("s1", tab_step(1, json!({"kind": "tabs.open"})));
    assert_eq!(steps.tab_for("s1", Some("http://localhost:5173/")).map(|t| t.id.as_str()), Some(TAB));
    assert!(steps.tab_for("s1", Some("http://localhost:3000/")).is_none());
    assert!(steps.tab_for("s1", None).is_none() && steps.tab_for("s1", Some("")).is_none());
    assert_eq!(steps.last_step_on("s1", TAB).map(|s| s.seq), Some(1));
    assert!(steps.last_step_on("s1", "other").is_none());
}

#[test]
fn the_focused_tab_wins_and_else_the_last_one() {
    let mut steps = Steps::default();
    steps.apply("s1", tab_step(1, json!({"kind": "tabs.open", "tabId": "a"})));
    steps.apply("s1", tab_step(2, json!({"kind": "tabs.open", "tabId": "b"})));
    steps.apply("s1", tab_step(3, json!({"kind": "tabs.focus", "tabId": "a"})));
    assert_eq!(steps.tab_for("s1", Some("http://localhost:5173/")).map(|t| t.id.as_str()), Some("a"));
    steps.set_tabs("s1", steps.tabs("s1").unwrap().tabs.clone(), None);
    assert_eq!(steps.tab_for("s1", Some("http://localhost:5173/")).map(|t| t.id.as_str()), Some("b"));
}

#[test]
fn same_app_means_same_scheme_port_and_machine() {
    use crate::conv::agent_tabs::same_app;
    assert!(same_app("http://127.0.0.1:5173/a", "http://localhost:5173/b"));
    assert!(same_app("http://[::1]:5173/", "http://shop.localhost:5173/"));
    assert!(!same_app("http://localhost:5173/", "https://localhost:5173/"));
    assert!(!same_app("http://localhost:5173/", "http://localhost:5174/"));
    assert!(!same_app("not a url", "http://localhost:5173/"));
    // An empty port reads as "80": `new URL` drops a scheme's own port.
    assert!(same_app("http://localhost/", "http://localhost:80/"));
    assert!(same_app("https://example.test/", "https://example.test:443/"));
    assert!(!same_app("http://example.test/", "http://other.test/"));
    assert!(!same_app("http://localhost:99999/", "http://localhost:99999/"), "a port out of range is no url");
}

#[test]
fn reads_what_a_step_says_about_its_tab_time_and_box() {
    let s = tab_step(1, json!({"box": {"x": 464, "y": 250, "width": 361, "height": 250}}));
    assert_eq!((s.tab_id.as_deref(), s.at.as_deref()), (Some(TAB), Some("2026-10-02T10:00:00Z")));
    assert_eq!(s.bounds.map(|b| (b.x, b.width)), Some((464.0, 361.0)));
    assert!(tab_step(2, json!({"box": {"x": 1}})).bounds.is_none());
}
