//! Settings → Folders (`SettingsPage.vue`'s `folders` section): the page header and the cards stacked under it. The Worktrees
//! and Storage cards are later pieces.
use super::folders_locations;
use crate::State;
use crate::theme::Theme;
use rune::{El, col};

pub fn content(s: &State, t: &Theme) -> El {
    col()
        .key("folders-section")
        .w_full()
        .gap(21.0)
        .child(super::super::settings::page_header(s, t, &crate::fleet::settings_nav::Page::Folders))
        .child(folders_locations::card_view(s, t))
}
