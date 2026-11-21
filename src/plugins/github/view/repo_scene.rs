//! Scenes of a repository's page (`github-repo*`): each state the page can be in, drawn from invented data with no server (`acme/rocket`,
//! signed in as `mara-quinn`, a few sessions on its pull requests).
use super::panel_scene::{self, Which};
use super::rows_scene;
use crate::State;
use crate::plugins::github::items::ItemSummary;
use crate::plugins::github::repo::{PullFilter, RepoPage, SEARCH, fleet_numbers};
use crate::plugins::github::{Where, mock, state};

/// The invented pull requests of `acme/rocket`, by number.
fn pulls(numbers: &[u64]) -> Vec<ItemSummary> {
    let all: Vec<ItemSummary> = mock::rows().as_array().into_iter().flatten().map(|r| ItemSummary::read(&r["item"])).collect();
    numbers.iter().filter_map(|n| all.iter().find(|i| i.number == *n).cloned()).collect()
}

pub fn set(s: &mut State, which: &str, dark: bool) {
    panel_scene::scene(s, dark, Which::OnRepo);
    rows_scene::link(s);
    let query = "repo:acme/rocket is:pr is:open sort:updated-desc";
    let open = pulls(&[218, 220, 221, 222]);
    let mut page = RepoPage { asked: Some(("acme".into(), "rocket".into())), ..RepoPage::default() };
    let g = state(&mut s.places);
    g.bookmarks.list.pop();
    g.bookmarks.list.pop();
    match which {
        "" => g.search.show(SEARCH, query, open, false, false, None),
        "more" => g.search.show(SEARCH, query, open, true, false, None),
        "loading" => g.search.show(SEARCH, query, Vec::new(), false, true, None),
        "error" => g.search.show(SEARCH, query, Vec::new(), false, false, Some("GitHub rate limit exceeded. Try again in a few minutes.")),
        "empty" => g.search.show(SEARCH, query, Vec::new(), false, false, None),
        "fleet" => page.filter = PullFilter::Fleet,
        other => panic!("no repository page scene called {other}"),
    }
    state(&mut s.places).repo = page;
    if which == "fleet" {
        let numbers = fleet_numbers(s, "acme", "rocket");
        let f = &mut state(&mut s.places).repo.fleet;
        f.key = Some(numbers.iter().map(u64::to_string).collect::<Vec<_>>().join(","));
        f.items = pulls(&numbers);
    }
    state(&mut s.places).at = Where::Repo { owner: "acme".into(), repo: "rocket".into() };
}
