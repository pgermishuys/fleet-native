//! Component 23, piece G: `JobsNavPanel.test.ts` (2) and `JobsScreen.test.ts`' queue tests (4) under their names, the first two
//! on the invented jobs of the scenes, the others against a fake home Fleet. Then what this app adds: Open session finds the
//! session on the machine that took the job, a refused Cancel says why, and the Agents tab is kept while the page stays.
use crate::fleet::labs::JobsTab;
use crate::tests::{FRAME, app, click};
use crate::tests_jobs_store::rig;
use crate::{Rail, State};
use rune::Runtime;
use serde_json::{Value, json};

fn text_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).and_then(|e| e.text)
}

fn shown(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn y_of(rt: &Runtime<State>, key: &str) -> f32 {
    rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}")).1
}

/// The five jobs of `JobsNavPanel.test.ts`, shown with no server.
fn listed() -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| {
        s.show_jobs(crate::mock::jobs::Case::Pick, true);
        s.open_jobs_address("/jobs");
    });
    rt.frame(FRAME);
    rt
}

fn job(id: &str, title: &str, state: &str) -> Value {
    json!({"id": id, "title": title, "ask": "Do it", "repository": "https://example.com/acme/ledger-api", "labels": [],
        "pinnedMachineId": null, "state": state, "machineId": null, "machineName": null, "sessionId": null,
        "whyWaiting": null, "error": null, "createdAt": "2026-10-27T08:00:00Z"})
}

fn machines() -> Value {
    json!([
        {"id": "m1", "name": "atlas", "isLocal": true, "seats": 2, "used": 1, "labels": [], "status": "online"},
        {"id": "m2", "name": "mini", "isLocal": false, "seats": 1, "used": 0, "labels": ["mac"], "status": "online"},
        {"id": "m3", "name": "laptop", "isLocal": false, "seats": 0, "used": 0, "labels": [], "status": "online"}
    ])
}

// ─── JobsNavPanel.test.ts ───────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn groups_jobs_running_queued_and_done_a_job_that_needs_you_staying_running_and_a_failed_one_done() {
    let rt = listed();
    let (running, queued, done) =
        (y_of(&rt, "jobs-group-running-label"), y_of(&rt, "jobs-group-queued-label"), y_of(&rt, "jobs-group-done-label"));
    assert!(running < queued && queued < done);
    let under = |id: &str, lower: f32, upper: f32| {
        let y = y_of(&rt, &format!("job-{id}"));
        assert!(lower < y && y < upper, "{id} sits in its group");
    };
    under("j1", running, queued);
    under("j2", running, queued);
    under("j3", queued, done);
    under("j4", done, f32::MAX);
    under("j5", done, f32::MAX);
    assert!(y_of(&rt, "job-j1") < y_of(&rt, "job-j2"), "each group keeps the server's order");
    assert_eq!(text_of(&rt, "job-j2-flag").as_deref(), Some("Needs you"));
    assert_eq!(text_of(&rt, "job-j5-flag").as_deref(), Some("Failed"));
    assert!(!shown(&rt, "job-j1-flag") && !shown(&rt, "job-j3-flag"), "only those two are flagged");
    assert_eq!(text_of(&rt, "jobs-group-running-label-count").as_deref(), Some("2"));
    assert_eq!(text_of(&rt, "jobs-group-queued-label-count").as_deref(), Some("1"));
}

#[test]
fn opens_a_job_at_its_own_route_and_marks_the_one_the_address_names() {
    let mut rt = listed();
    rt.update(|s, _| s.open_jobs_address("/jobs/j3"));
    rt.frame(FRAME);
    assert_eq!(rt.state.labs.job.as_deref(), Some("j3"));
    click(&mut rt, "job-j1");
    assert_eq!(rt.state.labs.job.as_deref(), Some("j1"));
    assert_eq!(rt.state.rail, Rail::Jobs);
    // The open one does nothing: the address stays and nothing else moves.
    let before = rt.state.labs.job.clone();
    click(&mut rt, "job-j1");
    assert_eq!(rt.state.labs.job, before);
}

#[test]
fn says_no_jobs_yet_when_every_group_is_empty_and_nothing_before_the_first_read() {
    let mut rt = listed();
    rt.update(|s, _| s.jobs.jobs.clear());
    rt.frame(FRAME);
    assert!(shown(&rt, "jobs-nav-empty"));
    assert!(shown(&rt, "jobs-group-running"), "an empty group still shows its label and 0");
    assert_eq!(text_of(&rt, "jobs-group-running-label-count").as_deref(), Some("0"));
    rt.update(|s, _| s.jobs.is_loaded = false);
    rt.frame(FRAME);
    assert!(!shown(&rt, "jobs-nav-empty"));
}

// ─── JobsScreen.test.ts ─────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn says_what_to_do_when_nothing_is_queued() {
    let mut r = rig();
    r.home.route("GET", "/api/jobs", 200, json!({"jobs": [], "machines": machines()}));
    r.show();
    r.wait("the first answer", |s| s.jobs.is_loaded);
    assert!(shown(&r.rt, "jobs-empty"));
    assert!(!shown(&r.rt, "jobs-pick"));
}

#[test]
fn shows_each_machines_seats_leaving_out_one_that_takes_no_jobs() {
    let mut r = rig();
    r.home.route("GET", "/api/jobs", 200, json!({"jobs": [], "machines": machines()}));
    r.show();
    r.wait("the first answer", |s| s.jobs.is_loaded);
    assert_eq!(text_of(&r.rt, "jobs-seat-m1-text").as_deref(), Some("atlas 1/2"));
    assert_eq!(text_of(&r.rt, "jobs-seat-m2-text").as_deref(), Some("mini 0/1"));
    assert!(!shown(&r.rt, "jobs-seat-m3"));
}

#[test]
fn asks_for_a_job_to_be_picked_when_some_exist_and_none_is_open() {
    let mut r = rig();
    r.home.route("GET", "/api/jobs", 200, json!({"jobs": [job("j1", "Add rate limits", "waiting")], "machines": machines()}));
    r.show();
    assert!(!shown(&r.rt, "jobs-pick"), "nothing is said before the first read answers");
    r.wait("the first answer", |s| s.jobs.is_loaded);
    assert!(shown(&r.rt, "jobs-pick"));
    assert!(!shown(&r.rt, "job-detail"));
}

#[test]
fn shows_the_job_the_address_names_and_cancels_only_a_waiting_one() {
    let mut r = rig();
    let mut running = job("j1", "Add rate limits", "running");
    running["machineId"] = json!("m1");
    running["machineName"] = json!("atlas");
    running["sessionId"] = json!("s1");
    let mut waiting = job("j2", "Update the docs", "waiting");
    waiting["whyWaiting"] = json!("No machine has a free seat.");
    r.home.route("GET", "/api/jobs", 200, json!({"jobs": [running, waiting], "machines": machines()}));
    r.home.route("DELETE", "/api/jobs/j2", 200, json!({}));
    r.rt.update(|s, _| s.open_jobs_address("/jobs/j1"));
    r.show();
    r.wait("the first answer", |s| s.jobs.is_loaded);
    assert!(shown(&r.rt, "job-open-session"));
    assert!(!shown(&r.rt, "job-cancel"));
    assert_eq!(text_of(&r.rt, "job-detail-state").as_deref(), Some("Running on atlas"));

    r.rt.update(|s, _| s.open_jobs_address("/jobs/j2"));
    r.rt.frame(FRAME);
    assert_eq!(text_of(&r.rt, "job-detail-why").as_deref(), Some("No machine has a free seat."));
    assert_eq!(text_of(&r.rt, "job-detail-state").as_deref(), Some("Waiting"));
    assert!(!shown(&r.rt, "job-open-session"));

    click(&mut r.rt, "job-cancel");
    r.wait("the cancel", |s| !s.jobs.cancel.busy && s.labs.job.is_none());
    assert_eq!(r.home.count("DELETE", "/api/jobs/j2"), 1);
    assert_eq!(r.rt.state.labs.job, None, "a good Cancel goes back to /jobs");
}

// ─── what this app adds ─────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_refused_cancel_says_why_under_the_page_and_stays_on_the_job() {
    let mut r = rig();
    r.home.route("DELETE", "/api/jobs/j1", 409, json!({"error": "It already started."}));
    r.rt.update(|s, _| s.open_jobs_address("/jobs/j1"));
    r.show();
    r.wait("the first answer", |s| s.jobs.is_loaded);
    click(&mut r.rt, "job-cancel");
    r.wait("the refusal", |s| s.jobs.cancel.error.is_some());
    r.rt.frame(FRAME);
    assert_eq!(text_of(&r.rt, "jobs-action-error-text").as_deref(), Some("It already started."));
    assert_eq!(r.rt.state.labs.job.as_deref(), Some("j1"));
}

#[test]
fn the_store_s_error_shows_under_the_header_and_the_tab_is_kept() {
    let mut rt = listed();
    rt.update(|s, _| s.jobs.error = Some("Couldn't reach Fleet.".into()));
    rt.frame(FRAME);
    assert_eq!(text_of(&rt, "jobs-error-text").as_deref(), Some("Couldn't reach Fleet."));
    click(&mut rt, "jobs-tab-agents");
    assert_eq!(rt.state.labs.jobs_tab, JobsTab::Agents);
    assert!(!shown(&rt, "jobs-pick"), "the Queue's body is gone with its tab");
    // Opening another job from the list keeps the tab: it isn't in the address.
    click(&mut rt, "job-j2");
    assert_eq!(rt.state.labs.jobs_tab, JobsTab::Agents);
    click(&mut rt, "jobs-tab-queue");
    assert!(shown(&rt, "job-detail"));
}

#[test]
fn open_session_remembers_the_machine_that_took_the_job_and_opens_the_session() {
    let mut r = rig();
    let mut other = job("j1", "Add rate limits", "running");
    other["machineId"] = json!("m2");
    other["machineName"] = json!("mini");
    other["sessionId"] = json!("s-mini");
    let mut home = job("j2", "Update the docs", "done");
    home["machineId"] = json!("m1");
    home["machineName"] = json!("atlas");
    home["sessionId"] = json!("s-home");
    r.home.route("GET", "/api/jobs", 200, json!({"jobs": [other, home], "machines": machines()}));
    r.rt.update(|s, _| s.open_jobs_address("/jobs/j1"));
    r.show();
    r.wait("the first answer", |s| s.jobs.is_loaded);
    click(&mut r.rt, "job-open-session");
    assert_eq!(
        r.rt.state.machines.session_machines.get("s-mini").map(String::as_str),
        Some("m2"),
        "another machine's session is found on that machine"
    );
    assert_eq!(r.rt.state.rail, Rail::Sessions);

    r.rt.update(|s, _| s.open_jobs_address("/jobs/j2"));
    r.rt.frame(FRAME);
    click(&mut r.rt, "job-open-session");
    assert_eq!(r.rt.state.machines.session_machines.get("s-home").map(String::as_str), Some(crate::fleet::settings::HOME_MACHINE_KEY));
}
