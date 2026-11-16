//! `use-running-work.test.ts`'s two `useRunningWorkAcrossSessions` cases and the counts and order of
//! `RunningWorkCounter.test.ts`, on the state: what the read answers, what the `work.*` events do, how the work is
//! grouped, and what Stop does to an item of a session that isn't open. The requests run on a thread in the app, so each
//! case hands the answer to the handler the thread would.
use super::reconnect::{app, two_sessions};
use super::*;
use crate::fleet::list_work::PlacedGroup;

fn shell(session: &str, id: &str, extra: Value) -> Value {
    let mut v = json!({
        "id": id, "sessionId": session, "workId": "sh_1", "kind": "shell", "title": "shell", "label": "bun run test:e2e",
        "status": "running", "background": true, "toolCallId": "call_bg", "canStop": true, "canReadOutput": true,
        "startedAt": "2026-10-04T10:00:00Z",
    });
    for (k, x) in extra.as_object().into_iter().flatten() {
        v[k] = x.clone();
    }
    v
}

fn subagent(session: &str, id: &str) -> Value {
    json!({
        "id": id, "sessionId": session, "workId": "call_sub", "kind": "subagent", "title": "code-reviewer",
        "label": "Review the diff", "status": "running", "background": true, "childSessionId": "child-1",
        "toolCallId": "call_sub", "canStop": true, "startedAt": "2026-10-04T10:02:00Z",
    })
}

fn ended(mut v: Value) -> Value {
    v["status"] = json!("completed");
    v["endedAt"] = json!("2026-10-04T10:05:00Z");
    v["endedReason"] = json!("completed");
    v["detail"] = json!("exit 0");
    v
}

fn emit(s: &mut crate::State, kind: &str, item: Value) {
    s.on_list_work_event(&Event::from_wire(&json!({"type": kind, "properties": item})).expect("an event"));
}

fn load(s: &mut crate::State, body: Value) {
    s.load_list_work();
    let load = s.list_work_load();
    s.on_list_work_loaded(load, &body);
}

fn ids(s: &crate::State) -> Vec<String> {
    s.list_work.running_sorted().into_iter().map(|i| i.id).collect()
}

// use-running-work.test.ts: "loads what runs everywhere and groups it by session"
#[test]
fn loads_what_runs_everywhere_and_groups_it_by_session() {
    let mut rt = app();
    rt.update(|s, _| {
        load(
            s,
            json!([
                shell("s1", "w-shell", json!({})),
                subagent("s2", "w-agent"),
                shell("s1", "w-shell-2", json!({"startedAt": "2026-10-04T10:03:00Z"}))
            ]),
        );
        assert_eq!(s.list_work.running_sorted().len(), 3);
        assert_eq!(s.list_work.session_count(), 2);
        let groups: Vec<(String, usize)> = s.list_work.groups().into_iter().map(|g| (g.session_id, g.items.len())).collect();
        assert_eq!(groups, vec![("s1".to_string(), 2), ("s2".to_string(), 1)]);
    });
}

// use-running-work.test.ts: "counts work as it starts and stops counting it when it ends, without asking again"
#[test]
fn counts_work_as_it_starts_and_stops_counting_it_when_it_ends_without_asking_again() {
    let mut rt = app();
    rt.update(|s, _| {
        load(s, json!([]));
        assert!(s.list_work.running_sorted().is_empty());
        let asked = s.list_work_load();
        emit(s, "work.started", shell("s1", "w-shell", json!({})));
        emit(s, "work.started", subagent("s2", "w-agent"));
        assert_eq!(s.list_work.session_count(), 2);
        emit(s, "work.ended", ended(subagent("s2", "w-agent")));
        assert_eq!(ids(s), vec!["w-shell"]);
        assert_eq!(s.list_work_load(), asked, "no read was asked for by the events");
    });
}

// RunningWorkCounter.test.ts: "counts what runs and in how many sessions"
#[test]
fn counts_what_runs_and_in_how_many_sessions() {
    let mut rt = app();
    rt.update(|s, _| {
        assert_eq!(s.list_work.label(), "0 running in 0 sessions");
        load(s, json!([shell("s2", "w-dev", json!({"label": "bun run dev"})), shell("s1", "w-e2e", json!({})), subagent("s1", "w-agent")]));
        assert_eq!(s.list_work.label(), "3 running in 2 sessions");
        emit(s, "work.ended", ended(shell("s2", "w-dev", json!({}))));
        assert_eq!(s.list_work.label(), "2 running in 1 session");
    });
}

// A read that was overtaken by a newer one doesn't overwrite it.
#[test]
fn a_slow_answer_does_not_replace_a_newer_one() {
    let mut rt = app();
    rt.update(|s, _| {
        s.load_list_work();
        let old = s.list_work_load();
        s.load_list_work();
        let new = s.list_work_load();
        s.on_list_work_loaded(new, &json!([shell("s1", "w-new", json!({}))]));
        s.on_list_work_loaded(old, &json!([shell("s1", "w-old", json!({}))]));
        assert_eq!(ids(s), vec!["w-new"]);
    });
}

// RunningWorkCounter.test.ts: "opens the work grouped by session, this session first"
#[test]
fn groups_by_session_this_session_first_with_each_sessions_place() {
    let mut rt = app();
    rt.update(|s, tx| {
        s.on_list(two_sessions(), tx);
        let (open, other) = (s.open_id().unwrap(), "second-session".to_string());
        let place = |s: &mut crate::State, id: &str, name: &str| {
            let (p, i) = s.row_of(id).unwrap();
            s.projects[p].sessions[i].listing.workspace_display_name = Some(name.into());
        };
        place(s, &other, "demo-workspace");
        load(
            s,
            json!([
                shell(&other, "w-dev", json!({"startedAt": "2026-10-04T09:00:00Z"})),
                shell(&open, "w-e2e", json!({})),
                subagent(&open, "w-agent"),
                shell("gone", "w-gone", json!({}))
            ]),
        );
        let groups: Vec<PlacedGroup> = s.placed_work_groups();
        assert_eq!(groups.iter().map(|g| g.group.session_id.as_str()).collect::<Vec<_>>(), vec![open.as_str(), other.as_str(), "gone"]);
        assert_eq!(groups[0].place.as_deref(), Some("this session"));
        assert_eq!(groups[1].place.as_deref(), Some("demo-workspace"));
        assert_eq!(groups[1].title, "Second one");
        assert_eq!((groups[2].title.as_str(), groups[2].place.as_deref()), ("Untitled session", None));
        assert_eq!(groups[0].group.items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), vec!["w-e2e", "w-agent"]);
    });
}

#[test]
fn a_session_with_no_workspace_name_is_placed_by_its_project() {
    let mut rt = app();
    rt.update(|s, tx| {
        s.on_list(two_sessions(), tx);
        let other = "second-session".to_string();
        let (p, i) = s.row_of(&other).unwrap();
        s.projects[p].sessions[i].listing.workspace_display_name = None;
        s.projects[p].sessions[i].listing.project_name = Some("demo-project".into());
        load(s, json!([shell(&other, "w-dev", json!({}))]));
        assert_eq!(s.placed_work_groups()[0].place.as_deref(), Some("demo-project"));
    });
}

// Actions on another session's item: its Details open, and the answer to its Stop takes it off the list.
#[test]
fn another_sessions_item_has_its_actions_and_stop_takes_it_off_the_list() {
    let mut rt = app();
    rt.update(|s, tx| {
        s.on_list(two_sessions(), tx);
        let other = "second-session".to_string();
        load(s, json!([shell(&other, "w-dev", json!({}))]));
        assert!(s.running_work().is_empty(), "it isn't the open session's work");
        s.toggle_work_output("w-dev");
        assert!(s.work.outputs.contains_key("w-dev"), "Output finds the item in the list");
        s.work.stopping.insert("w-dev".into());
        s.on_work_stopped("w-dev", Ok((200, ended(shell(&other, "w-dev", json!({}))))));
        assert!(s.list_work.running_sorted().is_empty());
        assert!(!s.work.stopping.contains("w-dev"));
    });
}

// A clock over the list's items: a second in a running item's first minute, then a minute; resting when nothing runs.
#[test]
fn the_popover_clock_ticks_by_the_youngest_item_and_rests_with_nothing_running() {
    use crate::conv::work_clock::next_tick_ms;
    use crate::fleet::work::{parse_iso_ms, to_items};
    let t0 = parse_iso_ms("2026-10-04T10:00:00Z").unwrap();
    let old = to_items(Some(&json!([shell("s1", "a", json!({}))])));
    assert_eq!(next_tick_ms(&old, t0 + 300_500), Some(59_500), "five minutes in, to the next whole minute");
    let young = to_items(Some(&json!([shell("s1", "a", json!({})), shell("s2", "b", json!({"startedAt": "2026-10-04T10:04:50Z"}))])));
    assert_eq!(next_tick_ms(&young, t0 + 300_500), Some(500), "the young one ticks each second");
    assert_eq!(next_tick_ms(&[], t0), None);
}
