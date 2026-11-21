//! Follow a repository (`components/AddRepositoryDialog.vue`): which of your repositories the dialog offers, what typing does to the
//! list, and what picking one does. The dialog itself is `view::follow`.
//!
//! Fleet lists the cached repositories you do not follow already, filtered as you type by the command list's `contains` (a plain,
//! case-blind substring of the full name).
use super::repos::Repo;
use super::{state, state_ref};
use crate::{State, Tx};
use rune::TextEdit;

pub const INPUT: &str = "github-follow-input";

#[derive(Debug, Default)]
pub struct Follow {
    pub open: bool,
    pub query: TextEdit,
    /// The highlighted row: none until you type, press an arrow or point at one (Fleet's list opens with no row lit; typing lights the
    /// first).
    pub active: Option<usize>,
}

/// The repositories to list: the cached ones not followed whose full name contains what was typed, ignoring case (cmdk's `contains`, so
/// there is no ranking: the cache's order stands).
pub fn listed(repos: &[Repo], followed: impl Fn(&str) -> bool, query: &str) -> Vec<Repo> {
    let q = query.to_lowercase();
    repos.iter().filter(|r| !followed(&r.full_name) && r.full_name.to_lowercase().contains(&q)).cloned().collect()
}

impl State {
    /// The rows as they stand now.
    pub fn github_follow_rows(&self) -> Vec<Repo> {
        let g = state_ref(&self.places);
        let (cache, marks) = (g.repos.get(), &g.bookmarks);
        listed(&cache.repos, |n| marks.has(n), &g.follow.query.text)
    }

    /// The panel's Follow a repository: the dialog opens empty with the search focused, and the cache is read again if it is stale.
    pub fn github_follow_open(&mut self, tx: &mut Tx) {
        state(&mut self.places).follow = Follow { open: true, ..Follow::default() };
        self.github_repos_refresh(false);
        tx.focus(INPUT, false);
    }

    /// Esc, the close button, a click outside, or a repository picked.
    pub fn github_follow_close(&mut self) {
        state(&mut self.places).follow.open = false;
    }

    /// ↓ and ↑ move the highlight and wrap at both ends; from no row they land on the first and the last.
    pub fn github_follow_step(&mut self, by: isize) -> Option<String> {
        let rows = self.github_follow_rows();
        if rows.is_empty() {
            return None;
        }
        let f = &mut state(&mut self.places).follow;
        let next = match f.active {
            Some(at) => (at as isize + by).rem_euclid(rows.len() as isize) as usize,
            None if by > 0 => 0,
            None => rows.len() - 1,
        };
        f.active = Some(next);
        rows.get(next).map(|r| r.full_name.clone())
    }

    /// The list changed under the search: the first row is lit.
    pub fn github_follow_typed(&mut self) {
        state(&mut self.places).follow.active = Some(0);
    }

    /// `add`: follows it, closes, and reads your work again once the server has said yes (`added`).
    pub fn github_follow_pick(&mut self, full_name: &str) {
        self.github_follow_close();
        self.github_bookmark_add_and_reload(full_name);
    }

    pub fn github_follow_pick_active(&mut self) {
        let Some(active) = state_ref(&self.places).follow.active else { return };
        if let Some(r) = self.github_follow_rows().get(active) {
            let name = r.full_name.clone();
            self.github_follow_pick(&name);
        }
    }
}
