//! The display cases of `BackgroundStrip.test.ts` (what shows, the summary, the linger and the drop, the words of each
//! row) as functions of the clock; what the view draws from them is tested in `src/tests.rs`.
use crate::conv::work_clock::*;
use crate::fleet::work::{RunningWorkItem, parse_iso_ms, to_item};
use serde_json::{Value, json};

fn at(s: &str) -> i64 {
    parse_iso_ms(s).unwrap()
}

/// The server's items, as `BackgroundStrip.test.ts` writes them.
fn shell() -> Value {
    json!({"id": "w-shell", "sessionId": "s1", "workId": "sh_1", "kind": "shell", "title": "shell", "label": "bun run test:e2e",
        "status": "running", "background": true, "toolCallId": "call_bg", "canStop": true, "canReadOutput": true,
        "startedAt": "2026-10-04T10:00:00Z"})
}

fn subagent() -> Value {
    json!({"id": "w-agent", "sessionId": "s1", "workId": "call_sub", "kind": "subagent", "title": "code-reviewer",
        "label": "Review the diff", "status": "running", "background": true, "childSessionId": "child-1", "canStop": true,
        "startedAt": "2026-10-04T10:01:32Z"})
}

fn finished_lint() -> Value {
    let mut v = shell();
    v["id"] = json!("w-lint");
    v["label"] = json!("bun run lint");
    v["status"] = json!("completed");
    v["endedAt"] = json!("2026-10-04T10:02:52Z");
    v["endedReason"] = json!("completed");
    v["detail"] = json!("exit 0");
    v["startedAt"] = json!("2026-10-04T10:02:30Z");
    v
}

fn items(v: &[Value]) -> Vec<RunningWorkItem> {
    v.iter().filter_map(to_item).collect()
}

const T0: &str = "2026-10-04T10:03:12Z";

#[test]
fn shows_nothing_when_nothing_runs() {
    let strip = WorkStrip::default();
    assert!(strip.visible(&[], at(T0)).is_empty());
    assert_eq!(strip.summary(&[], at(T0)), "");
    assert_eq!(strip.next_change_ms(&[], at(T0)), None);
}

#[test]
fn lists_running_work_with_kind_what_it_is_and_elapsed_time() {
    let list = items(&[shell(), subagent()]);
    let (strip, now) = (WorkStrip::default(), at(T0));
    assert_eq!(strip.summary(&list, now), "2 running");
    assert_eq!(strip.visible(&list, now).len(), 2);
    assert_eq!(what_text(&list[0]), "bun run test:e2e");
    assert_eq!(what_text(&list[1]), "code-reviewer \u{b7} Review the diff");
    assert_eq!(time_text(&list[0], now), "3 min");
    assert_eq!(time_text(&list[1], now), "1 min");
    // Past their first minute, times change once a minute, not every second.
    assert_eq!(strip.next_change_ms(&list, now), Some(20_000));
    assert_eq!(time_text(&list[0], now + 2_000), "3 min");
    assert_eq!(time_text(&list[0], now + 58_000), "4 min");
}

#[test]
fn ticks_every_second_in_the_first_minute() {
    let list = items(&[subagent()]);
    let now = at("2026-10-04T10:01:50.400Z");
    assert_eq!(time_text(&list[0], now), "18 s");
    assert_eq!(WorkStrip::default().next_change_ms(&list, now), Some(600));
}

#[test]
fn reads_what_it_was_asked_when_the_label_is_the_title_or_missing() {
    let mut same = subagent();
    same["title"] = json!("Review the diff");
    let mut bare = subagent();
    bare.as_object_mut().unwrap().remove("label");
    let list = items(&[same, bare]);
    assert_eq!(what_text(&list[0]), "Review the diff");
    assert_eq!(what_text(&list[1]), "code-reviewer");
}

#[test]
fn shows_finished_work_with_its_result_then_drops_it() {
    let mut strip = WorkStrip::default();
    let mut list = items(&[shell()]);
    strip.observe(&list, at(T0));
    list = items(&[finished_lint(), shell()]);
    // finishedLint ended before this window saw it run, 20 s ago by the server's clock.
    strip.observe(&list, at(T0));
    assert_eq!(strip.summary(&list, at(T0)), "1 running \u{b7} 1 finished");
    assert_eq!(time_text(&list[0], at(T0)), "just now");
    assert_eq!(strip.visible(&list, at(T0) + 11_000).len(), 1);
    assert_eq!(strip.summary(&list, at(T0) + 11_000), "1 running");
}

#[test]
fn counts_the_linger_from_when_this_window_saw_the_end() {
    let mut strip = WorkStrip::default();
    let running = items(&[shell()]);
    strip.observe(&running, at(T0));
    // The server's clock is a minute behind: its endedAt is long past, but this window just watched it end.
    let mut done = shell();
    done["status"] = json!("completed");
    done["endedAt"] = json!("2026-10-04T10:02:00Z");
    let done = items(&[done]);
    strip.observe(&done, at(T0));
    assert_eq!(strip.visible(&done, at(T0) + 29_000).len(), 1);
    assert!(strip.visible(&done, at(T0) + 31_000).is_empty());
    assert_eq!(strip.next_change_ms(&done, at(T0)), Some(30_000));
}

#[test]
fn says_how_long_ago_and_for_how_long_in_the_tooltip() {
    let list = items(&[shell(), finished_lint()]);
    let now = at(T0);
    assert_eq!(time_title(&list[0], now, 0), "Started Oct 4, 10:00:00 AM");
    assert_eq!(time_title(&list[1], now, 0), "Ran for 22 s, from Oct 4, 10:02:30 AM");
    assert_eq!(time_text(&list[1], at("2026-10-04T10:06:52Z")), "4 minutes ago");
    assert_eq!(time_text(&list[1], at("2026-10-04T10:03:52Z")), "1 minute ago");
    assert_eq!(time_title(&list[0], at("2027-01-01T00:00:00Z"), 0), "Started Oct 4, 2026, 10:00:00 AM");
}

#[test]
fn leaves_the_time_empty_when_the_end_cant_be_read() {
    let mut v = finished_lint();
    v["endedAt"] = json!("soon");
    assert_eq!(time_text(&items(&[v])[0], at(T0)), "");
}
