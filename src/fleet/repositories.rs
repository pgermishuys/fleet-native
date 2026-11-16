//! The Repositories page's data (`RepositoriesPage.vue`, rows C16.3, C16.6-C16.9, C16.14, C16.15): one read on arrival,
//! Refresh as a scan then a read, the list sorted by name. Plain state: the calls run on a thread (`State::feed_call`).
use super::new_session::feeds::{Repository, Scan, parse_scan};
use super::row_status::format_date_time;
use super::{http, store::Route};
use crate::conv::time::local_offset_secs;
use crate::{State, Tx};

/// What the page holds (`repositories`, `isLoading`, `isRefreshing`, `errorMessage`, `scannedAt`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Page {
    pub list: Vec<Repository>,
    /// The first read is running: the page shows its spinner box. Refresh keeps the list and spins the button instead.
    pub loading: bool,
    pub refreshing: bool,
    pub error: Option<String>,
    pub scanned_at: Option<i64>,
    /// Numbers the reads, so an answer to a read the page has left, or one Refresh overtook, is dropped.
    seq: u64,
}

/// What fills the page under its header (`isLoading`, `errorMessage`, an empty list, the list).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Body {
    Loading,
    Error,
    Empty,
    List,
}

impl Page {
    pub fn body(&self) -> Body {
        if self.loading {
            Body::Loading
        } else if self.error.is_some() {
            Body::Error
        } else if self.list.is_empty() {
            Body::Empty
        } else {
            Body::List
        }
    }

    /// The cards' order: by name (`localeCompare`: case-blind first, so "api" sits beside "API").
    pub fn sorted(&self) -> Vec<Repository> {
        let mut list = self.list.clone();
        list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.name.cmp(&b.name)));
        list
    }

    /// "Last scan: {label}": `Not scanned yet` until a scan has answered.
    pub fn last_scan(&self, now: i64, offset_secs: i64) -> String {
        let label = match self.scanned_at.filter(|&at| at > 0) {
            Some(at) => format_date_time(at, now, offset_secs),
            None => "Not scanned yet".to_string(),
        };
        format!("Last scan: {label}")
    }

    fn begin(&mut self, refresh: bool) -> u64 {
        self.seq += 1;
        // Every read starts by clearing the last failure (`errorMessage = null`).
        self.error = None;
        if refresh {
            self.refreshing = true;
        } else {
            self.loading = true;
        }
        self.seq
    }

    /// The answer: a failure clears the list and the scan time (C16.7), and the page says why.
    fn finish(&mut self, seq: u64, result: Result<Scan, String>) {
        if seq != self.seq {
            return;
        }
        self.loading = false;
        self.refreshing = false;
        match result {
            Ok(scan) => {
                self.list = scan.repositories;
                self.scanned_at = scan.scanned_at;
                self.error = None;
            }
            Err(e) => {
                self.list.clear();
                self.scanned_at = None;
                self.error = Some(if e.is_empty() { "Unable to load repositories.".to_string() } else { e });
            }
        }
    }
}

impl State {
    /// Whether a page no rail owns is showing (`NO_RAIL`): no list panel, nothing lit in the rail.
    pub fn on_bare_page(&self) -> bool {
        matches!(self.route, Route::Welcome | Route::Repositories)
    }

    pub fn on_repositories_page(&self) -> bool {
        self.route == Route::Repositories
    }

    /// `/welcome`: nothing links to it in Fleet either (C17.17); the scene and the tests are the ways in.
    pub fn open_welcome_page(&mut self) {
        self.go(Route::Welcome);
    }

    /// `/repositories`: reads the list on arrival (C16.14).
    pub fn open_repositories_page(&mut self) {
        let was = self.on_repositories_page();
        self.go(Route::Repositories);
        if !was {
            self.read_repositories_page(false);
        }
    }

    /// The page's read (`loadRepositories`), or Refresh (`forceRefresh`: scan, then read; a failed scan skips the read).
    pub fn read_repositories_page(&mut self, refresh: bool) {
        if self.repos_page.loading || self.repos_page.refreshing {
            return;
        }
        let seq = self.repos_page.begin(refresh);
        let key = self.machines.current_key().to_string();
        let started = self.feed_call(
            &key,
            move |base| {
                if refresh {
                    parse_scan(http::repositories_refresh(base))?;
                }
                parse_scan(http::repositories(base))
            },
            move |s, _: &mut Tx, result| s.repos_page.finish(seq, result),
        );
        // No Fleet to ask: the invented machine's list, so scenes and tests draw the same every time.
        if !started {
            let scan = if self.is_mock() { crate::mock::feeds::scan() } else { Scan::default() };
            self.repos_page.finish(seq, Ok(scan));
        }
    }

    /// "Last scan: …" for the open page.
    pub fn repositories_last_scan(&self) -> String {
        self.repos_page.last_scan(crate::view::message::now_ms(self), local_offset_secs())
    }

    /// "Workspace settings" and "Open settings": Settings → Folders.
    pub fn open_workspace_settings(&mut self, _tx: &mut Tx) {
        self.open_settings("folders", None);
    }
}
