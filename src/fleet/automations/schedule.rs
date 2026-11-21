//! When an automation runs, read out of the sentence that describes it ("every Monday at 9, summarise the open PRs")
//! (`lib/automation-schedule.ts`). No model is asked: a handful of patterns cover the phrases people use, and anything
//! else leaves the When chip asking. Times are on the user's clock; the automation stores that zone.
//!
//! Plain Rust. Where Fleet's code uses `Date` (the local clock), every function here takes the instant in unix ms and a
//! [`Tz`]: the seconds east of UTC at an instant. A test fixes it; the app passes `local_offset_at`. Day arithmetic is
//! done on local dates and the result looked up again, as `Date.setDate` does, so a run that crosses a daylight-saving
//! change keeps its local hour (a time inside the skipped hour moves forward, one in the repeated hour takes the first).
//!
//! Differences from the TypeScript, all in corners the parser can't reach in practice: `start` and `end` of a
//! [`ScheduleHit`] are byte offsets, not UTF-16 units; Rust's `regex` has no look-ahead, so `(?![\w'])` is `\b` plus a
//! check that no `'` follows (a match refused for that is not retried with a shorter day list); "every 0 hours" and
//! "every 0 minutes", which loop forever in the browser, have no next run.

use crate::conv::time::DAY;
use regex::{Captures, Regex};
use std::sync::LazyLock;

/// Seconds east of UTC at an instant (unix ms).
pub type Tz<'a> = &'a dyn Fn(i64) -> i64;

/// Hours and minutes, 24-hour.
pub type Hm = (i64, i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rel {
    Today,
    Tomorrow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum When {
    /// One run. `date` (YYYY-MM-DD) once it's fixed; before that, the next `day` of the week (0 = Sunday), or today or
    /// tomorrow.
    Once {
        t: Hm,
        day: Option<u8>,
        rel: Option<Rel>,
        date: Option<String>,
    },
    /// Days of the week (0 = Sunday). `ambiguous` when the sentence said "on Friday": every week, or just once?
    Weekly {
        days: Vec<u8>,
        t: Hm,
        ambiguous: bool,
    },
    Hours {
        n: i64,
    },
    Minutes {
        n: i64,
    },
    Monthly {
        dom: i64,
        t: Hm,
    },
    Cron {
        expr: String,
    },
    Event {
        event_type: String,
    },
}

impl When {
    fn once_day(day: u8, t: Hm) -> When {
        When::Once { t, day: Some(day), rel: None, date: None }
    }
    fn once_rel(rel: Rel, t: Hm) -> When {
        When::Once { t, day: None, rel: Some(rel), date: None }
    }
    fn weekly(days: Vec<u8>, t: Hm) -> When {
        When::Weekly { days, t, ambiguous: false }
    }
}

/// A schedule found in the text, and where (byte offsets), so the box can highlight it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleHit {
    pub when: When,
    pub start: usize,
    pub end: usize,
}

const DOW: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const DOW_LONG: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MON: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const EVERY_DAY: [u8; 7] = [0, 1, 2, 3, 4, 5, 6];
const WEEKDAYS: [u8; 5] = [1, 2, 3, 4, 5];

pub fn hm((h, m): Hm) -> String {
    format!("{h:02}:{m:02}")
}

fn ord(n: i64) -> String {
    let suffix = if n % 10 == 1 && n != 11 {
        "st"
    } else if n % 10 == 2 && n != 12 {
        "nd"
    } else if n % 10 == 3 && n != 13 {
        "rd"
    } else {
        "th"
    };
    format!("{n}{suffix}")
}

// ── The local clock ───────────────────────────────────────────────────────────

/// Days since 1970-01-01 of a civil date; `day` may run past the month's end and spills into the next, as `Date` does.
pub(crate) fn days_from_civil(year: i64, month0: i64, day: i64) -> i64 {
    let y = year + month0.div_euclid(12);
    let m = month0.rem_euclid(12) + 1;
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468 + day - 1
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let (y, m0, d, ..) = crate::conv::time::civil(days * DAY, 0);
    (y, m0 as i64, d)
}

/// A local wall-clock moment: the day (days since 1970-01-01), hour and minute.
#[derive(Clone, Copy)]
struct Local {
    day: i64,
    h: i64,
    m: i64,
}

impl Local {
    fn of(ms: i64, tz: Tz) -> Local {
        let local = ms + tz(ms) * 1000;
        let in_day = local.rem_euclid(DAY) / 60_000;
        Local { day: local.div_euclid(DAY), h: in_day / 60, m: in_day % 60 }
    }
    fn dow(self) -> u8 {
        (self.day + 4).rem_euclid(7) as u8
    }
    fn ymd(self) -> (i64, i64, i64) {
        civil_from_days(self.day)
    }
}

/// The instant of a local wall-clock time (`new Date(y, m, d, h, min)`): in the hour a spring change skips, the
/// offset from before the change (so 02:30 is 03:30); in the hour an autumn change repeats, the first one.
fn resolve(day: i64, h: i64, m: i64, tz: Tz) -> i64 {
    let wall = day * DAY + (h * 60 + m) * 60_000;
    let (o1, o2) = (tz(wall - DAY), tz(wall + DAY));
    let (c1, c2) = (wall - o1 * 1000, wall - o2 * 1000);
    match (tz(c1) == o1, tz(c2) == o2) {
        (true, true) => c1.min(c2),
        (false, true) => c2,
        _ => c1,
    }
}

/// `at(day, t)`: the local time `t` on the local date of `ms`, plus `plus_days` days.
fn at(ms: i64, t: Hm, plus_days: i64, tz: Tz) -> i64 {
    resolve(Local::of(ms, tz).day + plus_days, t.0, t.1, tz)
}

// ── The parser ────────────────────────────────────────────────────────────────

const DAY_RE: &str = "(sun(?:day)?|mon(?:day)?|tue(?:s(?:day)?)?|wed(?:nesday)?|thu(?:r(?:s(?:day)?)?)?|fri(?:day)?|sat(?:urday)?)";
const PART: &str = r"(?:\s+(?:in\s+the\s+)?(morning|afternoon|evening|night))?";
const TIME: &str =
    r"(?:\s*,?\s+(?:at\s+)?(noon|midnight|[0-9]{1,2}:[0-9]{2}\s*(?:am|pm)?|[0-9]{1,2}\s*(?:am|pm))|\s*,?\s+at\s+([0-9]{1,2}))?";
const LEAD: &str = r"(?:(?:that|which|to)\s+)?(?:runs?\s+)?";
/// Where Fleet has `(?![\w'])`: an ASCII word boundary, and an empty group `e` to read the next character at.
const EDGE: &str = r"(?-u:\b)(?P<e>)";
const B: &str = r"(?-u:\b)";

fn day_index(word: &str) -> u8 {
    match &word.to_lowercase()[..3] {
        "sun" => 0,
        "mon" => 1,
        "tue" => 2,
        "wed" => 3,
        "thu" => 4,
        "fri" => 5,
        _ => 6,
    }
}

fn part_time(part: &str) -> Hm {
    match part.to_lowercase().as_str() {
        "morning" => (9, 0),
        "afternoon" => (14, 0),
        "evening" => (18, 0),
        _ => (21, 0),
    }
}

static TIME_IN_TEXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([0-9]{1,2})(?::([0-9]{2}))?\s*(am|pm)?").unwrap());
static DAY_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!("(?i){DAY_RE}")).unwrap());

fn parse_time(text: Option<&str>) -> Option<Hm> {
    let t = text.filter(|t| !t.is_empty())?.to_lowercase();
    let t = t.trim();
    match t {
        "noon" => return Some((12, 0)),
        "midnight" => return Some((0, 0)),
        _ => {}
    }
    let m = TIME_IN_TEXT.captures(t)?;
    let mut h: i64 = m[1].parse().ok()?;
    let min: i64 = m.get(2).map_or(0, |x| x.as_str().parse().unwrap_or(0));
    match m.get(3).map(|x| x.as_str()) {
        Some("pm") if h < 12 => h += 12,
        Some("am") if h == 12 => h = 0,
        _ => {}
    }
    (h <= 23 && min <= 59).then_some((h, min))
}

fn time_from(part: Option<&str>, t1: Option<&str>, t2: Option<&str>) -> Hm {
    let t = t1.filter(|t| !t.is_empty()).or(t2);
    parse_time(t).or(part.filter(|p| !p.is_empty()).map(part_time)).unwrap_or((9, 0))
}

fn days_in(list: &str) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    for word in DAY_WORD.find_iter(list) {
        let i = day_index(word.as_str());
        if !out.contains(&i) {
            out.push(i);
        }
    }
    out.sort_unstable();
    out
}

/// The groups of a match, numbered as in Fleet's patterns: the empty `e` group is left out.
struct Groups<'t> {
    all: Vec<Option<&'t str>>,
}

impl<'t> Groups<'t> {
    fn g(&self, i: usize) -> Option<&'t str> {
        self.all.get(i).copied().flatten().filter(|s| !s.is_empty())
    }
    fn len(&self) -> usize {
        self.all.len()
    }
}

struct Pattern {
    re: Regex,
    make: fn(&Groups) -> When,
}

fn pattern(source: String, make: fn(&Groups) -> When) -> Pattern {
    Pattern { re: Regex::new(&format!("(?i){source}")).unwrap(), make }
}

// First match wins, so the specific phrases come before the general ones.
static PATTERNS: LazyLock<Vec<Pattern>> = LazyLock::new(|| {
    vec![
        pattern(
            format!(
                r"{B}{LEAD}(?:just\s+)?once\s+(?:on\s+)?(?:a\s+|the\s+|this\s+|next\s+|coming\s+)?(?:{DAY_RE}|(tomorrow|today)){EDGE}{PART}{TIME}"
            ),
            |m| {
                let t = time_from(m.g(3), m.g(4), m.g(5));
                match m.g(1) {
                    Some(d) => When::once_day(day_index(d), t),
                    None => {
                        When::once_rel(if m.g(2).is_some_and(|r| r.eq_ignore_ascii_case("today")) { Rel::Today } else { Rel::Tomorrow }, t)
                    }
                }
            },
        ),
        pattern(format!(r"{B}(?:next|this\s+coming)\s+{DAY_RE}{EDGE}{PART}{TIME}"), |m| {
            When::once_day(day_index(m.g(1).unwrap_or("mon")), time_from(m.g(2), m.g(3), m.g(4)))
        }),
        pattern(format!(r"{B}tomorrow{EDGE}{PART}{TIME}"), |m| When::once_rel(Rel::Tomorrow, time_from(m.g(1), m.g(2), m.g(3)))),
        pattern(format!(r"{B}{LEAD}every\s+([0-9]{{1,3}})\s*(minutes?|mins?|hours?|hrs?){B}"), |m| {
            let n = m.g(1).and_then(|n| n.parse().ok()).unwrap_or(1);
            if m.g(2).is_some_and(|u| u[..1].eq_ignore_ascii_case("m")) { When::Minutes { n } } else { When::Hours { n } }
        }),
        pattern(format!(r"{B}{LEAD}(?:every\s+hour|hourly){B}"), |_| When::Hours { n: 1 }),
        pattern(format!(r"{B}{LEAD}(?:(?:every|on)\s+)?week\s?days?{B}{PART}{TIME}"), |m| {
            When::weekly(WEEKDAYS.to_vec(), time_from(m.g(1), m.g(2), m.g(3)))
        }),
        pattern(
            format!(r"{B}{LEAD}(?:every|each)\s+({DAY_RE}s?(?:\s*(?:,|and|&)\s*{DAY_RE}s?)*){EDGE}{PART}{TIME}"),
            // The day list, then each DAY group, then PART and TIME: the last three captures.
            |m| {
                let n = m.len();
                When::weekly(days_in(m.g(1).unwrap_or("")), time_from(m.g(n - 3), m.g(n - 2), m.g(n - 1)))
            },
        ),
        pattern(format!(r"{B}(sundays|mondays|tuesdays|wednesdays|thursdays|fridays|saturdays){B}{PART}{TIME}"), |m| {
            When::weekly(vec![day_index(m.g(1).unwrap_or("mon"))], time_from(m.g(2), m.g(3), m.g(4)))
        }),
        pattern(format!(r"{B}{LEAD}(?:every\s+(day|morning|evening|night)|daily|each\s+day|nightly){B}{PART}{TIME}"), |m| {
            let whole = m.g(0).unwrap_or("");
            let part = m
                .g(2)
                .or(m.g(1).filter(|w| !w.eq_ignore_ascii_case("day")))
                .or(whole.to_lowercase().contains("nightly").then_some("night"));
            When::weekly(EVERY_DAY.to_vec(), time_from(part, m.g(3), m.g(4)))
        }),
        pattern(
            format!(
                r"{B}{LEAD}(?:on\s+the\s+([0-9]{{1,2}})(?:st|nd|rd|th)?\s+of\s+(?:every|each)\s+month|(?:every|each)\s+month(?:\s+on\s+the\s+([0-9]{{1,2}})(?:st|nd|rd|th)?)?|monthly){TIME}"
            ),
            |m| {
                let dom = m.g(1).or(m.g(2)).and_then(|d| d.parse::<i64>().ok()).unwrap_or(1);
                When::Monthly { dom: dom.clamp(1, 28), t: time_from(None, m.g(3), m.g(4)) }
            },
        ),
        pattern(format!(r"{B}{LEAD}on\s+(?:a\s+)?{DAY_RE}{EDGE}{PART}{TIME}"), |m| {
            // "on a Monday" means any Monday; "on Friday" could be this Friday or every Friday.
            let on_a = ON_A.is_match(m.g(0).unwrap_or(""));
            When::Weekly { days: vec![day_index(m.g(1).unwrap_or("mon"))], t: time_from(m.g(2), m.g(3), m.g(4)), ambiguous: !on_a }
        }),
    ]
});

fn groups<'t>(re: &Regex, caps: &Captures<'t>) -> Groups<'t> {
    let skip = re.capture_names().position(|n| n == Some("e"));
    Groups { all: (0..caps.len()).filter(|i| Some(*i) != skip).map(|i| caps.get(i).map(|m| m.as_str())).collect() }
}

/// The schedule in a sentence, or None when there's none it recognises.
pub fn parse_schedule(text: &str) -> Option<ScheduleHit> {
    for Pattern { re, make } in PATTERNS.iter() {
        let mut from = 0;
        while from <= text.len() {
            let Some(caps) = re.captures_at(text, from) else { break };
            let whole = caps.get(0).unwrap();
            // The look-ahead's `'`: "Monday's" is not the day. The edge is the end of the day word.
            if let Some(e) = caps.name("e")
                && text[e.end()..].starts_with('\'')
            {
                from = whole.start() + text[whole.start()..].chars().next().map_or(1, char::len_utf8);
                continue;
            }
            return Some(ScheduleHit { when: make(&groups(re, &caps)), start: whole.start(), end: whole.end() });
        }
    }
    None
}

static ON_A: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)on\s+a\s+").unwrap());
static CREATE_AN_AUTOMATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:please\s+)?(?:can\s+you\s+)?(?:create|make|set\s*up|add|schedule)\s+(?:me\s+)?(?:an?\s+)?(?:automation|job|task)\b\s*(?:that|which|to|for)?\s*")
        .unwrap()
});
static MANY_SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]{2,}").unwrap());
static SPACE_BEFORE_PUNCT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]+([,.;:])").unwrap());
static LEADING_JUNK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\s,;:.\-–]+").unwrap());
static LEADING_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(?:to|and|then)\s+").unwrap());
static TRAILING_JUNK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\s,;:\-–]+$").unwrap());

/// What the agent is asked to do: the text without the schedule words, and without "create an automation that…",
/// which is addressed to Fleet rather than the agent.
pub fn prompt_from(text: &str, hit: Option<&ScheduleHit>) -> String {
    let without_schedule = match hit {
        Some(h) => format!("{} {}", &text[..h.start], &text[h.end..]),
        None => text.to_string(),
    };
    let s = CREATE_AN_AUTOMATION.replace(&without_schedule, "").into_owned();
    // Nothing was taken out: the prompt is the text as written.
    if hit.is_none() && s == without_schedule {
        return text.trim().to_string();
    }
    let s = MANY_SPACES.replace_all(&s, " ");
    let s = SPACE_BEFORE_PUNCT.replace_all(&s, "$1");
    let s = LEADING_JUNK.replace(&s, "");
    let s = LEADING_WORD.replace(&s, "");
    let s = TRAILING_JUNK.replace(&s, "");
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

static FIRST_SENTENCE_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[.,:;!?](?:\s|$)(?s:.*)$").unwrap());
static WHITESPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// A name from the prompt's first few words, for when none is given.
pub fn auto_name(prompt: &str) -> String {
    if prompt.is_empty() {
        return String::new();
    }
    let head = FIRST_SENTENCE_END.replace(prompt, "");
    let words = WHITESPACE.split(&head).take(6).collect::<Vec<_>>().join(" ");
    if words.chars().count() > 42 { format!("{}…", words.chars().take(40).collect::<String>().trim_end()) } else { words }
}

/// The same schedule at another time of day; a weekly one also stops asking "every week, or once?".
pub fn with_time(when: &When, t: Hm) -> When {
    match when {
        When::Once { day, rel, date, .. } => When::Once { t, day: *day, rel: *rel, date: date.clone() },
        When::Weekly { days, .. } => When::Weekly { days: days.clone(), t, ambiguous: false },
        When::Monthly { dom, .. } => When::Monthly { dom: *dom, t },
        other => other.clone(),
    }
}

// ── Dates ─────────────────────────────────────────────────────────────────────

/// The date part of a local instant as YYYY-MM-DD.
pub fn local_date(ms: i64, tz: Tz) -> String {
    let (y, m0, d) = Local::of(ms, tz).ymd();
    format!("{y:04}-{:02}-{d:02}", m0 + 1)
}

/// The local weekday of an instant (0 = Sunday), as `getDay()`.
pub fn weekday(ms: i64, tz: Tz) -> u8 {
    Local::of(ms, tz).dow()
}

/// The local date after the one `ms` falls on, as YYYY-MM-DD (`setDate(getDate() + 1)`): by the calendar, not 24 hours on,
/// so a day a clock change makes 23 or 25 hours long is still one day.
pub fn tomorrow(ms: i64, tz: Tz) -> String {
    let (y, m0, d) = civil_from_days(Local::of(ms, tz).day + 1);
    format!("{y:04}-{:02}-{d:02}", m0 + 1)
}

fn day_date(ms: i64, tz: Tz) -> String {
    let l = Local::of(ms, tz);
    let (_, m0, d) = l.ymd();
    format!("{} {d} {}", DOW[l.dow() as usize], MON[m0 as usize])
}

/// When it runs next after `from`, on this clock; None for cron (the server knows) and events.
pub fn next_run(when: &When, from: i64, tz: Tz) -> Option<i64> {
    match when {
        When::Once { t, day, rel, date } => {
            if let Some(date) = date {
                let mut parts = date.split('-').map(|p| p.parse::<i64>().ok());
                let (y, mo, d) = (parts.next()??, parts.next()??, parts.next()??);
                return Some(resolve(days_from_civil(y, mo - 1, d), t.0, t.1, tz));
            }
            let today = at(from, *t, 0, tz);
            Some(match rel {
                Some(Rel::Tomorrow) => at(from, *t, 1, tz),
                Some(Rel::Today) => {
                    if today <= from {
                        at(from, *t, 1, tz)
                    } else {
                        today
                    }
                }
                None => {
                    let dow = Local::of(today, tz).dow() as i64;
                    let ahead = (day.unwrap_or(1) as i64 - dow + 7) % 7;
                    let d = at(from, *t, ahead, tz);
                    if d <= from { at(from, *t, ahead + 7, tz) } else { d }
                }
            })
        }
        When::Weekly { days, t, .. } => (0..8).map(|i| at(from, *t, i, tz)).find(|d| days.contains(&Local::of(*d, tz).dow()) && *d > from),
        When::Hours { n } => {
            if *n <= 0 {
                return None;
            }
            let l = Local::of(from, tz);
            let mut d = resolve(l.day, l.h, 0, tz);
            loop {
                let l = Local::of(d, tz);
                d = resolve(l.day, l.h + 1, 0, tz);
                if Local::of(d, tz).h % n == 0 {
                    return Some(d);
                }
            }
        }
        When::Minutes { n } => {
            if *n <= 0 {
                return None;
            }
            let mut d = from - from.rem_euclid(60_000);
            loop {
                d += 60_000;
                if Local::of(d, tz).m % n == 0 {
                    return Some(d);
                }
            }
        }
        When::Monthly { dom, t } => {
            let (y, m0, _) = Local::of(from, tz).ymd();
            let d = resolve(days_from_civil(y, m0, *dom), t.0, t.1, tz);
            Some(if d <= from { resolve(days_from_civil(y, m0 + 1, *dom), t.0, t.1, tz) } else { d })
        }
        _ => None,
    }
}

/// The cron expression for a repeating schedule; None for a one-off or an event.
pub fn cron_of(when: &When) -> Option<String> {
    Some(match when {
        When::Weekly { days, t, .. } => {
            let days = if days.len() == 7 {
                "*".to_string()
            } else if days.as_slice() == WEEKDAYS {
                "1-5".to_string()
            } else {
                days.iter().map(u8::to_string).collect::<Vec<_>>().join(",")
            };
            format!("{} {} * * {days}", t.1, t.0)
        }
        When::Hours { n: 1 } => "0 * * * *".into(),
        When::Hours { n } => format!("0 */{n} * * *"),
        When::Minutes { n: 1 } => "* * * * *".into(),
        When::Minutes { n } => format!("*/{n} * * * *"),
        When::Monthly { dom, t } => format!("{} {} {dom} * *", t.1, t.0),
        When::Cron { expr } => expr.clone(),
        _ => return None,
    })
}

/// The trigger the server stores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    pub trigger_type: String,
    pub trigger_config: String,
}

/// The trigger the server stores. A one-off is fixed to a date here, so it means the same thing tomorrow.
pub fn to_trigger(when: &When, from: i64, tz: Tz) -> Option<Trigger> {
    let (trigger_type, trigger_config) = match when {
        When::Event { event_type } => ("event", serde_json::json!({ "eventType": event_type }).to_string()),
        When::Once { .. } => {
            let d = next_run(when, from, tz)?;
            let l = Local::of(d, tz);
            ("once", format!("{}T{}", local_date(d, tz), hm((l.h, l.m))))
        }
        _ => ("schedule", cron_of(when)?),
    };
    Some(Trigger { trigger_type: trigger_type.into(), trigger_config })
}

static ONCE_CONFIG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([0-9]{4}-[0-9]{2}-[0-9]{2})T([0-9]{2}):([0-9]{2})$").unwrap());
static EVERY_HOURS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\*/([0-9]{1,2})$").unwrap());

fn is_num(s: Option<&str>) -> bool {
    s.is_some_and(|s| (1..=2).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit()))
}

/// A stored trigger as a When: the cron shapes the composer writes come back as the schedule they were.
pub fn from_trigger(trigger_type: &str, trigger_config: &str) -> Option<When> {
    match trigger_type {
        "event" => {
            let parsed: serde_json::Value = serde_json::from_str(trigger_config).ok()?;
            return Some(When::Event { event_type: parsed.get("eventType")?.as_str()?.to_string() });
        }
        "once" => {
            let m = ONCE_CONFIG.captures(trigger_config.trim())?;
            let num = |i: usize| m[i].parse::<i64>().ok();
            return Some(When::Once { t: (num(2)?, num(3)?), day: None, rel: None, date: Some(m[1].to_string()) });
        }
        "schedule" => {}
        _ => return None,
    }

    let expr = trigger_config.trim();
    let cron = || Some(When::Cron { expr: expr.to_string() });
    let parts: Vec<&str> = if expr.is_empty() { vec![""] } else { expr.split_whitespace().collect() };
    let p = |i: usize| parts.get(i).copied();
    let (min, hour, dom, month, dow) = (p(0), p(1), p(2), p(3), p(4));
    if parts.len() > 5 || month != Some("*") {
        return cron();
    }
    if (min, hour, dom, dow) == (Some("*"), Some("*"), Some("*"), Some("*")) {
        return Some(When::Minutes { n: 1 });
    }
    if (min, hour, dom, dow) == (Some("0"), Some("*"), Some("*"), Some("*")) {
        return Some(When::Hours { n: 1 });
    }
    let every = |s: Option<&str>| EVERY_HOURS.captures(s.unwrap_or("")).and_then(|c| c[1].parse::<i64>().ok());
    if min == Some("0")
        && dom == Some("*")
        && dow == Some("*")
        && let Some(n) = every(hour)
    {
        return Some(When::Hours { n });
    }
    if hour == Some("*")
        && dom == Some("*")
        && dow == Some("*")
        && let Some(n) = every(min)
    {
        return Some(When::Minutes { n });
    }
    if !is_num(min) || !is_num(hour) {
        return cron();
    }

    let t = (hour?.parse().ok()?, min?.parse().ok()?);
    if dom == Some("*") && dow == Some("*") {
        return Some(When::weekly(EVERY_DAY.to_vec(), t));
    }
    if dom == Some("*") && dow == Some("1-5") {
        return Some(When::weekly(WEEKDAYS.to_vec(), t));
    }
    if let (Some("*"), Some(list)) = (dom, dow) {
        let digits: Vec<u8> = list.split(',').filter_map(|d| (d.len() == 1).then(|| d.bytes().next()).flatten()).collect();
        if digits.len() == list.split(',').count() && digits.iter().all(|d| (b'0'..=b'6').contains(d)) {
            let mut days: Vec<u8> = digits.iter().map(|d| d - b'0').collect();
            days.sort_unstable();
            days.dedup();
            return Some(When::weekly(days, t));
        }
    }
    if is_num(dom) && dow == Some("*") {
        return Some(When::Monthly { dom: dom?.parse().ok()?, t });
    }
    cron()
}

// ── Words ─────────────────────────────────────────────────────────────────────

fn days_label(days: &[u8]) -> String {
    if days.len() == 7 {
        return "Every day".into();
    }
    if days == WEEKDAYS {
        return "Weekdays".into();
    }
    if let [d] = days {
        return format!("{}s", DOW_LONG[*d as usize]);
    }
    let names: Vec<&str> = days.iter().map(|d| DOW[*d as usize]).collect();
    match names.split_last() {
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    }
}

/// The When chip: "Mondays 09:00", "Once · Mon 21 Sep", "When?".
pub fn when_chip(when: Option<&When>, from: i64, tz: Tz) -> String {
    let Some(when) = when else { return "When?".into() };
    match when {
        When::Once { .. } => format!("Once · {}", next_run(when, from, tz).map(|d| day_date(d, tz)).unwrap_or_default()),
        When::Weekly { days, t, .. } => format!("{} {}", days_label(days), hm(*t)),
        When::Hours { n: 1 } => "Every hour".into(),
        When::Hours { n } => format!("Every {n} hours"),
        When::Minutes { n: 1 } => "Every minute".into(),
        When::Minutes { n } => format!("Every {n} min"),
        When::Monthly { dom, .. } => format!("Monthly, {}", ord(*dom)),
        When::Cron { expr } => expr.clone(),
        When::Event { .. } => "On an event".into(),
    }
}

/// The schedule in a sentence: "every Monday at 09:00", "once, on Mon 21 Sep at 09:00".
pub fn when_sentence(when: &When, from: i64, tz: Tz) -> String {
    match when {
        When::Once { t, .. } => format!("once, on {} at {}", next_run(when, from, tz).map(|d| day_date(d, tz)).unwrap_or_default(), hm(*t)),
        When::Weekly { days, t, .. } => {
            let words = if days.len() == 7 {
                "every day".to_string()
            } else if days.as_slice() == WEEKDAYS {
                "on weekdays".to_string()
            } else if let [d] = days.as_slice() {
                format!("every {}", DOW_LONG[*d as usize])
            } else {
                format!("every {}", days_label(days))
            };
            format!("{words} at {}", hm(*t))
        }
        When::Hours { n: 1 } => "every hour, on the hour".into(),
        When::Hours { n } => format!("every {n} hours, on the hour"),
        When::Minutes { n: 1 } => "every minute".into(),
        When::Minutes { n } => format!("every {n} minutes"),
        When::Monthly { dom, t } => format!("on the {} of every month at {}", ord(*dom), hm(*t)),
        When::Cron { expr } => format!("on the cron schedule {expr}"),
        When::Event { .. } => "when the event happens".into(),
    }
}

/// The weekday a "weekly" or dated "once" schedule falls on, in words, for "Every Friday instead".
pub fn day_name(day: u8) -> &'static str {
    DOW_LONG[day as usize % 7]
}

/// "Mon 21 Sep, 09:00".
pub fn describe_date(ms: i64, tz: Tz) -> String {
    let l = Local::of(ms, tz);
    format!("{}, {}", day_date(ms, tz), hm((l.h, l.m)))
}

/// A sidebar row's next run: "09:00" today, "Mon 09:00" this week, "21 Sep" later.
pub fn next_short(d: i64, from: i64, tz: Tz) -> String {
    let (ld, lf) = (Local::of(d, tz), Local::of(from, tz));
    let days = ld.day - lf.day;
    let time = hm((ld.h, ld.m));
    if days <= 0 {
        time
    } else if days < 7 {
        format!("{} {time}", DOW[ld.dow() as usize])
    } else {
        let (_, m0, dom) = ld.ymd();
        format!("{dom} {}", MON[m0 as usize])
    }
}
