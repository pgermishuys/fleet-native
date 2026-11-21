//! GitHub search (`composables/use-github-search.ts`): pull requests or issues a page of 30 at a time, `after` the last page's cursor.
//! A new query replaces the list after a short pause (250 ms on the app's clock) so typing in a filter doesn't send a request per
//! key; the first query of a search goes at once; no query asks for nothing. A newer request drops the answer of an older one.
//!
//! One `Search` per asker, named by it (the repository's page, the Your work page): the asker sets the query it wants each time it
//! draws, and `keep` (the plugin's poll) sends the ones whose pause is over. Nothing here waits; the request is on a thread.
use super::items::{ItemPage, ItemSummary, error_words};
use crate::State;
use crate::conv::export_history::encode_component;
use crate::fleet::http;
use crate::fleet::settings::HOME_MACHINE_KEY;
use std::collections::HashMap;

/// `DEBOUNCE_MS`, in the app's clock seconds.
pub const DEBOUNCE: f64 = 0.25;
/// Fleet's page size (`searchGitHubItems`' `first`).
pub const PAGE: usize = 30;
const FAILED: &str = "GitHub search failed.";

#[derive(Debug, Default)]
pub struct Search {
    /// The query asked for last (what Fleet's `watch` saw).
    pub query: Option<String>,
    pub items: Vec<ItemSummary>,
    pub total_count: u64,
    pub is_loading: bool,
    pub is_loading_more: bool,
    pub error: Option<String>,
    pub has_more: bool,
    cursor: Option<String>,
    /// Bumped by every request and by a cleared query: an answer of another generation is dropped (Fleet aborts the request).
    generation: u64,
    /// When the pause after a typed query is over, on the app's clock.
    due: Option<f64>,
    /// The first query goes at once (`previous === undefined`); every later one waits.
    asked_before: bool,
}

#[derive(Debug, Default)]
pub struct Searches(HashMap<String, Search>);

impl Searches {
    pub fn get(&self, id: &str) -> Option<&Search> {
        self.0.get(id)
    }
}

/// The request for one page: `q`, `first` and the cursor of the page before.
pub fn path(q: &str, after: Option<&str>) -> String {
    let mut path = format!("/api/integrations/github/search?q={}&first={PAGE}", encode_component(q));
    if let Some(after) = after.filter(|a| !a.is_empty()) {
        path.push_str(&format!("&after={}", encode_component(after)));
    }
    path
}

impl State {
    fn search_of(&mut self, id: &str) -> &mut Search {
        super::state(&mut self.places).search.0.entry(id.to_string()).or_default()
    }

    /// The query `id` wants now (Fleet's watch on the query). The same query again does nothing.
    pub fn github_search_set(&mut self, id: &str, query: Option<&str>) {
        let query = query.filter(|q| !q.is_empty()).map(str::to_string);
        let now = self.clock;
        let s = self.search_of(id);
        if s.asked_before && s.query == query {
            return;
        }
        let first = !s.asked_before;
        s.asked_before = true;
        s.query = query.clone();
        s.due = None;
        let Some(q) = query else {
            s.generation += 1;
            *s = Search { generation: s.generation, query: None, asked_before: true, ..Search::default() };
            return;
        };
        if first {
            self.github_search_load(id, &q, false);
        } else {
            s.is_loading = true;
            s.due = Some(now + DEBOUNCE);
        }
    }

    /// `refresh`: the first page again, whatever the pause.
    pub fn github_search_refresh(&mut self, id: &str) {
        if let Some(q) = self.search_of(id).query.clone() {
            self.github_search_load(id, &q, false);
        }
    }

    /// `loadMore`: the next page, added after the ones on screen; nothing when there is no next page.
    pub fn github_search_more(&mut self, id: &str) {
        let s = self.search_of(id);
        if let (Some(q), true) = (s.query.clone(), s.has_more) {
            self.github_search_load(id, &q, true);
        }
    }

    /// `onScopeDispose`: the asker is gone; its answer, if one is on its way, is dropped.
    pub fn github_search_drop(&mut self, id: &str) {
        super::state(&mut self.places).search.0.remove(id);
    }

    fn github_search_load(&mut self, id: &str, q: &str, append: bool) {
        let s = self.search_of(id);
        s.generation += 1;
        let generation = s.generation;
        if append {
            s.is_loading_more = true;
        } else {
            s.is_loading = true;
        }
        s.error = None;
        let path = path(q, if append { s.cursor.as_deref() } else { None });
        let key = id.to_string();
        let sent = self.feed_call(
            HOME_MACHINE_KEY,
            move |base| http::text_with("GET", base, &path, None),
            move |s, _, answer| {
                let Some(search) = super::state(&mut s.places).search.0.get_mut(&key) else { return };
                if search.generation != generation {
                    return;
                }
                search.is_loading = false;
                search.is_loading_more = false;
                match answer {
                    Ok((status, body)) if (200..300).contains(&status) => {
                        let page = ItemPage::read(&serde_json::from_str(&body).unwrap_or_default());
                        if append {
                            search.items.extend(page.items);
                        } else {
                            search.items = page.items;
                        }
                        search.total_count = page.total_count;
                        search.has_more = page.has_next_page;
                        search.cursor = page.end_cursor;
                    }
                    Ok((status, body)) => search.fail(append, error_words(status, &body)),
                    Err(_) => search.fail(append, FAILED.into()),
                }
            },
        );
        if !sent {
            let s = self.search_of(id);
            s.is_loading = false;
            s.is_loading_more = false;
            s.fail(append, FAILED.into());
        }
    }

    /// From the plugin's `keep`: sends the searches whose pause is over. True when something drawn changed.
    pub(super) fn keep_github_search(&mut self, now: f64) -> bool {
        let due: Vec<(String, String)> = super::state(&mut self.places)
            .search
            .0
            .iter_mut()
            .filter_map(|(id, s)| match (s.due, &s.query) {
                (Some(at), Some(q)) if at <= now => {
                    s.due = None;
                    Some((id.clone(), q.clone()))
                }
                _ => None,
            })
            .collect();
        for (id, q) in &due {
            self.github_search_load(id, q, false);
        }
        !due.is_empty()
    }
}

impl Search {
    /// A failed page: its reason, and no list when it was the first page.
    fn fail(&mut self, append: bool, why: String) {
        self.error = Some(why);
        if !append {
            self.items.clear();
        }
    }
}
