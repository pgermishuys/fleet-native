//! A repository's labels, milestones and assignees (`composables/use-github-metadata.ts`), for the issue filter's menus: one copy per
//! repository and kind, shared by everything that shows it, kept 5 minutes, one request in flight per repository. Each fetch has a
//! generation of its repository, so an answer that a newer fetch has overtaken is dropped.
use super::GitHub;
use crate::State;
use crate::fleet::http;
use crate::fleet::settings::HOME_MACHINE_KEY;
use serde_json::Value;
use std::collections::HashMap;

/// `CACHE_TTL_MS`, in the app's clock seconds.
pub const TTL: f64 = 5.0 * 60.0;

/// `GitHubLabel`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}

/// `GitHubMilestone`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Milestone {
    pub number: u64,
    pub title: String,
    pub open: bool,
    pub open_issues: u64,
    pub closed_issues: u64,
}

/// `GitHubAssignee`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignee {
    pub login: String,
    pub avatar_url: String,
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn read_label(v: &Value) -> Option<Label> {
    Some(Label { name: text(v, "name")?, color: text(v, "color").unwrap_or_default(), description: text(v, "description") })
}

fn read_milestone(v: &Value) -> Option<Milestone> {
    let count = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0);
    Some(Milestone {
        number: v.get("number")?.as_u64()?,
        title: text(v, "title")?,
        open: text(v, "state").as_deref() != Some("closed"),
        open_issues: count("open_issues"),
        closed_issues: count("closed_issues"),
    })
}

fn read_assignee(v: &Value) -> Option<Assignee> {
    Some(Assignee { login: text(v, "login")?, avatar_url: text(v, "avatar_url").unwrap_or_default() })
}

/// What is known of one repository's list of one kind (`RepoMetadataCacheEntry`).
#[derive(Debug)]
pub struct Entry<T> {
    pub data: Vec<T>,
    pub loading: bool,
    pub error: Option<String>,
    /// The app's clock when the copy arrived.
    pub updated: Option<f64>,
    generation: u64,
}

impl<T> Default for Entry<T> {
    fn default() -> Self {
        Entry { data: Vec::new(), loading: false, error: None, updated: None, generation: 0 }
    }
}

impl<T> Entry<T> {
    /// `isEntryStale`.
    pub fn stale(&self, now: f64) -> bool {
        self.updated.is_none_or(|at| now - at > TTL)
    }
}

/// One kind's entries, by `owner/repo`.
#[derive(Debug)]
pub struct Store<T>(HashMap<String, Entry<T>>);

impl<T> Default for Store<T> {
    fn default() -> Self {
        Store(HashMap::new())
    }
}

impl<T> Store<T> {
    pub fn get(&self, owner: &str, repo: &str) -> Option<&Entry<T>> {
        self.0.get(&format!("{owner}/{repo}"))
    }
}

/// The three kinds' stores, in the plugin's state.
#[derive(Debug, Default)]
pub struct Metadata {
    pub labels: Store<Label>,
    pub milestones: Store<Milestone>,
    pub assignees: Store<Assignee>,
}

impl Metadata {
    /// A repository's three lists as a scene draws them (`loading` leaves the labels unread and loading), with no request.
    pub fn show(&mut self, owner: &str, repo: &str, lists: (&Value, &Value, &Value), loading: bool) {
        fn put<T>(store: &mut Store<T>, owner: &str, repo: &str, list: &Value, read: fn(&Value) -> Option<T>, loading: bool) {
            let data = if loading { Vec::new() } else { list.as_array().into_iter().flatten().filter_map(read).collect() };
            let entry = Entry { data, loading, error: None, updated: (!loading).then_some(0.0), generation: 0 };
            store.0.insert(format!("{owner}/{repo}"), entry);
        }
        put(&mut self.labels, owner, repo, lists.0, read_label, loading);
        put(&mut self.milestones, owner, repo, lists.1, read_milestone, loading);
        put(&mut self.assignees, owner, repo, lists.2, read_assignee, loading);
    }
}

struct Kind<T: 'static> {
    /// "labels": in "Failed to fetch labels."
    name: &'static str,
    read: fn(&Value) -> Option<T>,
    store: fn(&mut GitHub) -> &mut Store<T>,
}

fn fetch<T: Send + 'static>(s: &mut State, kind: &'static Kind<T>, owner: &str, repo: &str, force: bool) {
    let key = format!("{owner}/{repo}");
    let now = s.clock;
    let entry = (kind.store)(super::state(&mut s.places)).0.entry(key.clone()).or_default();
    if (!force && !entry.stale(now)) || entry.loading {
        return;
    }
    entry.generation += 1;
    let generation = entry.generation;
    entry.loading = true;
    entry.error = None;
    let path = format!("/api/integrations/github/repos/{owner}/{repo}/{}", kind.name);
    let name = kind.name;
    let sent = s.feed_call(
        HOME_MACHINE_KEY,
        move |base| http::text_with("GET", base, &path, None),
        move |s, _, answer| {
            let clock = s.clock;
            let Some(entry) = (kind.store)(super::state(&mut s.places)).0.get_mut(&key) else { return };
            if entry.generation != generation {
                return;
            }
            entry.loading = false;
            match answer {
                Ok((status, body)) if (200..300).contains(&status) => {
                    let list = serde_json::from_str::<Value>(&body).unwrap_or_default();
                    entry.data = list.as_array().into_iter().flatten().filter_map(kind.read).collect();
                    entry.updated = Some(clock);
                }
                Ok((_, body)) => entry.error = Some(super::auth::words(&body, &format!("Failed to fetch {name}."))),
                Err(_) => entry.error = Some(format!("Failed to load {name}.")),
            }
        },
    );
    if !sent {
        let entry = (kind.store)(super::state(&mut s.places)).0.entry(format!("{owner}/{repo}")).or_default();
        entry.loading = false;
        entry.error = Some(format!("Failed to load {name}."));
    }
}

static LABELS: Kind<Label> = Kind { name: "labels", read: read_label, store: |g| &mut g.metadata.labels };
static MILESTONES: Kind<Milestone> = Kind { name: "milestones", read: read_milestone, store: |g| &mut g.metadata.milestones };
static ASSIGNEES: Kind<Assignee> = Kind { name: "assignees", read: read_assignee, store: |g| &mut g.metadata.assignees };

impl State {
    /// `useGitHubLabels`: reads the repository's labels when there is no fresh copy; `force` is its `refresh`.
    pub fn github_labels(&mut self, owner: &str, repo: &str, force: bool) {
        fetch(self, &LABELS, owner, repo, force);
    }
    pub fn github_milestones(&mut self, owner: &str, repo: &str, force: bool) {
        fetch(self, &MILESTONES, owner, repo, force);
    }
    pub fn github_assignees(&mut self, owner: &str, repo: &str, force: bool) {
        fetch(self, &ASSIGNEES, owner, repo, force);
    }
}
