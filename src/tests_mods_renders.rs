//! Component 22, piece D: `lib/mods/__tests__/renders.test.ts` (23 cases) under its names, against a fake Fleet that answers the
//! Mods switch, a session's sites, renders and actions. The cases mount a "card" with the rows the app shows
//! (`Renders::fixed_rows`); `state.sync_mod_rows()` is what the row's effect is in Fleet. A test waits for the request it
//! means, then lets everything on its way land, and counts only the requests it means.
use crate::fleet::events::{Event, Kind};
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::mods::renders::{MAX_INPUT_BYTES, MAX_OUTPUT_CHARS, RowHead, RowRead};
use crate::fleet::mods::types::{Action, ActionKind, Surface};
use crate::fleet::state::Topic;
use crate::tests::{FRAME, app};
use crate::{State, test_http, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
struct Fake {
    mods_on: bool,
    sites: (u16, Value),
    /// What renders answer: the next one each time, the last one kept.
    renders: Vec<Value>,
    action: (u16, Value),
    /// Every request: method, path, body.
    calls: Vec<(String, String, Value)>,
    /// A render waits for a permit when `gated`; `active` and `peak` count the ones waiting or running.
    gated: bool,
    permits: usize,
    active: usize,
    peak: usize,
}

type Shared = Arc<(Mutex<Fake>, Condvar)>;

const BOTH: fn() -> Value = || json!([{ "site": "ToolUse", "tools": ["bash"] }, { "site": "ToolResult", "tools": ["bash"] }]);

fn serve(fake: Shared) -> String {
    let listener = crate::test_http::bind().expect("a port");
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
    let req = test_http::read(&mut stream);
    if req.method.is_empty() {
        return;
    }
    let (lock, cond) = &*fake;
    let mut f = lock.lock().unwrap();
    let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
    f.calls.push((req.method.clone(), req.path.clone(), body));
    let (status, reply) = if req.path == "/api/features/mods" {
        (200, json!({ "on": f.mods_on, "safeMode": false }))
    } else if req.path.ends_with("/mods/sites") {
        f.sites.clone()
    } else if req.path.ends_with("/mods/action") {
        f.action.clone()
    } else if req.path.ends_with("/mods/render") {
        f.active += 1;
        f.peak = f.peak.max(f.active);
        while f.gated && f.permits == 0 {
            f = cond.wait(f).unwrap();
        }
        f.permits = f.permits.saturating_sub(1);
        f.active -= 1;
        let next = if f.renders.len() > 1 { Some(f.renders.remove(0)) } else { f.renders.first().cloned() };
        (200, next.unwrap_or_else(|| tree_answer("5 passed")))
    } else {
        (200, json!({}))
    };
    drop(f);
    let words = if status == 200 { "200 OK".to_string() } else { format!("{status} Error") };
    test_http::respond_json(&mut stream, words, &reply.to_string());
}

fn text(value: &str) -> Value {
    json!({ "type": "Text", "props": {}, "children": [value] })
}

fn tree_answer(value: &str) -> Value {
    json!({ "tree": text(value), "mods": [{ "name": "test-chips", "draft": false }] })
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Shared,
    rows: Vec<(RowHead, RowRead)>,
}

fn rig() -> Rig {
    let fake: Shared = Arc::new((
        Mutex::new(Fake {
            mods_on: true,
            sites: (200, json!({ "sites": BOTH() })),
            action: (200, json!({ "handled": true })),
            ..Default::default()
        }),
        Condvar::new(),
    ));
    let base = serve(fake.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(
        base,
        Post::new(move |job| {
            let _ = jobs.lock().unwrap().send(job);
        }),
    );
    let mut rt = app();
    rt.update(move |s, _| s.link = Some(link));
    rt.frame(FRAME);
    Rig { rt, queue, fake, rows: vec![] }
}

fn head(session: &str, call: &str, tool: &str, status: &str) -> RowHead {
    RowHead { session_id: session.into(), call_id: call.into(), raw_tool: tool.into(), status: status.into() }
}

fn read(output: &str) -> RowRead {
    RowRead { input: json!({ "command": "bun test" }), output: output.into(), title: "bun test".into(), duration_ms: None }
}

impl Rig {
    fn with(&self, change: impl FnOnce(&mut Fake)) {
        change(&mut self.fake.0.lock().unwrap());
    }

    /// Lets everything on its way land: jobs are applied and frames drawn until nothing is asked, and a frame later still not.
    fn settle(&mut self) {
        for _ in 0..3 {
            test_wait::drive(&mut self.rt, &self.queue, "every request to be answered", |s| s.mod_renders.idle(), |_| String::new());
            self.rt.frame(FRAME);
            self.rt.frame(FRAME);
        }
    }

    /// Mounts a row (a card), then shows the rows again.
    fn card(&mut self, call: &str, status: &str, output: &str) {
        self.card_with(head("s1", call, "bash", status), read(output));
    }

    fn card_with(&mut self, h: RowHead, r: RowRead) {
        self.rows.push((h, r));
        self.show();
    }

    fn show(&mut self) {
        let rows = self.rows.clone();
        self.rt.update(move |s, _| {
            s.mod_renders.fixed_rows = Some(rows);
            s.sync_mod_rows();
        });
    }

    /// A card's props changed.
    fn set(&mut self, at: usize, change: impl FnOnce(&mut RowHead, &mut RowRead)) {
        let (h, r) = &mut self.rows[at];
        change(h, r);
        self.show();
    }

    fn emit(&mut self, topic: Topic, kind: Kind, payload: Value) {
        self.rt.update(move |s, _| s.dispatch(&topic, &[Event::Other(kind, payload)]));
    }

    fn calls(&self, tail: &str) -> Vec<Value> {
        self.fake.0.lock().unwrap().calls.iter().filter(|c| c.1.ends_with(tail)).map(|c| c.2.clone()).collect()
    }
    fn sites(&self) -> usize {
        self.calls("/mods/sites").len()
    }
    fn renders(&self) -> Vec<Value> {
        self.calls("/mods/render")
    }
    fn view(&self, site: &str, call: &str) -> Option<Arc<crate::fleet::mods::points::ToolRowView>> {
        let site = crate::fleet::mods::renders::site_of(site).unwrap();
        self.rt.state.mod_renders.view(site, "s1", call)
    }
    fn authors(&self, site: &str, call: &str) -> Option<Vec<(String, bool)>> {
        self.view(site, call).map(|v| v.view.mods.iter().map(|m| (m.name.clone(), m.draft)).collect())
    }
    fn wait_renders(&mut self, n: usize) {
        let fake = self.fake.clone();
        test_wait::drive(
            &mut self.rt,
            &self.queue,
            &format!("{n} renders to reach Fleet"),
            move |_| fake.0.lock().unwrap().calls.iter().filter(|c| c.1.ends_with("/mods/render")).count() >= n,
            |_| String::new(),
        );
        self.settle();
    }
}

const CHIPS: fn() -> Vec<(String, bool)> = || vec![("test-chips".to_string(), false)];

// "asks both hooked sites with the contract's props and draws the trees"
#[test]
fn asks_both_hooked_sites_with_the_contracts_props_and_draws_the_trees() {
    let mut r = rig();
    r.card("c1", "Completed", "ok");
    r.wait_renders(2);
    let sent = r.renders();
    let mut sites: Vec<_> = sent.iter().map(|b| b["site"].as_str().unwrap().to_string()).collect();
    sites.sort();
    assert_eq!(sites, ["ToolResult", "ToolUse"]);
    for body in &sent {
        assert_eq!(body["requestId"], "c1");
        assert_eq!(
            body["props"],
            json!({ "tool": "bash", "rawTool": "bash", "category": "shell", "status": "completed", "title": "bun test",
                "input": { "command": "bun test" }, "inputTruncated": false, "output": "ok", "outputTruncated": false })
        );
    }
    assert_eq!(r.authors("ToolUse", "c1"), Some(CHIPS()));
    assert_eq!(r.view("ToolUse", "c1").unwrap().view.tree.as_ref(), &text("5 passed"));
    assert_eq!(r.view("ToolResult", "c1").unwrap().view.tree.as_ref(), &text("5 passed"));
}

// "maps Fleet's status words and sends the harness's tool name beside the canonical one"
#[test]
fn maps_fleets_status_words_and_sends_the_harnesss_tool_name_beside_the_canonical_one() {
    let mut r = rig();
    r.card_with(head("s1", "c1", "Bash", "Stopped"), read("ok"));
    r.wait_renders(2);
    let props = &r.renders()[0]["props"];
    assert_eq!(
        (props["tool"].as_str(), props["rawTool"].as_str(), props["status"].as_str()),
        (Some("bash"), Some("Bash"), Some("cancelled"))
    );
    // The two other words that map, and a word nobody knows.
    use crate::fleet::mods::renders::canonical_status as status;
    assert_eq!((status("Background"), status("Running"), status("whatever"), status("")), ("running", "running", "pending", "pending"));
}

// "never asks for a tool no mod hooks, or without a session or call"
#[test]
fn never_asks_for_a_tool_no_mod_hooks_or_without_a_session_or_call() {
    let mut r = rig();
    r.with(|f| f.sites = (200, json!({ "sites": [{ "site": "ToolUse", "tools": ["read"] }] })));
    r.card("c1", "Completed", "ok");
    r.card_with(head("", "c1", "bash", "Completed"), read("ok"));
    r.card_with(head("s1", "", "bash", "Completed"), read("ok"));
    r.settle();
    assert!(r.renders().is_empty());
}

// "asks only the site that is hooked, and any tool when tools is null"
#[test]
fn asks_only_the_site_that_is_hooked_and_any_tool_when_tools_is_null() {
    let mut r = rig();
    r.with(|f| f.sites = (200, json!({ "sites": [{ "site": "ToolResult", "tools": null }] })));
    r.card_with(head("s1", "c1", "read", "Completed"), read("ok"));
    r.wait_renders(1);
    assert_eq!(r.renders().iter().map(|b| b["site"].as_str().unwrap().to_string()).collect::<Vec<_>>(), ["ToolResult"]);
}

// "asks nothing when Mods is off (404 sites), and does not retry"
#[test]
fn asks_nothing_when_mods_is_off_404_sites_and_does_not_retry() {
    let mut r = rig();
    r.with(|f| f.sites = (404, json!({})));
    r.card("c1", "Completed", "ok");
    r.card("c2", "Completed", "ok");
    r.settle();
    assert!(r.renders().is_empty());
    assert_eq!(r.sites(), 1);
}

// "never asks for the sites while the Mods switch is off"
#[test]
fn never_asks_for_the_sites_while_the_mods_switch_is_off() {
    let mut r = rig();
    r.with(|f| f.mods_on = false);
    r.card("c1", "Completed", "ok");
    r.card("c2", "Completed", "ok");
    r.rt.update(|s, _| s.watch_mod_session("s1"));
    r.settle();
    assert_eq!(r.sites(), 0);
    assert!(r.renders().is_empty());
    // Nor when the mods change: they can't while it's off, and the answer would be the same 404.
    r.emit(Topic::Sessions, Kind::ModUi, json!({ "sessionId": null, "site": null, "requestId": null }));
    r.settle();
    assert_eq!(r.sites(), 0);
}

// "asks for the sites as soon as Mods is turned on in Settings, without a reload"
#[test]
fn asks_for_the_sites_as_soon_as_mods_is_turned_on_in_settings_without_a_reload() {
    let mut r = rig();
    r.with(|f| f.mods_on = false);
    r.card("c1", "Completed", "ok");
    r.settle();
    assert_eq!(r.sites(), 0);
    // Settings saves the choice as a preference; no event says the switch moved.
    r.rt.update(|s, _| {
        s.pickers.prefs.set("Mods", "true");
    });
    r.settle();
    assert_eq!(r.sites(), 1);
    r.wait_renders(2);
    assert_eq!(r.renders().len(), 2);
    assert_eq!(r.authors("ToolUse", "c1"), Some(CHIPS()));
}

// "still follows the switch after the page that first watched the session has gone (Settings is its own page)"
#[test]
fn still_follows_the_switch_after_the_page_that_first_watched_the_session_has_gone() {
    let mut r = rig();
    r.with(|f| f.mods_on = false);
    r.card("c1", "Completed", "ok");
    r.rows.clear();
    r.show();
    r.settle();
    r.rt.update(|s, _| {
        s.pickers.prefs.set("Mods", "true");
    });
    r.settle();
    assert_eq!(r.sites(), 1);
}

// "asks for the sites when the server says the switch turned on, and forgets the drawings when it turns off"
#[test]
fn asks_for_the_sites_when_the_server_says_the_switch_turned_on_and_forgets_the_drawings_when_it_turns_off() {
    let mut r = rig();
    r.with(|f| f.mods_on = false);
    r.card("c1", "Completed", "ok");
    r.settle();
    r.with(|f| f.mods_on = true);
    r.emit(Topic::Sessions, Kind::ModsChanged, json!({ "name": null, "reason": "switch", "sessionId": null }));
    r.settle();
    assert_eq!(r.sites(), 1);
    r.wait_renders(2);
    assert!(r.view("ToolUse", "c1").is_some());
    r.with(|f| f.mods_on = false);
    r.emit(Topic::Sessions, Kind::ModsChanged, json!({ "name": null, "reason": "switch", "sessionId": null }));
    r.settle();
    assert_eq!(r.sites(), 1);
    assert!(r.view("ToolUse", "c1").is_none());
}

// "shares one request between rows of a call"
#[test]
fn shares_one_request_between_rows_of_a_call() {
    let mut r = rig();
    r.card("c1", "Completed", "ok");
    r.card("c1", "Completed", "ok");
    r.wait_renders(2);
    assert_eq!(r.renders().len(), 2);
}

// "asks again when the status changes, not when only the output does"
#[test]
fn asks_again_when_the_status_changes_not_when_only_the_output_does() {
    let mut r = rig();
    r.card("c1", "Running", "");
    r.wait_renders(2);
    assert_eq!(r.renders().len(), 2);
    r.set(0, |_, read| read.output = "ok\nok\n".into());
    r.settle();
    assert_eq!(r.renders().len(), 2);
    r.set(0, |h, _| h.status = "Completed".into());
    r.wait_renders(4);
    assert_eq!(r.renders().len(), 4);
    assert_eq!(r.renders()[3]["props"]["status"], "completed");
    assert_eq!(r.renders()[3]["props"]["output"], "ok\nok\n");
}

// "removes a view when no mod draws any more, and keeps one a mod draws as nothing"
#[test]
fn removes_a_view_when_no_mod_draws_any_more_and_keeps_one_a_mod_draws_as_nothing() {
    let mut r = rig();
    r.card("c1", "Completed", "ok");
    r.wait_renders(2);
    assert!(r.view("ToolUse", "c1").is_some());
    r.with(|f| f.renders = vec![json!({ "tree": null, "mods": [] })]);
    r.emit(Topic::Session("s1".into()), Kind::ModUi, json!({ "sessionId": "s1", "site": null, "requestId": null, "mod": "test-chips" }));
    r.wait_renders(4);
    assert!(r.view("ToolUse", "c1").is_none());
    r.with(|f| f.renders = vec![json!({ "tree": null, "mods": [{ "name": "test-chips", "draft": false }] })]);
    r.emit(Topic::Sessions, Kind::ModUi, json!({ "sessionId": null, "site": null, "requestId": null, "mod": "test-chips" }));
    r.wait_renders(6);
    let view = r.view("ToolUse", "c1").expect("a view");
    assert!(view.view.tree.is_null());
}

// "draws again on a mod's own mod.ui with no session, without asking for the sites again"
#[test]
fn draws_again_on_a_mods_own_mod_ui_with_no_session_without_asking_for_the_sites_again() {
    let mut r = rig();
    r.card("c1", "Completed", "ok");
    r.wait_renders(2);
    let before = (r.sites(), r.renders().len());
    r.emit(Topic::Sessions, Kind::ModUi, json!({ "sessionId": null, "site": null, "requestId": null, "mod": "test-chips" }));
    r.wait_renders(before.1 + 1);
    assert!(r.renders().len() > before.1);
    assert_eq!(r.sites(), before.0);
}

// "renders again on mod.ui for the session, for one call, and leaves other calls and sessions alone"
#[test]
fn renders_again_on_mod_ui_for_the_session_for_one_call_and_leaves_other_calls_and_sessions_alone() {
    let mut r = rig();
    r.card("c1", "Completed", "ok");
    r.card("c2", "Completed", "ok");
    r.wait_renders(4);
    assert_eq!(r.renders().len(), 4);
    r.emit(Topic::Session("s1".into()), Kind::ModUi, json!({ "sessionId": "s1", "site": null, "requestId": "c2", "mod": "m" }));
    r.wait_renders(6);
    assert_eq!(r.renders().len(), 6);
    r.emit(Topic::Session("s1".into()), Kind::ModUi, json!({ "sessionId": "s1", "site": null, "requestId": null, "mod": "m" }));
    r.wait_renders(10);
    assert_eq!(r.renders().len(), 10);
}

// "fetches the sites again on mods.changed and renders again"
#[test]
fn fetches_the_sites_again_on_mods_changed_and_renders_again() {
    let mut r = rig();
    r.card("c1", "Completed", "ok");
    r.wait_renders(2);
    r.with(|f| f.sites = (200, json!({ "sites": [{ "site": "ToolUse", "tools": ["bash"] }] })));
    r.emit(Topic::Sessions, Kind::ModsChanged, json!({ "name": "test-chips", "reason": "version", "sessionId": null }));
    r.wait_renders(3);
    assert_eq!(r.sites(), 2);
    assert_eq!(r.renders().len(), 3);
    // The site that is no longer hooked loses its view.
    assert!(r.view("ToolResult", "c1").is_none());
}

// "asks for the sites again when Fleet's mods change under a row that nothing hooked yet"
#[test]
fn asks_for_the_sites_again_when_fleets_mods_change_under_a_row_that_nothing_hooked_yet() {
    let mut r = rig();
    // The host hadn't loaded the sample when the row appeared; Fleet's mod.ui with no session follows the load.
    r.with(|f| f.sites = (200, json!({ "sites": [] })));
    r.card("c1", "Completed", "ok");
    r.settle();
    assert!(r.renders().is_empty());
    r.with(|f| f.sites = (200, json!({ "sites": BOTH() })));
    r.emit(Topic::Sessions, Kind::ModUi, json!({ "sessionId": null, "site": null, "requestId": null }));
    r.wait_renders(2);
    assert_eq!(r.renders().len(), 2);
    assert_eq!(r.authors("ToolUse", "c1"), Some(CHIPS()));
}

// "sends an input over 64 KiB as {} and the last 262,144 characters of a long output"
#[test]
fn sends_an_input_over_64_kib_as_empty_and_the_last_262144_characters_of_a_long_output() {
    let mut r = rig();
    let input = json!({ "command": "x".repeat(MAX_INPUT_BYTES + 1) });
    let output = format!("{}{}", "a".repeat(10), "b".repeat(MAX_OUTPUT_CHARS));
    r.card_with(head("s1", "c1", "bash", "Completed"), RowRead { input, output, title: "bun test".into(), duration_ms: None });
    r.wait_renders(2);
    let props = &r.renders()[0]["props"];
    assert_eq!((&props["input"], &props["inputTruncated"], &props["outputTruncated"]), (&json!({}), &json!(true), &json!(true)));
    assert_eq!(props["output"].as_str().unwrap(), "b".repeat(MAX_OUTPUT_CHARS));
}

// "asks at most four at a time"
#[test]
fn asks_at_most_four_at_a_time() {
    let mut r = rig();
    r.with(|f| {
        f.sites = (200, json!({ "sites": [{ "site": "ToolUse", "tools": null }] }));
        f.renders = vec![json!({ "tree": null, "mods": [] })];
        f.gated = true;
    });
    for i in 0..10 {
        r.card(&format!("c{i}"), "Completed", "ok");
    }
    // Four reach the server and wait; the other six are queued here.
    let fake = r.fake.clone();
    test_wait::drive(&mut r.rt, &r.queue, "four renders to wait at Fleet", move |_| fake.0.lock().unwrap().active == 4, |_| String::new());
    r.rt.frame(FRAME);
    assert_eq!(r.renders().len(), 4);
    for released in 1..=10 {
        {
            let (lock, cond) = &*r.fake;
            lock.lock().unwrap().permits += 1;
            cond.notify_all();
        }
        if released < 10 {
            // The next one is asked once a place is free.
            let fake = r.fake.clone();
            let want = (4 + released).min(10);
            test_wait::drive(
                &mut r.rt,
                &r.queue,
                "the next render to be asked",
                move |_| fake.0.lock().unwrap().calls.iter().filter(|c| c.1.ends_with("/mods/render")).count() >= want,
                |_| String::new(),
            );
        }
    }
    r.settle();
    assert_eq!(r.renders().len(), 10);
    assert_eq!(r.fake.0.lock().unwrap().peak, 4);
}

// "asks for a phone row and shares the call's request with a desktop row"
#[test]
fn asks_for_a_phone_row_and_shares_the_calls_request_with_a_desktop_row() {
    // The phone is out of scope here (docs/parity/22-mods.md 14.1): a second row of the call stands for it.
    let mut r = rig();
    r.card("c1", "Completed", "ok");
    r.card("c1", "Completed", "ok");
    r.wait_renders(2);
    assert_eq!(r.renders().len(), 2);
    let props = &r.renders()[0]["props"];
    assert_eq!(
        (props["tool"].as_str(), props["status"].as_str(), &props["input"]),
        (Some("bash"), Some("completed"), &json!({ "command": "bun test" }))
    );
}

fn pressed(r: &mut Rig, handled: (u16, Value)) {
    r.with(|f| f.action = handled);
    r.card("c1", "Completed", "ok");
    r.wait_renders(2);
}

fn actions(r: &Rig) -> Vec<Value> {
    r.calls("/mods/action")
}

fn use_control(r: &mut Rig, action: Action, surface: Surface) {
    r.rt.update(move |s, _| s.mod_action("s1", "c1", action, surface));
    let fake = r.fake.clone();
    test_wait::drive(
        &mut r.rt,
        &r.queue,
        "the action to reach Fleet",
        move |_| fake.0.lock().unwrap().calls.iter().any(|c| c.1.ends_with("/mods/action")),
        |_| String::new(),
    );
    r.settle();
}

// "posts a used control to the server, with its surface"
#[test]
fn posts_a_used_control_to_the_server_with_its_surface() {
    let mut r = rig();
    pressed(&mut r, (200, json!({ "handled": true })));
    use_control(&mut r, Action { handle: "h1".into(), kind: ActionKind::Press, value: None }, Surface::Desktop);
    assert_eq!(actions(&r), [json!({ "handle": "h1", "kind": "press", "surface": "desktop" })]);
    let path = r.fake.0.lock().unwrap().calls.iter().find(|c| c.1.ends_with("/mods/action")).unwrap().1.clone();
    assert_eq!(path, "/api/sessions/s1/mods/action");
    // The mod redraws through mod.ui; the client doesn't ask on its own.
    assert_eq!(r.renders().len(), 2);
}

// "draws the call's row again when the server can't deliver the action (an expired handle)"
#[test]
fn draws_the_calls_row_again_when_the_server_cant_deliver_the_action() {
    let mut r = rig();
    pressed(&mut r, (200, json!({ "handled": false })));
    use_control(&mut r, Action { handle: "h1".into(), kind: ActionKind::Input, value: Some("x".into()) }, Surface::Phone);
    assert_eq!(actions(&r)[0], json!({ "handle": "h1", "kind": "input", "value": "x", "surface": "phone" }));
    r.wait_renders(4);
    assert_eq!(r.renders().len(), 4);
}

// "draws again when the request itself fails"
#[test]
fn draws_again_when_the_request_itself_fails() {
    let mut r = rig();
    pressed(&mut r, (500, json!({})));
    use_control(&mut r, Action { handle: "h1".into(), kind: ActionKind::Press, value: None }, Surface::Desktop);
    r.wait_renders(4);
    assert_eq!(r.renders().len(), 4);
}

// "does not ask for the sites of an empty session id"
#[test]
fn does_not_ask_for_the_sites_of_an_empty_session_id() {
    let mut r = rig();
    r.rt.update(|s, _| s.watch_mod_session(""));
    r.settle();
    // The link's own reads (the list, the plugins) are not Mods', and neither is the banner's read of the switch (piece K): only what is
    // about a session's mods is counted.
    assert!(r.fake.0.lock().unwrap().calls.iter().all(|c| !c.1.contains("/mods") || c.1 == "/api/features/mods"));
}
