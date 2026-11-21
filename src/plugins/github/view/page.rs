//! The page the GitHub rail draws beside its panel. Your work (`GitHubBrowserPage.vue`) is piece G and a repository's page is piece I;
//! until they are built the page says so, in the words every unbuilt rail uses.
use crate::State;
use crate::icons::GITHUB;
use crate::theme::{Theme, sans};
use crate::view::workflows::sheet;
use rune::{icon, text};

pub fn page(_: &State, t: &Theme) -> rune::El {
    sheet(t, "github-page")
        .center()
        .gap(10.0)
        .child(icon(&GITHUB, 22.0).color(t.muted))
        .child(text("GitHub").font(sans(14.0, 600.0, 1.3)).color(t.text))
        .child(text("Only the Sessions shell is rebuilt in this prototype.").font(sans(13.0, 400.0, 1.4)).color(t.muted))
}
