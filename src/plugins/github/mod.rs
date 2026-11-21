//! The GitHub plugin (`plugins/builtin/github/index.ts`): Fleet's pull requests, issues and checks beside its sessions. It is an
//! add-on: everything it puts in the app goes through the places core offers (`fleet::places`), and core never names it.
//!
//! Piece A of component 21 gives it its home and its places: the descriptor, the rail button, the settings page (the connection
//! page, `view::connect`), the connection's `keep`, and the addresses it owns (`Where`). Piece B adds what the pages read: items and the words made from them (`items`), the issue filter
//! (`filter`) and an invented GitHub for scenes and tests (`mock`). Piece C adds the three stores the pages share: your work (`work`), the repositories you follow (`bookmarks`) and the repository cache (`repos`). Its panel, page and the rest come in the pieces after.
pub mod auth;
pub mod bookmarks;
pub mod filter;
pub mod items;
pub mod mock;
pub mod repos;
pub mod view;
mod where_;
pub mod work;

#[cfg(test)]
mod tests_auth;
#[cfg(test)]
mod tests_filter;
#[cfg(test)]
mod tests_items;
#[cfg(test)]
mod tests_stores;
#[cfg(test)]
mod tests_where;

pub use where_::{Kind, Where};

use crate::fleet::places::{Entry, RailItem};
use crate::fleet::rails::Placement;
use crate::fleet::settings::plugins::{Added, Area, Plugin};
use crate::icons::GITHUB;
use crate::{State, Tx};

/// The plugin's id: its place in `plugins.disabled`, in the settings address and in the rail.
pub const ID: &str = "github";

/// What the plugin keeps in its entry in `State.places`.
#[derive(Default)]
pub struct GitHub {
    /// The connection page: the status, the device flow, the token box.
    pub auth: auth::Github,
    /// Where the plugin's page is (Fleet's route under `/github`).
    pub at: Where,
    /// Your work, the repositories you follow and the repository cache: shared by every page that shows them.
    pub work: work::WorkStore,
    pub bookmarks: bookmarks::Bookmarks,
    pub repos: repos::Cache,
}

/// The descriptor, the sidebar item and the settings page (`index.ts`'s manifest).
pub fn entry() -> Entry {
    let manifest = Plugin {
        id: ID,
        name: "GitHub",
        description: "Pull requests, issues and checks beside your sessions.",
        own_switch: false,
        switchless: false,
        page_title: "GitHub integration",
        nav_label: "GitHub",
        adds: &[
            Added { area: Area::Rail, detail: "GitHub (opens /github)", point: "sidebarItems" },
            Added { area: Area::Settings, detail: "GitHub integration", point: "configPage" },
            Added { area: Area::FolderPicker, detail: "Suggests your repositories to clone (github)", point: "repositorySources" },
            Added {
                area: Area::Board,
                detail: "Add GitHub issue sources from your bookmarks, optionally narrowing sync to matching labels.",
                point: "boardSources",
            },
        ],
    };
    let state = GitHub::default();
    let repo_cache = state.repos.clone();
    let mut e = Entry::new(manifest, state);
    e.rail = Some(RailItem { label: "GitHub", icon: &GITHUB, path: "/github", order: 100, placement: Placement::Plugin });
    e.settings = Some(view::connect::content);
    e.keep = Some(keep);
    e.repositories = Some(repo_cache.source());
    e
}

fn keep(s: &mut State, now: f64, tx: &mut Tx) -> bool {
    s.keep_github(now, tx) | s.keep_github_repos()
}

/// The connection's state, which lives in the plugin's entry.
pub fn auth(places: &mut crate::fleet::places::Places) -> &mut auth::Github {
    &mut places.state_mut::<GitHub>(ID).expect("the GitHub plugin is registered").auth
}

pub fn auth_ref(places: &crate::fleet::places::Places) -> &auth::Github {
    &places.state::<GitHub>(ID).expect("the GitHub plugin is registered").auth
}

/// The plugin's whole state.
pub fn state(places: &mut crate::fleet::places::Places) -> &mut GitHub {
    places.state_mut::<GitHub>(ID).expect("the GitHub plugin is registered")
}

pub fn state_ref(places: &crate::fleet::places::Places) -> &GitHub {
    places.state::<GitHub>(ID).expect("the GitHub plugin is registered")
}
