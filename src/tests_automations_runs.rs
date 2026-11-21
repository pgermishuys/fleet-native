//! Component 19, piece F: an automation's runs. Fleet's two F cases of `AutomationsPage.test.ts` under their names ("lists an
//! automation's runs, and Run now adds one straight away", "opens the session a run started"), against a fake Fleet; then
//! what the native page keeps: the poll is quick while a run runs and slow after, an answer for an automation no longer open
//! is dropped, a failed read keeps what was there, the empty state's hint, a run's words and notes.
use crate::fleet::automations::runs::{BUSY_POLL_MS, IDLE_POLL_MS};
use crate::tests::click;
use crate::tests_automations_page::{Rig, rig, weekly};
use serde_json::{Value, json};
use std::sync::atomic::Ordering;

fn two() -> Vec<Value> {
    vec![weekly("a1", "Weekly PR digest", json!({})), weekly("a2", "Old digest", json!({}))]
}

/// The page with `id` opened by its row and its runs read.
fn opened(list: Vec<Value>, id: &str) -> Rig {
    let mut r = rig(list, vec![]);
    r.open();
    r.wait("the list", |s| !s.shown_automations().is_loading);
    r.frames();
    click(&mut r.rt, &format!("automation-row-{id}"));
    r.wait("the runs", |s| s.automations_nav.runs.loaded);
    r.frames();
    r
}

fn run_json(n: u32, extra: Value) -> Value {
    let mut run =
        json!({"id": format!("r{n}"), "automationId": "a1", "trigger": "schedule", "startedAt": "2026-09-15T10:00:00Z", "state": "done"});
    for (k, v) in extra.as_object().into_iter().flatten() {
        run[k] = v.clone();
    }
    run
}

fn run_ids(r: &Rig) -> Vec<String> {
    r.rt.state.automations_nav.runs.list.iter().map(|x| x.id.clone()).collect()
}

fn set_now(r: &mut Rig, ms: i64) {
    r.rt.update(move |s, _| s.now_ms = Some(ms));
    r.frames();
}

fn now(r: &Rig) -> i64 {
    crate::view::message::now_ms(&r.rt.state)
}

// ─── AutomationsPage.test.ts: "Automations screen" (the F cases) ────────────────────────────────────────────────────

#[test]
fn lists_an_automation_s_runs_and_run_now_adds_one_straight_away() {
    let mut r = opened(two(), "a1");
    assert!(r.has("automation-runs"));
    assert_eq!(r.texts_of("automation-run-state"), ["Running", "Done"]);
    assert_eq!(r.texts_of("automation-runs-count"), ["1 of 2 finished"]);
    assert_eq!(r.home.calls("GET", "/api/automations/a1/runs"), 1);

    // Run now: the run is first at once, before the read that follows it has answered.
    r.home.hold_runs.store(true, Ordering::SeqCst);
    click(&mut r.rt, "automation-run-now");
    r.wait("the note", |s| s.automations_nav.notice.is_some());
    r.frames();

    assert_eq!(r.home.calls("POST", "/api/automations/a1/run"), 1);
    assert_eq!(run_ids(&r), ["rn3", "r2", "r1"]);
    assert_eq!(r.texts_of("automation-run-state"), ["Starting", "Running", "Done"]);
    assert_eq!(r.texts_of("automation-runs-count"), ["1 of 3 finished"]);

    // The read after it brings the server's own list, which holds the run too.
    let heard = r.home.handle();
    r.wait("the read after Run now", move |_| heard("GET", "/api/automations/a1/runs") >= 2);
    r.home.hold_runs.store(false, Ordering::SeqCst);
    r.wait("its answer", |s| !s.automations_nav.runs.busy() || s.automations_nav.runs.list.len() == 3);
    assert_eq!(run_ids(&r), ["rn3", "r2", "r1"]);
}

#[test]
fn opens_the_session_a_run_started() {
    let mut r = opened(two(), "a1");
    let session = r.rt.state.projects[0].sessions[0].id.clone();
    *r.home.runs.lock().unwrap() = vec![run_json(2, json!({"sessionId": session})), run_json(1, json!({}))];
    // The next poll reads them.
    let t = now(&r) + IDLE_POLL_MS;
    set_now(&mut r, t);
    r.wait("the new runs", |s| s.automations_nav.runs.list.first().is_some_and(|x| x.session_id.is_some()));
    r.frames();

    assert!(r.has("automation-run-r2") && r.has("automation-run-r1"));
    assert_eq!(r.texts_of("automation-run-open"), ["Open session →"], "only the run with a session has the way in");
    let role = |key: &str| r.rt.elements().iter().find(|e| e.key.as_deref() == Some(key)).map(|e| e.role);
    assert_eq!(role("automation-run-r2"), Some("button"));
    assert_ne!(role("automation-run-r1"), Some("button"), "a run with no session is not a button");

    click(&mut r.rt, "automation-run-r1");
    assert_eq!(r.rt.state.rail, crate::Rail::Automations, "nothing to open");
    click(&mut r.rt, "automation-run-r2");
    assert_eq!(r.rt.state.rail, crate::Rail::Sessions);
    assert_eq!(r.rt.state.route, crate::fleet::store::Route::Session(session));
}

// ─── What the native page keeps ─────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_poll_is_quick_while_a_run_runs_and_slow_after() {
    let mut r = opened(two(), "a1");
    let runs_read = |r: &Rig| r.home.calls("GET", "/api/automations/a1/runs");
    let (t0, lists0) = (now(&r), r.home.list_calls());
    assert_eq!(runs_read(&r), 1);

    // r2 runs, so the next read is due in 3 s, and it reads the list too.
    set_now(&mut r, t0 + BUSY_POLL_MS - 1);
    r.frames();
    assert_eq!(runs_read(&r), 1, "not yet");
    set_now(&mut r, t0 + BUSY_POLL_MS);
    let heard = r.home.handle();
    r.wait("the poll", move |_| heard("GET", "/api/automations/a1/runs") >= 2);
    // The poll reads the list too.
    let heard = r.home.handle();
    r.wait("the list read", move |_| heard("GET", "/api/automations") > lists0);
    // Its answer is in once the next read is scheduled, 3 s on (the clock stands still between).
    r.wait("its answer", move |s| s.automations_nav.runs.next_poll == t0 + 2 * BUSY_POLL_MS);

    // Nothing runs any more: slowly from here.
    *r.home.runs.lock().unwrap() = vec![run_json(2, json!({})), run_json(1, json!({}))];
    let t1 = t0 + BUSY_POLL_MS;
    set_now(&mut r, t1 + BUSY_POLL_MS);
    let heard = r.home.handle();
    r.wait("a second poll", move |_| heard("GET", "/api/automations/a1/runs") >= 3);
    r.wait("the answer", |s| !s.automations_nav.runs.busy());
    let read = runs_read(&r);
    let t2 = now(&r);
    set_now(&mut r, t2 + BUSY_POLL_MS);
    assert_eq!(runs_read(&r), read, "slow now: 3 s is not enough");
    set_now(&mut r, t2 + IDLE_POLL_MS - 1);
    assert_eq!(runs_read(&r), read);
    set_now(&mut r, t2 + IDLE_POLL_MS);
    let heard = r.home.handle();
    r.wait("the slow poll", move |_| heard("GET", "/api/automations/a1/runs") > read);
}

#[test]
fn an_answer_for_an_automation_no_longer_open_is_dropped() {
    let mut r = rig(two(), vec![]);
    r.home.hold_runs.store(true, Ordering::SeqCst);
    r.open();
    r.wait("the list", |s| !s.shown_automations().is_loading);
    r.frames();
    click(&mut r.rt, "automation-row-a1");
    let heard = r.home.handle();
    r.wait("a1's runs to be asked for", move |_| heard("GET", "/api/automations/a1/runs") > 0);
    assert!(!r.rt.state.automations_nav.runs.loaded);

    // Another automation opens before the answer comes.
    click(&mut r.rt, "automation-row-a2");
    r.frames();
    let heard = r.home.handle();
    r.wait("a2's runs to be asked for", move |_| heard("GET", "/api/automations/a2/runs") > 0);
    r.home.hold_runs.store(false, Ordering::SeqCst);
    r.wait("a2's runs", |s| s.automations_nav.runs.loaded);
    r.frames();
    let of = r.rt.state.automations_nav.runs.of.clone();
    assert_eq!(of.map(|(_, id)| id), Some("a2".to_string()));
    assert_eq!(r.rt.state.automations_nav.runs.list.len(), 2);
    assert!(r.rt.state.automations_nav.runs.list.iter().all(|x| x.id == "r2" || x.id == "r1"));

    // Leaving the page for the list drops them.
    r.rt.update(|s, _| s.automations_nav.clear_selection());
    r.frames();
    assert_eq!(r.rt.state.automations_nav.runs.of, None);
    assert!(!r.has("automation-runs"));
}

#[test]
fn a_failed_read_keeps_what_was_there() {
    let mut r = opened(two(), "a1");
    r.home.refuse_runs.store(true, Ordering::SeqCst);
    let t = now(&r) + BUSY_POLL_MS;
    let before = r.home.calls("GET", "/api/automations/a1/runs");
    set_now(&mut r, t);
    let heard = r.home.handle();
    r.wait("the refused poll", move |_| heard("GET", "/api/automations/a1/runs") > before);
    r.wait("its answer", |s| s.automations_nav.runs.list.len() == 2);
    r.frames();
    assert_eq!(run_ids(&r), ["r2", "r1"]);
    assert!(r.has("automation-runs"));
}

#[test]
fn an_empty_list_says_when_the_first_run_is() {
    let mut r = opened(two(), "a1");
    *r.home.runs.lock().unwrap() = vec![];
    let t = now(&r) + BUSY_POLL_MS;
    set_now(&mut r, t);
    r.wait("no runs", |s| s.automations_nav.runs.list.is_empty());
    r.frames();
    assert!(r.has("automation-no-runs") && !r.has("automation-runs"));
    let words = r.words();
    assert!(words.contains("No runs yet") && words.contains("The first run is Fri 18 Sep, 07:00, or press Run now."), "{words}");

    let hint = |r: &mut Rig, extra: Value| -> String {
        let a = crate::fleet::automations::model::Automation::from_json(&crate::tests_automations_store::automation("x", extra));
        r.rt.state.first_run_hint(&a)
    };
    assert_eq!(hint(&mut r, json!({"isEnabled": false})), "Switch it on, or press Run now.");
    assert_eq!(
        hint(&mut r, json!({"triggerType": "event", "triggerConfig": "{\"eventType\":\"session.completed\"}"})),
        "It runs when its event happens, or press Run now."
    );
    assert_eq!(
        hint(&mut r, json!({"triggerType": "schedule", "triggerConfig": "0 9 * * 1"})),
        "The first run is Mon 21 Sep, 09:00, or press Run now."
    );
    assert_eq!(hint(&mut r, json!({"triggerType": "schedule", "triggerConfig": "bad cron"})), "Press Run now to try it.");
}

#[test]
fn a_run_says_its_time_trigger_state_and_notes() {
    let mut r = opened(two(), "a1");
    *r.home.runs.lock().unwrap() = vec![
        run_json(
            5,
            json!({"state": "failed", "trigger": "manual", "error": "Couldn't start: no branch", "summary": "Looked at three PRs"}),
        ),
        run_json(4, json!({"state": "skipped", "scheduledFor": "2026-09-14T09:00:00Z", "error": "Another run was still going"})),
        run_json(3, json!({"state": "waiting", "trigger": "catch_up"})),
        run_json(2, json!({"state": "done", "machineName": "atlas", "summary": "Done in two minutes", "error": "ignored"})),
    ];
    let t = now(&r) + BUSY_POLL_MS;
    set_now(&mut r, t);
    r.wait("the runs", |s| s.automations_nav.runs.list.len() == 4);
    r.frames();

    assert_eq!(r.texts_of("automation-run-state"), ["Failed", "Skipped", "Needs you", "Done"]);
    assert_eq!(r.texts_of("automation-run-time"), ["Tue 15 Sep, 10:00", "Mon 14 Sep, 09:00", "Tue 15 Sep, 10:00", "Tue 15 Sep, 10:00"]);
    // A summary comes first; a failure always adds its error, a skipped or finished run only when there is no summary.
    assert_eq!(
        r.texts_of("automation-run-note"),
        ["Looked at three PRs", "Couldn't start: no branch", "Another run was still going", "Done in two minutes"]
    );
    assert_eq!(r.texts_of("automation-run-machine"), ["on atlas"]);
    assert!(r.words().contains("Run now") && r.words().contains("Caught up"), "{}", r.words());
    assert_eq!(r.texts_of("automation-runs-count"), ["1 of 4 finished"]);
}
