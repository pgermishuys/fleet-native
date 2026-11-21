//! The built-in plugins, and the composition root that registers them (`main.ts`'s `registerPlugins([...])`).
//!
//! The owner's rule: GitHub is an add-on. Core never imports a plugin; a plugin fills the places core offers
//! (`fleet::places`). `register` is the only line outside this folder that names a plugin, and
//! `plugins::tests_seam` fails any core file that reaches into one. A plugin keeps its own views in its own `view/` folder:
//! the "one file per panel" rule holds there as under `src/view/`.
use crate::fleet::places::Places;

pub mod github;

/// Called once, from `State`'s constructor.
pub fn register(mut places: Places) -> Places {
    places.register(github::entry());
    places
}

/// The scenes a plugin adds (`spec()`'s list is static and core's, so it names the scene and this routes it).
pub fn scene(s: &mut crate::State, name: &str, dark: bool) {
    if let Some(which) = name.strip_prefix("github-follow-") {
        github::view::panel_scene::follow(s, which, dark);
    } else if name.starts_with("github-home") || matches!(name, "github-connect" | "github-connect-waiting" | "github-connect-expired") {
        github::view::home_scene::set(s, name.strip_prefix("github-").unwrap_or(name), dark);
    } else if let Some(which) = name.strip_prefix("github-issues") {
        github::view::filters_scene::set(s, which.trim_start_matches('-'), dark);
    } else if name.starts_with("github-pull") || name == "github-issue" || name.starts_with("github-item-") {
        github::view::item_scene::set(s, name.strip_prefix("github-").unwrap_or(name), dark);
    } else if let Some(which) = name.strip_prefix("github-repo") {
        github::view::repo_scene::set(s, which.trim_start_matches('-'), dark);
    } else if name == "github-rows" {
        github::view::rows_scene::scene(s, dark);
    } else if let Some(which) = name.strip_prefix("github-panel-") {
        github::view::panel_scene::set(s, which, dark);
    } else if let Some(which) = name.strip_prefix("github-") {
        github::view::scene::set(s, which, dark);
    }
}

#[cfg(test)]
mod tests_seam;
