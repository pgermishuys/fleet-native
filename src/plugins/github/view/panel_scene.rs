//! Scenes of the GitHub rail's panel (`github-panel*`): each state it can be in, drawn from invented data with no server. The names are
//! the ones in `mock.rs` (owners `acme` and `lumen-labs`, signed in as `mara-quinn`).
use crate::State;
use crate::plugins::github::bookmarks::Bookmark;
use crate::plugins::github::items::Work;
use crate::plugins::github::{Where, mock, state};

#[derive(Clone, Copy)]
pub enum Which {
    /// Connected, two repositories followed, with their counts; Your work is the page.
    Followed,
    /// Connected, a repository's page open.
    OnRepo,
    /// Connected, the followed repositories still loading.
    Loading,
    /// Connected, the followed repositories refused.
    Failed,
    /// Connected, no repository followed.
    Empty,
    /// Not connected.
    Off,
}

pub fn set(s: &mut State, which: &str, dark: bool) {
    scene(
        s,
        dark,
        match which {
            "followed" => Which::Followed,
            "onrepo" => Which::OnRepo,
            "loading" => Which::Loading,
            "failed" => Which::Failed,
            "empty" => Which::Empty,
            "off" => Which::Off,
            other => panic!("no GitHub panel scene called {other}"),
        },
    )
}

pub fn scene(s: &mut State, dark: bool, which: Which) {
    s.dark = Some(dark);
    s.rail = crate::Rail::Plugin(crate::plugins::github::ID);
    let g = state(&mut s.places);
    // Nothing asks a server in a scene: the panel has read already.
    g.panel_asked = true;
    g.auth.connected = !matches!(which, Which::Off);
    g.at = Where::Home;
    if matches!(which, Which::Off) {
        return;
    }
    g.work.work = Some(Work::read(&mock::work()));
    g.bookmarks.has_loaded = true;
    let mark = |n: &str| Bookmark::of(n).expect("an owner and a name");
    match which {
        Which::Followed | Which::OnRepo => {
            g.bookmarks.list = vec![mark("acme/rocket"), mark("lumen-labs/atlas"), mark("lumen-labs/beacon")];
            if matches!(which, Which::OnRepo) {
                g.at = Where::Repo { owner: "acme".into(), repo: "rocket".into() };
            }
        }
        Which::Loading => {
            g.bookmarks.has_loaded = false;
            g.bookmarks.loading = true;
        }
        Which::Failed => g.bookmarks.error = Some("Failed to fetch bookmarks from server.".into()),
        _ => {}
    }
}
