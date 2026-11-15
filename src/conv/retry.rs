//! The words for a turn a model provider's limit stopped and for when Fleet tries it again (`lib/turn-retry.ts`,
//! `TurnRetryBar.vue`, and the two formatters it calls in `lib/format/`). No Rune in here.
use super::time::{DAY, MONTHS, civil, clock};
use crate::fleet::rows::ScheduledRetry;
use crate::fleet::work::parse_iso_ms;

/// The failure card's title for a turn a limit stopped (`limitTitle`); `None` for any other failure.
pub fn limit_title(kind: Option<&str>) -> Option<&'static str> {
    match kind? {
        "usage_limit" => Some("Usage limit reached"),
        "rate_limit" => Some("Rate limited"),
        "overloaded" => Some("The model's provider is overloaded"),
        _ => None,
    }
}

/// `formatDuration(ms, { precision: "minutes" })` from a minute up: "3 min", "2 h 05 min", "2 d 03 h".
fn minutes_text(ms: i64) -> String {
    let minutes = ms.max(0) / 60_000;
    let (hours, days) = (minutes / 60, minutes / 1440);
    if minutes < 60 {
        format!("{minutes} min")
    } else if hours < 24 {
        if minutes % 60 == 0 { format!("{hours} h") } else { format!("{hours} h {:02} min", minutes % 60) }
    } else if hours % 24 == 0 {
        format!("{days} d")
    } else {
        format!("{days} d {:02} h", hours % 24)
    }
}

/// How long until something is due, rounded up so it never reads early: "in 12 s", "in 3 min", "in 2 h 05 min";
/// "now" once it's due (`formatCountdown`).
pub fn format_countdown(left_ms: i64) -> String {
    if left_ms <= 0 {
        "now".into()
    } else if left_ms < 59_000 {
        format!("in {} s", (left_ms + 999) / 1000)
    } else {
        format!("in {}", minutes_text((left_ms + 59_999) / 60_000 * 60_000))
    }
}

/// A time near now, as short as stays clear: the clock today ("2:05 PM"), with the weekday within six days either way
/// ("Mon 2:05 PM"), else the day and time ("Oct 12, 2:05 PM", with the year when it isn't this year) (`formatWhen`, in
/// Fleet's en-US).
pub fn format_when(at: i64, now: i64, offset_secs: i64) -> String {
    let (y, mo, d, h, mi) = civil(at, offset_secs);
    let (ny, nmo, nd, ..) = civil(now, offset_secs);
    if (y, mo, d) == (ny, nmo, nd) {
        return clock(h, mi);
    }
    if (at - now).abs() < 6 * DAY {
        // 1 January 1970 was a Thursday.
        let day = (at + offset_secs * 1000).div_euclid(DAY);
        let weekday = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"][day.rem_euclid(7) as usize];
        return format!("{weekday} {}", clock(h, mi));
    }
    if y == ny { format!("{} {d}, {}", MONTHS[mo], clock(h, mi)) } else { format!("{} {d}, {y}, {}", MONTHS[mo], clock(h, mi)) }
}

/// Line 1 of the bar: "Fleet tries again at 2:05 PM \u{b7} in 2 min", or "Fleet is trying again now" once it's due.
pub fn retry_when(retry: &ScheduledRetry, now: i64, offset_secs: i64) -> String {
    let due = parse_iso_ms(&retry.due_at).unwrap_or(now);
    let left = format_countdown(due - now);
    if left == "now" {
        "Fleet is trying again now".into()
    } else {
        format!("Fleet tries again at {} \u{b7} {left}", format_when(due, now, offset_secs))
    }
}

/// Line 2 of the bar (`why`): which attempt it is, or that it carries on and nothing was lost.
pub fn retry_why(retry: &ScheduledRetry) -> String {
    let source = match (retry.provider_said, retry.kind.as_str()) {
        (true, "usage_limit") => "when the limit resets",
        (true, _) => "when the provider said to",
        (false, _) => "after a wait",
    };
    if retry.attempt > 1 { format!("Attempt {}, {source}.", retry.attempt) } else { format!("It carries on {source}. Nothing was lost.") }
}

/// Milliseconds until the bar's text changes: every second in the last minute before it is due, else every 15 s
/// (`tick`).
pub fn next_tick_ms(retry: &ScheduledRetry, now: i64) -> i64 {
    let left = parse_iso_ms(&retry.due_at).map_or(0, |due| due - now);
    if left > 0 && left <= 60_000 { 1000 } else { 15_000 }
}
