//! `RelativeTime` (`components/ui/relative-time`), in its compact form: "now", "5m", "2h", "3d", with the exact moment as
//! the tooltip. The formatting is `fleet::row_status`'s (the session list shares it); this reads Fleet's text times
//! (`createdAt`) and gives the two strings a view needs. Built here once so component 6 and the rest can use it.
use crate::fleet::row_status::{format_date_time, format_relative, to_timestamp};

/// The compact age of `at` (an ISO or SQLite time, or unix ms as text) against `now`; empty for no time.
pub fn compact(at: &str, now: i64, offset_secs: i64) -> String {
    to_timestamp(at, offset_secs).map(|ms| format_relative(ms, now, offset_secs)).unwrap_or_default()
}

/// `RelativeTime` without `compact`: "just now", "3 hours ago", "in 4 minutes"; a date from eight weeks on, as the compact form.
pub fn long(at: &str, now: i64, offset_secs: i64) -> String {
    let Some(ms) = to_timestamp(at, offset_secs) else { return String::new() };
    let (diff, minute) = (now - ms, 60_000);
    let span = diff.abs();
    if span < minute {
        return "just now".into();
    }
    if span >= 8 * 7 * 24 * 60 * minute {
        return format_relative(ms, now, offset_secs);
    }
    let (value, unit) = if span < 60 * minute {
        (span / minute, "minute")
    } else if span < 24 * 60 * minute {
        (span / (60 * minute), "hour")
    } else if span < 7 * 24 * 60 * minute {
        (span / (24 * 60 * minute), "day")
    } else {
        (span / (7 * 24 * 60 * minute), "week")
    };
    // `Intl.RelativeTimeFormat` with `numeric: "auto"`: one day or week away is a word of its own.
    match (unit, value, diff >= 0) {
        ("day", 1, true) => return "yesterday".into(),
        ("day", 1, false) => return "tomorrow".into(),
        ("week", 1, true) => return "last week".into(),
        ("week", 1, false) => return "next week".into(),
        _ => {}
    }
    let words = format!("{value} {unit}{}", if value == 1 { "" } else { "s" });
    if diff >= 0 { format!("{words} ago") } else { format!("in {words}") }
}

/// The tooltip: "Oct 12, 2:07 PM"; empty for no time.
pub fn exact(at: &str, now: i64, offset_secs: i64) -> String {
    to_timestamp(at, offset_secs).map(|ms| format_date_time(ms, now, offset_secs)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_792_065_600_000; // 2026-10-15T12:00:00Z

    #[test]
    fn reads_the_long_form() {
        assert_eq!(long("2026-10-15T11:59:30Z", NOW, 0), "just now");
        assert_eq!(long("2026-10-15T11:59:00Z", NOW, 0), "1 minute ago");
        assert_eq!(long("2026-10-15T10:00:00Z", NOW, 0), "2 hours ago");
        assert_eq!(long("2026-10-12 12:00:00", NOW, 0), "3 days ago");
        assert_eq!(long("2026-10-14 12:00:00", NOW, 0), "yesterday");
        assert_eq!(long("2026-10-08 12:00:00", NOW, 0), "last week");
        assert_eq!(long("2026-10-16 12:00:00", NOW, 0), "tomorrow");
        assert_eq!(long("2026-10-15T12:04:00Z", NOW, 0), "in 4 minutes");
    }

    #[test]
    fn reads_fleets_text_times_compactly() {
        assert_eq!(compact("2026-10-15T11:59:30Z", NOW, 0), "now");
        assert_eq!(compact("2026-10-15T11:55:00Z", NOW, 0), "5m");
        assert_eq!(compact("2026-10-15T10:00:00Z", NOW, 0), "2h");
        assert_eq!(compact("2026-10-12 12:00:00", NOW, 0), "3d");
        assert_eq!(compact("2026-10-01T12:00:00Z", NOW, 0), "2w");
    }

    #[test]
    fn is_empty_for_no_time_and_names_the_moment_in_the_tooltip() {
        assert_eq!(compact("", NOW, 0), "");
        assert_eq!(compact("soon", NOW, 0), "");
        assert_eq!(exact("2026-10-15T10:07:00Z", NOW, 0), "Oct 15, 10:07 AM");
    }
}
