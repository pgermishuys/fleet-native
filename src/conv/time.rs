//! The times a message shows (`lib/format-utils.ts`): relative ("2m ago") and absolute, in English as Fleet's
//! `Intl.DateTimeFormat("en-US")` writes them. No Rune in here. The caller gives the local offset from UTC, because
//! the standard library can't read the time zone and a crate for it is a decision of its own (checklist, row 2.13).

pub(crate) const DAY: i64 = 86_400_000;
pub(crate) const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// (year, month 0-11, day, hour 0-23, minute) of a unix-ms time `offset_secs` east of UTC.
pub(crate) fn civil(ms: i64, offset_secs: i64) -> (i64, usize, i64, i64, i64) {
    let local = ms + offset_secs * 1000;
    let days = local.div_euclid(DAY);
    let in_day = local.rem_euclid(DAY) / 1000;
    // Howard Hinnant's days-to-civil.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 2 } else { mp - 10 };
    let year = yoe + era * 400 + i64::from(month < 2);
    (year, month as usize, day, in_day / 3600, in_day % 3600 / 60)
}

pub(crate) fn clock(hour: i64, minute: i64) -> String {
    format!("{}:{minute:02} {}", if hour % 12 == 0 { 12 } else { hour % 12 }, if hour < 12 { "AM" } else { "PM" })
}

/// "2:34 PM" for today, "Mar 1, 2:34 PM" for another day (`formatTimestamp`); empty for no time.
pub fn format_timestamp(ms: i64, now: i64, offset_secs: i64) -> String {
    if ms == 0 {
        return String::new();
    }
    let (y, mo, d, h, mi) = civil(ms, offset_secs);
    let (ny, nmo, nd, ..) = civil(now, offset_secs);
    if (y, mo, d) == (ny, nmo, nd) { clock(h, mi) } else { format!("{} {d}, {}", MONTHS[mo], clock(h, mi)) }
}

/// "May 19, 2025, 11:16 PM": always with the date (`formatAbsoluteTimestamp`).
pub fn format_absolute_timestamp(ms: i64, offset_secs: i64) -> String {
    if ms == 0 {
        return String::new();
    }
    let (y, mo, d, h, mi) = civil(ms, offset_secs);
    format!("{} {d}, {y}, {}", MONTHS[mo], clock(h, mi))
}

/// "just now", "45s ago", "5m ago", "2h ago", then the timestamp (`formatRelativeTime`).
pub fn format_relative_time(ms: i64, now: i64, offset_secs: i64) -> String {
    let secs = (now - ms).div_euclid(1000);
    match secs {
        ..30 => "just now".to_string(),
        30..60 => format!("{secs}s ago"),
        60..3600 => format!("{}m ago", secs / 60),
        3600..86_400 => format!("{}h ago", secs / 3600),
        _ => format_timestamp(ms, now, offset_secs),
    }
}

/// Seconds east of UTC in the machine's zone, now. Unix reads it from the C library; Windows has no cheap equivalent in
/// the standard library, so there times show in UTC until the app takes a time crate (checklist, row 2.13).
pub fn local_offset_secs() -> i64 {
    #[cfg(unix)]
    {
        // SAFETY: `localtime_r` writes only into the `tm` it is given.
        unsafe {
            let now = libc::time(std::ptr::null_mut());
            let mut tm: libc::tm = std::mem::zeroed();
            if !libc::localtime_r(&now, &mut tm).is_null() {
                return tm.tm_gmtoff as i64;
            }
        }
    }
    0
}
