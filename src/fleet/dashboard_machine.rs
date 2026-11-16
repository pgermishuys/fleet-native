//! The home page's own state beyond the rules in `dashboard.rs` (`FleetDashboard.vue`): "Show N more", the retention filter,
//! and another machine's session list, which the page asks for itself once it is shown (`loadOther`). The live machine's rows
//! are the session list's; another machine's are read here, with a request number so an answer a newer one overtook is
//! dropped, and again every 15 s while the page is shown. No Rune: `State`'s methods at the end run it on a thread.
use super::machine::Machine;
use super::settings::HOME_MACHINE_KEY;
use super::store::Retention;
use crate::mock::Session;
use crate::{State, Tx};

/// How often another machine's list is read again (`setInterval(…, 15_000)`).
pub const POLL_SECS: f64 = 15.0;
/// `limit` of that read.
pub const LIMIT: u32 = 100;
/// What a failed read says when the machine gave no reason (`Couldn't read that machine's sessions`).
pub const UNREADABLE: &str = "Couldn't read that machine's sessions";

/// `GET /api/sessions?limit=100`, with the retention left out for the active list.
pub fn path(retention: Retention) -> String {
    format!("/api/sessions?limit={LIMIT}{}", retention.query())
}

/// The page while it is shown on another machine.
#[derive(Default)]
pub struct Other {
    /// The machine the page is on; none while it is on this one or not shown.
    pub key: Option<String>,
    /// The last good answer, kept through a failed one (`otherSessions`).
    pub sessions: Vec<Session>,
    /// Asked for once the page is shown, so until then it is loading, not empty (`otherLoading`).
    pub loading: bool,
    pub error: Option<String>,
    /// What the last read was for: a change of it reads again.
    retention: Retention,
    request: u64,
    /// When the next poll is due, on the app clock.
    due: Option<f64>,
}

impl Other {
    /// The page is shown on `key`: nothing is held, and the first read is due now.
    pub fn mount(&mut self, key: &str) {
        *self = Other { key: Some(key.to_string()), loading: true, request: self.request + 1, ..Default::default() };
    }

    /// The page is left: an answer still on its way is stale (`otherRequest++`).
    pub fn unmount(&mut self) {
        *self = Other { request: self.request + 1, ..Default::default() };
    }

    /// Whether a read is due at `now`: the first one, the poll, or the filter changed.
    pub fn wants_read(&self, now: f64, retention: Retention) -> bool {
        self.key.is_some() && (self.due.is_none_or(|due| now >= due) || self.retention != retention)
    }

    /// A read starts: the request's number. Loading shows only while there is nothing to show.
    pub fn begin(&mut self, now: f64, retention: Retention) -> u64 {
        self.request += 1;
        if self.sessions.is_empty() {
            self.loading = true;
        }
        self.retention = retention;
        self.due = Some(now + POLL_SECS);
        self.request
    }

    /// An answer. One a newer read overtook is dropped; a failure keeps what the page held and says why. True when it counted.
    pub fn finish(&mut self, request: u64, result: Result<Vec<Session>, String>) -> bool {
        if request != self.request {
            return false;
        }
        self.loading = false;
        match result {
            Ok(sessions) => {
                self.sessions = sessions;
                self.error = None;
            }
            Err(why) => self.error = Some(why),
        }
        true
    }
}

/// What the page keeps while it is shown.
#[derive(Default)]
pub struct Page {
    pub other: Other,
    /// "Show N more" pressed (`showAllRecent`); it ends with the page.
    pub show_all_recent: bool,
}

impl State {
    /// Whether the page shows another machine's sessions: Home is shown and the machine you're on isn't this one.
    pub fn dashboard_on_other(&self) -> Option<String> {
        let key = self.machines.current_key();
        (self.on_home() && key != HOME_MACHINE_KEY).then(|| key.to_string())
    }

    /// The filter's pick (`RetentionFilter`): the list is read again for it. Another machine's page reads its own.
    pub fn pick_retention(&mut self, next: Retention) {
        self.ui.open = None;
        if self.store.retention == next {
            return;
        }
        self.selection.clear();
        self.store.retention = next;
        if let Some(link) = &self.link {
            link.refetch_list();
        }
    }

    /// "Show N more" / "Show fewer".
    pub fn toggle_recent(&mut self) {
        self.dash.show_all_recent = !self.dash.show_all_recent;
    }

    /// Mounts, reads and leaves another machine's page as the window goes to and from it. True when something changed.
    pub(crate) fn keep_dashboard_machine(&mut self, now: f64, tx: &mut Tx) -> bool {
        let Some(key) = self.dashboard_on_other() else {
            let held = self.dash.other.key.is_some() || self.dash.other.loading;
            self.dash.other.unmount();
            if !self.on_home() {
                self.dash.show_all_recent = false;
            }
            return held;
        };
        let mut changed = false;
        if self.dash.other.key.as_deref() != Some(key.as_str()) {
            self.dash.other.mount(&key);
            changed = true;
        }
        let retention = self.store.retention;
        if self.dash.other.wants_read(now, retention) {
            self.read_other(&key, now, retention);
            changed = true;
        }
        tx.wake_after(std::time::Duration::from_secs(1));
        changed
    }

    /// `loadOther`: asks the machine on a thread, with its own token and the addresses it has.
    fn read_other(&mut self, key: &str, now: f64, retention: Retention) {
        let request = self.dash.other.begin(now, retention);
        let (Some(link), Some(machine)) = (self.link.as_ref(), self.machines.machine(key).cloned()) else { return };
        let post = link.post();
        let key = key.to_string();
        std::thread::spawn(move || {
            let result = read(&machine, retention);
            post.update(move |s, _| s.other_read(&key, request, result));
        });
    }

    /// The thread's answer, on the UI thread.
    fn other_read(&mut self, key: &str, request: u64, result: Result<Vec<Session>, String>) {
        let ids: Vec<String> = result.as_ref().map(|rows| rows.iter().map(|r| r.id.clone()).collect()).unwrap_or_default();
        if self.dash.other.finish(request, result) && !ids.is_empty() {
            self.machines.remember_sessions(key, &ids);
        }
    }
}

fn read(machine: &Machine, retention: Retention) -> Result<Vec<Session>, String> {
    let (status, body) = super::machine_routes::get_on(machine, &path(retention)).map_err(|_| UNREADABLE.to_string())?;
    if !(200..300).contains(&status) {
        return Err(UNREADABLE.to_string());
    }
    let rows: serde_json::Value = serde_json::from_str(&body).map_err(|_| UNREADABLE.to_string())?;
    if !rows.is_array() {
        return Err(UNREADABLE.to_string());
    }
    Ok(super::sessions::parse_sessions_for(&rows, super::link::now_ms(), retention))
}
