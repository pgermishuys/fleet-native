//! A repository's page (`pages/GitHubRepoPage.vue`, rows 8.x of docs/parity/21-github.md): which tab and filter it is on, the query each
//! makes, the pull requests Fleet's sessions are on (In Fleet), and what it asks for when it opens. The page that draws it is
//! `view::repo`. Pure but for `keep_github_repo`, which runs from the plugin's `keep`.
use super::bar::Bar;
use super::filter::{IssueFilter, issue_filter_query};
use super::item::Asked;
use super::items::ItemSummary;
use super::{ID, Where, state, state_ref};
use crate::fleet::smart_links::is_pull_request;
use crate::{Rail, State, Tx};
use rune::TextEdit;
use std::time::Duration;

/// The page's search, named in the plugin's searches.
pub const SEARCH: &str = "repo-page";
/// The filter box's key.
pub const FILTER_INPUT: &str = "github-repo-filter";
const FAILED: &str = "GitHub didn't answer.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Pulls,
    Issues,
}

/// `PullFilter`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PullFilter {
    #[default]
    Open,
    Review,
    Mine,
    Fleet,
    Closed,
}

/// `PULL_FILTERS`, in the order the chips are drawn.
pub const PULL_FILTERS: [PullFilter; 5] = [PullFilter::Open, PullFilter::Review, PullFilter::Mine, PullFilter::Fleet, PullFilter::Closed];

impl PullFilter {
    /// The `{id}` of `github-pull-filter-{id}`.
    pub fn id(self) -> &'static str {
        match self {
            PullFilter::Open => "open",
            PullFilter::Review => "review",
            PullFilter::Mine => "mine",
            PullFilter::Fleet => "fleet",
            PullFilter::Closed => "closed",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PullFilter::Open => "Open",
            PullFilter::Review => "Needs my review",
            PullFilter::Mine => "Mine",
            PullFilter::Fleet => "In Fleet",
            PullFilter::Closed => "Closed",
        }
    }

    fn qualifier(self) -> &'static str {
        match self {
            PullFilter::Open => "is:open",
            PullFilter::Review => "is:open review-requested:@me",
            PullFilter::Mine => "is:open author:@me",
            PullFilter::Fleet => "",
            PullFilter::Closed => "is:closed",
        }
    }

    /// `emptyWords` for this filter on the Pull requests tab.
    pub fn empty_words(self) -> &'static str {
        match self {
            PullFilter::Review => "Nobody is waiting on your review here.",
            PullFilter::Mine => "You have no open pull requests here.",
            PullFilter::Fleet => "No session is working on a pull request here.",
            PullFilter::Closed => "No closed pull requests match.",
            PullFilter::Open => "No open pull requests match.",
        }
    }
}

/// The pull requests Fleet's sessions are on, fetched by number.
#[derive(Debug, Default)]
pub struct FleetPulls {
    /// The numbers last asked for, as Fleet's watch key; `None` while the list isn't on screen.
    pub(super) key: Option<String>,
    /// Bumped by every change of the key: an answer of another generation is dropped (Fleet aborts the request).
    generation: u64,
    pub items: Vec<ItemSummary>,
    pub loading: bool,
    pub error: Option<String>,
}

#[derive(Debug, Default)]
pub struct RepoPage {
    /// The repository the page is set up for; a different one resets everything below.
    pub asked: Option<(String, String)>,
    pub tab: Tab,
    pub filter: PullFilter,
    /// The pull requests' filter box.
    pub text: TextEdit,
    pub issues: IssueFilter,
    /// The issue filter bar: its field and its open menu.
    pub bar: Bar,
    pub fleet: FleetPulls,
}

/// What the list on screen is (`list`).
pub struct Listed<'a> {
    pub items: &'a [ItemSummary],
    pub loading: bool,
    pub error: Option<&'a str>,
    pub has_more: bool,
    pub loading_more: bool,
}

impl RepoPage {
    /// `pullQuery`: `repo:o/r is:pr {qualifier} {typed} sort:updated-desc`, spaces collapsed; none on Issues or under In Fleet.
    pub fn pull_query(&self, owner: &str, repo: &str) -> Option<String> {
        if self.tab != Tab::Pulls || self.filter == PullFilter::Fleet {
            return None;
        }
        let q = format!("repo:{owner}/{repo} is:pr {} {} sort:updated-desc", self.filter.qualifier(), self.text.text.trim());
        Some(q.split_whitespace().collect::<Vec<_>>().join(" "))
    }

    /// `issueQuery`: `repo:o/r is:issue {the filter bar's qualifiers}`; none on Pull requests.
    pub fn issue_query(&self, owner: &str, repo: &str) -> Option<String> {
        (self.tab == Tab::Issues).then(|| format!("repo:{owner}/{repo} is:issue {}", issue_filter_query(&self.issues)))
    }

    /// The query the search should hold now.
    pub fn query(&self, owner: &str, repo: &str) -> Option<String> {
        if self.tab == Tab::Issues { self.issue_query(owner, repo) } else { self.pull_query(owner, repo) }
    }

    /// `emptyWords`.
    pub fn empty_words(&self) -> &'static str {
        if self.tab == Tab::Issues { "No issues match." } else { self.filter.empty_words() }
    }
}

/// `toggleLabel` on Pull requests: `label:x` (quoted when it has a space) is added to the typed filter, or taken out when it is there.
pub fn toggled_text(text: &str, label: &str) -> String {
    let qualifier = if label.chars().any(char::is_whitespace) { format!("label:\"{label}\"") } else { format!("label:{label}") };
    if text.contains(&qualifier) {
        text.replacen(&qualifier, "", 1).trim().to_string()
    } else {
        format!("{text} {qualifier}").trim().to_string()
    }
}

/// `fleetNumbers`: the pull request numbers of this repository any session's header links point at, newest number first.
pub fn fleet_numbers(s: &State, owner: &str, repo: &str) -> Vec<u64> {
    let prefix = format!("{owner}/{repo}#").to_lowercase();
    let mut numbers: Vec<u64> = s
        .smart_links
        .header_by_session
        .values()
        .flatten()
        .filter(|link| is_pull_request(link))
        .filter_map(|link| link.resource_id.to_lowercase().strip_prefix(&prefix).and_then(|n| n.parse::<u64>().ok()))
        .filter(|n| *n > 0)
        .collect();
    numbers.sort_unstable_by(|a, b| b.cmp(a));
    numbers.dedup();
    numbers
}

impl State {
    /// The repository the page shows, when the GitHub rail is on a repository.
    pub fn github_repo_shown(&self) -> Option<(String, String)> {
        match (&state_ref(&self.places).at, self.active_rail()) {
            (Where::Repo { owner, repo }, Some(Rail::Plugin(ID))) => Some((owner.clone(), repo.clone())),
            _ => None,
        }
    }

    /// The list on screen, with the numbers of In Fleet already read.
    pub fn github_repo_list(&self) -> Listed<'_> {
        let g = state_ref(&self.places);
        if g.repo.tab == Tab::Pulls && g.repo.filter == PullFilter::Fleet {
            let f = &g.repo.fleet;
            return Listed { items: &f.items, loading: f.loading, error: f.error.as_deref(), has_more: false, loading_more: false };
        }
        match g.search.get(SEARCH) {
            Some(q) => Listed {
                items: &q.items,
                loading: q.is_loading,
                error: q.error.as_deref(),
                has_more: q.has_more,
                loading_more: q.is_loading_more,
            },
            None => Listed { items: &[], loading: false, error: None, has_more: false, loading_more: false },
        }
    }

    /// `refresh`: the list on screen again (nothing for In Fleet's).
    pub fn github_repo_refresh(&mut self) {
        let r = &state_ref(&self.places).repo;
        if r.tab == Tab::Issues || r.filter != PullFilter::Fleet {
            self.github_search_refresh(SEARCH);
        }
    }

    /// `loadMore`.
    pub fn github_repo_more(&mut self) {
        self.github_search_more(SEARCH);
    }

    pub fn github_repo_tab(&mut self, tab: Tab) {
        state(&mut self.places).repo.tab = tab;
    }

    pub fn github_repo_filter(&mut self, filter: PullFilter) {
        state(&mut self.places).repo.filter = filter;
    }

    /// A press on a label of a row.
    pub fn github_repo_toggle_label(&mut self, label: &str) {
        let r = &mut state(&mut self.places).repo;
        if r.tab == Tab::Pulls {
            let next = toggled_text(&r.text.text, label);
            r.text.replace_text(&next);
        } else if r.issues.labels.iter().any(|l| l == label) {
            r.issues.labels.retain(|l| l != label);
        } else {
            r.issues.labels.push(label.to_string());
        }
    }

    /// What the page does while it is on screen: reads the work (the counts) and the sessions' links when it opens, starts over for
    /// another repository, keeps the search on the query the tab and filter make, and fetches In Fleet's numbers when they change.
    /// True when something drawn changed.
    pub(super) fn keep_github_repo(&mut self, tx: &mut Tx) -> bool {
        let shown = self.github_repo_shown();
        let mut changed = false;
        if shown != state_ref(&self.places).repo.asked {
            if state_ref(&self.places).repo.asked.is_some() {
                self.github_search_drop(SEARCH);
            }
            state(&mut self.places).repo = RepoPage { asked: shown.clone(), ..RepoPage::default() };
            changed = true;
            if let Some((owner, repo)) = &shown {
                super::sessions::ensure_loaded(self);
                self.github_work_load();
                // `useGitHubLabels` and the others read when the page mounts; the bar's menus open on what they hold.
                self.github_labels(owner, repo, false);
                self.github_milestones(owner, repo, false);
                self.github_assignees(owner, repo, false);
            }
        }
        let Some((owner, repo)) = shown else { return changed };
        let query = state_ref(&self.places).repo.query(&owner, &repo);
        let before = state_ref(&self.places).search.get(SEARCH).map(|q| (q.query.clone(), q.is_loading));
        self.github_search_set(SEARCH, query.as_deref());
        changed |= before != state_ref(&self.places).search.get(SEARCH).map(|q| (q.query.clone(), q.is_loading));
        changed |= self.keep_github_fleet(&owner, &repo);
        if state_ref(&self.places).repo.tab == Tab::Issues {
            changed |= self.keep_github_bar(tx);
        }
        // A typed query is sent after its pause; the clock has to come round to it.
        if state_ref(&self.places).search.get(SEARCH).is_some_and(|q| q.is_loading) {
            tx.wake_after(Duration::from_millis(100));
        }
        changed
    }

    /// `watch(fleetNumbers)`: a new set of numbers while In Fleet is on screen reloads; another one drops the answer on its way.
    fn keep_github_fleet(&mut self, owner: &str, repo: &str) -> bool {
        let r = &state_ref(&self.places).repo;
        let wanted = (r.tab == Tab::Pulls && r.filter == PullFilter::Fleet)
            .then(|| fleet_numbers(self, owner, repo).iter().map(u64::to_string).collect::<Vec<_>>().join(","));
        if wanted == r.fleet.key {
            return false;
        }
        let f = &mut state(&mut self.places).repo.fleet;
        f.key = wanted.clone();
        f.generation += 1;
        let generation = f.generation;
        let Some(key) = wanted else {
            f.loading = false;
            return true;
        };
        f.loading = true;
        f.error = None;
        let numbers: Vec<u64> = key.split(',').filter_map(|n| n.parse().ok()).collect();
        let asked = self.github_items_by_number(owner, repo, &numbers, move |s, _, answer| {
            let f = &mut state(&mut s.places).repo.fleet;
            if f.generation != generation {
                return;
            }
            f.loading = false;
            match answer {
                Ok(items) => f.items = items,
                Err(why) => f.error = Some(why),
            }
        });
        let f = &mut state(&mut self.places).repo.fleet;
        match asked {
            Asked::Nothing => {
                f.items.clear();
                f.loading = false;
            }
            Asked::Unreachable => {
                f.loading = false;
                f.error = Some(FAILED.into());
            }
            Asked::Sent => {}
        }
        true
    }
}
