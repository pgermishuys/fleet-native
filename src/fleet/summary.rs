//! The home page's summary bar (`SummaryBar.vue`, `use-fleet-summary.ts`, `use-analytics-daily.ts`): the fleet's counts from
//! `GET /api/fleet/summary` every 30 s and the last fourteen days' spend from `GET /api/analytics/daily` every 5 minutes, while
//! the page is shown. Numbered requests drop an answer a newer one overtook; a failure keeps what was held. The pure parts
//! are at the top; `State`'s methods at the end run the reads on threads. With no server (mock data) the numbers come
//! from the invented sessions, so scenes and tests come out the same every time.
use super::groups::Listed;
use super::machine::Machine;
use super::settings::HOME_MACHINE_KEY;
use crate::mock::Session;
use crate::{State, Tx};
use serde_json::Value;

/// `SPARK_DAYS`: the bars, one per day, today last.
pub const SPARK_DAYS: usize = 14;
/// `DEFAULT_POLL_INTERVAL_MS` of the summary.
pub const SUMMARY_SECS: f64 = 30.0;
/// The spend's own poll (`5 * 60_000`).
pub const DAILY_SECS: f64 = 300.0;
/// The height the tallest bar fills (`.summary-bar__spark`).
pub const SPARK_H: f32 = 36.0;
const DAY_MS: i64 = 86_400_000;

/// `FleetSummaryResponse`: what the server counts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub active: u32,
    pub idle: u32,
    pub queued: u32,
    pub tokens: f64,
    pub cost: f64,
}

/// A body with none of the five numbers is no answer; a missing one is zero (`current?.queuedTasks ?? 0`).
pub fn parse_summary(body: &Value) -> Option<Summary> {
    let obj = body.as_object()?;
    let n = |k: &str| obj.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    let count = |k: &str| n(k).max(0.0) as u32;
    Some(Summary {
        active: count("activeSessions"),
        idle: count("idleSessions"),
        queued: count("queuedTasks"),
        tokens: n("totalTokens"),
        cost: n("totalCost"),
    })
}

/// One day of `DailyAnalytics`: its date, "2026-11-16", and what it cost.
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub date: String,
    pub cost: f64,
}

pub fn parse_daily(body: &Value) -> Option<Vec<Point>> {
    Some(super::analytics::parse_days(body)?.into_iter().map(|d| Point { date: d.date, cost: d.cost }).collect())
}

/// "2026-11-16" for a day counted from 1970-01-01 (`isoDay`, which Fleet takes in UTC).
pub fn iso_day(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The request's query: the last `SPARK_DAYS` days up to today.
pub fn daily_path(now_ms: i64) -> String {
    let today = now_ms.div_euclid(DAY_MS);
    format!("/api/analytics/daily?from={}&to={}", iso_day(today - (SPARK_DAYS as i64 - 1)), iso_day(today))
}

/// One bar: the day, and what it cost.
#[derive(Clone, Debug, PartialEq)]
pub struct Day {
    pub key: String,
    pub cost: f64,
}

/// One bar per day, the days with no activity filled in (`days`).
pub fn days(daily: &[Point], now_ms: i64) -> Vec<Day> {
    let today = now_ms.div_euclid(DAY_MS);
    (0..SPARK_DAYS as i64)
        .map(|i| {
            let key = iso_day(today - (SPARK_DAYS as i64 - 1) + i);
            let cost = daily.iter().find(|p| p.date == key).map_or(0.0, |p| p.cost);
            Day { key, cost }
        })
        .collect()
}

/// `barHeight` as pixels of `SPARK_H`: 2 with nothing to show, else at least 8% and at most the whole.
pub fn bar_height(cost: f64, max: f64) -> f32 {
    if max <= 0.0 || cost <= 0.0 {
        return 2.0;
    }
    SPARK_H * ((cost / max * 100.0).max(8.0) / 100.0) as f32
}

/// The sum of the bars (`spendTotal`) and the tallest one (`spendMax`).
pub fn totals(days: &[Day]) -> (f64, f64) {
    (days.iter().map(|d| d.cost).sum(), days.iter().map(|d| d.cost).fold(0.0, f64::max))
}

/// The invented fortnight of the mock data: each bar a day's cost, today last.
const MOCK_SPEND: [f64; SPARK_DAYS] = [0.0, 1.2, 3.4, 2.1, 0.0, 0.0, 4.8, 6.3, 2.9, 5.1, 7.4, 3.8, 6.0, 4.2];

/// What the page keeps while it is shown.
#[derive(Default)]
pub struct Bar {
    /// The machine the bar is reading: none until the page is shown, and again once it is left (the unmount).
    key: Option<String>,
    pub summary: Option<Summary>,
    pub daily: Vec<Point>,
    pub(crate) summary_request: u64,
    pub(crate) daily_request: u64,
    summary_due: Option<f64>,
    daily_due: Option<f64>,
    /// Whether an archive was holding rows back at the last look: when it stops, the summary is asked for again.
    held: bool,
    /// Counts the reads that were asked for, for a test.
    pub asked: u32,
}

impl Bar {
    /// The page is shown on `key`: nothing is held and both reads are due now.
    fn mount(&mut self, key: &str) {
        *self = Bar {
            key: Some(key.to_string()),
            summary_request: self.summary_request + 1,
            daily_request: self.daily_request + 1,
            asked: self.asked,
            ..Default::default()
        };
    }

    /// The page is left: an answer on its way is stale.
    fn unmount(&mut self) {
        *self = Bar {
            summary_request: self.summary_request + 1,
            daily_request: self.daily_request + 1,
            asked: self.asked,
            ..Default::default()
        };
    }

    pub(crate) fn summary_answer(&mut self, request: u64, result: Result<Summary, String>) -> bool {
        if request != self.summary_request {
            return false;
        }
        if let Ok(next) = result {
            self.summary = Some(next);
        }
        true
    }

    pub(crate) fn daily_answer(&mut self, request: u64, result: Result<Vec<Point>, String>) -> bool {
        if request != self.daily_request {
            return false;
        }
        if let Ok(next) = result {
            self.daily = next;
        }
        true
    }
}

/// Where a read goes: this machine's server, or another machine's with its own token and addresses.
pub(crate) enum Target {
    Here(String),
    There(Box<Machine>),
}

impl Target {
    pub(crate) fn get(&self, path: &str) -> Result<Value, String> {
        match self {
            Target::Here(base) => super::http::get(base, path),
            Target::There(machine) => {
                let (status, body) = super::machine_routes::get_on(machine, path).map_err(|e| e.to_string())?;
                if !(200..300).contains(&status) {
                    return Err(format!("HTTP {status}"));
                }
                serde_json::from_str(&body).map_err(|e| e.to_string())
            }
        }
    }
}

impl State {
    /// The bar is on screen: Home is shown in the Sessions rail (a rail pressed from Home covers the page).
    fn summary_shown(&self) -> bool {
        self.on_home() && self.rail == crate::Rail::Sessions
    }

    /// What the bar draws: the server's counts less a session archived a moment ago (`countsWithoutArchiving`).
    pub fn summary_counts(&self) -> (u32, u32) {
        let Some(summary) = &self.bar.summary else { return (0, 0) };
        let listed = self.projects.iter().flat_map(|p| p.sessions.iter());
        super::dashboard::counts_without_archiving((summary.active, summary.idle), listed, &|id| self.archive.hides(id))
    }

    /// The fortnight as the bar draws it.
    pub fn summary_days(&self) -> Vec<Day> {
        days(&self.bar.daily, self.now_ms.unwrap_or_else(super::link::now_ms))
    }

    /// Mounts, reads and leaves as the page is shown and left. True when something changed.
    pub(crate) fn keep_summary(&mut self, now: f64, tx: &mut Tx) -> bool {
        if !self.summary_shown() {
            let held = self.bar.key.is_some();
            self.bar.unmount();
            return held;
        }
        let key = self.machines.current_key().to_string();
        let mut changed = false;
        if self.bar.key.as_deref() != Some(key.as_str()) {
            self.bar.mount(&key);
            changed = true;
        }
        // An archive that has settled changes what the server counts (`watch(pendingIds)`).
        let held = self.archive.any_hidden();
        let settled = self.bar.held && !held;
        self.bar.held = held;
        let wants_summary = settled || self.bar.summary_due.is_none_or(|due| now >= due);
        let wants_daily = self.bar.daily_due.is_none_or(|due| now >= due);
        if wants_summary || wants_daily {
            changed |= self.read_summary(&key, now, wants_summary, wants_daily);
        }
        tx.wake_after(std::time::Duration::from_secs(1));
        changed
    }

    fn read_summary(&mut self, key: &str, now: f64, summary: bool, daily: bool) -> bool {
        let now_ms = self.now_ms.unwrap_or_else(super::link::now_ms);
        if summary {
            self.bar.summary_due = Some(now + SUMMARY_SECS);
        }
        if daily {
            self.bar.daily_due = Some(now + DAILY_SECS);
        }
        self.bar.asked += u32::from(summary) + u32::from(daily);
        let target = if key == HOME_MACHINE_KEY {
            match &self.link {
                Some(link) => Target::Here(link.base.clone()),
                None => return self.mock_summary(summary, daily),
            }
        } else {
            match self.machines.machine(key).cloned() {
                Some(machine) if self.link.is_some() => Target::There(Box::new(machine)),
                _ => return false,
            }
        };
        let Some(link) = &self.link else { return false };
        let post = link.post();
        self.bar.summary_request += u64::from(summary);
        self.bar.daily_request += u64::from(daily);
        let (summary_request, daily_request) = (self.bar.summary_request, self.bar.daily_request);
        std::thread::spawn(move || {
            // Side by side, so a slow spend read doesn't hold the counts back.
            let target = std::sync::Arc::new(target);
            if daily {
                let (target, post) = (target.clone(), post.clone());
                std::thread::spawn(move || {
                    let result =
                        target.get(&daily_path(now_ms)).and_then(|b| parse_daily(&b).ok_or_else(|| "No data returned".to_string()));
                    post.update(move |s, _| {
                        s.bar.daily_answer(daily_request, result);
                    });
                });
            }
            if summary {
                let result = target.get("/api/fleet/summary").and_then(|b| parse_summary(&b).ok_or_else(|| "No data returned".to_string()));
                post.update(move |s, _| {
                    s.bar.summary_answer(summary_request, result);
                });
            }
        });
        false
    }

    /// No server: the counts of the invented sessions and a fixed fortnight.
    fn mock_summary(&mut self, summary: bool, daily: bool) -> bool {
        if summary {
            let top = self
                .projects
                .iter()
                .flat_map(|p| p.sessions.iter())
                .filter(|s: &&Session| super::dashboard::is_top_level(s) && !s.row.archived);
            let (mut active, mut idle) = (0, 0);
            for s in top {
                if s.status_word() == "active" {
                    active += 1;
                } else {
                    idle += 1;
                }
            }
            self.bar.summary = Some(Summary { active, idle, queued: 2, tokens: 1_240_000.0, cost: 18.42 });
        }
        if daily {
            let today = self.now_ms.unwrap_or_else(super::link::now_ms).div_euclid(DAY_MS);
            self.bar.daily = MOCK_SPEND
                .iter()
                .enumerate()
                .map(|(i, cost)| Point { date: iso_day(today - (SPARK_DAYS as i64 - 1) + i as i64), cost: *cost })
                .collect();
        }
        true
    }
}
