//! Component 16, piece W: Settings → Updates & about › Machines. `UpdatesMachines.test.ts` (10) under its names, as native cases,
//! then the clicks the plan adds: a rule choice, a machine's own rule, Restart, Update machines now and Stop. The machines and
//! their update blocks are invented and put into the state; where a click should reach home, a fake Fleet answers. Nothing
//! here updates a real machine.
use crate::fleet::machines::Info;
use crate::fleet::updates::Rollout;
use crate::tests::{app_tall, click};
use crate::tests_updates::rig;
use crate::{Rail, State};
use rune::Runtime;
use serde_json::{Value, json};

/// Two hours before the clock the page is drawn at (`mock::rows::NOW`).
fn hours_ago() -> String {
    crate::fleet::session_commands::iso_utc(crate::mock::rows::NOW - 2 * 3_600_000)
}

fn downloaded() -> Value {
    json!({ "status": "staged", "latestVersion": "0.59.1", "stagedVersion": "0.59.1", "checkedAt": hours_ago(), "canRestart": true, "rule": "idle" })
}

fn current() -> Value {
    json!({ "status": "uptodate", "latestVersion": "0.59.1", "stagedVersion": null, "checkedAt": hours_ago(), "canRestart": true, "rule": "idle" })
}

fn info(id: &str, name: &str, os: &str, update: Value, working: u32, needs_you: u32) -> Info {
    Info::from_json(&json!({
        "id": id, "name": name, "hostName": name, "os": os, "version": "0.59.0", "apiVersion": 1, "authMode": "token",
        "remoteReachable": true, "requiresToken": false, "webApp": false,
        "capabilities": { "permissionLevel": "all", "harnesses": [], "sessions": { "working": working, "needsYou": needs_you } },
        "update": update,
    }))
    .expect("an object")
}

fn rolling(steps: Value) -> Rollout {
    Rollout::from_json(&json!({ "id": "r1", "status": "running", "startedAt": "2026-11-17T09:00:00Z", "steps": steps })).expect("a rollout")
}

/// Home ("atlas", one working and two waiting) and "mini", with the given update blocks, on the Machines group.
fn put_machines(s: &mut State, home: Value, mini: Value) {
    s.now_ms = Some(crate::mock::rows::NOW);
    s.rail = Rail::Settings;
    s.machines.home = Some(info("home-id", "atlas", "linux", home, 1, 2));
    let mut mini_machine = crate::mock::machines::machine("m-mini", "mini");
    mini_machine.os = Some("macos".into());
    s.machines.list = vec![mini_machine];
    s.machines.others.insert("m-mini".into(), Default::default());
    s.machines.identities.insert("m-mini".into(), info("m-mini", "mini", "macos", mini, 0, 0));
    s.pickers.prefs.fetched = true;
    s.updates.last_seen_loaded_for_scene();
    s.open_settings("updates", None);
}

fn page(home: Value, mini: Value) -> Runtime<State> {
    let mut rt = app_tall();
    rt.update(move |s, _| put_machines(s, home, mini));
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    rt
}

/// What the element keyed `key` shows: its own text, else the text of everything inside it, in order.
fn text(rt: &Runtime<State>, key: &str) -> String {
    let all = rt.elements();
    let at = all.iter().position(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    if let Some(own) = all[at].text.clone().filter(|t| !t.is_empty()) {
        return own;
    }
    let inside = |mut i: usize| {
        while let Some(p) = all[i].parent {
            if p == at {
                return true;
            }
            i = p;
        }
        false
    };
    (0..all.len()).filter(|&i| inside(i)).filter_map(|i| all[i].text.clone()).collect::<Vec<_>>().join(" ")
}

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn top(rt: &Runtime<State>, key: &str) -> f32 {
    rt.elements().iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}")).rect[1]
}

// ─── UpdatesMachines.test.ts ─────────────────────────────────────────────────────

#[test]
fn has_the_machines_group_between_the_harnesses_and_whats_new_with_no_preference_set() {
    let rt = page(downloaded(), current());
    assert!(has(&rt, "settings-group-machines"));
    assert!(top(&rt, "settings-group-machines") > top(&rt, "settings-group-harnesses"), "after the harnesses");
    assert!(rt.state.pickers.prefs.get("FleetUpdateRule").is_none(), "no preference is set");
    assert!(top(&rt, "settings-group-whats-new") > top(&rt, "settings-group-machines"), "What's new comes last");
}

#[test]
fn shows_a_version_line_on_each_card_and_restart_only_on_the_machine_with_an_update_downloaded() {
    let rt = page(downloaded(), current());
    assert_eq!(text(&rt, "machine-version-text-home"), "Fleet 0.59.0 \u{2192} 0.59.1");
    assert_eq!(text(&rt, "machine-version-state-home"), "0.59.1 downloaded \u{b7} updates itself when its 3 sessions finish");
    assert_eq!(text(&rt, "update-restart-home"), "Restart into 0.59.1");
    assert!(
        rt.elements().iter().find(|e| e.key.as_deref() == Some("update-restart-home")).is_some_and(|e| e.opacity < 1.0),
        "disabled while its sessions work"
    );
    assert!(has(&rt, "update-rule-home"), "the rule menu");
    assert_eq!(text(&rt, "machine-version-text-m-mini"), "Fleet 0.59.0");
    assert_eq!(text(&rt, "machine-version-state-m-mini"), "up to date");
    assert!(!has(&rt, "update-restart-m-mini"));
    assert_eq!(rt.elements().iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("machine-version-text-"))).count(), 2);
}

#[test]
fn says_a_machine_cant_be_reached_with_the_version_last_seen_and_offers_it_no_rule_menu() {
    let mut rt = page(downloaded(), current());
    rt.update(|s, _| {
        s.machines.others.get_mut("m-mini").unwrap().error = Some("Can't reach mini.".into());
    });
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    assert_eq!(text(&rt, "machine-version-state-m-mini"), "can't be reached \u{b7} last seen on 0.59.0");
    assert!(!has(&rt, "update-rule-m-mini"));
}

#[test]
fn remembers_each_machines_version_on_this_device() {
    let mut rt = page(downloaded(), current());
    rt.update(|s, _| s.updates.last_seen = Default::default());
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    let seen = &rt.state.updates.last_seen;
    assert_eq!((seen.get("home"), seen.get("m-mini")), (Some("0.59.0"), Some("0.59.0")));
    let kept: Value = serde_json::from_str(&rt.state.sidebar.to_json()).unwrap();
    assert_eq!(kept["machineVersions"], json!({ "home": "0.59.0", "m-mini": "0.59.0" }), "written with the rest of the settings file");
}

#[test]
fn says_what_a_machine_was_last_seen_on_when_it_hasnt_answered_since_the_page_opened() {
    let mut rt = app_tall();
    rt.update(|s, _| {
        put_machines(s, downloaded(), current());
        s.machines.identities.remove("m-mini");
        s.machines.others.get_mut("m-mini").unwrap().error = Some("Can't reach mini.".into());
        s.updates.last_seen.0.insert("m-mini".into(), "0.58.2".into());
    });
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    assert_eq!(text(&rt, "machine-version-text-m-mini"), "Fleet 0.58.2");
    assert_eq!(text(&rt, "machine-version-state-m-mini"), "can't be reached \u{b7} last seen on 0.58.2");
}

#[test]
fn shows_the_release_line_with_update_machines_now_when_something_can_update() {
    let rt = page(current(), downloaded());
    let line = text(&rt, "updates-headline");
    assert!(line.contains("Fleet 0.59.1 is out."), "{line}");
    assert!(line.contains("1 machine has it downloaded and switches when it restarts."), "{line}");
    assert!(has(&rt, "update-default-rule"));
    assert_eq!(text(&rt, "updates-update-machines"), "Update machines now (1)");
}

#[test]
fn says_every_machine_is_up_to_date_in_one_line_and_when_it_last_checked() {
    let rt = page(current(), current());
    let line = text(&rt, "updates-headline");
    assert!(line.contains("Every machine that can be reached is on Fleet 0.59.1."), "{line}");
    assert!(line.contains("Checked 2 hours ago."), "{line}");
    assert!(!has(&rt, "updates-update-machines"));
    assert!(has(&rt, "updates-check-now"));
}

fn waiting_rollout() -> Rollout {
    rolling(json!([
        { "machineId": "m-mini", "name": "mini", "state": "waiting", "waitingSessions": 2, "toVersion": "0.59.1" },
        { "machineId": "home", "name": "atlas", "state": "pending", "waitingSessions": 0, "toVersion": "0.59.1" },
    ]))
}

#[test]
fn tells_where_a_rollout_is_with_a_stop_after_the_machine_being_handled() {
    let mut rt = page(downloaded(), downloaded());
    rt.update(|s, _| s.updates.rollout = Some(waiting_rollout()));
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    let narrative = text(&rt, "updates-narrative");
    assert!(narrative.contains("Updating machines to 0.59.1, one at a time."), "{narrative}");
    assert!(narrative.contains("mini is waiting for 2 sessions; atlas is next."), "{narrative}");
    assert_eq!(text(&rt, "updates-stop-rollout"), "Stop after mini");
    assert_eq!(text(&rt, "machine-version-state-m-mini"), "Updating: waiting for 2 sessions to finish");
    assert_eq!(text(&rt, "machine-version-state-home"), "Updating: next, after mini");
    assert!(!has(&rt, "updates-headline") && !has(&rt, "update-default-rule"), "the release line gives way to the narrative");
}

#[test]
fn says_a_machine_in_the_rollouts_queue_that_cant_be_reached_will_be_skipped_not_that_its_next() {
    let mut rt = page(downloaded(), downloaded());
    rt.update(|s, _| {
        s.machines.others.get_mut("m-mini").unwrap().error = Some("Can't reach mini.".into());
        s.updates.rollout = Some(rolling(
            json!([{ "machineId": "m-mini", "name": "mini", "state": "pending", "waitingSessions": 0, "toVersion": "0.59.1" }]),
        ));
    });
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    assert!(text(&rt, "updates-narrative").contains("mini can't be reached, so it'll be skipped."));
    assert!(text(&rt, "machine-version-state-m-mini").starts_with("can't be reached"));
}

#[test]
fn says_no_release_was_found_when_no_machine_knows_of_one() {
    let none = json!({ "status": "unknown", "latestVersion": null, "stagedVersion": null, "checkedAt": hours_ago(), "canRestart": true, "rule": "idle" });
    let rt = page(none.clone(), none);
    assert!(text(&rt, "updates-headline").contains("Fleet hasn't found a release yet."));
    assert!(!has(&rt, "update-default-rule") && !has(&rt, "updates-rule-hint"), "no rule to choose until a release is out");
}

/// Puts the machines in and draws. The fake Fleet answers a machine list with none, so a later read would take them away:
/// each test presses what it needs before it waits for anything.
fn seed(r: &mut crate::tests_updates::Rig, home: Value, mini: Value) {
    r.rt.update(move |s, _| put_machines(s, home, mini));
    r.rt.frame(1.0 / 60.0);
    r.rt.frame(1.0 / 60.0);
}

// ─── what the plan adds ──────────────────────────────────────────────────────────

#[test]
fn a_rule_choice_writes_fleet_update_rule_and_a_machines_override_writes_the_overrides_and_default_removes_it() {
    let mut r = rig("0.59.0", None, false);
    seed(&mut r, downloaded(), downloaded());
    // The strip's list: Only overnight.
    r.rt.update(|s, _| s.harness_setup.close());
    r.rt.frame(1.0 / 60.0);
    click(&mut r.rt, "update-default-rule");
    click(&mut r.rt, "update-default-rule-list-opt3");
    r.wait("the rule to be kept", |s| s.pickers.prefs.get("FleetUpdateRule") == Some("overnight"));
    assert!(text(&r.rt, "updates-rule-hint").starts_with("For machines that run long jobs during the day."));
    // A machine's own: mini asks first; then Default takes the override away.
    click(&mut r.rt, "update-rule-m-mini");
    click(&mut r.rt, "update-machine-rule-list-opt1");
    r.wait("the override to be kept", |s| s.pickers.prefs.get("FleetUpdateRules") == Some(r#"{"m-mini":"ask"}"#));
    r.rt.frame(1.0 / 60.0);
    assert_eq!(text(&r.rt, "update-rule-m-mini"), "Ask me");
    click(&mut r.rt, "update-rule-m-mini");
    click(&mut r.rt, "update-machine-rule-list-opt0");
    r.wait("the override to go", |s| s.pickers.prefs.get("FleetUpdateRules") == Some("{}"));
    assert_eq!(text(&r.rt, "update-rule-m-mini"), "Default (Overnight)");
}

#[test]
fn update_machines_now_posts_the_rollout_and_follows_it() {
    let mut r = rig("0.59.0", None, false);
    seed(&mut r, current(), downloaded());
    r.rt.update(|s, _| s.harness_setup.close());
    r.rt.frame(1.0 / 60.0);
    click(&mut r.rt, "updates-update-machines");
    let home = r.home.clone();
    r.wait("home to hear the rollout", move |_| {
        home.lock().unwrap().heard.iter().any(|(m, p, _)| m == "POST" && p == "/api/update/rollout")
    });
    let body = r.heard(&r.home).into_iter().find(|(m, p, _)| m == "POST" && p == "/api/update/rollout").expect("a post").2;
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap(), json!({ "machineIds": [] }));
    r.rt.frame(1.0 / 60.0);
    r.rt.frame(1.0 / 60.0);
    r.wait("the rollout to run", |s| s.updates.rollout.as_ref().is_some_and(|x| x.running()));
    r.rt.frame(1.0 / 60.0);
    r.rt.frame(1.0 / 60.0);
    assert!(has(&r.rt, "updates-narrative") && has(&r.rt, "updates-stop-rollout"), "the strip turns into the narrative");
    r.rt.update(|s, _| s.harness_setup.close());
    r.rt.frame(1.0 / 60.0);
    click(&mut r.rt, "updates-stop-rollout");
    let home = r.home.clone();
    r.wait("home to hear the stop", move |_| {
        home.lock().unwrap().heard.iter().any(|(m, p, _)| m == "DELETE" && p == "/api/update/rollout")
    });
}

#[test]
fn restart_on_another_machine_is_a_rollout_of_that_machine_and_on_home_posts_apply() {
    let mut r = rig("0.59.0", None, false);
    seed(&mut r, downloaded(), downloaded());
    // Home's own button is off while its sessions work: a press does nothing.
    r.rt.update(|s, _| s.harness_setup.close());
    r.rt.frame(1.0 / 60.0);
    click(&mut r.rt, "update-restart-home");
    assert!(r.rt.state.updates.restarting.is_none());
    // Another machine's is a rollout of that machine alone.
    click(&mut r.rt, "update-restart-m-mini");
    let home = r.home.clone();
    r.wait("home to hear the rollout of one", move |_| {
        home.lock().unwrap().heard.iter().any(|(m, p, b)| m == "POST" && p == "/api/update/rollout" && b.contains("m-mini"))
    });
    assert!(!r.did(&r.home, "POST", "/api/update/apply"));
}

#[test]
fn shows_the_servers_words_when_a_rollout_is_refused() {
    let mut r = rig("0.59.0", None, false);
    r.home.lock().unwrap().refuse_rollout = Some((409, r#"{"error":"A rollout is already running."}"#.into()));
    seed(&mut r, current(), downloaded());
    click(&mut r.rt, "updates-update-machines");
    r.wait("the refusal", |s| s.updates.action_error.is_some());
    r.rt.frame(1.0 / 60.0);
    assert_eq!(text(&r.rt, "updates-error"), "A rollout is already running.");
}

#[test]
fn says_how_a_finished_rollout_went_and_nothing_while_one_runs() {
    let mut rt = page(current(), downloaded());
    rt.update(|s, _| {
        s.updates.rollout = Some(
            Rollout::from_json(&json!({ "id": "r1", "status": "done", "startedAt": "2026-11-17T09:00:00Z", "steps": [
                { "machineId": "m-mini", "name": "mini", "state": "done", "waitingSessions": 0, "toVersion": "0.59.1" }] }))
            .unwrap(),
        );
    });
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    assert_eq!(text(&rt, "updates-outcome"), "Updated mini.");
    rt.update(|s, _| s.updates.rollout = Some(waiting_rollout()));
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    assert!(!has(&rt, "updates-outcome"));
}
