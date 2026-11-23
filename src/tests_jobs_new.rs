//! Component 23, piece H: the Agents tab (`JobsScreen.test.ts`), New job (`NewJobDialog`; no test of its own in Fleet, so these
//! are this app's), `MachineJobSeats.test.ts` (3) and `MachinesSectionJobsOff.test.ts` (1), against a fake home Fleet.
use crate::fleet::jobs::new_job::{labels_key, seats_key};
use crate::fleet::labs::JobsTab;
use crate::tests::{FRAME, click, press_key};
use crate::tests_jobs_store::{Rig, rig};
use crate::{Rail, State};
use rune::{Key, Mods, Runtime, TextEdit};
use serde_json::{Value, json};

fn shown(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn text_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).and_then(|e| e.text)
}

fn machines() -> Value {
    json!([
        {"id": "m1", "name": "atlas", "isLocal": true, "seats": 2, "used": 1, "labels": [], "status": "online"},
        {"id": "m2", "name": "mini", "isLocal": false, "seats": 1, "used": 0, "labels": ["mac"], "status": "online"},
        {"id": "m3", "name": "laptop", "isLocal": false, "seats": 0, "used": 0, "labels": [], "status": "online"}
    ])
}

fn running() -> Value {
    json!({"id": "j1", "title": "Add rate limits", "ask": "Do it", "repository": "https://example.com/acme/ledger-api", "labels": [],
        "pinnedMachineId": null, "state": "running", "machineId": "m1", "machineName": "atlas", "sessionId": "s1",
        "whyWaiting": null, "error": null, "createdAt": "2026-10-27T08:00:00Z"})
}

fn type_into(rt: &mut Runtime<State>, key: &str, words: &str) {
    click(rt, key);
    press_key(rt, Key::Text(words.into()), Mods::default());
}

// ─── JobsScreen.test.ts ─────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn shows_each_machines_seats_on_the_agents_tab() {
    let mut r = rig();
    r.home.route("GET", "/api/jobs", 200, json!({"jobs": [running()], "machines": machines()}));
    r.show();
    r.wait("the first answer", |s| s.jobs.is_loaded);
    click(&mut r.rt, "jobs-tab-agents");
    assert_eq!(r.rt.state.labs.jobs_tab, JobsTab::Agents);

    // One card for every machine, in the server's order: atlas holds the running job and has one seat free; mini's one seat is
    // free; the laptop takes no jobs.
    let (a, b, c) = (r.rt.find("jobs-agent-m1").unwrap(), r.rt.find("jobs-agent-m2").unwrap(), r.rt.find("jobs-agent-m3").unwrap());
    assert!(a.0 < b.0 && c.0 < b.0 && a.1 < c.1 && b.1 < c.1, "two to a row, the third under the first");
    assert_eq!(text_of(&r.rt, "jobs-agent-m1-seats").as_deref(), Some("this machine · 1 of 2 seats"));
    assert_eq!(text_of(&r.rt, "jobs-agent-m2-seats").as_deref(), Some("0 of 1 seats"));
    assert!(shown(&r.rt, "jobs-agent-job-j1"));
    assert!(shown(&r.rt, "jobs-agent-m1-free-0") && !shown(&r.rt, "jobs-agent-m1-free-1"));
    assert!(shown(&r.rt, "jobs-agent-m2-free-0"));
    assert_eq!(text_of(&r.rt, "jobs-agent-m3-none").as_deref(), Some("Takes no jobs. Give it seats in Settings → Machines."));
    assert!(!shown(&r.rt, "jobs-agent-m3-free-0"));
}

// ─── New job ────────────────────────────────────────────────────────────────────────────────────────────────────────

fn repositories() -> Value {
    json!([
        {"name": "ledger-api", "repository": "https://example.com/acme/ledger-api", "machines": ["atlas", "mini"]},
        {"name": "docs-site", "repository": "https://example.com/acme/docs-site", "machines": ["atlas"]}
    ])
}

fn new_job_rig() -> Rig {
    let mut r = rig();
    r.home.route("GET", "/api/jobs", 200, json!({"jobs": [], "machines": machines()}));
    r.home.route("GET", "/api/jobs/repositories", 200, repositories());
    r.show();
    r.wait("the first answer", |s| s.jobs.is_loaded);
    click(&mut r.rt, "jobs-new");
    r.wait("the repositories", |s| s.jobs.repositories.list.is_some());
    r
}

#[test]
fn new_job_reads_the_repositories_and_picks_the_first() {
    let r = new_job_rig();
    assert!(shown(&r.rt, "new-job-dialog"));
    assert_eq!(r.rt.state.jobs.new.as_ref().unwrap().repository, "https://example.com/acme/ledger-api");
    assert!(!shown(&r.rt, "new-job-no-repositories"));
    assert_eq!(r.home.count("GET", "/api/jobs/repositories"), 1);
}

#[test]
fn new_job_posts_only_with_a_title_an_ask_and_a_repository_and_sends_them_trimmed() {
    let mut r = new_job_rig();
    r.home.route("POST", "/api/jobs", 200, json!({}));
    let can = |r: &Rig| r.rt.state.jobs.new.as_ref().unwrap().can_post(r.rt.state.jobs.post.busy);
    assert!(!can(&r));
    type_into(&mut r.rt, "new-job-title", "  Add rate limits ");
    type_into(&mut r.rt, "new-job-ask", "   ");
    assert!(!can(&r), "a blank ask is no ask");
    type_into(&mut r.rt, "new-job-ask", "Limit the public endpoints.  ");
    type_into(&mut r.rt, "new-job-labels", " mac, gpu ,mac");
    assert!(can(&r));
    r.rt.update(|s, _| s.jobs.new.as_mut().unwrap().pinned = "m2".into());

    click(&mut r.rt, "new-job-post");
    r.wait("the post", |s| s.jobs.new.is_none());
    assert_eq!(
        r.home.last("POST", "/api/jobs").body,
        json!({"title": "Add rate limits", "ask": "Limit the public endpoints.", "repository": "https://example.com/acme/ledger-api",
            "labels": ["mac", "gpu"], "pinnedMachineId": "m2"})
    );
}

#[test]
fn new_job_pins_nothing_for_any_machine_that_fits_and_enter_in_a_field_posts() {
    let mut r = new_job_rig();
    r.home.route("POST", "/api/jobs", 200, json!({}));
    type_into(&mut r.rt, "new-job-ask", "Do it");
    type_into(&mut r.rt, "new-job-title", "Fix it");
    press_key(&mut r.rt, Key::Enter, Mods::default());
    r.wait("the post", |s| s.jobs.new.is_none());
    let body = r.home.last("POST", "/api/jobs").body;
    assert_eq!((body["pinnedMachineId"].clone(), body["labels"].clone()), (Value::Null, json!([])));
}

#[test]
fn new_job_keeps_itself_open_with_the_servers_words_when_refused_and_cancel_clears_it() {
    let mut r = new_job_rig();
    r.home.route("POST", "/api/jobs", 400, json!({"error": "That repository isn't on a machine with seats."}));
    type_into(&mut r.rt, "new-job-title", "Fix it");
    type_into(&mut r.rt, "new-job-ask", "Do it");
    click(&mut r.rt, "new-job-post");
    r.wait("the refusal", |s| s.jobs.new.as_ref().is_some_and(|f| f.error.is_some()));
    r.rt.frame(FRAME);
    assert_eq!(text_of(&r.rt, "new-job-error-text").as_deref(), Some("That repository isn't on a machine with seats."));
    assert_eq!(r.rt.state.jobs.new.as_ref().unwrap().title.text, "Fix it", "what was typed stays");

    click(&mut r.rt, "new-job-cancel");
    assert!(r.rt.state.jobs.new.is_none() && !shown(&r.rt, "new-job-dialog"));
    click(&mut r.rt, "jobs-new");
    let f = r.rt.state.jobs.new.as_ref().unwrap();
    assert_eq!((f.title.text.as_str(), f.ask.text.as_str(), f.error.clone()), ("", "", None), "closing clears every field and the error");
}

#[test]
fn new_job_says_so_when_no_machine_has_a_repository_and_shows_a_failed_read() {
    let mut r = rig();
    r.home.route("GET", "/api/jobs", 200, json!({"jobs": [], "machines": machines()}));
    r.home.route("GET", "/api/jobs/repositories", 200, json!([]));
    r.show();
    r.wait("the first answer", |s| s.jobs.is_loaded);
    click(&mut r.rt, "jobs-new");
    r.wait("the list", |s| s.jobs.repositories.list.is_some());
    assert!(shown(&r.rt, "new-job-no-repositories"));
    let f = r.rt.state.jobs.new.as_ref().unwrap();
    assert!(!f.can_post(false) && f.repository.is_empty());

    click(&mut r.rt, "new-job-cancel");
    r.home.route("GET", "/api/jobs/repositories", 500, json!({"error": "No machine answered."}));
    click(&mut r.rt, "jobs-new");
    r.wait("the refusal", |s| s.jobs.repositories.error.is_some());
    r.rt.frame(FRAME);
    assert_eq!(text_of(&r.rt, "new-job-error-text").as_deref(), Some("No machine answered."));
}

#[test]
fn new_job_picks_a_repository_and_a_machine_from_its_selects() {
    let mut r = new_job_rig();
    click(&mut r.rt, "new-job-repository");
    click(&mut r.rt, "new-job-repository-opt1");
    assert_eq!(r.rt.state.jobs.new.as_ref().unwrap().repository, "https://example.com/acme/docs-site");
    assert!(shown(&r.rt, "new-job-dialog"), "a pick closes only the list");
    click(&mut r.rt, "new-job-machine");
    // "Any machine that fits", then every job machine by name.
    click(&mut r.rt, "new-job-machine-opt2");
    assert_eq!(r.rt.state.jobs.new.as_ref().unwrap().pinned, "m2");
}

// ─── MachineJobSeats.test.ts ────────────────────────────────────────────────────────────────────────────────────────

/// Settings → Machines on the app connected to the fake, with only falcon beside home so both cards fit on the screen.
fn seats_rig() -> Rig {
    let mut r = rig();
    r.home.route(
        "GET",
        "/api/jobs",
        200,
        json!({"jobs": [], "machines": [
            {"id": "m1", "name": "atlas", "isLocal": true, "seats": 2, "used": 0, "labels": [], "status": "online"},
            {"id": "m-falcon", "name": "falcon", "isLocal": false, "seats": 0, "used": 0, "labels": [], "status": "online"}
        ]}),
    );
    r.rt.update(|s, _| {
        s.show_machines_page(true, false);
        s.rail = Rail::Settings;
        s.machines.list.retain(|m| m.id == "m-falcon");
        // The seats are on the machine's own page (`MachinePage.vue`), home's first.
        s.manage_machine("home");
    });
    r.rt.frame(FRAME);
    r.wait("the queue", |s| s.jobs.is_loaded && s.jobs.seat_fields.contains_key("m1"));
    r
}

fn set(r: &mut Rig, key: &str, which: fn(&mut crate::fleet::jobs::new_job::SeatFields) -> &mut TextEdit, id: &'static str, words: &str) {
    click(&mut r.rt, key);
    let words = words.to_string();
    r.rt.update(move |s, _| *which(s.jobs.seat_fields.get_mut(id).unwrap()) = TextEdit::new(words));
    press_key(&mut r.rt, Key::Enter, Mods::default());
}

#[test]
fn shows_this_machines_seats_on_its_own_page_and_saves_a_change() {
    let mut r = seats_rig();
    assert_eq!(r.rt.state.jobs.seat_fields["m1"].seats.text, "2");
    assert!(shown(&r.rt, &seats_key("m1")));
    r.home.route(
        "PUT",
        "/api/jobs/machines/m1",
        200,
        json!({"id": "m1", "name": "atlas", "isLocal": true, "seats": 3, "used": 0, "labels": [], "status": "online"}),
    );
    set(&mut r, &seats_key("m1"), |f| &mut f.seats, "m1", "3");
    r.wait("the save", |s| !s.jobs.save.busy && s.jobs.machines[0].seats == 3);
    assert_eq!(r.home.last("PUT", "/api/jobs/machines/m1").body, json!({"seats": 3}));
}

#[test]
fn gives_another_machine_labels_trimmed_and_without_repeats() {
    let mut r = seats_rig();
    r.rt.update(|s, _| s.manage_machine("m-falcon"));
    r.rt.frame(FRAME);
    r.home.route(
        "PUT",
        "/api/jobs/machines/m-falcon",
        200,
        json!({"id": "m-falcon", "name": "falcon", "isLocal": false, "seats": 0, "used": 0, "labels": ["mac", "ios"], "status": "online"}),
    );
    set(&mut r, &labels_key("m-falcon"), |f| &mut f.labels, "m-falcon", " mac, ios ,mac");
    r.wait("the save", |s| !s.jobs.save.busy && !s.jobs.machines[1].labels.is_empty());
    assert_eq!(r.home.last("PUT", "/api/jobs/machines/m-falcon").body, json!({"labels": ["mac", "ios"]}));
}

#[test]
fn draws_nothing_for_a_machine_the_home_fleet_doesnt_list() {
    let mut r = seats_rig();
    r.rt.update(|s, _| s.jobs.machines.retain(|m| m.id == "m1"));
    r.rt.frame(FRAME);
    assert!(shown(&r.rt, "machine-job-seats-m1"));
    r.rt.update(|s, _| s.manage_machine("m-falcon"));
    r.rt.frame(FRAME);
    assert!(!shown(&r.rt, "machine-job-seats-m-falcon") && !shown(&r.rt, &seats_key("m-falcon")));
}

#[test]
fn a_refused_save_says_why_and_clamps_the_seats_it_sent() {
    let mut r = seats_rig();
    r.home.route("PUT", "/api/jobs/machines/m1", 400, json!({"error": "Seats are 0 to 16."}));
    set(&mut r, &seats_key("m1"), |f| &mut f.seats, "m1", "99.7");
    r.wait("the refusal", |s| s.jobs.save.error.is_some());
    r.rt.frame(FRAME);
    assert_eq!(r.home.last("PUT", "/api/jobs/machines/m1").body, json!({"seats": 16}), "truncated and clamped to 0..16");
    assert_eq!(text_of(&r.rt, "machine-job-seats-error-m1-text").as_deref(), Some("Seats are 0 to 16."));
}

#[test]
fn leaving_a_field_saves_it_once() {
    let mut r = seats_rig();
    r.home.route(
        "PUT",
        "/api/jobs/machines/m1",
        200,
        json!({"id": "m1", "name": "atlas", "isLocal": true, "seats": 4, "used": 0, "labels": [], "status": "online"}),
    );
    click(&mut r.rt, &seats_key("m1"));
    r.rt.update(|s, _| s.jobs.seat_fields.get_mut("m1").unwrap().seats = TextEdit::new("4"));
    r.rt.frame(FRAME);
    click(&mut r.rt, &labels_key("m1"));
    r.wait("the save", |s| !s.jobs.save.busy && s.jobs.machines[0].seats == 4);
    assert_eq!(r.home.count("PUT", "/api/jobs/machines/m1"), 1);
}

// ─── MachinesSectionJobsOff.test.ts ─────────────────────────────────────────────────────────────────────────────────

#[test]
fn renders_as_it_always_did_and_asks_nothing_about_jobs() {
    let mut r = rig();
    r.rt.update(|s, _| {
        s.pickers.prefs.adopt(&json!({"Jobs": "false"}));
        s.show_machines_page(true, false);
        s.rail = Rail::Settings;
    });
    r.later(5.0);
    assert!(
        shown(&r.rt, "machine-card-home")
            || r.rt.elements().iter().any(|e| e.key.as_deref().is_some_and(|k| k.starts_with("machine-card-")))
    );
    assert!(!r.rt.elements().iter().any(|e| e.key.as_deref().is_some_and(|k| k.starts_with("machine-job-seats"))));
    assert_eq!(r.home.jobs_total(), 0, "Jobs is off: nothing asks about jobs");
}
