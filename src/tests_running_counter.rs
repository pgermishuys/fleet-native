//! The status bar's running-work counter (component 9, piece H): the four cases of `RunningWorkCounter.test.ts` under
//! their names, plus the chip's look and what the popover's headings do. Fleet's tests mount the status bar; these draw it.
use crate::fleet::events::Event;
use crate::mock::running::Case;
use crate::tests::{FRAME, app, click};
use crate::ui::Open;
use rune::{ElementInfo, Runtime};
use serde_json::{Value, json};

fn shell(session: &str, id: &str, label: &str) -> Value {
    json!({
        "id": id, "sessionId": session, "workId": format!("sh_{id}"), "kind": "shell", "title": "shell", "label": label,
        "status": "running", "background": true, "canStop": true, "canReadOutput": true, "startedAt": "2026-10-04T10:00:00Z",
    })
}

fn info(rt: &Runtime<crate::State>, key: &str) -> Option<ElementInfo> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn text_of(rt: &Runtime<crate::State>, key: &str) -> String {
    info(rt, key).unwrap_or_else(|| panic!("no element keyed {key:?}")).text.unwrap_or_default()
}

fn many(open: bool) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_running(Case::Many, open));
    rt.frame(FRAME);
    rt
}

fn emit(rt: &mut Runtime<crate::State>, kind: &'static str, item: Value) {
    rt.update(move |s, _| s.on_list_work_event(&Event::from_wire(&json!({"type": kind, "properties": item})).expect("an event")));
    rt.frame(FRAME);
}

// RunningWorkCounter.test.ts: "is hidden when nothing runs"
#[test]
fn is_hidden_when_nothing_runs() {
    let mut rt = app();
    rt.frame(FRAME);
    assert!(info(&rt, "running-work-counter").is_none());
    // And a popover left open when the last item ends goes with it: Fleet's popover is inside `v-if="running.length > 0"`.
    rt.update(|s, _| s.show_running(Case::One, true));
    rt.frame(FRAME);
    assert!(info(&rt, "running-work-popover").is_some());
    emit(&mut rt, "work.ended", {
        let mut v = shell("s1", "w1", "npm run dev -- --port 5174");
        v["status"] = json!("completed");
        v
    });
    assert!(info(&rt, "running-work-counter").is_none());
    assert!(info(&rt, "running-work-popover").is_none(), "the list shuts with nothing left to list");
}

// RunningWorkCounter.test.ts: "counts what runs and in how many sessions"
#[test]
fn counts_what_runs_and_in_how_many_sessions() {
    let rt = many(false);
    assert_eq!(text_of(&rt, "running-work-label"), "4 running in 3 sessions");
    let mut rt = app();
    rt.update(|s, _| s.show_running(Case::One, false));
    rt.frame(FRAME);
    assert_eq!(text_of(&rt, "running-work-label"), "1 running in 1 session");
}

// RunningWorkCounter.test.ts: "follows work starting and ending in any session without asking Fleet again"
#[test]
fn follows_work_starting_and_ending_in_any_session_without_asking_fleet_again() {
    let mut rt = app();
    rt.frame(FRAME);
    let asked = rt.state.list_work_load();
    emit(&mut rt, "work.started", shell("s1", "w-e2e", "npm run test:e2e"));
    assert_eq!(text_of(&rt, "running-work-label"), "1 running in 1 session");
    let mut done = shell("s1", "w-e2e", "npm run test:e2e");
    done["status"] = json!("completed");
    done["endedAt"] = json!("2026-10-04T10:03:12Z");
    done["endedReason"] = json!("completed");
    emit(&mut rt, "work.ended", done);
    assert!(info(&rt, "running-work-counter").is_none());
    assert_eq!(rt.state.list_work_load(), asked, "no read was asked for");
}

// RunningWorkCounter.test.ts: "opens the work grouped by session, this session first"
#[test]
fn opens_the_work_grouped_by_session_this_session_first() {
    let mut rt = many(false);
    // Open the second session's work first in the list's own order, so "this session first" isn't the order it came in.
    rt.update(|s, _| s.set_open((0, 1)));
    rt.frame(FRAME);
    click(&mut rt, "running-work-counter");
    assert_eq!(rt.state.ui.open, Some(Open::Popover("running")));
    let titles: Vec<String> = ["s2", "s1", "s3"].iter().map(|id| text_of(&rt, &format!("running-work-title-{id}"))).collect();
    assert_eq!(titles, ["Move the billing job to the queue", "Speed up the search index", "Tidy the onboarding copy"]);
    let head = |id: &str| info(&rt, &format!("running-work-session-{id}")).unwrap_or_else(|| panic!("no heading for {id}")).rect[1];
    assert!(head("s2") < head("s1") && head("s1") < head("s3"), "the open session's group is first, the rest in start order");
    assert_eq!(text_of(&rt, "running-work-place-s2"), "\u{b7} this session");
    assert_eq!(text_of(&rt, "running-work-place-s1"), "\u{b7} demo-app");
    // The first group's rows are the strip's own, oldest start first: its shell, then the other shell.
    assert_eq!(text_of(&rt, "work-what-w1"), "npm run dev -- --port 5174");
    assert!(
        head("s1") < info(&rt, "work-what-w1").unwrap().rect[1]
            && info(&rt, "work-what-w1").unwrap().rect[1] < info(&rt, "work-what-w3").unwrap().rect[1]
    );
    // The sub-agent's row offers Open, as in Fleet.
    assert!(info(&rt, "work-open-w2").is_some());
    assert_eq!(info(&rt, "running-work-popover").unwrap().label.as_deref(), Some("Running in the background"));
    // A press outside shuts it.
    click(&mut rt, "running-work-scrim");
    assert_eq!(rt.state.ui.open, None);
}

#[test]
fn a_heading_opens_that_session_and_closes_the_list() {
    let mut rt = many(true);
    assert_eq!(rt.state.open_id().as_deref(), Some("s1"));
    click(&mut rt, "running-work-session-s3");
    assert_eq!(rt.state.open_id().as_deref(), Some("s3"));
    assert_eq!(rt.state.ui.open, None);
}

#[test]
fn the_chip_is_drawn_as_fleet_draws_it() {
    let mut rt = many(false);
    let t = crate::theme::Theme::of(true);
    let chip = info(&rt, "running-work-counter").unwrap();
    assert_eq!(chip.rect[3], 18.0, "18 px tall");
    assert_eq!(chip.fill, t.running.alpha(0.14));
    assert_eq!(chip.help.clone().unwrap_or_default(), "4 running in 3 sessions: show what runs in the background");
    assert_eq!(info(&rt, "running-work-label").unwrap().text_color, Some(t.running_text));
    click(&mut rt, "running-work-counter");
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    assert_eq!(info(&rt, "running-work-counter").unwrap().fill, t.running.alpha(0.24), "lit while its list is open");
}

#[test]
fn the_lists_times_tick_while_it_is_open() {
    let mut rt = many(true);
    let before = text_of(&rt, "work-time-w4");
    rt.update(|s, _| s.now_ms = s.now_ms.map(|n| n + 5_000));
    for _ in 0..90 {
        rt.frame(FRAME);
    }
    assert_ne!(text_of(&rt, "work-time-w4"), before, "20 s became 25 s");
}
