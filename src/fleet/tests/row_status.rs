//! `lib/__tests__/session-row-status.test.ts` and the compact-time cases of `lib/format/__tests__/time.test.ts`, under
//! Fleet's names. The clock is UTC and fixed, so the dates read the same everywhere.
use crate::fleet::row_status::*;
use crate::fleet::rows::{Failure, Interruption, Retrying, ScheduledRetry};

const MIN: i64 = 60_000;
const HOUR: i64 = 60 * MIN;
const DAY: i64 = 24 * HOUR;
/// 4 October 2024, 12:00 UTC.
const NOW: i64 = 20_000 * DAY + 12 * HOUR;

struct Item {
    status: &'static str,
    activity: &'static str,
    retrying: Retrying,
    scheduled: Option<ScheduledRetry>,
    interrupted: Option<Interruption>,
    failed: Option<Failure>,
    updated: Option<i64>,
}

fn item(status: &'static str) -> Item {
    Item {
        status,
        activity: "idle",
        retrying: Retrying::default(),
        scheduled: None,
        interrupted: None,
        failed: None,
        updated: Some(NOW - 5 * MIN),
    }
}

impl Item {
    fn row(&self) -> RowItem<'_> {
        RowItem {
            status: self.status,
            activity: self.activity,
            retrying: &self.retrying,
            scheduled_retry: self.scheduled.as_ref(),
            interrupted: self.interrupted.as_ref(),
            failed: self.failed.as_ref(),
            updated: self.updated,
            created: None,
        }
    }
    fn status(&self) -> RowStatus {
        row_status(&self.row(), NOW, 0, true)
    }
}

fn iso(ms: i64) -> String {
    let (y, mo, d, h, mi) = crate::conv::time::civil(ms, 0);
    format!("{y:04}-{:02}-{d:02}T{h:02}:{mi:02}:00Z", mo + 1)
}

fn scheduled(due_in_ms: i64, kind: &str) -> ScheduledRetry {
    ScheduledRetry { due_at: iso(NOW + due_in_ms), attempt: 1, kind: kind.into(), reason: String::new(), provider_said: true }
}

#[test]
fn sessionrowstatus_a_failed_last_turn_says_failed_in_the_error_tone_instead_of_the_age() {
    let mut i = item("idle");
    i.failed = Some(Failure { at: iso(NOW - MIN), message: "  The model returned no answer ".into() });
    let s = i.status();
    assert_eq!((s.label.as_str(), s.tone), ("Failed", Tone::Error));
    assert_eq!(s.description, "Last turn failed: The model returned no answer");
    i.failed = Some(Failure { at: iso(NOW - MIN), message: " ".into() });
    assert_eq!(i.status().description, "Last turn failed");
}

#[test]
fn sessionrowstatus_a_failed_last_turn_leaves_a_retry_fleet_has_scheduled_to_say_so() {
    let mut i = item("idle");
    i.failed = Some(Failure { at: iso(NOW - MIN), message: "Rate limited".into() });
    i.scheduled = Some(scheduled(2 * HOUR + 5 * MIN, "rate_limit"));
    let s = i.status();
    assert_eq!(s.tone, Tone::Retry);
    assert_eq!(s.label, "Retry 2:05 PM");
}

#[test]
fn sessionrowstatus_a_failed_last_turn_is_plain_idle_once_the_failure_is_cleared() {
    let mut i = item("idle");
    i.failed = None;
    let s = i.status();
    assert_eq!((s.label.as_str(), s.tone, s.description.as_str()), ("5m", Tone::Quiet, "Idle"));
}

#[test]
fn an_idle_rows_age_is_a_table_of_ages_to_compact_labels() {
    let cases: [(i64, &str); 14] = [
        (0, "now"),
        (59_000, "now"),
        (MIN, "1m"),
        (59 * MIN, "59m"),
        (HOUR, "1h"),
        (23 * HOUR + 59 * MIN, "23h"),
        (DAY, "1d"),
        (6 * DAY, "6d"),
        (7 * DAY, "1w"),
        (13 * DAY, "1w"),
        (14 * DAY, "2w"),
        (55 * DAY, "7w"),
        (56 * DAY, "Aug 9"),
        (119 * 7 * DAY, "Jun 24, 2022"),
    ];
    for (age, label) in cases {
        let mut i = item("idle");
        i.updated = Some(NOW - age);
        assert_eq!(i.status().label, label, "{} ms ago", age);
    }
}

#[test]
fn an_idle_rows_age_is_a_date_past_eight_weeks_never_119w() {
    let mut i = item("idle");
    i.updated = Some(NOW - 119 * 7 * DAY);
    assert!(!i.status().label.contains('w'));
}

#[test]
fn an_idle_rows_age_carries_the_exact_time_as_its_tooltip() {
    let mut i = item("idle");
    i.updated = Some(NOW - 3 * HOUR - 17 * MIN);
    assert_eq!(i.status().title.as_deref(), Some("Oct 4, 8:43 AM"));
    i.updated = Some(NOW - 400 * DAY);
    assert_eq!(i.status().title.as_deref(), Some("Aug 31, 2023, 12:00 PM"));
}

#[test]
fn an_idle_rows_age_is_empty_when_the_session_has_no_time() {
    let mut i = item("idle");
    i.updated = None;
    let s = i.status();
    assert_eq!((s.label.as_str(), s.title), ("", None));
}

#[test]
fn sessionrowstatus_asks_for_attention_when_the_session_waits_on_the_user() {
    let s = item("waiting_input").status();
    assert_eq!((s.label.as_str(), s.tone, s.description.as_str()), ("Needs input", Tone::Attention, "Needs input"));
}

#[test]
fn sessionrowstatus_shows_no_word_for_working_sessions_because_the_glyph_animates() {
    let mut i = item("active");
    i.activity = "busy";
    let s = i.status();
    assert_eq!((s.label.as_str(), s.tone, s.description.as_str()), ("", Tone::Working, "Working"));
    i.activity = "delegating";
    assert_eq!(i.status().description, "Delegating");
}

#[test]
fn sessionrowstatus_names_a_retry_with_the_attempt_when_the_harness_reported_one() {
    let mut i = item("active");
    i.activity = "retry";
    assert_eq!(i.status().label, "Retrying");
    i.retrying.attempt = Some(3);
    let s = i.status();
    assert_eq!((s.label.as_str(), s.tone), ("Retry 3", Tone::Retry));
    assert_eq!(s.description, "Retrying · attempt 3");
}

#[test]
fn sessionrowstatus_names_a_retry_out_of_how_many_and_says_when_and_why() {
    let mut i = item("active");
    i.activity = "retry";
    i.retrying = Retrying {
        attempt: Some(3),
        max_attempts: Some(10),
        message: Some(" API overloaded (529) ".into()),
        next: Some(iso(NOW).replace(":00:00Z", ":00:12Z")),
    };
    let s = i.status();
    assert_eq!(s.label, "Retry 3/10");
    assert_eq!(s.description, "Retrying · attempt 3 of 10 · in 12 s · API overloaded (529)");
}

#[test]
fn sessionrowstatus_names_lifecycle_states_quietly() {
    let s = item("completed").status();
    assert_eq!((s.label.as_str(), s.tone, s.description.as_str()), ("Done", Tone::Quiet, "Done"));
    let s = item("disconnected").status();
    assert_eq!((s.label.as_str(), s.tone, s.description.as_str()), ("Offline", Tone::Quiet, "Offline"));
    let s = item("error").status();
    assert_eq!((s.label.as_str(), s.tone, s.description.as_str()), ("Error", Tone::Error, "Error"));
}

#[test]
fn sessionrowstatus_shows_last_activity_for_idle_sessions() {
    let s = item("idle").status();
    assert_eq!((s.label.as_str(), s.tone, s.description.as_str()), ("5m", Tone::Quiet, "Idle"));
}

#[test]
fn sessionrowstatus_says_when_fleet_tries_a_turn_a_limit_stopped_again() {
    let mut i = item("idle");
    i.scheduled = Some(scheduled(2 * HOUR + 5 * MIN, "usage_limit"));
    let s = i.status();
    assert_eq!((s.label.as_str(), s.tone), ("Retry 2:05 PM", Tone::Retry));
    assert_eq!(s.description, "Stopped by a usage limit. Fleet tries again at 2:05 PM (in 2 h 05 min)");
    i.scheduled = Some(scheduled(-MIN, "overloaded"));
    assert!(i.status().description.starts_with("Stopped by an overloaded provider. Fleet tries again at"));
    assert!(i.status().description.ends_with("(now)"));
}

#[test]
fn sessionrowstatus_shows_last_activity_for_stopped_sessions_which_wake_on_the_next_prompt() {
    let s = item("stopped").status();
    assert_eq!((s.label.as_str(), s.tone), ("5m", Tone::Quiet));
}

#[test]
fn sessionrowstatus_a_restart_cut_off_turn_says_so() {
    let mut i = item("idle");
    i.interrupted = Some(Interruption { at: iso(NOW), waiting: false, last_call: None, changed_files: None, auto: false });
    assert_eq!((i.status().label.as_str(), i.status().tone), ("Cut off by a restart", Tone::Retry));
    i.interrupted.as_mut().unwrap().waiting = true;
    assert_eq!(i.status().label, "Cut off · question lost");
}

#[test]
fn issessionlive_treats_active_waiting_input_and_error_as_live() {
    for status in ["active", "waiting_input", "error"] {
        assert!(is_live(status), "{status}");
    }
}

#[test]
fn issessionlive_treats_idle_stopped_completed_and_disconnected_as_quiet() {
    for status in ["idle", "stopped", "completed", "disconnected"] {
        assert!(!is_live(status), "{status}");
    }
}

fn dim_of(status: &'static str, ago: Option<i64>, created_ago: Option<i64>) -> Dim {
    let mut i = item(status);
    i.updated = ago.map(|a| NOW - a);
    let mut row = i.row();
    row.created = created_ago.map(|a| NOW - a);
    row_dim(&row, NOW)
}

#[test]
fn sessionrowdim_keeps_a_session_at_full_contrast_within_a_day_of_its_last_activity() {
    assert_eq!(dim_of("idle", Some(DAY - 1), None), Dim::None);
}

#[test]
fn sessionrowdim_dims_after_a_day_without_activity_and_further_after_three() {
    assert_eq!(dim_of("idle", Some(DAY), None), Dim::One);
    assert_eq!(dim_of("idle", Some(3 * DAY - 1), None), Dim::One);
    assert_eq!(dim_of("idle", Some(3 * DAY), None), Dim::Two);
}

#[test]
fn sessionrowdim_never_dims_live_sessions_however_old_their_last_activity() {
    for status in ["active", "waiting_input", "error"] {
        assert_eq!(dim_of(status, Some(30 * DAY), None), Dim::None, "{status}");
    }
}

#[test]
fn sessionrowdim_falls_back_to_the_creation_time_and_stays_bright_without_either() {
    assert_eq!(dim_of("idle", None, Some(4 * DAY)), Dim::Two);
    assert_eq!(dim_of("idle", None, None), Dim::None);
}

#[test]
fn sessionrowstatus_while_its_machine_isnt_answering_says_a_working_session_s_machine_isnt_answering_instead_of_working_on() {
    let mut i = item("active");
    i.activity = "busy";
    let s = row_status(&i.row(), NOW, 0, false);
    assert_eq!((s.label.as_str(), s.tone), ("Not answering", Tone::Quiet));
    assert_eq!(s.description, "Its machine isn't answering. It was working when last heard.");
}

#[test]
fn sessionrowstatus_while_its_machine_isnt_answering_leaves_a_session_that_wasnt_working_as_it_was() {
    let s = row_status(&item("idle").row(), NOW, 0, false);
    assert_eq!(s.label, "5m");
}

#[test]
fn totimestamp_reads_unix_ms_and_iso_and_sqlite_strings() {
    assert_eq!(to_timestamp("1790000000000", 0), Some(1_790_000_000_000));
    assert_eq!(to_timestamp("2026-11-14T09:30:00Z", 0), Some(1_794_648_600_000));
    // SQLite writes `2026-11-14 09:30:00`, which is UTC.
    assert_eq!(to_timestamp("2026-11-14 09:30:00", 0), Some(1_794_648_600_000));
}

#[test]
fn totimestamp_treats_a_string_without_a_zone_as_utc_the_way_sqlite_writes_it() {
    assert_eq!(to_timestamp("2026-11-14 09:30:00", 7200), to_timestamp("2026-11-14T09:30:00Z", 0));
}

#[test]
fn totimestamp_reads_a_bare_date_as_that_days_local_midnight() {
    // Two hours east of UTC, local midnight is 22:00 UTC the day before.
    assert_eq!(to_timestamp("2026-11-14", 7200), to_timestamp("2026-11-13T22:00:00Z", 0));
    assert_eq!(to_timestamp("2026-11-14", 0), to_timestamp("2026-11-14T00:00:00Z", 0));
}

#[test]
fn totimestamp_gives_nothing_for_nothing_and_for_nonsense() {
    for text in ["", "  ", "soon", "0"] {
        assert_eq!(to_timestamp(text, 0), None, "{text:?}");
    }
    assert_eq!(from_ms(0), None);
    assert_eq!(from_ms(-5), None);
}

#[test]
fn formatrelative_compact_is_now_for_the_first_minute() {
    assert_eq!(format_relative(NOW - 59_999, NOW, 0), "now");
}

#[test]
fn formatrelative_compact_counts_minutes_from_60_seconds_rounding_down() {
    assert_eq!(format_relative(NOW - 60_000, NOW, 0), "1m");
    assert_eq!(format_relative(NOW - 119_999, NOW, 0), "1m");
}

#[test]
fn formatrelative_compact_counts_hours_from_60_minutes_up_to_23() {
    assert_eq!(format_relative(NOW - HOUR, NOW, 0), "1h");
    assert_eq!(format_relative(NOW - 23 * HOUR - 59 * MIN, NOW, 0), "23h");
}

#[test]
fn formatrelative_compact_counts_days_up_to_6_then_weeks_from_7_days() {
    assert_eq!(format_relative(NOW - 6 * DAY - 23 * HOUR, NOW, 0), "6d");
    assert_eq!(format_relative(NOW - 7 * DAY, NOW, 0), "1w");
}

#[test]
fn formatrelative_compact_is_a_date_from_eight_weeks_never_119w() {
    assert_eq!(format_relative(NOW - 56 * DAY, NOW, 0), "Aug 9");
    assert_eq!(format_relative(NOW - 119 * 7 * DAY, NOW, 0), "Jun 24, 2022");
}

#[test]
fn formatrelative_compact_writes_the_year_once_the_date_is_in_another_year() {
    // 12 January 2025: seventy days back is in 2024, so the year shows; ten weeks and a day is too.
    let now = 20_100 * DAY;
    assert_eq!(format_relative(now - 70 * DAY, now, 0), "Nov 3, 2024");
    assert_eq!(format_relative(now - 300 * DAY, now, 0), "Mar 18, 2024");
    // Within the same year there is none.
    assert_eq!(format_date(now - 8 * DAY, now, 0), "Jan 4");
}

#[test]
fn formatrelative_compact_reads_a_future_time_as_in_and_a_few_seconds_ahead_as_now() {
    assert_eq!(format_relative(NOW + 4 * MIN, NOW, 0), "in 4m");
    assert_eq!(format_relative(NOW + 30_000, NOW, 0), "now");
}

#[test]
fn the_title_is_trimmed_and_an_empty_one_reads_untitled_session() {
    assert_eq!(display_title(None, "  Fix the build "), "Fix the build");
    assert_eq!(display_title(None, "   "), "Untitled session");
    assert_eq!(display_title(Some("Review step"), "Fix"), "Review step");
}

#[test]
fn the_glyph_is_drawn_only_for_a_live_status_and_a_cut_off_idle_row_gets_the_dot() {
    let mut i = item("idle");
    assert_eq!(glyph(&i.row()), Glyph::Quiet);
    i.interrupted = Some(Interruption { at: iso(NOW), waiting: false, last_call: None, changed_files: None, auto: false });
    assert_eq!(glyph(&i.row()), Glyph::Cut);
    for (status, activity, want) in [
        ("active", "busy", Glyph::Working),
        ("active", "retry", Glyph::Retry),
        ("waiting_input", "idle", Glyph::Waiting),
        ("error", "idle", Glyph::Error),
        ("completed", "idle", Glyph::Quiet),
    ] {
        let mut i = item(status);
        i.activity = activity;
        assert_eq!(glyph(&i.row()), want, "{status}");
    }
}

// ─── `lib/__tests__/session-progress.test.ts` (the row's parts) and the chip's words ───────────────────────────────

mod progress {
    use crate::fleet::row_chips::*;
    use crate::fleet::row_status::Tone;
    use serde_json::json;

    #[test]
    fn session_progress_reads_a_row_summary() {
        let (id, p) = parse_summary(&json!({"sessionId": "s1", "kind": "todos", "done": 1, "total": 3, "current": "Next"})).unwrap();
        assert_eq!((id.as_str(), p.done, p.total, p.current.as_deref()), ("s1", 1, 3, Some("Next")));
        let (_, p) = parse_summary(&json!({"sessionId": "s1", "kind": "todos", "done": 0, "total": 0})).unwrap();
        assert_eq!(p.current, None);
    }

    #[test]
    fn session_progress_rejects_a_malformed_summary() {
        for v in [
            json!(null),
            json!("nope"),
            json!({"kind": "todos", "done": 1, "total": 3}),
            json!({"sessionId": "", "kind": "todos", "done": 1, "total": 3}),
            json!({"sessionId": "s1", "done": 1, "total": 3}),
            json!({"sessionId": "s1", "kind": "todos", "done": -1, "total": 3}),
            json!({"sessionId": "s1", "kind": "todos", "done": 1.5, "total": 3}),
        ] {
            assert!(parse_summary(&v).is_none(), "{v}");
        }
    }

    #[test]
    fn session_progress_compares_summaries_by_what_the_row_draws() {
        let a = Progress { kind: "todos".into(), done: 1, total: 3, current: Some("Next".into()) };
        assert!(same_progress(Some(&a), Some(&a.clone())));
        assert!(same_progress(None, None));
        assert!(!same_progress(Some(&a), None));
        for b in [
            Progress { done: 2, ..a.clone() },
            Progress { total: 4, ..a.clone() },
            Progress { current: None, ..a.clone() },
            Progress { kind: "plan".into(), ..a.clone() },
        ] {
            assert!(!same_progress(Some(&a), Some(&b)));
        }
    }

    #[test]
    fn the_ring_and_its_words() {
        assert_eq!(ring_state(1, 4), (0.25, false));
        assert_eq!(ring_state(5, 4), (1.0, true));
        assert_eq!(ring_state(0, 0), (0.0, false));
        let p = Progress { kind: "todos".into(), done: 3, total: 7, current: Some("Write it".into()) };
        assert_eq!(progress_description(&p), "3 of 7 done. Now: Write it");
        assert_eq!(progress_description(&Progress { current: None, ..p.clone() }), "3 of 7 done");
        assert_eq!(running_label(1), "1 thing running in the background");
        assert_eq!(running_label(3), "3 things running in the background");
        assert!(progress_view(Some(&Progress { done: 7, total: 7, ..p.clone() }), Tone::Quiet).is_none());
        assert!(progress_view(Some(&Progress { done: 7, total: 7, ..p }), Tone::Working).unwrap().count);
    }

    #[test]
    fn a_progress_event_changes_the_row() {
        use crate::fleet::rows::RowPatch;
        let patch = RowPatch::from_wire(
            &json!({"type": "session_progress", "properties": {"sessionId": "a", "kind": "todos", "done": 2, "total": 5, "current": null}}),
        )
        .unwrap();
        let mut project = crate::mock::rows::chips();
        project.sessions[0].id = "a".into();
        let mut projects = [project];
        patch.apply(&mut projects);
        assert_eq!(projects[0].sessions[0].row.progress.as_ref().map(|p| p.done), Some(2));
        assert!(RowPatch::from_wire(&json!({"type": "session_progress", "properties": {"sessionId": "a", "done": "many"}})).is_none());
    }
}
