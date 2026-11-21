//! The page the GitHub rail draws beside its panel. Your work (`GitHubBrowserPage.vue`) is `home` (piece G) and a repository's page is `repo` (piece I);
//! until they are built the page says so, in the words every unbuilt rail uses.
use super::row::{RowArgs, item_row};
use crate::State;
use crate::icons::GITHUB;
use crate::plugins::github::sessions::sessions_of;
use crate::plugins::github::{Where, state, state_ref};
use crate::theme::{Theme, sans};
use crate::view::workflows::sheet;
use rune::{icon, text};

pub fn page(s: &State, t: &Theme) -> rune::El {
    let demo = &state_ref(&s.places).demo_rows;
    if !demo.is_empty() {
        return sheet(t, "github-page").padding(16.0).gap(0.0).children(demo.iter().enumerate().map(|(i, (item, reason))| {
            let sessions = sessions_of(s, item);
            let key = format!("github-row-{i}");
            item_row(
                s,
                t,
                &RowArgs {
                    key: &key,
                    item,
                    sessions: &sessions,
                    show_repo: true,
                    reason: reason.as_deref(),
                    on_label: Some(|s, name| state(&mut s.places).demo_label = Some(name.to_string())),
                },
            )
        }));
    }
    if state_ref(&s.places).at == Where::Home {
        return super::home::home(s, t);
    }
    if matches!(state_ref(&s.places).at, Where::Repo { .. }) {
        return super::repo::repo(s, t);
    }
    if matches!(state_ref(&s.places).at, Where::Item { .. }) {
        return super::item::item(s, t);
    }
    sheet(t, "github-page")
        .center()
        .gap(10.0)
        .child(icon(&GITHUB, 22.0).color(t.muted))
        .child(text("GitHub").font(sans(14.0, 600.0, 1.3)).color(t.text))
        .child(text("Only the Sessions shell is rebuilt in this prototype.").font(sans(13.0, 400.0, 1.4)).color(t.muted))
}
