//! Component 23, piece F: `jobs.test.ts` "groups every state exactly once", `parseLabels` and the shapes the server writes.
use crate::fleet::jobs::*;
use serde_json::json;

fn job(id: &str, state: JobState) -> Job {
    Job { id: id.into(), state, ..Default::default() }
}

#[test]
fn groups_every_state_exactly_once() {
    let states = [
        ("waiting", JobState::Waiting),
        ("running", JobState::Running),
        ("needs_you", JobState::NeedsYou),
        ("done", JobState::Done),
        ("failed", JobState::Failed),
    ];
    let jobs: Vec<Job> = states.iter().map(|(id, state)| job(id, *state)).collect();
    let groups = job_groups(&jobs);
    let ids: Vec<(GroupId, Vec<&str>)> = groups.iter().map(|g| (g.id, g.jobs.iter().map(|j| j.id.as_str()).collect())).collect();
    assert_eq!(
        ids,
        [(GroupId::Running, vec!["running", "needs_you"]), (GroupId::Queued, vec!["waiting"]), (GroupId::Done, vec!["done", "failed"]),]
    );
    assert_eq!(groups.map(|g| g.label), ["Running", "Queued", "Done"]);
}

#[test]
fn keeps_the_server_s_order_in_a_group_and_lists_no_job_of_an_unknown_state() {
    let jobs = [job("b", JobState::Done), job("x", JobState::Other), job("a", JobState::Failed)];
    let groups = job_groups(&jobs);
    assert_eq!(groups[2].jobs.iter().map(|j| j.id.as_str()).collect::<Vec<_>>(), ["b", "a"]);
    assert_eq!(groups.iter().map(|g| g.jobs.len()).sum::<usize>(), 2);
}

#[test]
fn parses_labels_trimmed_without_blanks_or_repeats() {
    assert_eq!(parse_labels("mac, gpu"), ["mac", "gpu"]);
    assert_eq!(parse_labels(" mac ,, GPU , Mac,gpu, "), ["mac", "GPU"], "the first spelling stays");
    assert!(parse_labels("").is_empty());
    assert!(parse_labels(" , ,").is_empty());
}

#[test]
fn caps_a_machine_s_seats_at_sixteen() {
    assert_eq!(MAX_JOB_SEATS, 16);
}

#[test]
fn reads_a_job_the_way_the_server_writes_it() {
    let body = json!({
        "jobs": [{
            "id": "j1", "title": "Update the docs", "ask": "Update the README", "repository": "https://example.com/acme/docs-site",
            "labels": ["gpu"], "pinnedMachineId": null, "state": "waiting", "machineId": null, "machineName": null, "sessionId": null,
            "whyWaiting": "No machine has `gpu`.", "error": null, "createdAt": "2026-10-27T08:00:00Z",
        }, {"id": "j2", "state": "running", "machineId": "m1", "machineName": "atlas", "sessionId": "s9", "pinnedMachineId": "m1"},
           {"title": "no id"}],
        "machines": [{"id": "m1", "name": "atlas", "isLocal": true, "seats": 2, "used": 1, "labels": ["mac"], "status": "online"}],
    });
    let (jobs, machines) = parse_jobs(&body).expect("both lists");
    assert_eq!(jobs.len(), 2, "a job with no id is left out");
    assert_eq!(jobs[0].why_waiting.as_deref(), Some("No machine has `gpu`."));
    assert_eq!((jobs[0].state, jobs[0].labels.clone()), (JobState::Waiting, vec!["gpu".to_string()]));
    assert_eq!(
        (jobs[1].state, jobs[1].session_id.as_deref(), jobs[1].pinned_machine_id.as_deref()),
        (JobState::Running, Some("s9"), Some("m1"))
    );
    assert_eq!(
        machines,
        [JobMachine {
            id: "m1".into(),
            name: "atlas".into(),
            is_local: true,
            seats: 2,
            used: 1,
            labels: vec!["mac".into()],
            status: "online".into()
        }]
    );
    assert!(parse_jobs(&json!({"jobs": []})).is_none(), "no machines list is no answer");
}

#[test]
fn writes_a_new_job_as_the_server_reads_it() {
    let job = NewJob { title: "T".into(), ask: "A".into(), repository: "r".into(), labels: vec!["mac".into()], pinned_machine_id: None };
    assert_eq!(job.body(), json!({"title": "T", "ask": "A", "repository": "r", "labels": ["mac"], "pinnedMachineId": null}));
}

#[test]
fn reads_the_repositories_new_job_can_pick() {
    let body = json!([{"name": "docs-site", "repository": "https://example.com/acme/docs-site", "machines": ["atlas", "harbor"]}]);
    let list = parse_repositories(&body).expect("a list");
    assert_eq!((list[0].name.as_str(), list[0].machines.len()), ("docs-site", 2));
    assert!(parse_repositories(&json!({})).is_none());
}
