//! Component 19, piece I: the composer's box, plan line, create and save. Fleet's seven I cases of `AutomationsPage.test.ts`
//! under their names, against a fake Fleet; then what the native composer keeps: Enter creates and Shift+Enter is a new line,
//! a refused save, the form error going with another automation, and the edit state made when the list arrives after the
//! page opened. Nothing sent without a folder is K's Fleet case, in `tests_automations_strip`.
use crate::fleet::automations::nav::ViewMode;
use crate::fleet::automations::submit::{SAVED, time_zone};
use crate::fleet::new_session::request::{Folder, Workspace};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::tests::{click, press_key};
use crate::tests_automations_page::{Rig, rig, weekly};
use rune::{Key, Mods};
use serde_json::{Value, json};
use std::sync::atomic::Ordering;

/// The repository the invented automations run in.
pub(crate) const KETTLE: &str = "/work/kettle";

pub(crate) fn weekly_digest(extra: Value) -> Value {
    let mut extra = extra;
    for (k, v) in json!({"workspaceId": KETTLE, "isolation": "worktree", "baseBranch": null}).as_object().unwrap() {
        if extra.get(k).is_none() {
            extra[k] = v.clone();
        }
    }
    weekly("a1", "Weekly PR digest", extra)
}

/// The page on a fake Fleet whose machine has `/work/kettle`, which New session used last; the list loaded.
pub(crate) fn page(list: Vec<Value>, remembered: bool) -> Rig {
    let mut r = rig(list, vec![]);
    *r.home.repositories.lock().unwrap() = vec![json!({"name": "kettle", "path": KETTLE, "parentRoot": "/work"})];
    if remembered {
        r.rt.update(|s, _| {
            s.sidebar
                .update_new_session_defaults(HOME_MACHINE_KEY, |d| d.remember(&Folder::Repository(KETTLE.into()), &Workspace::New, None))
        });
    }
    r.open();
    r.wait("the list", |s| !s.shown_automations().is_loading);
    r.frames();
    r
}

/// Selects all in the focused box and types `words` over it, as `setValue` does.
pub(crate) fn type_over(r: &mut Rig, words: &str) {
    click(&mut r.rt, "automation-message");
    press_key(&mut r.rt, Key::Command('a'), Mods::default());
    press_key(&mut r.rt, Key::Text(words.into()), Mods::default());
    r.frames();
}

/// New automation, then the sentence typed; waits for the folder New session would start in when one was remembered.
pub(crate) fn start_new(r: &mut Rig, words: &str) {
    click(&mut r.rt, "new-automation");
    r.frames();
    r.wait("the repositories", |s| !s.feeds.repos(HOME_MACHINE_KEY).loading && s.feeds.repos(HOME_MACHINE_KEY).scanned_at.is_some());
    r.frames();
    type_over(r, words);
}

pub(crate) fn plan(r: &Rig) -> String {
    r.texts_of("automation-plan").concat()
}

fn when_chip(r: &Rig) -> String {
    r.rt.elements().iter().find(|e| e.key.as_deref() == Some("automation-when-chip")).and_then(|e| e.label.clone()).unwrap_or_default()
}

pub(crate) fn submit_shown(r: &Rig) -> bool {
    r.has("automation-submit")
}

fn submit_enabled(r: &Rig) -> bool {
    r.rt.elements().iter().any(|e| e.key.as_deref() == Some("automation-submit") && e.focusable)
}

/// Presses the plan line's ask (Rune draws the link as its own element, keyed by its id).
fn press_ask(r: &mut Rig) {
    click(&mut r.rt, "automation-plan-ask");
    r.frames();
}

fn note(r: &Rig) -> Vec<String> {
    r.texts_of("automation-action-message-text")
}

fn form_error(r: &Rig) -> Vec<String> {
    r.texts_of("automation-form-error-text")
}

// ─── AutomationsPage.test.ts: "Automations screen" (the I cases) ────────────────────────────────────────────────────

#[test]
fn reads_when_it_runs_out_of_the_sentence_and_creates_it_on_in_a_new_worktree_each_run() {
    let mut r = page(vec![weekly_digest(json!({}))], true);
    start_new(&mut r, "Every Monday at 9am, summarise the open PRs");
    r.wait("the folder New session used last", |s| s.automations_nav.draft.folder.is_some());
    r.frames();

    let draft = &r.rt.state.automations_nav.draft;
    assert_eq!(&draft.text[draft.highlight().expect("a highlight")], "Every Monday at 9am");
    assert!(when_chip(&r).contains("Mondays 09:00"), "{}", when_chip(&r));
    let plan = plan(&r);
    let zone = time_zone();
    assert!(plan.contains(&format!("Runs every Monday at 09:00 ({zone} time), next Mon 21 Sep, 09:00.")), "{plan}");
    assert!(plan.contains(&format!("a new worktree of {KETTLE}")), "{plan}");
    assert!(plan.contains("The agent gets “Summarise the open PRs”"), "{plan}");
    let draft_row = r.rt.elements().iter().find(|e| e.key.as_deref() == Some("automation-draft-row")).and_then(|e| e.label.clone());
    assert_eq!(draft_row.as_deref(), Some("Summarise the open PRs"));

    click(&mut r.rt, "automation-submit");
    let heard = r.home.handle();
    r.wait("the create", move |_| heard("POST", "/api/automations") > 0);
    r.wait("the new one open", |s| s.automations_nav.open_id() == Some("a2"));
    r.frames();

    let body = r.home.body_of("POST", "/api/automations");
    for (k, v) in [
        ("name", json!("Summarise the open PRs")),
        ("prompt", json!("Summarise the open PRs")),
        ("triggerType", json!("schedule")),
        ("triggerConfig", json!("0 9 * * 1")),
        ("timeZone", json!(zone)),
        ("workspaceId", json!(KETTLE)),
        ("isolation", json!("worktree")),
        ("targetType", json!("new_session")),
        ("maxConcurrentRuns", json!(1)),
    ] {
        assert_eq!(body[k], v, "{k} in {body}");
    }
    assert_eq!(r.home.calls("POST", "/api/automations"), 1);
    assert_eq!(r.rows(), ["Weekly PR digest", "Summarise the open PRs"]);
    assert!(r.words().contains("Summarise the open PRs"), "the header names it");
    assert!(note(&r)[0].starts_with("Created and on. First run"), "{:?}", note(&r));
    assert_eq!(r.rt.state.automations_nav.draft.text, "", "the draft went");
}

#[test]
fn stores_once_on_a_monday_as_that_monday_s_date_and_leaves_the_words_to_fleet() {
    let mut r = page(vec![weekly_digest(json!({}))], true);
    start_new(&mut r, "Create an automation that runs once on a monday to check whether the release notes cover everything.");
    r.wait("the folder", |s| s.automations_nav.draft.folder.is_some());
    r.frames();

    assert!(when_chip(&r).contains("Once · Mon 21 Sep"), "{}", when_chip(&r));
    click(&mut r.rt, "automation-submit");
    let heard = r.home.handle();
    r.wait("the create", move |_| heard("POST", "/api/automations") > 0);

    let body = r.home.body_of("POST", "/api/automations");
    assert_eq!(body["prompt"], "Check whether the release notes cover everything.");
    assert_eq!(body["triggerType"], "once");
    assert_eq!(body["triggerConfig"], "2026-09-21T09:00");
}

#[test]
fn asks_when_it_should_run_and_won_t_create_one_without_it() {
    let mut r = page(vec![weekly_digest(json!({}))], true);
    start_new(&mut r, "Summarise the open PRs");

    assert!(when_chip(&r).contains("When?"), "{}", when_chip(&r));
    assert!(plan(&r).contains("When should it run?"), "{}", plan(&r));
    assert!(submit_shown(&r) && !submit_enabled(&r), "Send is shown, and can't be pressed");
    press_key(&mut r.rt, Key::Enter, Mods::default());
    r.frames();
    assert_eq!(r.home.calls("POST", "/api/automations"), 0);
}

#[test]
fn asks_whether_on_friday_means_every_friday_and_can_make_it_just_once() {
    let mut r = page(vec![weekly_digest(json!({}))], true);
    start_new(&mut r, "Tidy the changelog on Friday.");

    assert!(plan(&r).contains("Every Friday, or just once?"), "{}", plan(&r));
    press_ask(&mut r);

    assert!(when_chip(&r).contains("Once · Fri 18 Sep"), "{}", when_chip(&r));
    assert_eq!(r.texts_of("automation-plan-ask"), ["Every Friday instead"]);

    // "Just once" answered that sentence; a new one that says "every" repeats.
    type_over(&mut r, "Every Monday at 9am, summarise the open PRs");
    assert!(when_chip(&r).contains("Mondays 09:00"), "{}", when_chip(&r));
    type_over(&mut r, "Tidy the changelog on Friday.");
    assert!(plan(&r).contains("Every Friday, or just once?"), "{}", plan(&r));
}

#[test]
fn says_why_the_server_refused_a_create_and_keeps_the_text() {
    let mut r = page(vec![weekly_digest(json!({}))], true);
    *r.home.refuse_next_create.lock().unwrap() = Some("That time has already passed. Pick a later one.".into());
    start_new(&mut r, "Every Monday at 9am, summarise the open PRs");
    r.wait("the folder", |s| s.automations_nav.draft.folder.is_some());
    r.frames();

    click(&mut r.rt, "automation-submit");
    r.wait("the refusal", |s| s.automations_nav.form_error.is_some());
    r.frames();

    assert!(form_error(&r)[0].contains("That time has already passed"), "{:?}", form_error(&r));
    assert_eq!(r.rt.state.automations_nav.draft_box.text, "Every Monday at 9am, summarise the open PRs");
    assert_eq!(r.rt.state.automations_nav.mode, ViewMode::Create, "still the new one");
}

/// Opens automation `id` by its row and waits for its composer.
pub(crate) fn open(r: &mut Rig, id: &str) {
    click(&mut r.rt, &format!("automation-row-{id}"));
    r.frames();
    r.frames();
}

#[test]
fn offers_save_only_once_something_changed_and_keeps_the_schedule_it_had() {
    // A prompt as someone typed it, lower case and all, isn't a change.
    let mut r = page(vec![weekly_digest(json!({"prompt": "summarise the open PRs:"}))], true);
    open(&mut r, "a1");
    assert!(!submit_shown(&r), "nothing changed yet");
    assert_eq!(r.rt.state.automations_nav.edit_box.text, "summarise the open PRs:", "the box holds the prompt as saved");

    type_over(&mut r, "Summarise the open PRs and the drafts");
    assert!(submit_shown(&r) && submit_enabled(&r));
    click(&mut r.rt, "automation-submit");
    let heard = r.home.handle();
    r.wait("the save", move |_| heard("PUT", "/api/automations/a1") > 0);
    r.wait("saved", |s| s.automations_nav.notice.is_some());
    r.frames();

    let body = r.home.body_of("PUT", "/api/automations/a1");
    for (k, v) in [
        ("prompt", json!("Summarise the open PRs and the drafts")),
        ("triggerType", json!("schedule")),
        ("triggerConfig", json!("0 9 * * 1")),
        ("isolation", json!("worktree")),
    ] {
        assert_eq!(body[k], v, "{k} in {body}");
    }
    assert_eq!(note(&r), [SAVED]);
    assert!(!submit_shown(&r), "the saved automation is the edit state again");
}

#[test]
fn warns_that_saving_moves_an_older_utc_schedule_to_your_time_zone() {
    // Fleet's case is an older automation (no zone, so UTC) seen from a browser in another zone. A machine whose zone is UTC
    // (Windows today, row 16.4) has nothing to move, so there the case is the other way round: saved in a zone, saved now in UTC.
    let zone = time_zone();
    let saved = if zone == "UTC" { json!("Africa/Johannesburg") } else { Value::Null };
    // `weekly` saves it in UTC; this one keeps the zone it was given.
    let mut a1 = weekly_digest(json!({}));
    a1["timeZone"] = saved.clone();
    let mut r = page(vec![a1], true);
    open(&mut r, "a1");

    let from = saved.as_str().unwrap_or("UTC");
    assert!(plan(&r).contains(&format!("Saving moves it from {from} time to {zone}.")), "{}", plan(&r));
}

// ─── What the native composer keeps ─────────────────────────────────────────────────────────────────────────────────

#[test]
fn enter_creates_and_shift_enter_is_a_new_line() {
    let mut r = page(vec![weekly_digest(json!({}))], true);
    start_new(&mut r, "Every Monday at 9am, summarise the open PRs");
    r.wait("the folder", |s| s.automations_nav.draft.folder.is_some());
    r.frames();

    press_key(&mut r.rt, Key::Enter, Mods { shift: true, ..Mods::default() });
    r.frames();
    assert_eq!(r.rt.state.automations_nav.draft.text, "Every Monday at 9am, summarise the open PRs\n", "a new line, nothing sent");
    assert_eq!(r.home.calls("POST", "/api/automations"), 0);

    press_key(&mut r.rt, Key::Enter, Mods::default());
    let heard = r.home.handle();
    r.wait("the create", move |_| heard("POST", "/api/automations") > 0);
    r.wait("the new one open", |s| s.automations_nav.mode == ViewMode::Edit);
    assert_eq!(r.home.calls("POST", "/api/automations"), 1);
}

#[test]
fn new_automation_puts_the_keys_in_the_box() {
    let mut r = page(vec![weekly_digest(json!({}))], true);
    click(&mut r.rt, "new-automation");
    r.frames();
    assert!(r.rt.elements().iter().any(|e| e.focused && e.key.as_deref() == Some("automation-message")), "the box has the focus");
}

#[test]
fn a_refused_save_says_why_over_the_composer_and_keeps_the_change() {
    let mut r = page(vec![weekly_digest(json!({}))], true);
    *r.home.refuse_verb.lock().unwrap() = Some(("save".into(), "Invalid cron expression: bad format".into()));
    open(&mut r, "a1");
    type_over(&mut r, "Summarise the open PRs and the drafts");
    click(&mut r.rt, "automation-submit");
    r.wait("the refusal", |s| s.automations_nav.form_error.is_some());
    r.frames();

    assert_eq!(form_error(&r), ["Invalid cron expression: bad format"]);
    assert_eq!(r.rt.state.automations_nav.edit.text, "Summarise the open PRs and the drafts");
    assert!(submit_shown(&r), "still changed");
}

#[test]
fn another_automation_drops_the_form_error_and_the_unsaved_change() {
    let mut r = page(vec![weekly_digest(json!({})), weekly("a2", "Old digest", json!({}))], true);
    *r.home.refuse_verb.lock().unwrap() = Some(("save".into(), "No".into()));
    open(&mut r, "a1");
    type_over(&mut r, "Something else");
    click(&mut r.rt, "automation-submit");
    r.wait("the refusal", |s| s.automations_nav.form_error.is_some());

    open(&mut r, "a2");
    assert!(r.rt.state.automations_nav.form_error.is_none());
    open(&mut r, "a1");
    assert_eq!(r.rt.state.automations_nav.edit.text, "Summarise the open PRs", "the change went with the visit");
    assert!(!submit_shown(&r));
}

#[test]
fn an_automation_opened_before_the_list_arrived_gets_its_composer_when_it_does() {
    let mut r = rig(vec![weekly_digest(json!({}))], vec![]);
    r.home.hold_list.store(true, Ordering::SeqCst);
    r.rt.update(|s, _| s.set_active_automation("a1"));
    r.open();
    r.frames();
    assert!(!r.has("automation-message"), "no composer for an automation the list doesn't hold yet");

    r.home.hold_list.store(false, Ordering::SeqCst);
    r.wait("the list", |s| !s.shown_automations().is_loading);
    r.frames();
    r.frames();
    assert_eq!(r.rt.state.automations_nav.edit_box.text, "Summarise the open PRs");
    assert!(!submit_shown(&r));
}
