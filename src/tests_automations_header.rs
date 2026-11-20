//! Component 19, piece E: an open automation's header and actions. Fleet's four E cases under their names ("shows a
//! switch-off in the sidebar too", "says why a switch-off failed" from `AutomationsPage.test.ts`; "asks first, naming it, and
//! deletes once confirmed", "deletes nothing when the confirm is cancelled" from `BuiltinAutomation.test.ts`), against a fake
//! Fleet; then what the native header keeps: Run now's three notices, the info note going after 4 s while an error stays,
//! a note belonging to the automation it was about, and the line beside the switch.
use crate::fleet::actions::{CONFIRM_ACTION, CONFIRM_CANCEL};
use crate::fleet::automations::actions::{NOTICE_MS, STARTED, header_meta};
use crate::fleet::automations::model::Automation;
use crate::fleet::automations::nav::ViewMode;
use crate::tests::click;
use crate::tests_automations_page::{Rig, rig, weekly};
use serde_json::{Value, json};

fn two() -> Vec<Value> {
    vec![weekly("a1", "Weekly PR digest", json!({})), weekly("a2", "Old digest", json!({}))]
}

/// The page with the list loaded and `id` opened by its row.
fn opened(list: Vec<Value>, id: &str) -> Rig {
    let mut r = rig(list, vec![]);
    r.open();
    r.wait("the list", |s| !s.shown_automations().is_loading);
    r.frames();
    click(&mut r.rt, &format!("automation-row-{id}"));
    r.frames();
    r
}

fn note(r: &Rig) -> Vec<String> {
    r.texts_of("automation-action-message-text")
}

// ─── AutomationsPage.test.ts ────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn shows_a_switch_off_in_the_sidebar_too() {
    let mut r = opened(two(), "a1");
    let heard = r.home.handle();
    click(&mut r.rt, "automation-enabled");
    r.wait("the disable to be heard", {
        let heard = r.home.handle();
        move |_| heard("POST", "/api/automations/a1/disable") > 0
    });
    r.wait("the list to say off", |s| s.shown_automations().get("a1").is_some_and(|a| !a.is_enabled) && !s.automations_nav.toggling);
    r.frames();

    assert_eq!(heard("POST", "/api/automations/a1/disable"), 1);
    assert_eq!(r.texts_of("automation-row-status")[0], "Off");
    assert_eq!(r.texts_of("automation-header-meta"), ["Off. It won't run until you switch it on."]);
}

#[test]
fn says_why_a_switch_off_failed() {
    let mut r = opened(two(), "a1");
    *r.home.refuse_verb.lock().unwrap() = Some(("disable".into(), "Automation 'a1' was not found.".into()));
    click(&mut r.rt, "automation-enabled");
    r.wait("the refusal", |s| s.automations_nav.notice.is_some());
    r.frames();

    assert_eq!(note(&r), ["Automation 'a1' was not found."]);
    assert!(r.rt.state.automations_nav.notice.as_ref().is_some_and(|n| n.error && n.until.is_none()), "an error note stays");
    assert!(!r.rt.state.automations_nav.toggling, "the switch is free again");
    assert!(r.texts_of("automation-header-meta")[0].starts_with("Next run"), "still on");
}

// ─── BuiltinAutomation.test.ts ──────────────────────────────────────────────────────────────────────────────────────

fn choose_delete(r: &mut Rig) {
    click(&mut r.rt, "automation-more-menu");
    r.frames();
    click(&mut r.rt, "automation-delete");
    r.frames();
}

#[test]
fn asks_first_naming_it_and_deletes_once_confirmed() {
    let mut r = opened(two(), "a1");
    choose_delete(&mut r);

    let ask = r.rt.state.ui.confirm.clone().expect("a question");
    assert_eq!(
        (ask.title.as_str(), ask.subject.as_str(), ask.confirm_label.as_str(), ask.destructive),
        ("Delete this automation?", "Weekly PR digest", "Delete automation", true)
    );
    assert_eq!(ask.body, "It stops running. The sessions its runs started stay in Sessions.");
    assert_eq!(r.home.calls("DELETE", "/api/automations/a1"), 0, "nothing is deleted before the answer");

    click(&mut r.rt, CONFIRM_ACTION);
    r.wait("the delete to be heard", {
        let heard = r.home.handle();
        move |_| heard("DELETE", "/api/automations/a1") > 0
    });
    r.wait("back to the list", |s| s.automations_nav.mode == ViewMode::List);
    r.frames();

    assert!(r.rt.state.ui.confirm.is_none());
    assert_eq!(r.rows(), ["Old digest"], "the list reads again without it");
}

#[test]
fn deletes_nothing_when_the_confirm_is_cancelled() {
    let mut r = opened(two(), "a1");
    choose_delete(&mut r);
    assert!(r.rt.state.ui.confirm.is_some());

    click(&mut r.rt, CONFIRM_CANCEL);
    r.frames();

    assert!(r.rt.state.ui.confirm.is_none());
    assert_eq!(r.home.calls("DELETE", "/api/automations/a1"), 0);
    assert_eq!(r.rt.state.automations_nav.open_id(), Some("a1"), "still open");
}

// ─── What the native header keeps ───────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_refused_delete_says_so_and_keeps_the_automation_open() {
    let mut r = opened(two(), "a1");
    *r.home.refuse_verb.lock().unwrap() = Some(("delete".into(), "It is running a job.".into()));
    choose_delete(&mut r);
    click(&mut r.rt, CONFIRM_ACTION);
    r.wait("the refusal", |s| s.automations_nav.notice.is_some());
    r.frames();

    assert_eq!(note(&r), ["It is running a job."]);
    assert_eq!(r.rt.state.automations_nav.open_id(), Some("a1"));
    assert!(r.has("automation-header"));
}

#[test]
fn run_now_says_a_run_started_and_the_note_goes_after_four_seconds() {
    let mut r = opened(two(), "a1");
    click(&mut r.rt, "automation-run-now");
    r.wait("the note", |s| s.automations_nav.notice.is_some());
    r.frames();

    assert_eq!(r.home.calls("POST", "/api/automations/a1/run"), 1);
    assert_eq!(note(&r), [STARTED]);
    assert!(r.rt.state.automations_nav.notice.as_ref().is_some_and(|n| !n.error));

    let now = crate::view::message::now_ms(&r.rt.state);
    r.rt.update(move |s, _| s.now_ms = Some(now + NOTICE_MS - 1));
    r.frames();
    assert_eq!(note(&r), [STARTED], "still there just before 4 s");
    r.rt.update(move |s, _| s.now_ms = Some(now + NOTICE_MS));
    r.frames();
    assert!(note(&r).is_empty(), "gone at 4 s");
}

#[test]
fn run_now_says_why_a_run_was_skipped_and_the_note_stays() {
    let mut r = opened(two(), "a1");
    *r.home.run_answer.lock().unwrap() = Some(json!({"state": "skipped", "error": "Another run was still going"}));
    click(&mut r.rt, "automation-run-now");
    r.wait("the note", |s| s.automations_nav.notice.is_some());
    r.frames();
    assert_eq!(note(&r), ["Another run was still going"]);
    assert!(r.rt.state.automations_nav.notice.as_ref().is_some_and(|n| n.error));

    // A skipped run with no reason says that it was skipped.
    *r.home.run_answer.lock().unwrap() = Some(json!({"state": "skipped"}));
    click(&mut r.rt, "automation-run-now");
    r.wait("the second note", |s| s.automations_nav.notice.as_ref().is_some_and(|n| n.text == "The run was skipped."));
    r.frames();
    assert_eq!(note(&r), ["The run was skipped."]);

    let now = crate::view::message::now_ms(&r.rt.state);
    r.rt.update(move |s, _| s.now_ms = Some(now + 60 * NOTICE_MS));
    r.frames();
    assert_eq!(note(&r).len(), 1, "an error note doesn't go by itself");
}

#[test]
fn run_now_says_why_a_refused_run_failed() {
    let mut r = opened(two(), "a1");
    *r.home.refuse_verb.lock().unwrap() = Some(("run".into(), "Workflows are off.".into()));
    click(&mut r.rt, "automation-run-now");
    r.wait("the refusal", |s| s.automations_nav.notice.is_some());
    r.frames();
    assert_eq!(note(&r), ["Workflows are off."]);
}

#[test]
fn opening_another_automation_drops_the_note_but_opening_the_same_one_keeps_it() {
    let mut r = opened(two(), "a1");
    r.rt.update(|s, _| s.automation_say(true, "Couldn't start a run."));
    r.frames();
    assert_eq!(note(&r).len(), 1);

    click(&mut r.rt, "automation-row-a1");
    r.frames();
    assert_eq!(note(&r).len(), 1, "the same automation keeps what was said");

    click(&mut r.rt, "automation-row-a2");
    // The pane eases from one automation to the next; what left is still drawn until it has.
    for _ in 0..40 {
        r.rt.frame(crate::tests::FRAME);
    }
    assert!(note(&r).is_empty(), "{:?}", note(&r));
    assert_eq!(r.rt.state.automations_nav.open_id(), Some("a2"));
}

#[test]
fn the_header_names_the_automation_with_its_switch_and_buttons() {
    let r = opened(two(), "a2");
    for key in ["automation-enabled", "automation-header-meta", "automation-run-now", "automation-more-menu"] {
        assert!(r.has(key), "{key}");
    }
    let switch = r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("automation-enabled")).expect("the switch");
    assert_eq!(switch.label.as_deref(), Some("On"));
    assert!(r.words().contains("Old digest"));
}

#[test]
fn the_list_arriving_after_the_page_opened_shows_the_open_automation_when_it_appears() {
    let mut r = rig(two(), vec![]);
    r.home.hold_list.store(true, std::sync::atomic::Ordering::SeqCst);
    r.rt.update(|s, _| {
        s.automations_nav.set_active("a1");
        s.press_rail(crate::Rail::Automations);
    });
    r.frames();
    assert!(!r.has("automation-run-now"), "nothing to show an automation by while the list is out");

    r.home.hold_list.store(false, std::sync::atomic::Ordering::SeqCst);
    r.wait("the list", |s| !s.shown_automations().is_loading);
    r.frames();
    assert!(r.has("automation-run-now") && r.has("automation-enabled"));
}

// ─── The line beside the switch (`headerMeta`) ──────────────────────────────────────────────────────────────────────

fn automation(extra: Value) -> Automation {
    let list = Automation::list_from_json(&json!({"automations": [crate::tests_automations_store::automation("a1", extra)]}));
    list.into_iter().next().expect("one")
}

#[test]
fn the_line_beside_the_switch_reads_the_trigger_and_the_next_run() {
    let tz = |_: i64| 0;
    let said = |extra: Value| header_meta(&automation(extra), &tz);

    assert_eq!(said(json!({"nextRunAt": "2026-09-21T09:00:00Z"})), "Next run Mon 21 Sep, 09:00");
    assert_eq!(said(json!({})), "", "no next run, nothing to say");
    assert_eq!(said(json!({"isEnabled": false})), "Off. It won't run until you switch it on.");
    assert_eq!(said(json!({"triggerType": "event", "triggerConfig": "{\"eventType\":\"session_created\"}"})), "Runs when a session starts");
    let ran = json!({"id": "r", "automationId": "a1", "trigger": "once", "startedAt": "2026-09-18T16:00:02Z", "state": "done"});
    assert_eq!(said(json!({"isEnabled": false, "triggerType": "once", "lastRun": ran})), "Off. Its one run was Fri 18 Sep, 16:00.");
    assert_eq!(said(json!({"isEnabled": false, "triggerType": "once"})), "Off. It won't run until you switch it on.");
}
