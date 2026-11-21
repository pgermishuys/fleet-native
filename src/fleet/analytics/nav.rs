//! Which section Analytics shows, and how it is entered (`composables/use-analytics-section.ts`, `use-analytics-menu.ts`'s
//! `select`, `core-rails.ts`). Fleet keeps the section in the address; here it is on `State`, and `path` is the address it
//! stands for.
use super::sections::{Section, section_path};
use crate::State;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Nav {
    pub section: Section,
}

impl Default for Nav {
    fn default() -> Nav {
        Nav { section: Section::Overview }
    }
}

impl Nav {
    /// The address of the section on show.
    pub fn path(&self) -> String {
        section_path(self.section)
    }
}

impl State {
    /// Opening Analytics from anywhere (the rail, the palette, the spend sparkline): the rail's address is `/analytics`, so it
    /// is always Overview, never the section last shown (row 14.1). Coming from another view it forgets the machine pick
    /// (row 13.3); coming from one of its own sections it keeps it.
    pub fn enter_analytics(&mut self) {
        if self.rail != crate::Rail::Analytics {
            self.analytics.choice.reset();
        }
        self.rail = crate::Rail::Analytics;
        self.analytics_nav.section = Section::Overview;
    }

    /// The menu's item: opens the section by its own address; the open one does nothing (`select`).
    pub fn open_analytics_section(&mut self, section: Section) {
        if self.analytics_nav.section != section {
            self.analytics_nav.section = section;
            // Fleet's chart is destroyed with its section: its hidden lines come back.
            self.analytics.chart = Default::default();
        }
    }

    /// An address under `/analytics` (a stale link too) opens the section it names, else Overview (`analyticsSectionFromPath`).
    pub fn open_analytics_address(&mut self, path: &str) {
        self.enter_analytics();
        self.analytics_nav.section = super::sections::section_from_path(path);
        self.analytics.chart = Default::default();
    }
}
