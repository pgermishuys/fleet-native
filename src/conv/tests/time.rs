//! `lib/format-utils.ts`: the relative and absolute times a message shows. Fleet formats in the machine's zone; these
//! cases pass the offset, so they read the same everywhere.
use crate::conv::time::*;

// 2025-05-19 23:16:00 UTC
const T: i64 = 1_747_696_560_000;

#[test]
fn says_just_now_then_seconds_minutes_and_hours() {
    assert_eq!(format_relative_time(T, T + 29_000, 0), "just now");
    assert_eq!(format_relative_time(T, T + 45_000, 0), "45s ago");
    assert_eq!(format_relative_time(T, T + 5 * 60_000, 0), "5m ago");
    assert_eq!(format_relative_time(T, T + 2 * 3_600_000 + 1, 0), "2h ago");
}

#[test]
fn falls_back_to_the_timestamp_after_a_day() {
    assert_eq!(format_relative_time(T, T + 25 * 3_600_000, 0), "May 19, 11:16 PM");
}

#[test]
fn writes_time_only_for_today_and_the_date_for_another_day() {
    assert_eq!(format_timestamp(T, T + 60_000, 0), "11:16 PM");
    assert_eq!(format_timestamp(T, T + 3_600_000, 0), "May 19, 11:16 PM");
    assert_eq!(format_timestamp(0, T, 0), "");
}

#[test]
fn writes_the_absolute_time_with_the_date_and_follows_the_offset() {
    assert_eq!(format_absolute_timestamp(T, 0), "May 19, 2025, 11:16 PM");
    assert_eq!(format_absolute_timestamp(T, 2 * 3600), "May 20, 2025, 1:16 AM");
    assert_eq!(format_absolute_timestamp(T - 11 * 3_600_000 - 16 * 60_000, 0), "May 19, 2025, 12:00 PM");
}
