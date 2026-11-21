//! Component 23, piece E: a machine's view and the peek. `MissionControlPage.test.ts`' "lists one machine's sessions with their now
//! lines, and regroups them", "says it's asking a machine, not that nothing is working or waiting on it, until it answers", "says a
//! machine that can't be reached can't, not that it has no sessions", "says so for a machine that isn't in the list" and "peeks at a
//! tile and sends a nudge to its machine, steering where the harness does" under their names, against the two fake machines of
//! piece C, then what this app adds: the nudge where the harness doesn't steer, a refused nudge, the pick going with its tile, the
//! now line following the machine on the tile.
use crate::fleet::mission::Grouping;
use crate::fleet::mission::peeking::Sent;
use crate::fleet::mission::store::NOW_EVERY;
use crate::tests::{FRAME, click, press_key};
use crate::tests_mission_live::{Node, Rig, rig, row};
use crate::{Rail, State};
use rune::{Key, Mods, Runtime};
use serde_json::{Value, json};

fn text_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).and_then(|e| e.text)
}

fn shown(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn now_line(text: &str) -> Value {
    json!({"text": text, "kind": "tool", "at": "t"})
}

/// Home holds `h1` (working, with a now line) and `h2` (waiting); the mini holds `m1` and `m2`, which work and wait.
fn machines() -> Rig {
    let r = rig();
    r.home.route("/api/sessions", 200, json!([row("h1", "active"), row("h2", "waiting_input")]));
    r.home.route("/api/sessions/now", 200, json!({"h1": now_line("bash · pnpm test webhooks")}));
    r.home.route("/api/sessions/h2/permissions", 200, json!([]));
    r.home.route("/api/sessions/h2/messages", 200, json!({"messages": []}));
    r.home.route("/api/harnesses", 200, json!([{"type": "opencode2", "capabilities": {"supportsSteering": true}}]));
    r.mini.route("/api/sessions/m2/permissions", 200, json!([]));
    r.mini.route("/api/sessions/m2/messages", 200, json!({"messages": []}));
    r
}

/// Shows home's own page and waits for the machines' first answers and home's first now line.
fn open_home(r: &mut Rig) {
    r.rt.update(|s, _| s.open_fleet_address("/fleet/machines/home"));
    r.open();
    r.wait("home's now line", |s| s.mission.machines().iter().any(|m| m.id == "home" && m.now.contains_key("h1")));
    r.rt.frame(FRAME);
}

/// Picks the tile `key`, then waits until the peek has read it.
fn pick(r: &mut Rig, key: &str) {
    click(&mut r.rt, &format!("tile-{key}"));
    r.rt.frame(FRAME);
}

fn peek_events(r: &Rig) -> Vec<String> {
    r.rt.state.mission.peek.events.iter().map(|e| e.text.clone()).collect()
}

fn work_messages() -> Value {
    json!({"messages": [
        {"id": "u1", "role": "user", "timestamp": "2026-11-21T16:39:00.000+00:00", "textContent": "fix the tests", "parts": [
            {"type": "text", "text": "fix the tests"}]},
        {"id": "a1", "role": "assistant", "timestamp": "2026-11-21T16:39:01.000+00:00", "textContent": "", "parts": [
            {"type": "tool", "toolCallId": "c1", "toolName": "Bash", "state": 2, "arguments": {"command": "pnpm test webhooks"}},
            {"type": "text", "text": "The webhook tests pass."}]}]})
}

/// What a node heard of the requests that start with `prefix`, with the `Authorization` each carried.
fn heard(node: &Node, method: &str, prefix: &str) -> Vec<(String, String)> {
    node.heard
        .lock()
        .unwrap()
        .iter()
        .filter(|h| h.method == method && h.path.starts_with(prefix))
        .map(|h| (h.path.clone(), h.body.clone()))
        .collect()
}

// ─── MissionControlPage.test.ts ─────────────────────────────────────────────────────────────────────────────────────

#[test]
fn lists_one_machine_s_sessions_with_their_now_lines_and_regroups_them() {
    let r = &mut machines();
    open_home(r);
    let tiles: Vec<String> =
        r.rt.state.mission_wall().tiles.iter().filter(|t| t.machine_id == "home").map(|t| format!("tile-machine-{}", t.key)).collect();
    assert_eq!(tiles.len(), 2, "home's two, not the mini's");
    assert!(tiles.iter().all(|k| text_of(&r.rt, k).as_deref() == Some("This machine")), "each tile names its machine");
    assert_eq!(text_of(&r.rt, "tile-now-home:h1").as_deref(), Some("bash · pnpm test webhooks"), "the working one shows its now line");
    assert!(!shown(&r.rt, "tile-m-mini:m1") && !shown(&r.rt, "group-machine"), "no other machine's tile, and no Group by machine");
    // Folder is the default: both sessions are in the one project.
    assert_eq!(text_of(&r.rt, "mission-group-label-ledger-api").as_deref(), Some("ledger-api"));
    assert_eq!(text_of(&r.rt, "mission-group-count-ledger-api").as_deref(), Some("2"));

    click(&mut r.rt, "group-state");
    r.rt.frame(FRAME);
    assert_eq!(r.rt.state.mission.grouping, Grouping::State);
    let labels = ["needs-you", "working"].map(|k| text_of(&r.rt, &format!("mission-group-label-{k}")));
    assert_eq!(labels, [Some("Needs you".into()), Some("Working".into())]);
    assert!(!shown(&r.rt, "mission-group-label-ledger-api"));
    let at = |key: &str| r.rt.find(key).expect("a heading").1;
    assert!(at("mission-group-label-needs-you") < at("mission-group-label-working"), "what needs you first");
}

#[test]
fn says_it_s_asking_a_machine_not_that_nothing_is_working_or_waiting_on_it_until_it_answers() {
    let r = &mut machines();
    r.rt.update(|s, _| s.open_fleet_address("/fleet/machines/home"));
    r.rt.frame(FRAME);
    // Both feeds are open and nothing has come back: the answers are applied only by `wait`.
    assert!(shown(&r.rt, "fleet-machine-loading") && !shown(&r.rt, "fleet-machine-empty"));
    assert_eq!(
        r.rt.elements().iter().find(|e| e.key.as_deref() == Some("fleet-machine-loading")).and_then(|e| e.label.clone()).as_deref(),
        Some("Asking This machine for its sessions…")
    );
    r.wait("both answers", |s| s.mission.machines().iter().all(|m| m.last_heard_at.is_some()));
    r.rt.frame(FRAME);
    assert!(!shown(&r.rt, "fleet-machine-loading") && !shown(&r.rt, "fleet-machine-empty"));
    assert_eq!(r.rt.state.mission_wall().tiles.iter().filter(|t| t.machine_id == "home").count(), 2);
    assert!(shown(&r.rt, "tile-home:h1") && shown(&r.rt, "tile-home:h2"));
}

#[test]
fn says_a_machine_that_can_t_be_reached_can_t_not_that_it_has_no_sessions() {
    let r = &mut machines();
    r.home.route("/api/sessions", 503, json!({"error": "down"}));
    r.rt.update(|s, _| s.open_fleet_address("/fleet/machines/home"));
    r.rt.frame(FRAME);
    r.wait("home's refusal", |s| s.mission.status("home") == Some(crate::fleet::mission::FeedStatus::Unreachable));
    r.rt.frame(FRAME);
    assert!(!shown(&r.rt, "fleet-machine-empty") && !shown(&r.rt, "fleet-machine-loading"));
    assert_eq!(text_of(&r.rt, "fleet-machine-unreachable-text").as_deref(), Some("Couldn't reach This machine."));
}

#[test]
fn says_so_for_a_machine_that_isn_t_in_the_list() {
    let r = &mut machines();
    r.rt.update(|s, _| s.open_fleet_address("/fleet/machines/gone"));
    r.open();
    r.rt.frame(FRAME);
    assert!(shown(&r.rt, "fleet-machine-gone"));
    assert!(!shown(&r.rt, "fleet-machine-list") && !shown(&r.rt, "fleet-machine-empty"));
}

#[test]
fn follows_each_machine_s_now_line_every_few_seconds_on_its_tile() {
    let r = &mut machines();
    open_home(r);
    assert_eq!(text_of(&r.rt, "tile-now-home:h1").as_deref(), Some("bash · pnpm test webhooks"));
    r.home.route("/api/sessions/now", 200, json!({"h1": now_line("edit · src/sync/rules.ts")}));
    r.later(NOW_EVERY + 0.1);
    let home = r.home.counter("GET", "/api/sessions/now");
    r.wait("the second read", move |_| home() == 2);
    r.wait("the new line", |s| s.mission.machines().iter().any(|m| m.id == "home" && m.now["h1"].text.starts_with("edit")));
    r.rt.frame(FRAME);
    assert_eq!(text_of(&r.rt, "tile-now-home:h1").as_deref(), Some("edit · src/sync/rules.ts"));
}

#[test]
fn peeks_at_a_tile_and_sends_a_nudge_to_its_machine_steering_where_the_harness_does() {
    let r = &mut machines();
    r.home.route("/api/sessions/h1/messages", 200, work_messages());
    r.home.route("/api/sessions/h1/work", 200, json!([
        {"id": "w1", "sessionId": "h1", "workId": "x", "kind": "task", "title": "explore", "label": "every use", "status": "running", "startedAt": "2026-11-21T16:39:00Z"},
        {"id": "w2", "sessionId": "h1", "workId": "y", "kind": "task", "title": "old", "status": "completed", "startedAt": "2026-11-21T16:30:00Z", "endedAt": "2026-11-21T16:31:00Z"}]));
    r.home.route("/api/sessions/h1/prompt", 200, json!({}));
    open_home(r);
    assert!(!shown(&r.rt, "mission-peek"), "nothing is picked yet");

    pick(r, "home:h1");
    let (messages, work) = (r.home.counter("GET", "/api/sessions/h1/messages"), r.home.counter("GET", "/api/sessions/h1/work"));
    r.wait("the peek's two reads", move |_| messages() == 1 && work() == 1);
    r.wait("the peek's answers", |s| !s.mission.peek.events.is_empty() && !s.mission.peek.helpers.is_empty());
    r.rt.frame(FRAME);
    assert_eq!(text_of(&r.rt, "peek-title").as_deref(), Some("h1"));
    assert_eq!(text_of(&r.rt, "peek-state").as_deref(), Some("Working · This machine"));
    assert_eq!(peek_events(r), ["bash · pnpm test webhooks", "The webhook tests pass."], "the user's words are left out");
    assert_eq!(r.rt.state.mission.peek.helpers.iter().map(|h| h.title.as_str()).collect::<Vec<_>>(), ["explore"], "only what runs");
    assert!(shown(&r.rt, "peek-helper-w1") && !shown(&r.rt, "peek-helper-w2"));
    assert_eq!(r.mini.count("GET", "/api/sessions/h1"), 0, "the other machine is not asked about it");

    // Enter sends the trimmed words: the harness steers, so it goes into the turn.
    click(&mut r.rt, "peek-nudge");
    press_key(&mut r.rt, Key::Text("  also cover the timeout path  ".into()), Mods::default());
    press_key(&mut r.rt, Key::Enter, Mods::default());
    let prompt = r.home.counter("POST", "/api/sessions/h1/prompt");
    r.wait("the nudge", move |_| prompt() == 1);
    r.wait("its outcome", |s| s.mission.peek.sent.is_some());
    r.rt.frame(FRAME);
    let said = heard(&r.home, "POST", "/api/sessions/h1/prompt");
    let body: Value = serde_json::from_str(&said[0].1).unwrap();
    assert_eq!((body["text"].as_str(), body["delivery"].as_str()), (Some("also cover the timeout path"), Some("steer")));
    assert!(body["userMessageId"].as_str().is_some_and(|id| !id.is_empty()));
    assert_eq!(text_of(&r.rt, "peek-sent").as_deref(), Some("Sent."));
    assert_eq!(r.rt.state.mission.peek.nudge.text, "", "the field cleared");
    assert_eq!(r.mini.count("POST", "/api/sessions"), 0, "nothing went to the other machine");
    assert_eq!(heard(&r.home, "GET", "/api/harnesses").len(), 1, "it asked the machine's harnesses first");
}

// ─── what this app adds ─────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn queues_a_nudge_for_a_harness_that_does_not_steer_and_says_so() {
    let r = &mut machines();
    r.home.route("/api/harnesses", 200, json!([{"type": "opencode2", "capabilities": {"supportsSteering": false}}]));
    r.home.route("/api/sessions/h1/queue", 200, json!({}));
    open_home(r);
    pick(r, "home:h1");
    click(&mut r.rt, "peek-nudge");
    press_key(&mut r.rt, Key::Text("when you can".into()), Mods::default());
    press_key(&mut r.rt, Key::Enter, Mods::default());
    let queue = r.home.counter("POST", "/api/sessions/h1/queue");
    r.wait("the queued nudge", move |_| queue() == 1);
    r.wait("its outcome", |s| s.mission.peek.sent.is_some());
    r.rt.frame(FRAME);
    assert_eq!(r.rt.state.mission.peek.sent, Some(Sent::Queued));
    assert_eq!(text_of(&r.rt, "peek-sent").as_deref(), Some("Queued: it goes when the turn ends."));
    assert_eq!(
        serde_json::from_str::<Value>(&heard(&r.home, "POST", "/api/sessions/h1/queue")[0].1).unwrap(),
        json!({"text": "when you can"})
    );
    assert_eq!(r.home.count("POST", "/api/sessions/h1/prompt"), 0, "it was not a prompt");
}

#[test]
fn a_session_that_is_not_working_just_gets_the_prompt() {
    let r = &mut machines();
    r.home.route("/api/sessions/h2/prompt", 200, json!({}));
    open_home(r);
    pick(r, "home:h2");
    click(&mut r.rt, "peek-nudge");
    press_key(&mut r.rt, Key::Text("hello".into()), Mods::default());
    press_key(&mut r.rt, Key::Enter, Mods::default());
    let prompt = r.home.counter("POST", "/api/sessions/h2/prompt");
    r.wait("the prompt", move |_| prompt() == 1);
    r.wait("its outcome", |s| s.mission.peek.sent.is_some());
    let body: Value = serde_json::from_str(&heard(&r.home, "POST", "/api/sessions/h2/prompt")[0].1).unwrap();
    assert!(body.get("delivery").is_none() && body["userMessageId"].is_string(), "{body}");
    assert_eq!(heard(&r.home, "GET", "/api/harnesses").len(), 0, "a session that isn't working needs no answer about steering");
    assert_eq!(r.rt.state.mission.peek.sent, Some(Sent::Sent));
}

#[test]
fn says_why_a_nudge_did_not_go_through_and_keeps_the_words() {
    let r = &mut machines();
    r.home.route("/api/sessions/h1/prompt", 500, json!({"error": "The harness is busy."}));
    open_home(r);
    pick(r, "home:h1");
    click(&mut r.rt, "peek-nudge");
    press_key(&mut r.rt, Key::Text("try again".into()), Mods::default());
    press_key(&mut r.rt, Key::Enter, Mods::default());
    r.wait("the refusal", |s| s.mission.peek.error.is_some());
    r.rt.frame(FRAME);
    assert_eq!(text_of(&r.rt, "peek-error-text").as_deref(), Some("The harness is busy."));
    assert_eq!(r.rt.state.mission.peek.nudge.text, "try again", "the words stay to send again");
    assert!(!r.rt.state.mission.peek.sending && r.rt.state.mission.peek.sent.is_none());
}

#[test]
fn sends_nothing_for_blank_words_and_one_nudge_at_a_time() {
    let r = &mut machines();
    r.home.route("/api/sessions/h1/prompt", 200, json!({}));
    open_home(r);
    pick(r, "home:h1");
    click(&mut r.rt, "peek-nudge");
    press_key(&mut r.rt, Key::Text("   ".into()), Mods::default());
    press_key(&mut r.rt, Key::Enter, Mods::default());
    assert!(!r.rt.state.mission.peek.sending, "blank words are not sent");
    press_key(&mut r.rt, Key::Text("one".into()), Mods::default());
    press_key(&mut r.rt, Key::Enter, Mods::default());
    r.rt.update(|s, _| s.peek_nudge());
    let prompt = r.home.counter("POST", "/api/sessions/h1/prompt");
    r.wait("the nudge", move |_| prompt() >= 1);
    r.wait("its outcome", |s| s.mission.peek.sent.is_some());
    assert_eq!(r.home.count("POST", "/api/sessions/h1/prompt"), 1, "a second Enter while one is on its way sends nothing");
}

#[test]
fn picking_another_tile_clears_the_field_and_the_line_and_the_pick_goes_with_its_tile() {
    let r = &mut machines();
    open_home(r);
    pick(r, "home:h1");
    // A nudge that went through: its line is up. (The send itself is tested above; here no read of the machine is left in flight.)
    r.rt.update(|s, _| s.mission.peek.sent = Some(Sent::Sent));
    press_key(&mut r.rt, Key::Text("half typed".into()), Mods::default());
    pick(r, "home:h2");
    r.rt.frame(FRAME);
    let peek = &r.rt.state.mission.peek;
    assert_eq!((peek.nudge.text.as_str(), peek.sent, peek.error.as_deref()), ("", None, None));
    assert_eq!(text_of(&r.rt, "peek-title").as_deref(), Some("h2"));

    // The tile goes (the machine no longer lists it): the pick goes and the peek closes.
    r.home.route("/api/sessions", 200, json!([row("h1", "active")]));
    r.rt.update(|s, _| s.mission_sessions_event("home"));
    // The event is noted on one frame and its fold runs from there, so the clock moves on in two.
    r.later(0.1);
    r.later(crate::fleet::mission::feed::FOLD + 0.1);
    r.wait("home's new list", |s| s.mission.machines().iter().any(|m| m.id == "home" && m.sessions.len() == 1));
    r.rt.frame(FRAME);
    assert!(!shown(&r.rt, "mission-peek") && r.rt.state.mission.picked.is_none());
}

#[test]
fn reads_the_peek_again_when_the_tile_s_now_line_changes_and_drops_an_answer_for_a_tile_no_longer_picked() {
    let r = &mut machines();
    r.home.route("/api/sessions/h1/messages", 200, work_messages());
    open_home(r);
    pick(r, "home:h1");
    let messages = r.home.counter("GET", "/api/sessions/h1/messages");
    r.wait("the first peek read", {
        let messages = r.home.counter("GET", "/api/sessions/h1/messages");
        move |_| messages() == 1
    });
    r.home.route("/api/sessions/now", 200, json!({"h1": now_line("edit · src/sync/rules.ts")}));
    r.later(NOW_EVERY + 0.1);
    r.wait("the peek read again", move |_| messages() == 2);
    r.wait("its events", |s| !s.mission.peek.events.is_empty());
    // Another tile is picked before the next answer: the peek is the new tile's, empty until its own read comes.
    pick(r, "home:h2");
    assert_eq!(r.rt.state.mission.peek.key.as_deref(), Some("home:h2"));
    assert!(r.rt.state.mission.peek.events.is_empty(), "{:?}", peek_events(r));
}

#[test]
fn a_machine_view_asks_nothing_of_another_machine_for_its_peek() {
    let r = &mut machines();
    r.mini.route("/api/sessions/m2/prompt", 200, json!({}));
    r.rt.update(|s, _| s.open_fleet_address("/fleet/machines/m-mini"));
    r.open();
    r.wait("the mini's now line", |s| s.mission.machines().iter().any(|m| m.id == "m-mini"));
    r.rt.frame(FRAME);
    pick(r, "m-mini:m2");
    let messages = r.mini.counter("GET", "/api/sessions/m2/messages");
    r.wait("the mini's peek read", move |_| messages() >= 2);
    r.rt.frame(FRAME);
    assert_eq!(text_of(&r.rt, "peek-state").as_deref(), Some("Needs you · Mini"));
    assert_eq!(r.home.count("GET", "/api/sessions/m2"), 0, "home is not asked about the mini's session");
    // Every request the mini heard carried its own key, and none of home's went to it.
    assert!(r.mini.everything().iter().all(|h| h.auth.as_deref() == Some(&format!("Bearer {}", crate::tests_mission_live::TOKEN))));
    assert!(r.rt.state.rail == Rail::Fleet);
}
