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
