//! Scenes of the issue filter bar (`github-issues*`): the Issues tab of `acme/rocket` and each menu of the bar open over it, drawn from
//! invented data with no server.
use super::panel_scene::{self, Which};
use super::rows_scene;
use crate::State;
use crate::plugins::github::bar::Menu;
use crate::plugins::github::filter::{IssueFilter, StateChoice, issue_filter_query, serialize_filter_expression};
use crate::plugins::github::items::ItemSummary;
use crate::plugins::github::repo::{RepoPage, SEARCH, Tab};
use crate::plugins::github::{Where, mock, state};

fn issues() -> Vec<ItemSummary> {
    mock::issues().as_array().into_iter().flatten().map(ItemSummary::read).collect()
}

/// `which`: "" (the tab), "filtered" (labels, an author and a milestone chosen), "closed", "searching", "empty", or a menu: "label-menu",
/// "author-menu", "milestone-menu", "assignee-menu", "sort-menu", "label-menu-loading", "label-menu-empty".
pub fn set(s: &mut State, which: &str, dark: bool) {
    panel_scene::scene(s, dark, Which::OnRepo);
    rows_scene::link(s);
    let mut filter = IssueFilter::default();
    let mut menu = None;
    let (mut typed, mut found, mut searching, mut loading) = ("", issues(), false, false);
    match which {
        "" => {}
        "filtered" => {
            filter.labels = vec!["bug".into(), "good first issue".into()];
            filter.author = Some("kit-lee".into());
            found.retain(|i| i.number == 203);
        }
        "closed" => {
            filter.state = StateChoice::Closed;
            found.clear();
        }
        "searching" => searching = true,
        "empty" => {
            filter.search = "tomorrow".into();
            found.clear();
        }
        "label-menu" => menu = Some(Menu::Label),
        "label-menu-picked" => {
            filter.labels = vec!["bug".into(), "performance".into()];
            menu = Some(Menu::Label);
            found.retain(|i| i.number == 205);
        }
        "label-menu-typed" => {
            menu = Some(Menu::Label);
            typed = "per";
        }
        "label-menu-loading" => {
            menu = Some(Menu::Label);
            loading = true;
        }
        "label-menu-empty" => {
            menu = Some(Menu::Label);
            typed = "zzz";
            loading = false;
        }
        "author-menu" => {
            menu = Some(Menu::Author);
            filter.author = Some("devon-park".into());
            found.retain(|i| i.author.as_deref() == Some("devon-park"));
        }
        "milestone-menu" => menu = Some(Menu::Milestone),
        "assignee-menu" => menu = Some(Menu::Assignee),
        "sort-menu" => menu = Some(Menu::Sort),
        other => panic!("no issue filter scene called {other}"),
    }
    let query = format!("repo:acme/rocket is:issue {}", issue_filter_query(&filter));
    let g = state(&mut s.places);
    g.bookmarks.list.pop();
    g.bookmarks.list.pop();
    g.search.show(SEARCH, &query, found, false, searching, None);
    let empty = serde_json::Value::Array(Vec::new());
    let lists = (&mock::labels(), &mock::milestones(), &mock::assignees());
    g.metadata.show("acme", "rocket", if which == "label-menu-empty" { (&empty, lists.1, lists.2) } else { lists }, loading);
    let mut page = RepoPage { asked: Some(("acme".into(), "rocket".into())), tab: Tab::Issues, ..RepoPage::default() };
    page.bar.expr.replace_text(serialize_filter_expression(&filter));
    page.bar.menu = menu;
    page.bar.query.replace_text(typed);
    page.issues = filter;
    g.repo = page;
    g.at = Where::Repo { owner: "acme".into(), repo: "rocket".into() };
}
