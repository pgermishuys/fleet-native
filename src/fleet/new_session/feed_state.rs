//! What the New session page has read, and the rules that keep it right while answers arrive in any order (component 7,
//! piece C). Pure: no Rune, no I/O. Every read is numbered when it starts; an answer whose number is no longer the latest is
//! dropped, which is how Fleet's `AbortController` and `latestRequest` behave. Kept per machine key (`home` or a machine's
//! id), because the page can start a session on another machine.
use super::feeds::*;
use super::harness_choice::{Harnesses, Load};
use crate::fleet::prefs::Prefs;
use std::collections::HashMap;

/// One thing read for a path (a repository's detail, its worktrees, a folder's listing): the last answer, whether another is
/// coming, and why the last failed. Starting a read for another path makes any answer still on its way stale.
#[derive(Clone, Debug)]
pub struct Slot<T> {
    pub path: Option<String>,
    pub value: Option<T>,
    pub loading: bool,
    pub error: Option<String>,
    seq: u64,
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Slot { path: None, value: None, loading: false, error: None, seq: 0 }
    }
}

impl<T: Default> Slot<T> {
    /// Starts reading `path`; `None` cancels and clears. Returns the read's number, or `None` when there is nothing to ask.
    /// `keep` leaves the last answer up while the next loads (worktrees do; a repository's detail doesn't).
    pub fn begin(&mut self, path: Option<&str>, keep: bool) -> Option<u64> {
        self.seq += 1;
        self.path = path.map(str::to_string);
        self.error = None;
        if path.is_none() || !keep {
            self.value = None;
        }
        self.loading = path.is_some();
        path.map(|_| self.seq)
    }

    /// An answer for read `seq`; false when a newer read has overtaken it (nothing changes).
    pub fn finish(&mut self, seq: u64, result: Result<T, String>) -> bool {
        self.finish_or(seq, result, false)
    }

    /// As `finish`, but a failure also empties the value (`useWorktrees` shows none after an error).
    pub fn finish_or(&mut self, seq: u64, result: Result<T, String>, empty_on_error: bool) -> bool {
        if seq != self.seq {
            return false;
        }
        self.loading = false;
        match result {
            Ok(v) => self.value = Some(v),
            Err(e) => {
                self.error = Some(e);
                if empty_on_error {
                    self.value = Some(T::default());
                }
            }
        }
        true
    }

    /// The page closed or the box went away: whatever is on its way is dropped.
    pub fn cancel(&mut self) {
        self.begin(None, false);
    }
}

/// The repositories of one machine (`useRepositories`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RepoFeed {
    pub list: Vec<Repository>,
    pub scanned_at: Option<i64>,
    pub loading: bool,
    pub error: Option<String>,
}

impl RepoFeed {
    /// The list saved on the last visit shows at once, before the machine answers.
    pub fn seeded(saved: Scan) -> RepoFeed {
        RepoFeed { list: saved.repositories, scanned_at: saved.scanned_at, ..RepoFeed::default() }
    }

    pub fn begin(&mut self) {
        self.loading = true;
        self.error = None;
    }

    /// The scan came back. A failure keeps the list there was. True when the list was replaced (to be saved).
    pub fn finish(&mut self, result: Result<Scan, String>) -> bool {
        self.loading = false;
        match result {
            Ok(scan) => {
                self.list = scan.repositories;
                self.scanned_at = scan.scanned_at;
                true
            }
            Err(e) => {
                self.error = Some(e);
                false
            }
        }
    }
}

/// A machine's harness list as the page reads it (`useHarnesses`).
#[derive(Clone, Debug, Default)]
pub struct HarnessFeed {
    pub harnesses: Harnesses,
    pub error: Option<String>,
    /// A plain read is running: every list asked for meanwhile shares it.
    pub running: bool,
    /// When this page last had an answer from the machine (ms); none until it has one.
    pub answered_at: Option<i64>,
    seq: u64,
}

/// Lists asked for within this long of the last answer use it without asking (`REUSE_FOR_MS`).
pub const REUSE_FOR_MS: i64 = 10_000;

impl HarnessFeed {
    /// Whether a page that mounts now should ask.
    pub fn wants_read(&self, now: i64) -> bool {
        self.answered_at.is_none_or(|at| now - at >= REUSE_FOR_MS)
    }

    /// Starts a read. A plain one while another runs is that one: `None`. `fresh` always asks (Check again, a change).
    pub fn begin(&mut self, fresh: bool) -> Option<u64> {
        if !fresh && self.running {
            return None;
        }
        self.seq += 1;
        self.error = None;
        if self.harnesses.all.is_empty() {
            self.harnesses.load = Load::Loading;
        }
        if !fresh {
            self.running = true;
        }
        Some(self.seq)
    }

    /// An answer for read `seq`: an older, slower one never overwrites a newer one.
    pub fn finish(&mut self, seq: u64, fresh: bool, now: i64, result: Result<Vec<super::harness_choice::HarnessEntry>, String>) -> bool {
        if !fresh {
            self.running = false;
        }
        if seq != self.seq {
            return false;
        }
        match result {
            Ok(list) => {
                self.harnesses.all = list;
                self.harnesses.load = Load::Done;
                self.answered_at = Some(now);
            }
            Err(e) => {
                self.error = Some(e);
                self.harnesses.load = Load::Failed;
            }
        }
        true
    }
}

/// How long a folder's catalog is shown without asking again (`FRESH_FOR_MS`).
pub const CATALOG_FRESH_MS: i64 = 5 * 60_000;

/// A catalog and when it came.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cached {
    pub catalog: Catalog,
    pub fetched_at: i64,
}

/// Which catalog is asked for: the harness, the folder (none for a quick chat) and the profile (none for the default).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CatalogKey {
    pub harness: String,
    pub directory: Option<String>,
    pub profile: Option<String>,
}

/// What `CatalogFeed::select` says to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    /// Nothing is asked for (no harness): the shown catalog is cleared.
    Nothing,
    /// A fresh one is cached: it shows, no call.
    Cached,
    /// Make the call; its answer comes back with this number.
    Fetch(u64),
}

/// The agents and models a harness offers in a folder, before a session exists (`useHarnessCatalog`): per machine, the
/// last catalog of each folder shown at once and asked for again once it is old or the harness says it changed. While another
/// folder's loads, the previous one stays up so the pickers don't blink; `is_current` says whether it is this folder's.
#[derive(Clone, Debug, Default)]
pub struct CatalogFeed {
    cache: HashMap<CatalogKey, Cached>,
    pub wanted: Option<CatalogKey>,
    pub shown: Option<Catalog>,
    pub loaded_for: Option<CatalogKey>,
    pub loading: bool,
    pub error: Option<String>,
    seq: u64,
}

impl CatalogFeed {
    pub fn is_current(&self) -> bool {
        self.wanted.is_some() && self.wanted == self.loaded_for
    }

    pub fn supported(&self) -> bool {
        self.shown.as_ref().is_some_and(|c| c.supported)
    }

    /// The picker now asks for `key`: shows what is cached, and says whether to call.
    pub fn select(&mut self, key: Option<CatalogKey>, now: i64) -> Ask {
        self.error = None;
        self.seq += 1;
        self.wanted = key.clone();
        let Some(key) = key else {
            self.shown = None;
            self.loaded_for = None;
            self.loading = false;
            return Ask::Nothing;
        };
        if let Some(c) = self.cache.get(&key) {
            self.shown = Some(c.catalog.clone());
            self.loaded_for = Some(key);
            if now - c.fetched_at < CATALOG_FRESH_MS {
                self.loading = false;
                return Ask::Cached;
            }
        }
        self.loading = true;
        Ask::Fetch(self.seq)
    }

    /// The answer to a `Fetch(seq)`. A failure keeps the catalog there was; with none, the pickers show nothing.
    pub fn finish(&mut self, seq: u64, now: i64, result: Result<Catalog, String>) -> bool {
        if seq != self.seq {
            return false;
        }
        let Some(key) = self.wanted.clone() else { return false };
        self.loading = false;
        match result {
            Ok(catalog) => {
                self.cache.insert(key.clone(), Cached { catalog: catalog.clone(), fetched_at: now });
                self.shown = Some(catalog);
                self.loaded_for = Some(key);
            }
            Err(e) => {
                self.error = Some(e);
                if !self.cache.contains_key(&key) && self.shown.is_none() {
                    self.shown = None;
                    self.loaded_for = Some(key);
                }
            }
        }
        true
    }

    /// The harness says what it offers changed: forgets the catalogs it is about, so they are asked for again when shown.
    /// True when the one on screen is among them and should be asked for again now (`forgetChanged`).
    pub fn on_change(&mut self, change: &CatalogChange) -> bool {
        self.cache.retain(|k, _| !change.is_for(&k.harness, k.directory.as_deref(), k.profile.as_deref()));
        self.wanted.as_ref().is_some_and(|k| change.is_for(&k.harness, k.directory.as_deref(), k.profile.as_deref()))
    }

    /// Signing in to a provider or out changes the models everywhere: forgets every catalog of `harness`
    /// (`forgetHarnessCatalogs`).
    pub fn forget_harness(&mut self, harness: &str) {
        self.cache.retain(|k, _| k.harness != harness);
    }

    /// Whether a catalog is cached for `key` (for tests and the pickers).
    pub fn is_cached(&self, key: &CatalogKey) -> bool {
        self.cache.contains_key(key)
    }

    /// Asks again for the catalog on screen, whatever its age (a pushed change). The old one stays up meanwhile.
    pub fn refetch(&mut self) -> Option<u64> {
        self.wanted.as_ref()?;
        self.seq += 1;
        self.loading = true;
        self.error = None;
        Some(self.seq)
    }
}

/// The worktree naming templates in force for a repository (`useWorktreeNamingStore`).
#[derive(Clone, Debug, Default)]
pub struct NamingFeed {
    pub answer: NamingAnswer,
    pub loading: bool,
    pub error: Option<String>,
    /// The repository the loaded templates are for, so the page can tell a stale answer.
    pub loaded_for: Option<String>,
    /// The machine they were read on (a machine key).
    pub loaded_on: Option<String>,
    seq: u64,
}

impl NamingFeed {
    pub fn begin(&mut self) -> u64 {
        self.seq += 1;
        self.loading = true;
        self.error = None;
        self.seq
    }

    /// A failure leaves Fleet's own defaults (or the last answer) in place: the plan line previews with what it has.
    pub fn finish(&mut self, seq: u64, directory: Option<&str>, machine: &str, result: Result<NamingAnswer, String>) -> bool {
        if seq != self.seq {
            return false;
        }
        self.loading = false;
        match result {
            Ok(a) => {
                self.answer = a;
                self.loaded_for = directory.map(str::to_string);
                self.loaded_on = Some(machine.to_string());
            }
            Err(e) => self.error = Some(e),
        }
        true
    }
}

/// Everything the page has read, per machine key where a machine has its own.
#[derive(Debug, Default)]
pub struct Feeds {
    pub repos: HashMap<String, RepoFeed>,
    pub harnesses: HashMap<String, HarnessFeed>,
    /// Other machines' preferences; home's are the app's own (`State::prefs_for`).
    pub prefs: HashMap<String, Prefs>,
    pub profiles: HashMap<(String, String), Vec<Profile>>,
    pub catalogs: HashMap<String, CatalogFeed>,
    pub naming: NamingFeed,
    pub detail: Slot<Detail>,
    pub worktrees: Slot<Vec<Worktree>>,
    pub listing: Slot<Listing>,
    /// The locations that exist on the machine the folder menu is for; empty when the call failed.
    pub roots: Vec<String>,
    pub roots_error: Option<String>,
    pub defaults: Option<DirectoryDefaults>,
}

impl Feeds {
    pub fn repos(&self, machine: &str) -> RepoFeed {
        self.repos.get(machine).cloned().unwrap_or_default()
    }

    /// The home folder `~` stands for, once the machine says; otherwise paths are written as they are.
    pub fn home_folder(&self) -> Option<&str> {
        self.defaults.as_ref().and_then(|d| d.home.as_deref())
    }

    /// The branch a new folder starts on; `main` until the machine answers.
    pub fn first_branch(&self) -> &str {
        self.defaults.as_ref().map_or("main", |d| d.first_branch.as_str())
    }
}
