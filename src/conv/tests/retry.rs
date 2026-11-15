//! `lib/__tests__/turn-retry.test.ts` and `components/session/__tests__/TurnRetryBar.test.ts`, under the same names (the
//! bar's text is a function here, so each case reads it at a given time instead of mounting and advancing timers).
use crate::conv::retry::*;
use crate::fleet::rows::ScheduledRetry;
use crate::fleet::work::parse_iso_ms;
use serde_json::json;

// 7 October 2026, 01:51:43 UTC (the zone offset is passed, so these read the same everywhere).
const NOW: i64 = 1_791_337_903_000;

fn at(ms: i64) -> String {
    let (secs, millis) = (ms.div_euclid(1000), ms.rem_euclid(1000));
    let days = secs.div_euclid(86_400);
    let (h, mi, s) = (secs.rem_euclid(86_400) / 3600, secs.rem_euclid(3600) / 60, secs.rem_euclid(60));
    // Days since 1970-01-01 to a civil date, as `time.rs` does.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{millis:03}Z")
}

fn retry(due_in_ms: i64, attempt: i64, kind: &str, provider_said: bool) -> ScheduledRetry {
    ScheduledRetry {
        due_at: at(NOW + due_in_ms),
        attempt,
        kind: kind.into(),
        reason: "You've hit your session limit".into(),
        provider_said,
    }
}

fn usage(due_in_ms: i64) -> ScheduledRetry {
    retry(due_in_ms, 1, "usage_limit", true)
}

#[test]
fn the_helper_writes_times_fleet_can_read_back() {
    assert_eq!(parse_iso_ms(&at(NOW)), Some(NOW));
}

#[test]
fn limit_title_names_each_limit_and_nothing_else() {
    assert_eq!(limit_title(Some("usage_limit")), Some("Usage limit reached"));
    assert_eq!(limit_title(Some("rate_limit")), Some("Rate limited"));
    assert_eq!(limit_title(Some("overloaded")), Some("The model's provider is overloaded"));
    assert_eq!(limit_title(None), None);
    assert_eq!(limit_title(Some("APIError")), None);
}

#[test]
fn to_scheduled_retry_reads_the_servers_shape_and_refuses_anything_else() {
    let full = json!({ "dueAt": "2026-10-07T03:44:14Z", "attempt": 2, "kind": "rate_limit", "reason": "Too many requests", "providerSaid": false });
    let got = ScheduledRetry::from_wire(&full).unwrap();
    assert_eq!(
        got,
        ScheduledRetry {
            due_at: "2026-10-07T03:44:14Z".into(),
            attempt: 2,
            kind: "rate_limit".into(),
            reason: "Too many requests".into(),
            provider_said: false
        }
    );
    assert_eq!(ScheduledRetry::from_wire(&json!(null)), None);
    assert_eq!(ScheduledRetry::from_wire(&json!({ "dueAt": "soon", "kind": "rate_limit" })), None);
    assert_eq!(ScheduledRetry::from_wire(&json!({ "dueAt": "2026-10-07T03:44:14Z", "kind": "billing" })), None);
}

#[test]
fn format_retry_in_reads_each_case() {
    assert_eq!(format_countdown(0), "now");
    assert_eq!(format_countdown(-5), "now");
    assert_eq!(format_countdown(40_000), "in 40 s");
    assert_eq!(format_countdown(40_001), "in 41 s");
    assert_eq!(format_countdown(75_000), "in 2 min");
    assert_eq!(format_countdown(12 * 60_000), "in 12 min");
    assert_eq!(format_countdown(112 * 60_000), "in 1 h 52 min");
    assert_eq!(format_countdown(125 * 60_000), "in 2 h 05 min");
    assert_eq!(format_countdown(180 * 60_000), "in 3 h");
    assert_eq!(format_countdown(26 * 3_600_000), "in 1 d 02 h");
    assert_eq!(format_countdown(72 * 3_600_000), "in 3 d");
}

#[test]
fn format_retry_clock_writes_today_this_week_and_further_off() {
    // Same day: the clock. Within six days: the weekday (7 October 2026 is a Wednesday). Further: the date.
    assert_eq!(format_when(NOW + 112 * 60_000, NOW, 0), "3:43 AM");
    assert_eq!(format_when(NOW + 2 * 86_400_000, NOW, 0), "Fri 1:51 AM");
    assert_eq!(format_when(NOW + 9 * 86_400_000, NOW, 0), "Oct 16, 1:51 AM");
    assert_eq!(format_when(NOW + 100 * 86_400_000, NOW, 0), "Jan 15, 2027, 1:51 AM");
}

#[test]
fn says_when_fleet_tries_again_and_that_it_waits_for_the_limits_reset() {
    let bar = usage(112 * 60_000);
    let clock = format_when(NOW + 112 * 60_000, NOW, 0);
    assert_eq!(retry_when(&bar, NOW, 0), format!("Fleet tries again at {clock} \u{b7} in 1 h 52 min"));
    assert_eq!(retry_why(&bar), "It carries on when the limit resets. Nothing was lost.");
}

#[test]
fn counts_down_by_the_second_in_the_last_minute() {
    let bar = usage(75_000);
    assert!(retry_when(&bar, NOW, 0).contains("in 2 min"));
    assert_eq!(next_tick_ms(&bar, NOW), 15_000);
    assert!(retry_when(&bar, NOW + 30_000, 0).contains("in 45 s"));
    assert_eq!(next_tick_ms(&bar, NOW + 30_000), 1000);
    assert_eq!(retry_when(&bar, NOW + 76_000, 0), "Fleet is trying again now");
}

#[test]
fn says_which_attempt_it_is_when_the_limit_keeps_stopping_the_session() {
    assert_eq!(retry_why(&retry(60_000, 3, "rate_limit", false)), "Attempt 3, after a wait.");
    assert_eq!(retry_why(&retry(60_000, 2, "overloaded", true)), "Attempt 2, when the provider said to.");
}
