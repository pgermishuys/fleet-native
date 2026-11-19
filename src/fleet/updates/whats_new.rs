//! What's new on Settings → Updates & about (`WhatsNewSection.vue`, `use-whats-new.ts`): where a "What's new" link asked to go,
//! which version rows are open, the badge each carries, and the lines the card says when there is little to show. Plain Rust;
//! `view/settings/updates_whats_new.rs` draws it, and `State::open_whats_new` is what every entry point calls.
use super::compare_versions;
use super::notes::ReleaseNote;
use crate::conv::time::civil;
use crate::fleet::work::parse_iso_ms;
use std::cmp::Ordering;
use std::collections::BTreeMap;

/// The public list the app and the CLI update from (`ALL_RELEASES_URL`).
pub const ALL_RELEASES_URL: &str = "https://github.com/pgermishuys/weave-fleet/releases";
/// The row a link named flashes this long (`whats-new-flash`, 1.6 s).
pub const FLASH_SECS: f64 = 1.6;

/// Where What's new was asked to go: a version's notes, or the top of the card (`WhatsNewRequest`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Request {
    pub version: Option<String>,
    /// Also open every version after this one up to `version`: the ones an update skipped over.
    pub since: Option<String>,
}

/// What the page keeps between frames.
#[derive(Debug, Default)]
pub struct WhatsNewState {
    /// Asked for by a link; handled once the notes have loaded, then cleared (`clear`).
    pub request: Option<Request>,
    /// Rows the user (or a request) opened or closed, by version. A row not here is open when the range says so.
    pub opened: BTreeMap<String, bool>,
    /// The row that flashes and when it started, on the app's clock (`flashed`).
    pub flash: Option<(String, f64)>,
    /// A scroll waiting for the rows to be drawn: the key of the element to bring to the top.
    pub scroll: Option<String>,
}

/// The pill beside a version (`badge`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Badge {
    ReadyToInstall,
    Installed,
    Available,
}

impl Badge {
    pub fn label(self) -> &'static str {
        match self {
            Badge::ReadyToInstall => "Ready to install",
            Badge::Installed => "Installed",
            Badge::Available => "Available",
        }
    }

    /// The accent ones are the news; "Installed" is muted.
    pub fn accent(self) -> bool {
        self != Badge::Installed
    }
}

/// "Ready to install" for the version an update has downloaded, "Installed" for the one this Fleet runs, "Available" for any
/// newer one (`badge`).
pub fn badge(version: &str, installed: Option<&str>, ready: Option<&str>) -> Option<Badge> {
    if ready == Some(version) {
        return Some(Badge::ReadyToInstall);
    }
    let installed = installed.filter(|i| !i.is_empty())?;
    match version == installed {
        true => Some(Badge::Installed),
        false if compare_versions(version, installed) == Ordering::Greater => Some(Badge::Available),
        false => None,
    }
}

/// The line under a version's notes (`internalLine`).
pub fn internal_line(count: usize) -> String {
    if count == 1 { "And 1 behind-the-scenes change.".into() } else { format!("And {count} behind-the-scenes changes.") }
}

/// What the card says when there are no notes at all: the reason, then where they are.
pub fn empty_line(error: Option<&str>) -> String {
    match error.filter(|e| !e.is_empty()) {
        Some(e) => format!("Fleet couldn't get the release notes ({e})."),
        None => "There are no release notes yet.".into(),
    }
}

/// Above saved notes when GitHub couldn't be reached (`whats-new-stale`).
pub fn stale_line(error: &str, saved_on: &str) -> String {
    format!("Couldn't check GitHub for newer notes ({error}). These are from {saved_on}.")
}

/// `formatDate` (Fleet's en-US): "Oct 12", with the year ("Oct 12, 2026") when asked or when it isn't this year. Empty when the time can't be read.
pub fn day(iso: Option<&str>, with_year: bool, now_ms: i64, offset_secs: i64) -> String {
    let Some(at) = iso.and_then(parse_iso_ms) else { return String::new() };
    let (y, mo, d, ..) = civil(at, offset_secs);
    let (this_year, ..) = civil(now_ms, offset_secs);
    if with_year || y != this_year {
        format!("{} {d}, {y}", crate::conv::time::MONTHS[mo])
    } else {
        format!("{} {d}", crate::conv::time::MONTHS[mo])
    }
}

/// The versions a request opens: the one it names and, with `since`, every release after `since` up to it (`goToRequest`).
pub fn versions_to_open(request: &Request, releases: &[ReleaseNote]) -> Vec<String> {
    let Some(version) = request.version.as_deref() else { return Vec::new() };
    let mut open: Vec<String> = match request.since.as_deref() {
        Some(since) => releases
            .iter()
            .filter(|r| {
                compare_versions(&r.version, since) == Ordering::Greater && compare_versions(&r.version, version) != Ordering::Greater
            })
            .map(|r| r.version.clone())
            .collect(),
        None => Vec::new(),
    };
    if releases.iter().any(|r| r.version == version) && !open.iter().any(|v| v == version) {
        open.push(version.to_string());
    }
    open
}

/// The key of a version's row, which the scroll looks for.
pub fn row_key(version: &str) -> String {
    format!("whats-new-version-{version}")
}

impl crate::State {
    /// `openWhatsNew`: Settings → Updates & about, at a version's notes (or the top of the card with none). With `since`, every
    /// version after it up to `version` opens. A new request each time, so asking twice scrolls twice.
    pub fn open_whats_new(&mut self, version: Option<&str>, since: Option<&str>) {
        self.updates.whats_new.request = Some(Request { version: version.map(String::from), since: since.map(String::from) });
        self.open_settings("updates", None);
    }

    /// A row's chevron: opens or closes it (the `details` element's own toggle).
    pub fn toggle_whats_new_row(&mut self, version: &str, was_open: bool) {
        self.updates.whats_new.opened.insert(version.to_string(), !was_open);
    }

    /// Called from `poll`. Once the notes of the machine Settings shows have loaded, a waiting request opens its rows, starts the
    /// flash and asks for a scroll; the scroll goes out on the next call, when the rows have been drawn. Handled once.
    pub fn keep_whats_new(&mut self, now: f64, tx: &mut crate::Tx) -> bool {
        if self.rail != crate::Rail::Settings || self.settings.page != crate::fleet::settings_nav::Page::Updates {
            self.updates.whats_new.scroll = None;
            return false;
        }
        let mut changed = false;
        if let Some(key) = self.updates.whats_new.scroll.take() {
            tx.scroll_key_to(key, 0.0);
            changed = true;
        }
        let key = self.settings.choice.key(&self.machines).to_string();
        let loaded = self.updates.releases.get(&key).is_some_and(|n| n.loaded && !n.loading);
        if loaded && let Some(request) = self.updates.whats_new.request.take() {
            let clock = self.clock;
            let releases = &self.updates.releases[&key].releases;
            let open = versions_to_open(&request, releases);
            let target = request.version.as_deref().filter(|v| open.iter().any(|o| o == v));
            let scroll = target.map_or_else(|| "settings-group-whats-new".to_string(), row_key);
            let flash = target.map(|v| (v.to_string(), clock));
            for v in open {
                self.updates.whats_new.opened.insert(v, true);
            }
            self.updates.whats_new.flash = flash;
            self.updates.whats_new.scroll = Some(scroll);
            changed = true;
        }
        if let Some((_, at)) = &self.updates.whats_new.flash {
            if now - at >= FLASH_SECS {
                self.updates.whats_new.flash = None;
            } else {
                tx.wake_after(std::time::Duration::from_millis(33));
            }
            changed = true;
        }
        if self.updates.whats_new.scroll.is_some() {
            tx.wake_after(std::time::Duration::from_millis(16));
        }
        changed
    }
}
