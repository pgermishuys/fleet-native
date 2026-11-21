//! `AnalyticsFilters.vue`'s date fields' own calendar (`<input type="date">`'s picker, which Chrome draws outside the page):
//! which end it is open for and which month it shows. A day picked sets that end as a typed whole date does.
//! From's latest day is To and To's earliest is From, which limit the picker only (row 3.6).
use super::filters::{Filters, today};
use crate::State;
use crate::fleet::summary::iso_day;
use crate::ui::Open;
use crate::ui::calendar::Month;

/// The popover's name for `Open::Popover`.
pub const POPOVER: &str = "analytics-calendar";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    From,
    To,
}

impl End {
    pub fn key(self) -> &'static str {
        match self {
            End::From => "from",
            End::To => "to",
        }
    }

    pub fn of(self, f: &Filters) -> &str {
        match self {
            End::From => &f.from,
            End::To => &f.to,
        }
    }
}

/// The calendar that is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Calendar {
    pub end: End,
    pub month: Month,
}

impl State {
    fn analytics_today(&self) -> String {
        iso_day(today(self.now_ms.unwrap_or_else(crate::fleet::link::now_ms)))
    }

    /// The glyph in a field: opens that field's calendar on the month it holds (this month when it holds none).
    pub fn analytics_open_calendar(&mut self, end: End) {
        if self.ui.open == Some(Open::Popover(POPOVER)) && self.analytics.calendar.is_some_and(|c| c.end == end) {
            return self.analytics_close_calendar();
        }
        let month = Month::of(end.of(&self.analytics.filters))
            .or_else(|| Month::of(&self.analytics_today()))
            .unwrap_or(Month { year: 1970, month: 1 });
        self.analytics.calendar = Some(Calendar { end, month });
        self.ui.open = Some(Open::Popover(POPOVER));
    }

    pub fn analytics_close_calendar(&mut self) {
        self.analytics.calendar = None;
        if self.ui.open == Some(Open::Popover(POPOVER)) {
            self.ui.open = None;
        }
    }

    pub fn analytics_calendar_shift(&mut self, by: i32) {
        if let Some(c) = &mut self.analytics.calendar {
            c.month = c.month.shifted(by);
        }
    }

    /// A day picked: that end takes it and every section reads again; the calendar closes (Chrome's does).
    pub fn analytics_calendar_pick(&mut self, day: &str) {
        let Some(c) = self.analytics.calendar else { return };
        let mut next = self.analytics.filters.clone();
        match c.end {
            End::From => next.from = day.to_string(),
            End::To => next.to = day.to_string(),
        }
        self.analytics_close_calendar();
        self.analytics_set_filters(next);
    }
}
