//! The words the machine menu says (`formatRelative`, `formatDateTime`, `OS_NAMES`; rows B5.8-B5.12), checked without a window.
use super::machines::{format_date_time, format_relative, os_name};

const NOW: i64 = 1_792_000_000_000; // 2026-10-14 in UTC
const MIN: i64 = 60_000;
const HOUR: i64 = 60 * MIN;
const DAY: i64 = 24 * HOUR;

#[test]
fn format_relative_says_just_now_under_a_minute() {
    assert_eq!(format_relative(NOW - 59_000, NOW, 0), "just now");
}

#[test]
fn format_relative_counts_whole_minutes_hours_days_and_weeks() {
    assert_eq!(format_relative(NOW - MIN, NOW, 0), "1 minute ago");
    assert_eq!(format_relative(NOW - 3 * MIN - 59_000, NOW, 0), "3 minutes ago");
    assert_eq!(format_relative(NOW - 2 * HOUR, NOW, 0), "2 hours ago");
    assert_eq!(format_relative(NOW - DAY, NOW, 0), "1 day ago");
    assert_eq!(format_relative(NOW - 6 * DAY, NOW, 0), "6 days ago");
    assert_eq!(format_relative(NOW - 7 * DAY, NOW, 0), "1 week ago");
    assert_eq!(format_relative(NOW - 55 * DAY, NOW, 0), "7 weeks ago");
}

#[test]
fn format_relative_switches_to_a_date_at_eight_weeks() {
    assert_eq!(format_relative(NOW - 56 * DAY, NOW, 0), "Aug 19");
    assert_eq!(format_relative(NOW - 400 * DAY, NOW, 0), "Sep 9, 2025");
}

#[test]
fn format_relative_reads_a_time_ahead_as_in() {
    assert_eq!(format_relative(NOW + 4 * MIN, NOW, 0), "in 4 minutes");
}

#[test]
fn format_date_time_writes_the_day_and_the_time_with_the_year_when_it_isnt_this_one() {
    assert_eq!(format_date_time(NOW - 2 * DAY, NOW, 0), "Oct 12, 5:46 PM");
    assert_eq!(format_date_time(NOW - 400 * DAY, NOW, 0), "Sep 9, 2025, 5:46 PM");
}

#[test]
fn os_names_are_the_three_systems_and_nothing_else() {
    assert_eq!(
        (os_name("linux"), os_name("macos"), os_name("windows"), os_name("haiku")),
        (Some("Linux"), Some("macOS"), Some("Windows"), None)
    );
}
