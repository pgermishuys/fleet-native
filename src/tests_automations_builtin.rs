//! Component 19, piece G: the built-in Dreaming automation. Fleet's six cases of `BuiltinAutomation.test.ts` under their names,
//! against a fake Fleet; then what the native page keeps: Settings → Memory's link opens Dreaming, one action at a time, a
//! refusal's words and the fallbacks, a time that isn't one, and the words the page says about the cron and the first night.
use crate::Rail;
use crate::fleet::automations::builtin::{clock, description, no_runs};
use crate::fleet::automations::model::Automation;
use crate::fleet::automations::nav::ViewMode;
use crate::fleet::settings::dream_nightly::{BAD_TIME, parse_nightly};
use crate::fleet::settings_nav::Page;
use crate::tests::click;
use crate::tests_automations_page::{Rig, rig, weekly};
use serde_json::{Value, json};

const NIGHTLY: &str = "/api/memory/dreaming/nightly";

fn dreaming() -> Value {
    weekly(
        "a-dream",
        "Dreaming",
        json!({"builtin": "dreaming", "targetType": "dreaming", "triggerConfig": "0 3 * * *", "nextRunAt": "2026-10-28T01:00:00Z", "prompt": ""}),
    )
}

fn mine() -> Value {
    weekly("a-mine", "Weekly bump", json!({"prompt": "Bump"}))
}

fn night(n: u32, extra: Value) -> Value {
    let mut run = json!({"id": format!("n{n}"), "automationId": "a-dream", "trigger": "schedule", "startedAt": "2026-10-27T01:00:00Z", "state": "done"});
    for (k, v) in extra.as_object().into_iter().flatten() {
        run[k] = v.clone();
    }
    run
}

/// The page with Dreaming's row pressed (it is first in the list) and its runs read.
fn opened(runs: Vec<Value>) -> Rig {
    let mut r = rig(vec![mine(), dreaming()], vec![]);
    *r.home.runs.lock().unwrap() = runs;
    r.open();
    r.wait("the list", |s| !s.shown_automations().is_loading);
    r.frames();
    click(&mut r.rt, "automation-row-a-dream");
    r.wait("the runs", |s| s.automations_nav.runs.loaded);
    r.frames();
    r
}

fn type_time(r: &mut Rig, time: &'static str) {
    r.rt.update(move |s, _| s.automations_nav.builtin.time.replace_text(time));
    r.frames();
}

fn note(r: &Rig) -> Vec<String> {
    r.texts_of("automation-action-message-text")
}

fn zone() -> Value {
    json!(crate::fleet::settings::dream_nightly::local_time_zone().unwrap_or_else(|| "UTC".into()))
}

// ─── BuiltinAutomation.test.ts: "The built-in Dreaming automation" ──────────────────────────────────────────────────

#[test]
fn is_first_in_the_list_with_a_built_in_pill_and_only_it_has_one() {
    let r = opened(vec![]);
    assert_eq!(r.rows(), ["Dreaming", "Weekly bump"]);
    assert_eq!(r.rt.elements().iter().filter(|e| e.key.as_deref() == Some("automation-row-builtin")).count(), 1);
    assert_eq!(r.words().matches("Built in").count(), 2, "its row's pill and the page's own: {}", r.words());
}

#[test]
fn opens_a_compact_view_with_its_time_and_no_delete_or_editing() {
    let r = opened(vec![]);
    assert_eq!(r.rt.state.automations_nav.mode, ViewMode::Edit);
    assert!(r.texts_of("builtin-description")[0].contains("Every day at 03:00."), "{}", r.words());
    assert!(r.words().contains("Experimental"));
    assert!(r.has("automation-builtin-pill"));
    assert!(!r.has("automation-delete") && !r.has("automation-more-menu"), "no ⋯ menu, so nothing to delete with");
    assert!(!r.has("automation-enabled"), "no switch: Pause is its button");
    assert!(!r.has("builtin-time"), "the time box shows once Change time is pressed");
}

#[test]
fn changes_the_time_through_the_nightly_endpoint_in_the_browser_s_zone() {
    let mut r = opened(vec![]);
    click(&mut r.rt, "builtin-change-time");
    r.frames();
    assert!(r.has("builtin-time") && r.has("builtin-time-save") && r.has("builtin-time-cancel"));
    assert_eq!(r.rt.state.automations_nav.builtin.time.text, "03:00", "it starts on the clock time");
    type_time(&mut r, "04:30");
    click(&mut r.rt, "builtin-time-save");
    let heard = r.home.handle();
    r.wait("the time to be sent", move |_| heard("PUT", NIGHTLY) > 0);
    r.wait("the box to close", |s| !s.automations_nav.builtin.editing && !s.automations_nav.builtin.busy);
    r.frames();

    let sent = r.home.body_of("PUT", NIGHTLY);
    assert_eq!(sent, json!({"time": "04:30", "timeZone": zone()}));
    assert_eq!(r.home.calls("PUT", "/api/automations/a-dream"), 0, "the automation itself is not edited");
    assert!(r.has("builtin-change-time") && !r.has("builtin-time"));
}

#[test]
fn pauses_by_disabling_the_automation() {
    let mut r = opened(vec![]);
    assert!(r.texts_of("automation-header-meta")[0].starts_with("Next run "));
    click(&mut r.rt, "builtin-pause");
    let heard = r.home.handle();
    r.wait("the disable", move |_| heard("POST", "/api/automations/a-dream/disable") > 0);
    r.wait("the list again", |s| s.shown_automations().get("a-dream").is_some_and(|a| !a.is_enabled));
    r.frames();
    assert_eq!(r.texts_of("automation-header-meta"), ["Paused. It won't run until you resume it."]);
    assert!(r.rt.elements().iter().any(|e| e.key.as_deref() == Some("builtin-pause") && e.label.as_deref() == Some("Resume")));
}

#[test]
fn opens_settings_memory() {
    let mut r = opened(vec![]);
    click(&mut r.rt, "builtin-settings");
    assert_eq!(r.rt.state.rail, Rail::Settings);
    assert_eq!(r.rt.state.settings.page, Page::Memory);
}

#[test]
fn shows_a_night_s_summary_and_a_skipped_night_s_reason() {
    let r = opened(vec![
        night(1, json!({"summary": "kettle: 4 notes changed · weave: 1 note added · 9 min, 214k tokens"})),
        night(2, json!({"state": "skipped", "error": "Skipped: no new sessions since the last run, nothing to read."})),
    ]);
    assert_eq!(
        r.texts_of("automation-run-note"),
        [
            "kettle: 4 notes changed · weave: 1 note added · 9 min, 214k tokens",
            "Skipped: no new sessions since the last run, nothing to read."
        ]
    );
    assert_eq!(r.texts_of("automation-run-state"), ["Done", "Skipped"]);
}

// ─── What the native page keeps ─────────────────────────────────────────────────────────────────────────────────────

#[test]
fn settings_memory_s_link_selects_dreaming_and_opens_the_page() {
    let mut r = rig(vec![mine(), dreaming()], vec![]);
    r.rt.update(|s, _| {
        s.open_settings("memory", None);
        s.settings.memory.nightly.nightly =
            parse_nightly(&json!({"enabled": true, "automationId": "a-dream", "paused": false, "time": "03:00"}));
        s.open_dream_automation();
    });
    r.wait("the list", |s| s.rail == Rail::Automations && !s.shown_automations().is_loading);
    r.frames();
    assert_eq!(r.rt.state.automations_nav.open_id(), Some("a-dream"));
    assert!(r.has("builtin-description"), "it shows Dreaming's page, not the list's body");
}

#[test]
fn run_now_puts_the_night_first_and_a_refusal_is_the_error_note_with_the_fallback_words() {
    let mut r = opened(vec![night(1, json!({}))]);
    click(&mut r.rt, "automation-run-now");
    let heard = r.home.handle();
    r.wait("the run", move |_| heard("POST", "/api/automations/a-dream/run") > 0);
    r.wait("the run to be first", |s| s.automations_nav.runs.list.first().is_some_and(|x| x.id != "n1") && !s.automations_nav.builtin.busy);
    r.frames();
    assert!(note(&r).is_empty(), "a started run says nothing here: it shows in the list");
    assert_eq!(r.texts_of("automation-run-state")[0], "Starting");

    *r.home.refuse_verb.lock().unwrap() = Some(("run".into(), "Dreaming is switched off".into()));
    click(&mut r.rt, "automation-run-now");
    r.wait("the refusal", |s| s.automations_nav.notice.is_some());
    r.frames();
    assert_eq!(note(&r), ["Dreaming is switched off"]);

    *r.home.refuse_verb.lock().unwrap() = Some(("disable".into(), String::new()));
    click(&mut r.rt, "builtin-pause");
    r.wait("the second refusal", |s| s.automations_nav.notice.as_ref().is_some_and(|n| n.text != "Dreaming is switched off"));
    r.frames();
    assert_eq!(note(&r), ["Couldn't change it."]);
    assert!(!r.rt.state.automations_nav.builtin.busy, "the buttons work again");
}

#[test]
fn one_action_runs_at_a_time() {
    let mut r = opened(vec![]);
    r.rt.update(|s, _| s.automations_nav.builtin.busy = true);
    r.rt.update(|s, tx| {
        s.builtin_run_now(tx);
        s.builtin_toggle_paused(tx);
    });
    r.frames();
    assert_eq!(r.home.calls("POST", "/api/automations/a-dream/run") + r.home.calls("POST", "/api/automations/a-dream/disable"), 0);
    let disabled = |key: &str| r.rt.elements().iter().find(|e| e.key.as_deref() == Some(key)).is_some_and(|e| e.opacity < 1.0);
    assert!(disabled("automation-run-now") && disabled("builtin-pause"), "both buttons wait");
}

#[test]
fn a_time_that_is_not_one_is_said_and_nothing_is_sent_and_a_refused_save_keeps_the_box_open() {
    let mut r = opened(vec![]);
    click(&mut r.rt, "builtin-change-time");
    r.frames();
    type_time(&mut r, "25:99");
    click(&mut r.rt, "builtin-time-save");
    r.frames();
    assert_eq!(note(&r), [BAD_TIME]);
    assert_eq!(r.home.calls("PUT", NIGHTLY), 0);

    type_time(&mut r, "");
    r.frames();
    let save = r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("builtin-time-save")).expect("Save");
    assert!(save.opacity < 1.0, "an empty box can't be saved");

    type_time(&mut r, "4:05");
    *r.home.refuse_time.lock().unwrap() = Some("Memory is off".into());
    click(&mut r.rt, "builtin-time-save");
    r.wait("the refusal", |s| s.automations_nav.notice.as_ref().is_some_and(|n| n.text == "Memory is off"));
    r.frames();
    assert!(r.has("builtin-time"), "the box stays for another try");
    assert_eq!(r.home.body_of("PUT", NIGHTLY)["time"], "04:05");

    click(&mut r.rt, "builtin-time-cancel");
    r.frames();
    assert!(!r.has("builtin-time") && r.has("builtin-change-time"));
}

#[test]
fn its_runs_do_not_open_and_the_empty_state_says_when_the_first_night_is() {
    let mut r = opened(vec![night(1, json!({"sessionId": "s-x"}))]);
    click(&mut r.rt, "automation-run-n1");
    assert_eq!(r.rt.state.rail, Rail::Automations, "Fleet binds no open on this page");

    *r.home.runs.lock().unwrap() = vec![];
    click(&mut r.rt, "automation-row-a-mine");
    r.frames();
    click(&mut r.rt, "automation-row-a-dream");
    r.wait("no runs", |s| s.automations_nav.runs.loaded && s.automations_nav.runs.list.is_empty());
    r.frames();
    let words = r.texts_of("automation-no-runs");
    assert_eq!(words.len(), 1);
    assert!(words[0].starts_with("No runs yet. The first is ") && words[0].ends_with(", or press Run now."), "{words:?}");
}

#[test]
fn the_words_follow_the_cron() {
    assert_eq!(clock("0 3 * * *").as_deref(), Some("03:00"));
    assert_eq!(clock(" 30 14 * * * ").as_deref(), Some("14:30"));
    assert_eq!(clock("5 4 * * 1"), None);
    assert_eq!(clock("*/5 * * * *"), None);
    let mut a = Automation { trigger_config: "0 3 * * 1".into(), ..Automation::default() };
    assert!(description(&a).starts_with("On the schedule 0 3 * * 1. Starts one dreaming session for each repository"));
    a.trigger_config = "0 3 * * *".into();
    assert!(description(&a).ends_with("Skips the rest at no cost."));
    assert_eq!(no_runs(&a), "No runs yet. The first is tonight, or press Run now.");
}
