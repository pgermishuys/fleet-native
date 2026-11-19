//! The anchor rule of `SettingsPage.vue` (row 1.31): landing on a group, a row or a marked spot (Folders › Storage, What
//! agents can do › Browser, Machines › Add a machine) scrolls it into view once and then lets the anchor go, so coming back
//! to the page doesn't scroll again. Harnesses and Skills & tools pick their tab from the anchor, so they keep it. An anchor
//! whose target isn't drawn yet is kept, so one that appears later still gets scrolled to.
//!
//! The rule is plain Rust over what the page draws: [`targets`] lists the anchors each built page has and the key of the
//! element behind each. A page that isn't built yet has none, so an anchor aimed at it waits, as Fleet's does for a group that
//! isn't there. Each piece of component 16 adds its page's groups here when it draws them.
use super::super::settings_nav::{Page, SettingsNav};
use crate::{State, Tx};

/// Pages that read the anchor themselves (`OWN_ANCHOR_PAGES`).
pub fn keeps_anchor(page: &Page) -> bool {
    matches!(page, Page::Harnesses | Page::Skills)
}

/// What a page offers to scroll to: `(anchor, key of the element)`. A group is `settings-group-<id>`, a row
/// `settings-row-<id>` and a marked spot `settings-anchor-<id>` (`data-settings-anchor`), the three Fleet looks for.
pub fn targets(page: &Page) -> &'static [(&'static str, &'static str)] {
    match page {
        Page::Machines => {
            &[("machines", "settings-group-machines"), ("add-machine", "settings-row-add-machine"), ("phones", "settings-group-phones")]
        }
        Page::General => {
            &[("new-sessions", "settings-group-new-sessions"), ("away", "settings-group-away"), ("rail", "settings-group-rail")]
        }
        // Fleet's ids are `look` and `conversation`, plus the rows inside them (`font-size`, `code-theme`, `thinking`).
        Page::Appearance => &[
            ("look", "settings-group-look"),
            ("conversation", "settings-group-conversation"),
            ("theme", "settings-row-theme"),
            ("code-theme", "settings-row-code-theme"),
            ("font", "settings-row-font"),
            ("font-size", "settings-row-font-size"),
            ("thinking", "settings-row-thinking"),
        ],
        Page::Harnesses => &[
            ("default-harness", "settings-group-default-harness"),
            ("may-do", "settings-anchor-may-do"),
            ("providers", "settings-group-providers"),
            ("profiles", "settings-group-profiles"),
            ("harness-settings", "settings-group-harness-settings"),
        ],
        Page::Permissions => &[
            ("edit-files", "reach-row-edits"),
            ("run-commands", "reach-row-commands"),
            ("session-messages", "reach-row-messages"),
            ("other-machines", "reach-row-machines"),
            ("unattended-runs", "reach-row-unattended"),
            ("browser", "settings-anchor-browser"),
        ],
        Page::Folders => &[("storage", "settings-anchor-storage")],
        Page::Updates => &[
            ("fleet", "settings-group-fleet"),
            ("harnesses", "settings-group-harnesses"),
            ("machines", "settings-group-machines"),
            ("whats-new", "settings-group-whats-new"),
        ],
        _ => &[],
    }
}

/// What to do about the anchor this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Settle {
    /// No anchor, or its target isn't drawn: nothing to do, and a kept anchor stays.
    Wait,
    /// Scroll to the element with this key. `release` says whether the anchor is spent after.
    Scroll { key: String, release: bool },
}

/// `SettingsPage.vue`'s watch, as a function of the page and its anchor.
pub fn settle(page: &Page, anchor: Option<&str>) -> Settle {
    let Some(anchor) = anchor else { return Settle::Wait };
    let Some((_, key)) = targets(page).iter().find(|(name, _)| *name == anchor) else { return Settle::Wait };
    Settle::Scroll { key: (*key).to_string(), release: !keeps_anchor(page) }
}

impl SettingsNav {
    /// Applies [`settle`]: lets go of the anchor when it is spent and returns the key to scroll to, if any. A kept anchor
    /// (Harnesses, Skills & tools) scrolls once, not every frame.
    pub fn settle_anchor(&mut self) -> Option<String> {
        if self.scrolled {
            return None;
        }
        match settle(&self.page, self.anchor.as_deref()) {
            Settle::Wait => None,
            Settle::Scroll { key, release } => {
                if release {
                    self.anchor = None;
                } else {
                    self.scrolled = true;
                }
                Some(key)
            }
        }
    }
}

impl State {
    /// Called from `poll` while Settings is showing: scrolls to the page's anchor, once.
    pub fn keep_settings_anchor(&mut self, tx: &mut Tx) -> bool {
        if self.rail != crate::Rail::Settings {
            return false;
        }
        match self.settings.settle_anchor() {
            Some(key) => {
                // Fleet scrolls with `block: start`. `scroll_into_view` only reveals the top edge, so a group whose top is already
                // on screen but whose body is below it (the Browser group) wouldn't move; this puts the top at the top.
                tx.scroll_key_to(key, 0.0);
                true
            }
            None => false,
        }
    }
}
