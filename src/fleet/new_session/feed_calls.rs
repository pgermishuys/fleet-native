//! The calls that fill `Feeds`, each on a thread of its own (component 7, piece C). Never on the UI thread: the answer is
//! handed back with `Post::update`, and the state's own rules (`feed_state`) drop an answer a newer read has overtaken.
//! Every call goes to the machine the page is working on: home is the Fleet the app is connected to, another machine is
//! reached with its own address and token (`useMachineTarget`). With no server (the invented data) nothing is asked.
use super::feed_state::{Ask, CatalogKey, Feeds, RepoFeed};
use super::feeds::*;
use super::harness_choice::{DEFAULT_HARNESS_PREFERENCE_KEY, Harnesses};
use crate::fleet::events::{Event, Kind};
use crate::fleet::groups::{ProjectInfo, ProjectKind};
use crate::fleet::http;
use crate::fleet::link::now_ms;
use crate::fleet::prefs::Prefs;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::{State, Tx};

pub(crate) type Done<T> = Box<dyn FnOnce(&mut State, &mut Tx, T) + Send>;

impl State {
    /// Where machine `key` is: home's address, or the address of a machine home lists. None with no server or an unknown key.
    pub fn feed_base(&self, key: &str) -> Option<String> {
        let link = self.link.as_ref()?;
        if key == HOME_MACHINE_KEY {
            return Some(link.base.clone());
        }
        self.machines.machine(key).map(|m| m.base().to_string())
    }

    /// Runs `call` against machine `key` on a thread; `done` runs on the UI thread. False (and nothing runs) when the
    /// machine can't be reached from here.
    pub(crate) fn feed_call<T: Send + 'static>(
        &self,
        key: &str,
        call: impl FnOnce(&str) -> T + Send + 'static,
        done: impl FnOnce(&mut State, &mut Tx, T) + Send + 'static,
    ) -> bool {
        let (Some(base), Some(link)) = (self.feed_base(key), &self.link) else { return false };
        let post = link.post();
        std::thread::spawn(move || {
            let answer = call(&base);
            post.update(move |s, tx| done(s, tx, answer));
        });
        true
    }

    /// Preferences of machine `key`: home's are the app's own, shared with the composer.
    pub fn prefs_for(&self, key: &str) -> Prefs {
        if key == HOME_MACHINE_KEY {
            return Prefs { values: self.pickers.prefs.values.clone(), fetched: self.pickers.prefs.fetched };
        }
        self.feeds.prefs.get(key).map(|p| Prefs { values: p.values.clone(), fetched: p.fetched }).unwrap_or_default()
    }

    /// The user's default harness on machine `key`: its preference once known, else (another machine) the one saved from
    /// the last visit, shown at once.
    fn preferred_harness(&self, key: &str) -> Option<String> {
        let prefs = self.prefs_for(key);
        if prefs.fetched {
            return prefs.get(DEFAULT_HARNESS_PREFERENCE_KEY).filter(|v| !v.is_empty()).map(str::to_string);
        }
        (key != HOME_MACHINE_KEY).then(|| self.sidebar.saved_default_harness(key).map(str::to_string)).flatten()
    }

    /// The harness list of machine `key` with its preferred one filled in: what the page's choice logic reads.
    pub fn harness_choice(&self, key: &str) -> Harnesses {
        let mut h = self.feeds.harnesses.get(key).map(|f| f.harnesses.clone()).unwrap_or_default();
        h.preferred = self.preferred_harness(key);
        h
    }

    /// The projects of machine `key` for the Project field: the same read the session list makes (`GET /api/projects`),
    /// without the Scratch one, which is the default group.
    pub fn new_session_projects(&self, key: &str) -> Vec<ProjectInfo> {
        let all = if key == HOME_MACHINE_KEY {
            self.project_load.projects.clone()
        } else {
            self.machines.others.get(key).map(|m| m.projects.clone()).unwrap_or_default()
        };
        all.into_iter().filter(|p| p.kind != ProjectKind::Scratch).collect()
    }

    // ─── repositories ───────────────────────────────────────────────────────────

    /// The scans saved on the last visit show at once (called when the settings file is read).
    pub fn seed_feeds_from_settings(&mut self) {
        let saved: Vec<(String, Scan)> = self.sidebar.saved_repositories().map(|(k, s)| (k.clone(), s.clone())).collect();
        for (key, scan) in saved {
            let feed = self.feeds.repos.entry(key).or_default();
            if feed.scanned_at.is_none() && feed.list.is_empty() {
                *feed = RepoFeed::seeded(scan);
            }
        }
    }

    /// `GET /api/repositories` (on mount), or `POST /api/repositories/refresh` after a folder is added.
    pub fn read_repositories(&mut self, key: &str, refresh: bool) {
        if self.feed_base(key).is_none() {
            return;
        }
        self.feeds.repos.entry(key.to_string()).or_default().begin();
        let key = key.to_string();
        let machine = key.clone();
        self.feed_call(
            &machine,
            move |base| parse_scan(if refresh { http::repositories_refresh(base) } else { http::repositories(base) }),
            move |s, _, result| {
                let saved = result.clone().ok();
                if s.feeds.repos.entry(key.clone()).or_default().finish(result)
                    && let Some(scan) = saved
                {
                    s.sidebar.save_repositories(&key, &scan);
                    s.save_settings();
                }
            },
        );
    }

    /// A repository's branches and remotes; `None` cancels the read still on its way and clears.
    pub fn read_detail(&mut self, key: &str, path: Option<&str>) {
        let Some(seq) = self.feeds.detail.begin(path, false) else { return };
        let asked = path.unwrap_or_default().to_string();
        self.feed_call(
            key,
            move |base| parse_detail(http::repository_detail(base, &asked)),
            move |s, _, r| {
                s.feeds.detail.finish(seq, r);
            },
        );
    }

    /// A repository's worktrees; the last list stays up while the next loads, and a failure leaves none.
    pub fn read_worktrees(&mut self, key: &str, path: Option<&str>) {
        let Some(seq) = self.feeds.worktrees.begin(path, true) else { return };
        let asked = path.unwrap_or_default().to_string();
        self.feed_call(
            key,
            move |base| parse_worktrees(http::repository_worktrees(base, &asked)),
            move |s, _, r| {
                s.feeds.worktrees.finish_or(seq, r, true);
            },
        );
    }

    // ─── folders ────────────────────────────────────────────────────────────────

    /// The locations on disk, each time the folder menu opens: any failure leaves none (no Location, no Create or Clone).
    pub fn read_roots(&mut self, key: &str) {
        self.feed_call(
            key,
            |base| parse_roots(http::workspace_roots(base)),
            |s, _, r| match r {
                Ok(roots) => {
                    s.feeds.roots = roots;
                    s.feeds.roots_error = None;
                }
                Err(e) => {
                    s.feeds.roots.clear();
                    s.feeds.roots_error = Some(e);
                }
            },
        );
    }

    /// What a new folder gets and where `~` is, once per page; a failure is `main` and an unknown home.
    pub fn read_directory_defaults(&mut self, key: &str) {
        self.feed_call(key, |base| parse_defaults(http::directory_defaults(base)), |s, _, d| s.feeds.defaults = Some(d));
    }

    /// Whether a folder exists, is a git repository and is inside the locations. One answer, for whoever asked.
    pub fn inspect_folder(&self, key: &str, path: &str, done: Done<Result<Inspection, String>>) -> bool {
        let path = path.to_string();
        self.feed_call(key, move |base| parse_inspection(http::inspect_directory(base, &path)), done)
    }

    /// Lets sessions run in a folder (`POST /api/workspace-roots`). The caller scans again with `read_repositories`.
    pub fn add_workspace_root(&self, key: &str, path: &str, done: Done<Result<(), String>>) -> bool {
        let path = path.to_string();
        self.feed_call(key, move |base| parse_added_root(http::add_workspace_root(base, &path)), done)
    }

    /// The folders in `path`, anywhere on the machine; `None` (the menu closed, or a newer listing starts) drops what is
    /// still on its way.
    pub fn list_folder(&mut self, key: &str, path: Option<&str>) {
        let Some(seq) = self.feeds.listing.begin(path, false) else { return };
        let asked = path.unwrap_or_default().to_string();
        self.feed_call(
            key,
            move |base| parse_listing(&asked, http::list_directory(base, &asked)),
            move |s, _, r| {
                let error = r.as_ref().err().cloned();
                if s.feeds.listing.finish(seq, r) {
                    // A failed listing's words stay until a later one succeeds (B2.133).
                    s.folder_menu.listing_error = error;
                }
            },
        );
    }

    // ─── harnesses, profiles, preferences ───────────────────────────────────────

    /// The harness list: a plain read is shared with one already running; `fresh` makes the machine check every harness
    /// again first (Check again, a harness that just changed).
    pub fn read_harnesses(&mut self, key: &str, fresh: bool) {
        if self.feed_base(key).is_none() {
            return;
        }
        let Some(seq) = self.feeds.harnesses.entry(key.to_string()).or_default().begin(fresh) else { return };
        let machine = key.to_string();
        self.feed_call(
            key,
            move |base| parse_harnesses(http::harness_list(base, fresh)),
            move |s, _, r| {
                s.feeds.harnesses.entry(machine).or_default().finish(seq, fresh, now_ms(), r);
            },
        );
    }

    /// A harness's profiles; a failure leaves the list as it was.
    pub fn read_profiles(&mut self, key: &str, harness: &str) {
        let (machine, harness) = (key.to_string(), harness.to_string());
        let asked = harness.clone();
        self.feed_call(
            key,
            move |base| parse_profiles(http::harness_profiles(base, &asked)),
            move |s, _, r| {
                if let Some(list) = r {
                    s.feeds.profiles.insert((machine, harness), list);
                }
            },
        );
    }

    /// The machine's preferences. Home's are the app's own (`load_preferences`); another machine's are kept here, and its
    /// default harness is saved for the next visit.
    pub fn read_preferences(&mut self, key: &str) {
        if key == HOME_MACHINE_KEY {
            self.load_preferences();
            return;
        }
        let machine = key.to_string();
        self.feed_call(
            key,
            |base| http::preference_map(base).ok().filter(|(s, _)| (200..300).contains(s)).and_then(|(_, b)| serde_json::from_str(&b).ok()),
            move |s, _, body: Option<serde_json::Value>| {
                let Some(body) = body else { return };
                s.feeds.prefs.entry(machine.clone()).or_default().adopt(&body);
                let preferred = s.feeds.prefs[&machine].get(DEFAULT_HARNESS_PREFERENCE_KEY).filter(|v| !v.is_empty()).map(str::to_string);
                s.sidebar.save_default_harness(&machine, preferred.as_deref());
                s.save_settings();
            },
        );
    }

    // ─── naming and the catalog ─────────────────────────────────────────────────

    /// The worktree naming templates in force for `directory` (the user's own when none) on machine `key`.
    pub fn read_naming(&mut self, key: &str, directory: Option<&str>) {
        if self.feed_base(key).is_none() {
            return;
        }
        let seq = self.feeds.naming.begin();
        let (machine, dir) = (key.to_string(), directory.map(str::to_string));
        let asked = dir.clone();
        self.feed_call(
            key,
            move |base| parse_naming(http::worktree_naming(base, asked.as_deref())),
            move |s, _, r| {
                s.feeds.naming.finish(seq, dir.as_deref(), &machine, r);
            },
        );
    }

    /// The picker asks for the agents and models of `want`: what is cached shows at once, and the harness is asked when the
    /// cache is old or empty. `None` (no harness) clears.
    pub fn select_catalog(&mut self, key: &str, want: Option<CatalogKey>) {
        let ask = self.feeds.catalogs.entry(key.to_string()).or_default().select(want, now_ms());
        if let Ask::Fetch(seq) = ask {
            self.fetch_catalog(key, seq);
        }
    }

    fn fetch_catalog(&self, key: &str, seq: u64) {
        let Some(wanted) = self.feeds.catalogs.get(key).and_then(|c| c.wanted.clone()) else { return };
        let machine = key.to_string();
        self.feed_call(
            key,
            move |base| parse_catalog(http::harness_catalog(base, &wanted.harness, wanted.directory.as_deref(), wanted.profile.as_deref())),
            move |s, _, r| {
                if let Some(feed) = s.feeds.catalogs.get_mut(&machine) {
                    feed.finish(seq, now_ms(), r);
                }
            },
        );
    }

    /// `harness.catalog_changed`: the catalogs it is about are forgotten, and the one on screen is asked for again with the
    /// old one up meanwhile. Pushed by the Fleet the app is connected to, so only home's catalogs hear it.
    pub fn on_feeds_event(&mut self, event: &Event) {
        let Event::Other(Kind::HarnessCatalogChanged, props) = event else { return };
        let Some(change) = CatalogChange::parse(props) else { return };
        let Some(feed) = self.feeds.catalogs.get_mut(HOME_MACHINE_KEY) else { return };
        if feed.on_change(&change)
            && let Some(seq) = feed.refetch()
        {
            self.fetch_catalog(HOME_MACHINE_KEY, seq);
        }
    }
}

impl Feeds {
    /// The profiles of `harness` on machine `key`; none until read.
    pub fn profiles_of(&self, key: &str, harness: &str) -> &[Profile] {
        self.profiles.get(&(key.to_string(), harness.to_string())).map_or(&[], Vec::as_slice)
    }

    /// The default profile (`defaultFor`).
    pub fn default_profile(&self, key: &str, harness: &str) -> Option<&Profile> {
        self.profiles_of(key, harness).iter().find(|p| p.is_default)
    }
}
