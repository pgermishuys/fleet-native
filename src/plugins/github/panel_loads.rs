//! What the rail's panel asks for when it is drawn (`GitHubPanel.vue`'s `onMounted` and `watch(isConnected)`): the connection's status,
//! the repositories you follow, and, once connected, your work. Run from the plugin's `keep`; each answer lands on the UI thread.
use super::{ID, state, state_ref};
use crate::{Rail, State};

impl State {
    /// The panel is on screen: GitHub's rail is the active one and its context panel is not collapsed.
    pub fn github_panel_shown(&self) -> bool {
        self.active_rail() == Some(Rail::Plugin(ID)) && self.context_panel_shown()
    }

    /// Asks once per visit to the panel. True when it asked, so the next frame draws what came of it.
    pub(super) fn keep_github_panel(&mut self) -> bool {
        let shown = self.github_panel_shown();
        let asked = state_ref(&self.places).panel_asked;
        if shown == asked {
            return false;
        }
        state(&mut self.places).panel_asked = shown;
        if shown {
            self.github_bookmarks_use();
            self.load_github_status();
            if super::auth_ref(&self.places).connected {
                self.github_work_load();
            }
        }
        shown
    }

    /// `watch(isConnected)`: a connection that arrives while the panel is there reads your work.
    pub(super) fn github_panel_connected(&mut self) {
        if self.github_panel_shown() && super::auth_ref(&self.places).connected {
            self.github_work_load();
        }
    }
}
