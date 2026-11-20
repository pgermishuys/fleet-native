//! The store behind the Analytics page and its menu (`use-analytics-{summary,daily,models,sessions}.ts`,
//! `use-analytics-machine.ts`, and `AnalyticsContent.vue`'s `refreshAnalytics` / `refreshSessionsOnly`).
//!
//! Fleet runs the reads twice, once in the page and once in the menu; here one store feeds both, with the same answers
//! (row 5.8). It holds the machine Analytics shows, that machine's filters and sessions sort, and the five reads: the page's
//! summary, daily, models and sessions, and the menu's range-only summary (the project list and Projects' count).
//!
//! Each read is numbered: an answer a newer request overtook is dropped; a success clears the read's error; a failure keeps
//! its data and says the client's own words. Every request runs on a thread and is handed back on the UI thread.
//! No view here: C to J read these fields.
use super::filters::Filters;
use super::sessions_sort::{SortBy, SortDir};
use super::{Daily, ModelRow, SessionRow, Summary};
use crate::State;
use crate::fleet::machine_choice::MachineChoice;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::summary::Target;
use rune::Tx;

/// Every read polls this often while Analytics is shown (`DEFAULT_POLL_INTERVAL_MS`).
pub const POLL_SECS: f64 = 30.0;

/// The five reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The page's summary: the range and the project.
    Summary,
    /// The menu's summary: the range only, so the menu can list the range's projects.
    Range,
    Daily,
    Models,
    Sessions,
}

impl Kind {
    /// The client's own words for a failure, whatever the server said (a 503 "Analytics is disabled." reads the same).
    pub fn failure(self) -> &'static str {
        match self {
            Kind::Summary | Kind::Range => "Failed to fetch analytics summary",
            Kind::Daily => "Failed to fetch daily analytics",
            Kind::Models => "Failed to fetch model analytics",
            Kind::Sessions => "Failed to fetch session analytics",
        }
    }
}

/// One read: what it last got, whether it has answered yet, and why the last answer failed.
#[derive(Clone, Debug)]
pub struct Read<T> {
    pub data: T,
    /// True until the first answer, success or failure; a poll or a refetch doesn't bring it back.
    pub is_loading: bool,
    pub error: Option<String>,
    request: u64,
}

impl<T: Default> Read<T> {
    fn new(request: u64) -> Read<T> {
        Read { data: T::default(), is_loading: true, error: None, request }
    }

    /// A numbered answer: `false` when a newer request overtook it.
    fn take(&mut self, request: u64, result: Option<T>, failure: &str) -> bool {
        if request != self.request {
            return false;
        }
        match result {
            Some(data) => {
                self.data = data;
                self.error = None;
            }
            None => self.error = Some(failure.to_string()),
        }
        self.is_loading = false;
        true
    }
}

/// What a read handed back, ready to apply.
enum Answer {
    Summary(Option<Summary>),
    Range(Option<Summary>),
    Daily(Option<Vec<Daily>>),
    Models(Option<Vec<ModelRow>>),
    Sessions(Option<Vec<SessionRow>>),
}

/// Which batch a read belongs to; a poll belongs to none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Batch {
    None,
    All(u64),
    Sessions(u64),
}

/// A batch in flight: how many of its reads have not settled (answered, failed or been overtaken: `Promise.allSettled`).
#[derive(Clone, Copy, Debug, Default)]
struct Pending {
    id: u64,
    left: usize,
}

impl Pending {
    fn start(&mut self, reads: usize) -> u64 {
        self.id += 1;
        self.left = reads;
        self.id
    }

    fn settle(&mut self, id: u64) {
        if id == self.id {
            self.left = self.left.saturating_sub(1);
        }
    }
}

/// What a change asked for, done at the next look so the reads leave from the poll that has the clock.
#[derive(Clone, Copy, Debug, Default)]
struct Wants {
    all: bool,
    range: bool,
    sessions: bool,
}

pub struct Analytics {
    /// The machine the page shows: the machine you're on until its switch picks another (`useAnalyticsMachine`).
    pub choice: MachineChoice,
    /// The machine the reads are for: none until Analytics is shown, and again once it is left.
    mounted: Option<String>,
    pub filters: Filters,
    pub sort: (SortBy, SortDir),
    pub summary: Read<Option<Summary>>,
    pub range: Read<Option<Summary>>,
    pub daily: Read<Vec<Daily>>,
    pub models: Read<Vec<ModelRow>>,
    pub sessions: Read<Vec<SessionRow>>,
    all: Pending,
    only_sessions: Pending,
    wants: Wants,
    due: Option<f64>,
    /// The window was off screen at the last look: it reads at once when it comes back.
    hidden: bool,
    /// Counts the requests sent, for a test.
    pub asked: u32,
    /// The request numbers go on across mounts so that an answer for an earlier visit never lands in this one.
    next_request: u64,
}

impl Default for Analytics {
    fn default() -> Analytics {
        Analytics {
            choice: MachineChoice::default(),
            mounted: None,
            filters: Filters::default(),
            sort: (SortBy::CreatedAt, SortDir::Desc),
            summary: Read::new(0),
            range: Read::new(0),
            daily: Read::new(0),
            models: Read::new(0),
            sessions: Read::new(0),
            all: Pending::default(),
            only_sessions: Pending::default(),
            wants: Wants::default(),
            due: None,
            hidden: false,
            asked: 0,
            next_request: 0,
        }
    }
}

impl Analytics {
    /// `isRefreshingAnalytics`: a filter change's refetch of the four page reads is running.
    pub fn refreshing_all(&self) -> bool {
        self.all.left > 0
    }

    /// `isRefreshingSessions`: a sort change's refetch of the sessions is running.
    pub fn refreshing_sessions(&self) -> bool {
        self.only_sessions.left > 0
    }

    /// The machine the reads are for, while Analytics is shown.
    pub fn machine(&self) -> Option<&str> {
        self.mounted.as_deref()
    }

    /// Row 5.7, Overview's chart and Daily: the daily read, or a refresh of all.
    pub fn daily_loading(&self) -> bool {
        self.daily.is_loading || self.refreshing_all()
    }

    /// Overview's model bars fall back to the summary's top models, so they wait for both.
    pub fn models_bars_loading(&self) -> bool {
        self.models.is_loading || self.summary.is_loading || self.refreshing_all()
    }

    /// Overview's project bars and Projects.
    pub fn projects_loading(&self) -> bool {
        self.summary.is_loading || self.refreshing_all()
    }

    pub fn models_loading(&self) -> bool {
        self.models.is_loading || self.refreshing_all()
    }

    pub fn sessions_loading(&self) -> bool {
        self.sessions.is_loading || self.refreshing_all() || self.refreshing_sessions()
    }

    /// Analytics is shown on `key`: every read starts over (the page remounts, `MachineScope :key`) and asks at once.
    fn mount(&mut self, key: &str, filters: Filters) {
        let n = self.next_request + 1;
        self.mounted = Some(key.to_string());
        self.filters = filters;
        self.sort = (SortBy::CreatedAt, SortDir::Desc);
        self.summary = Read::new(n);
        self.range = Read::new(n);
        self.daily = Read::new(n);
        self.models = Read::new(n);
        self.sessions = Read::new(n);
        self.next_request = n;
        // A batch of the visit before must not end this visit's, and its answers are dropped by their request numbers.
        self.all = Pending { id: self.all.id + 1, left: 0 };
        self.only_sessions = Pending { id: self.only_sessions.id + 1, left: 0 };
        self.wants = Wants { all: true, range: true, sessions: false };
        self.due = None;
        self.hidden = false;
    }

    /// Analytics is left: an answer on its way is stale and the polls stop.
    fn unmount(&mut self) {
        let next = self.next_request + 1;
        for number in [&mut self.summary.request, &mut self.range.request, &mut self.daily.request, &mut self.models.request] {
            *number = next;
        }
        self.sessions.request = next;
        self.next_request = next;
        self.all = Pending { id: self.all.id + 1, left: 0 };
        self.only_sessions = Pending { id: self.only_sessions.id + 1, left: 0 };
        self.mounted = None;
        self.wants = Wants::default();
        self.due = None;
    }

    fn read_number(&mut self, kind: Kind) -> u64 {
        self.next_request += 1;
        let n = self.next_request;
        match kind {
            Kind::Summary => self.summary.request = n,
            Kind::Range => self.range.request = n,
            Kind::Daily => self.daily.request = n,
            Kind::Models => self.models.request = n,
            Kind::Sessions => self.sessions.request = n,
        }
        n
    }

    /// An answer comes in: applied when it is still the newest of its read; its batch settles either way.
    fn answer(&mut self, kind: Kind, request: u64, answer: Answer, batch: Batch) -> bool {
        let words = kind.failure();
        let fresh = match answer {
            Answer::Summary(r) => self.summary.take(request, r.map(Some), words),
            Answer::Range(r) => self.range.take(request, r.map(Some), words),
            Answer::Daily(r) => self.daily.take(request, r, words),
            Answer::Models(r) => self.models.take(request, r, words),
            Answer::Sessions(r) => self.sessions.take(request, r, words),
        };
        match batch {
            Batch::All(id) => self.all.settle(id),
            Batch::Sessions(id) => self.only_sessions.settle(id),
            Batch::None => {}
        }
        fresh || !matches!(batch, Batch::None)
    }
}

impl State {
    /// The machine Analytics shows (`useAnalyticsMachine().key`).
    pub fn analytics_machine(&self) -> String {
        self.analytics.choice.key(&self.machines).to_string()
    }

    /// Changes the range or the project: every section reads again at once, and the range alone also reads the project list.
    /// The filters are kept for the machine shown (`setFrom`, `setTo`, `setProjectId`).
    pub fn analytics_set_filters(&mut self, next: Filters) {
        if next == self.analytics.filters {
            return;
        }
        let range_moved = next.from != self.analytics.filters.from || next.to != self.analytics.filters.to;
        let key = self.analytics.mounted.clone().unwrap_or_else(|| self.analytics_machine());
        self.sidebar.save_analytics_filters(&key, &next);
        self.save_settings();
        self.analytics.filters = next;
        self.analytics.wants.all = true;
        self.analytics.wants.range |= range_moved;
    }

    /// Reset: back to the last 30 days, every project (`resetFilters`).
    pub fn analytics_reset_filters(&mut self) {
        let now = self.now_ms.unwrap_or_else(crate::fleet::link::now_ms);
        self.analytics_set_filters(Filters::defaults(now));
    }

    /// A sort change reads the sessions alone (`refreshSessionsOnly`). A refresh of everything already reads them.
    pub fn analytics_set_sort(&mut self, by: SortBy, dir: SortDir) {
        if self.analytics.sort == (by, dir) {
            return;
        }
        self.analytics.sort = (by, dir);
        self.analytics.wants.sessions = !self.analytics.wants.all;
    }

    /// Mounts, reads, polls and leaves as Analytics is shown and left. True when something changed.
    pub(crate) fn keep_analytics(&mut self, now: f64, tx: &mut Tx) -> bool {
        if self.rail != crate::Rail::Analytics {
            let held = self.analytics.mounted.is_some();
            if held {
                self.analytics.unmount();
            }
            return held;
        }
        let key = self.analytics_machine();
        let mut changed = false;
        if self.analytics.mounted.as_deref() != Some(key.as_str()) {
            let now_ms = self.now_ms.unwrap_or_else(crate::fleet::link::now_ms);
            let filters = Filters::opened(self.sidebar.analytics_filters(&key), now_ms);
            self.analytics.mount(&key, filters);
            changed = true;
        }
        tx.wake_after(std::time::Duration::from_secs(1));
        // Fleet asks nothing while its document is hidden, and asks at once when it shows again.
        if !self.quiet.visible {
            self.analytics.hidden = true;
            return changed;
        }
        if std::mem::take(&mut self.analytics.hidden) {
            self.analytics.due = Some(now);
        }
        let wants = std::mem::take(&mut self.analytics.wants);
        if wants.all {
            let mut kinds = vec![Kind::Summary, Kind::Daily, Kind::Sessions, Kind::Models];
            kinds.extend(wants.range.then_some(Kind::Range));
            let id = self.analytics.all.start(4);
            self.analytics_ask(&key, &kinds, Batch::All(id));
            self.analytics.due = Some(now + POLL_SECS);
            changed = true;
        } else if wants.sessions {
            let id = self.analytics.only_sessions.start(1);
            self.analytics_ask(&key, &[Kind::Sessions], Batch::Sessions(id));
            changed = true;
        }
        if self.analytics.due.is_none_or(|due| now >= due) {
            self.analytics.due = Some(now + POLL_SECS);
            self.analytics_ask(&key, &[Kind::Summary, Kind::Range, Kind::Daily, Kind::Models, Kind::Sessions], Batch::None);
            changed = true;
        }
        changed
    }

    /// Sends `kinds` to the machine `key`, each on its own thread so a slow one doesn't hold the others back.
    fn analytics_ask(&mut self, key: &str, kinds: &[Kind], batch: Batch) {
        let target = if key == HOME_MACHINE_KEY {
            match &self.link {
                Some(link) => Target::Here(link.base.clone()),
                None => return self.analytics_mock(kinds, batch),
            }
        } else {
            match self.machines.machine(key).cloned() {
                Some(machine) if self.link.is_some() => Target::There(Box::new(machine)),
                _ => return,
            }
        };
        let Some(post) = self.link.as_ref().map(|l| l.post()) else { return };
        let target = std::sync::Arc::new(target);
        for &kind in kinds {
            let request = self.analytics.read_number(kind);
            let path = self.analytics_path(kind);
            self.analytics.asked += 1;
            let (target, post) = (target.clone(), post.clone());
            let batch = if kind == Kind::Range { Batch::None } else { batch };
            std::thread::spawn(move || {
                let body = target.get(&path).ok();
                let answer = match kind {
                    Kind::Summary => Answer::Summary(body.as_ref().and_then(super::parse_summary)),
                    Kind::Range => Answer::Range(body.as_ref().and_then(super::parse_summary)),
                    Kind::Daily => Answer::Daily(body.as_ref().and_then(super::parse_days)),
                    Kind::Models => Answer::Models(body.as_ref().and_then(super::parse_models)),
                    Kind::Sessions => Answer::Sessions(body.as_ref().and_then(super::parse_sessions)),
                };
                post.update(move |s, _| {
                    s.analytics.answer(kind, request, answer, batch);
                });
            });
        }
    }

    fn analytics_path(&self, kind: Kind) -> String {
        let f = &self.analytics.filters;
        match kind {
            Kind::Summary => super::summary_path(f, false),
            Kind::Range => super::summary_path(f, true),
            Kind::Daily => super::daily_path(f),
            Kind::Models => super::models_path(f),
            Kind::Sessions => super::sessions_path(f, self.analytics.sort.0, self.analytics.sort.1),
        }
    }

    /// No server: the invented answers, in place of the reads.
    fn analytics_mock(&mut self, kinds: &[Kind], batch: Batch) {
        use crate::mock::analytics as mock;
        for &kind in kinds {
            let request = self.analytics.read_number(kind);
            let batch = if kind == Kind::Range { Batch::None } else { batch };
            let answer = match kind {
                Kind::Summary => Answer::Summary(Some(mock::summary())),
                Kind::Range => Answer::Range(Some(mock::summary())),
                Kind::Daily => Answer::Daily(Some(mock::daily())),
                Kind::Models => Answer::Models(Some(mock::models())),
                Kind::Sessions => Answer::Sessions(Some(mock::sessions())),
            };
            self.analytics.answer(kind, request, answer, batch);
        }
    }
}
