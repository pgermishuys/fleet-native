//! Component 23, piece D: the fleet-menu and Needs you. `FleetNavPanel.test.ts` (3) and `MissionControlPage.test.ts` ("shows the
//! asks of every machine on Needs you, and only those", "says it's asking, not that nothing is waiting on you, until the machines
//! answer", "answers a question and a permission in place, each on its own machine") under their names, against the two fake
//! machines of piece C (home, and a machine with a token of its own), then what this app adds: a refused answer says why, a card
//! opens its session, the menu marks the open view.
use crate::fleet::labs::FleetView;
use crate::tests::{FRAME, click};
use crate::tests_mission_live::{Rig, rig, row};
use crate::{Rail, State};
use rune::Runtime;
use serde_json::json;

fn text_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).and_then(|e| e.text)
}

fn shown(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn y_of(rt: &Runtime<State>, key: &str) -> f32 {
    rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}")).1
}

/// Home's session `h1` waits on a one-pick question, the mini's `m2` on a permission; `m1` and `h2` work.
fn asking() -> Rig {
    let r = rig();
    r.home.route("/api/sessions", 200, json!([row("h1", "waiting_input"), row("h2", "active")]));
    r.home.route("/api/sessions/h1/permissions", 200, json!([]));
    r.home.route(
        "/api/sessions/h1/messages",
        200,
        json!({"messages": [{"id": "x1", "role": "assistant", "timestamp": "2026-11-21T16:39:01.184+00:00", "textContent": "", "parts": [
            {"type": "tool", "toolCallId": "call-q", "toolName": "question", "state": 1,
             "arguments": {"questions": [{"header": "Retries", "question": "Three or five?",
                 "options": [{"label": "Three", "description": "Fail fast."}, {"label": "Five", "description": ""}]}]}}]}]}),
    );
    r.mini.route(
        "/api/sessions/m2/permissions",
        200,
        json!([{"id": "perm1", "sessionId": "m2", "tool": "bash", "kind": "shell", "title": "pnpm install"}]),
    );
    r
}

/// The cards drawn, by tile key: every tile the machines know, kept when its card is on the page.
fn cards(r: &Rig) -> Vec<String> {
    let mut keys: Vec<String> =
        r.rt.state.mission_wall().tiles.iter().map(|t| format!("ask-{}", t.key)).filter(|key| shown(&r.rt, key)).collect();
    keys.sort();
    keys
}

// ─── FleetNavPanel.test.ts ──────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn lists_needs_you_first_then_one_entry_per_machine_with_counts() {
    let r = &mut asking();
    r.open();
    r.rt.frame(FRAME);
    let rt = &r.rt;
    let order = ["fleet-view-needs-you", "fleet-view-machine-home", "fleet-view-machine-m-mini"];
    assert!(order.windows(2).all(|w| y_of(rt, w[0]) < y_of(rt, w[1])), "Needs you first, then home, then the other machine");
    // Two wait (home's question, the mini's permission); each machine holds its own working and waiting sessions.
    assert_eq!(text_of(rt, "fleet-view-needs-you-count").as_deref(), Some("2"));
    assert_eq!(text_of(rt, "fleet-view-machine-home-count").as_deref(), Some("2"));
    assert_eq!(text_of(rt, "fleet-view-machine-m-mini-count").as_deref(), Some("2"));
    assert!(!shown(rt, "fleet-view-machine-m-mini-offline"), "a machine that answers isn't offline");
}

#[test]
fn marks_needs_you_on_fleet_and_a_machine_on_its_own_route() {
    let r = &mut asking();
    r.open();
    let marked = |r: &Rig, key: &str| r.rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && e.fill.a > 0.5);
    r.rt.update(|s, _| s.open_fleet_address("/fleet"));
    r.rt.frame(FRAME);
    assert!(marked(r, "fleet-view-needs-you") && !marked(r, "fleet-view-machine-m-mini"));
    r.rt.update(|s, _| s.open_fleet_address("/fleet/machines/m-mini"));
    r.rt.frame(FRAME);
    assert!(marked(r, "fleet-view-machine-m-mini") && !marked(r, "fleet-view-needs-you") && !marked(r, "fleet-view-machine-home"));
}

#[test]
fn opens_a_view_by_its_own_route_and_does_nothing_for_the_one_already_open() {
    let r = &mut asking();
    r.open();
    click(&mut r.rt, "fleet-view-machine-m-mini");
    assert_eq!(r.rt.state.labs.fleet_view, FleetView::Machine("m-mini".into()));
    r.rt.update(|s, _| s.labs.fleet_view = FleetView::Machine("m-mini".into()));
    click(&mut r.rt, "fleet-view-machine-m-mini");
    assert_eq!(r.rt.state.labs.fleet_view, FleetView::Machine("m-mini".into()), "pressing the open one changes nothing");
    click(&mut r.rt, "fleet-view-needs-you");
    assert_eq!(r.rt.state.labs.fleet_view, FleetView::NeedsYou);
}

// ─── MissionControlPage.test.ts ─────────────────────────────────────────────────────────────────────────────────────

#[test]
fn shows_the_asks_of_every_machine_on_needs_you_and_only_those() {
    let r = &mut asking();
    r.open();
    r.rt.frame(FRAME);
    assert_eq!(cards(r), ["ask-home:h1", "ask-m-mini:m2"], "the two that wait, not the two that work");
    assert_eq!(text_of(&r.rt, "ask-question-home:h1").as_deref(), Some("Three or five?"));
    assert_eq!(text_of(&r.rt, "ask-code-m-mini:m2").as_deref(), Some("pnpm install"));
    assert_eq!(text_of(&r.rt, "ask-machine-m-mini:m2").as_deref(), Some("Mini"));
    assert_eq!(text_of(&r.rt, "fleet-header-description").as_deref(), Some("2 running · 0 helpers · 2 need you"));
    assert_eq!(text_of(&r.rt, "needs-you-count").as_deref(), Some("2"));
}

#[test]
fn says_it_s_asking_not_that_nothing_is_waiting_on_you_until_the_machines_answer() {
    let r = &mut rig();
    // Nothing waits on either machine.
    r.mini.route("/api/sessions", 200, json!([row("m1", "active")]));
    r.rt.update(|s, _| s.rail = Rail::Fleet);
    r.rt.frame(FRAME);
    // Both feeds are open and nothing has come back: the answers are applied only by `wait`.
    assert!(shown(&r.rt, "mission-needs-you-loading") && !shown(&r.rt, "mission-needs-you-empty"));
    r.wait("both answers", |s| s.mission.machines().iter().all(|m| m.last_heard_at.is_some()));
    r.rt.frame(FRAME);
    assert!(!shown(&r.rt, "mission-needs-you-loading") && shown(&r.rt, "mission-needs-you-empty"), "now it can say nothing waits");
}

#[test]
fn answers_a_question_and_a_permission_in_place_each_on_its_own_machine() {
    let r = &mut asking();
    r.home.route("/api/sessions/h1/questions/call-q/answer", 200, json!({}));
    r.mini.route("/api/sessions/m2/permissions/perm1", 200, json!({}));
    r.open();
    r.rt.frame(FRAME);

    // Once answered, the machine says the ask is gone: its next list leaves the session out of the waiting.
    r.home.route("/api/sessions", 200, json!([row("h2", "active")]));
    click(&mut r.rt, "ask-option-home:h1-0");
    let home = r.home.counter("POST", "/api/sessions/h1/questions/call-q/answer");
    r.wait("the answer at home", move |_| home() == 1);
    let said = r
        .home
        .heard
        .lock()
        .unwrap()
        .iter()
        .find(|h| h.method == "POST" && h.path.contains("/questions/"))
        .map(|h| h.body.clone())
        .expect("the answer was sent");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&said).unwrap(), json!({"answers": [["Three"]]}));
    assert_eq!(r.mini.count("POST", "/api/sessions"), 0, "the other machine was not asked");
    // Home is read again at once and its card leaves; the mini's, which still waits, stays.
    r.wait("home's card leaves", |s| !s.mission.machines().iter().any(|m| m.id == "home" && m.asks.contains_key("h1")));
    r.rt.frame(FRAME);
    assert_eq!(cards(r), ["ask-m-mini:m2"]);

    r.mini.route("/api/sessions", 200, json!([row("m1", "active")]));
    click(&mut r.rt, "ask-once-m-mini:m2");
    let mini = r.mini.counter("POST", "/api/sessions/m2/permissions/perm1");
    r.wait("the answer at the mini", move |_| mini() == 1);
    let said = r
        .mini
        .heard
        .lock()
        .unwrap()
        .iter()
        .find(|h| h.method == "POST" && h.path.contains("/permissions/"))
        .map(|h| h.body.clone())
        .expect("the answer was sent");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&said).unwrap(), json!({"reply": "once"}));
    assert_eq!(r.home.count("POST", "/api/sessions/m2"), 0, "nor was home");
    r.wait("the mini's card leaves", |s| s.mission.machines().iter().all(|m| m.asks.is_empty()));
    r.rt.frame(FRAME);
    assert!(cards(r).is_empty(), "{:?}", cards(r));
}

#[test]
fn says_what_a_refused_answer_said_under_its_card_and_clears_it_when_a_later_one_goes_through() {
    let r = &mut asking();
    r.mini.route("/api/sessions/m2/permissions/perm1", 500, json!({"error": "The harness is busy."}));
    r.open();
    r.rt.frame(FRAME);
    click(&mut r.rt, "ask-always-m-mini:m2");
    r.wait("the refusal", |s| s.mission.errors.get("m-mini:m2").is_some_and(|e| !e.is_empty()));
    r.rt.frame(FRAME);
    assert_eq!(text_of(&r.rt, "ask-error-m-mini:m2-text").as_deref(), Some("The harness is busy."));
    assert!(!r.rt.state.mission.busy.contains("m-mini:m2"), "its buttons are back");

    r.mini.route("/api/sessions/m2/permissions/perm1", 200, json!({}));
    click(&mut r.rt, "ask-once-m-mini:m2");
    r.wait("the good answer", |s| s.mission.errors.get("m-mini:m2").is_some_and(String::is_empty));
    r.rt.frame(FRAME);
    assert!(!shown(&r.rt, "ask-error-m-mini:m2"));
}

#[test]
fn says_a_machine_that_left_the_list_is_not_in_it_and_sends_nothing() {
    let r = &mut asking();
    r.open();
    r.rt.frame(FRAME);
    r.rt.update(|s, _| s.machines.apply_list(crate::fleet::machines::parse_machine_list(200, r#"{"machines": []}"#)));
    let tile = r.rt.state.mission_wall().waiting.into_iter().find(|t| t.machine_id == "m-mini").expect("the mini's card");
    r.rt.update(move |s, _| s.mission_answer_permission(&tile, crate::fleet::asks::Reply::Once));
    r.wait("the words", |s| s.mission.errors.get("m-mini:m2").is_some_and(|e| !e.is_empty()));
    assert_eq!(r.rt.state.mission.errors["m-mini:m2"], "That machine isn't in the list any more.");
    assert_eq!(r.mini.count("POST", "/api/sessions"), 0);
}

#[test]
fn opens_the_session_of_a_card_that_has_no_ask_to_answer_in_place() {
    let r = &mut asking();
    // The mini's permissions list is empty: its waiting session has nothing to answer here, so its card says Open.
    r.mini.route("/api/sessions/m2/permissions", 200, json!([]));
    r.mini.route("/api/sessions/m2/messages", 200, json!({"messages": []}));
    r.open();
    r.rt.frame(FRAME);
    assert!(shown(&r.rt, "ask-open-m-mini:m2") && shown(&r.rt, "ask-now-m-mini:m2"));
    click(&mut r.rt, "ask-open-m-mini:m2");
    assert_eq!(r.rt.state.rail, Rail::Sessions);
    assert_eq!(r.rt.state.machine_key_of("m2"), "m-mini", "its machine remembers it");
}

/// Fleet's DOM, measured in a scratch Fleet at 1280 x 760: a question card is 121.5 px tall and a permission card 125 (1 px
/// borders, 10.5 padding, 3.5 and 7 px between the lines, a 28 px button), 10.5 px apart. An earlier build drew them 1.5 to 2 px
/// off, each a little shorter or taller, so every card after the first drifted.
#[test]
fn a_card_is_as_tall_as_fleets_and_they_sit_10_and_a_half_apart() {
    let r = &mut asking();
    r.open();
    r.rt.frame(FRAME);
    let height = |key: &str| r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).map(|e| e.rect).expect(key);
    let question = height("ask-home:h1");
    let permission = height("ask-m-mini:m2");
    assert!((question[3] - 121.5).abs() < 0.6, "the question card is {} px", question[3]);
    assert!((permission[3] - 125.0).abs() < 0.6, "the permission card is {} px", permission[3]);
    let (upper, lower) = if question[1] < permission[1] { (question, permission) } else { (permission, question) };
    assert!((lower[1] - (upper[1] + upper[3]) - 10.5).abs() < 0.6, "the cards are {} px apart", lower[1] - (upper[1] + upper[3]));
}
