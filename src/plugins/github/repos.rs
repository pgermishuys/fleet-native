//! Your repositories, cached (`composables/use-github-repos.ts`): every page of `GET .../repos?page=N&per_page=100&sort=updated`
//! until one comes back short, kept 15 minutes, one request in flight. `clear` (on disconnect) bumps a generation so an answer
//! that was already on its way is dropped. The Clone view reads the same cache through GitHub's `RepositorySource`.
//!
//! The cache is shared with that source (a `Send + Sync` object that cannot reach `State`): the source only says it is wanted, and the
//! plugin's `keep` (run from `poll`) then asks, on a thread, and hands the answer back.
use crate::State;
use crate::fleet::http;
use crate::fleet::link::now_ms;
use crate::fleet::new_session::clone::{RemoteRepository, RepositorySource};
use crate::fleet::settings::HOME_MACHINE_KEY;
use serde_json::Value;
use std::sync::{Arc, Mutex, MutexGuard};

pub const PER_PAGE: usize = 100;
/// `CACHE_TTL_MS`, in the app's clock seconds.
pub const TTL: f64 = 15.0 * 60.0;
/// A bound on the loop: 100 pages is ten thousand repositories.
const MAX_PAGES: usize = 100;

/// `CachedGitHubRepo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repo {
    pub id: String,
    pub full_name: String,
    pub name: String,
    pub owner_login: String,
    pub private: bool,
    pub language: Option<String>,
    pub stars: u64,
}

#[derive(Debug, Default)]
pub struct Inner {
    pub repos: Vec<Repo>,
    pub loading: bool,
    pub error: Option<String>,
    /// The app's clock when the cache was filled (for the 15 minutes), and the wall clock then (for "updated 3m ago").
    pub(super) updated: Option<f64>,
    pub updated_ms: Option<i64>,
    generation: u64,
    /// The Clone view asked for it; `keep` answers.
    wanted: bool,
}

impl Inner {
    fn stale(&self, now: f64) -> bool {
        self.updated.is_none_or(|at| now - at > TTL)
    }

    /// `repoCacheSummary`: the line under "Repository cache".
    pub fn summary(&self, now_ms: i64, offset_secs: i64) -> String {
        if self.loading && self.repos.is_empty() {
            return "Loading repositories\u{2026}".into();
        }
        if let (Some(why), true) = (&self.error, self.repos.is_empty()) {
            return why.clone();
        }
        if self.repos.is_empty() {
            return "No repositories cached yet.".into();
        }
        match self.updated_ms {
            Some(at) => format!(
                "{} repos loaded \u{b7} updated {}",
                self.repos.len(),
                crate::fleet::new_session::machines::format_relative(at, now_ms, offset_secs)
            ),
            None => format!("{} repos loaded", self.repos.len()),
        }
    }

    /// The error under the summary, when there is one beside repositories that did load.
    pub fn error_beside(&self) -> Option<&str> {
        self.error.as_deref().filter(|_| !self.repos.is_empty())
    }
}

/// What the plugin and the Clone view both hold.
#[derive(Clone, Default)]
pub struct Cache(Arc<Mutex<Inner>>);

impl Cache {
    pub fn get(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `clear`: forgets everything and drops what is on its way.
    pub fn clear(&self) {
        let mut c = self.get();
        c.generation += 1;
        *c = Inner { generation: c.generation, ..Inner::default() };
    }

    /// The source the Clone view asks (`githubRepositorySource`).
    pub fn source(&self) -> Arc<dyn RepositorySource> {
        Arc::new(self.clone())
    }
}

impl RepositorySource for Cache {
    fn icon(&self) -> &'static str {
        "github"
    }
    fn refresh(&self) {
        self.get().wanted = true;
    }
    fn repositories(&self) -> Vec<RemoteRepository> {
        self.get()
            .repos
            .iter()
            .map(|r| RemoteRepository { id: r.id.clone(), full_name: r.full_name.clone(), name: r.name.clone() })
            .collect()
    }
}

/// `toCachedRepo`; an entry with no `full_name` is not a repository.
pub fn read_repo(v: &Value) -> Option<Repo> {
    let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let id = match v.get("id")? {
        Value::String(s) => s.clone(),
        n => n.as_u64()?.to_string(),
    };
    Some(Repo {
        id,
        full_name: text("full_name")?,
        name: text("name").unwrap_or_default(),
        owner_login: v.get("owner").and_then(|o| o.get("login")).and_then(Value::as_str).unwrap_or_default().to_string(),
        private: v.get("private").and_then(Value::as_bool).unwrap_or(false),
        language: text("language"),
        stars: v.get("stargazers_count").and_then(Value::as_u64).unwrap_or(0),
    })
}

/// Every page until a short one. Runs on a thread.
fn fetch_all(base: &str) -> Result<Vec<Repo>, String> {
    let mut all = Vec::new();
    for page in 1..=MAX_PAGES {
        let path = format!("/api/integrations/github/repos?page={page}&per_page={PER_PAGE}&sort=updated");
        let (status, body) = http::text_with("GET", base, &path, None).map_err(|_| "Failed to load repositories.".to_string())?;
        if !(200..300).contains(&status) {
            return Err(serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string))
                .unwrap_or_else(|| "Failed to fetch repositories.".into()));
        }
        let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&body) else { return Err("Failed to load repositories.".into()) };
        all.extend(items.iter().filter_map(read_repo));
        if items.len() < PER_PAGE {
            break;
        }
    }
    Ok(all)
}

impl State {
    /// `refresh(force)`: nothing when the cache is fresh (unless forced) or a request is out already.
    pub fn github_repos_refresh(&mut self, force: bool) {
        let cache = super::state(&mut self.places).repos.clone();
        let generation = {
            let mut c = cache.get();
            if (!force && !c.stale(self.clock)) || c.loading {
                return;
            }
            c.loading = true;
            c.error = None;
            c.generation
        };
        let into = cache.clone();
        let sent = self.feed_call(HOME_MACHINE_KEY, fetch_all, move |s, _, answer| {
            let mut c = into.get();
            if c.generation != generation {
                return;
            }
            c.loading = false;
            match answer {
                Ok(repos) => {
                    c.repos = repos;
                    c.updated = Some(s.clock);
                    c.updated_ms = Some(s.now_ms.unwrap_or_else(now_ms));
                    c.error = None;
                }
                Err(why) => c.error = Some(why),
            }
        });
        if !sent {
            let mut c = cache.get();
            c.loading = false;
            c.error = Some("Failed to load repositories.".into());
        }
    }

    /// From the plugin's `keep`: answers the Clone view when it asked. True when something drawn changed.
    pub(super) fn keep_github_repos(&mut self) -> bool {
        let cache = super::state(&mut self.places).repos.clone();
        let wanted = std::mem::take(&mut cache.get().wanted);
        if wanted {
            self.github_repos_refresh(false);
        }
        wanted
    }
}
