//! The Archived view (`SessionsPanel.vue`, rows 1.77-1.81): the same list asked for `retentionStatus=archived`, with no Pinned
//! group, no machine headings and Restore where the bar had Archive.
use super::store::Retention;
use crate::State;

impl State {
    /// `isArchivedView`.
    pub fn is_archived_view(&self) -> bool {
        self.store.retention == Retention::Archived
    }

    /// `showArchived`: lets go of the picked rows and reads the list again for the other retention. The rows already held
    /// are filtered at once (`hides`), so nothing of the other list shows while the answer is on its way.
    pub fn show_archived(&mut self, show: bool) {
        self.selection.clear();
        let next = if show { Retention::Archived } else { Retention::Active };
        if self.store.retention == next {
            return;
        }
        self.store.retention = next;
        if let Some(link) = &self.link {
            link.refetch_list();
        }
    }
}

impl State {
    /// A scene: the Archived view with three archived sessions in two projects (the list isn't asked for from a server).
    pub fn show_archived_scene(&mut self) {
        for (p, i) in [(0, 1), (0, 3), (1, 0)] {
            self.projects[p].sessions[i].row.archived = true;
        }
        self.store.retention = Retention::Archived;
    }
}

impl State {
    /// `toggleActivityFilter` (palette "Show all sessions"): the active list becomes the whole list, anything else goes back
    /// to active only.
    pub fn toggle_activity_filter(&mut self) {
        let next = if self.store.retention == Retention::Active { Retention::All } else { Retention::Active };
        self.pick_retention(next);
    }

    /// `toggleInlineToolDiffs`.
    pub fn toggle_inline_diffs(&mut self) {
        self.inline_diffs = !self.inline_diffs;
    }

    /// `zoomDocument`: a step of 10 percent, kept between 70 and 150.
    pub fn zoom_by(&mut self, delta: f32) {
        self.zoom_pct = (self.zoom_pct + delta).clamp(70.0, 150.0);
    }
}

impl State {
    /// A scene: "Show all sessions" on, with the same three archived sessions among the active ones.
    pub fn show_all_sessions_scene(&mut self) {
        self.show_archived_scene();
        self.store.retention = Retention::All;
    }
}
