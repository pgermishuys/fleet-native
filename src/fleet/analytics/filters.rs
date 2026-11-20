//! `composables/use-analytics-filters.ts`: the range and the project that scope every section, kept for each machine.
//!
//! A day is the **UTC** day, as the date fields send it and the server reads it, so "today" is the UTC day whatever
//! zone the computer is in.
use crate::fleet::automations::schedule::days_from_civil;
use crate::fleet::summary::iso_day;
use serde_json::{Value, json};

const DAY_MS: i64 = 86_400_000;

/// `AnalyticsFilters`. An empty end is an open end.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filters {
    pub from: String,
    pub to: String,
    /// The project's name (row 3.7: Fleet sends the name where the server wants an id).
    pub project_id: String,
}

/// The days since 1970-01-01 of "2026-11-20" (`Date.parse` of a bare date), `None` where Fleet gets `NaN`.
pub fn day_number(day: &str) -> Option<i64> {
    let mut parts = day.splitn(3, '-');
    let (y, m, d): (i64, i64, i64) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
    if day.len() != 10 || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, m - 1, d))
}

/// `addDays`: the day `days` after `day`.
fn add_days(day: i64, days: i64) -> String {
    iso_day(day + days)
}

/// Today's UTC day for a clock in milliseconds (`toIso(new Date())`).
pub fn today(now_ms: i64) -> i64 {
    now_ms.div_euclid(DAY_MS)
}

impl Filters {
    /// `getDefaultFilters`: the last 30 days, ending today, every project.
    pub fn defaults(now_ms: i64) -> Filters {
        let today = today(now_ms);
        Filters { from: add_days(today, -30), to: iso_day(today), project_id: String::new() }
    }

    /// `endingToday`: a range saved on an earlier day, moved to end today and kept the same length, so Analytics doesn't
    /// open on the numbers from the day the range was picked. A range with no end already runs to today; one whose end
    /// isn't a date is left as it is.
    pub fn ending_today(&self, now_ms: i64) -> Filters {
        let today = today(now_ms);
        if self.to.is_empty() || self.to == iso_day(today) {
            return self.clone();
        }
        let Some(to) = day_number(&self.to) else { return self.clone() };
        let shift = today - to;
        let from = if self.from.is_empty() { Some(String::new()) } else { day_number(&self.from).map(|f| add_days(f, shift)) };
        match from {
            Some(from) => Filters { from, to: iso_day(today), project_id: self.project_id.clone() },
            None => self.clone(),
        }
    }

    /// What the page opens on: what was saved (or the defaults), moved to end today.
    pub fn opened(saved: Option<&Filters>, now_ms: i64) -> Filters {
        saved.cloned().unwrap_or_else(|| Filters::defaults(now_ms)).ending_today(now_ms)
    }

    /// `isDefault`: from, to and project are the defaults, so there's nothing to reset.
    pub fn is_default(&self, now_ms: i64) -> bool {
        *self == Filters::defaults(now_ms)
    }

    pub fn to_json(&self) -> Value {
        json!({ "from": self.from, "to": self.to, "projectId": self.project_id })
    }

    /// Reads what `to_json` wrote; a field of the wrong kind is empty, and a value that isn't an object is no filters.
    pub fn from_json(v: &Value) -> Option<Filters> {
        v.is_object().then(|| {
            let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
            Filters { from: s("from"), to: s("to"), project_id: s("projectId") }
        })
    }
}
