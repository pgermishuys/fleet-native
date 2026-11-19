//! Which folded runs of a file's Diff the user has opened (`collapseUnchanged`'s bar opens on a click). A fold is named by the first
//! row it hides, so a changed text folds afresh, as CodeMirror's does when its chunks are worked out again.
use crate::State;
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct DiffFolds {
    opened: HashSet<(String, String, usize)>,
}

impl DiffFolds {
    pub fn is_open(&self, session: &str, path: &str, first: usize) -> bool {
        self.opened.contains(&(session.to_string(), path.to_string(), first))
    }

    pub fn open(&mut self, session: &str, path: &str, first: usize) {
        self.opened.insert((session.to_string(), path.to_string(), first));
    }

    /// A file's tab closed: what it had opened goes with it.
    pub fn forget(&mut self, session: &str, path: &str) {
        self.opened.retain(|(s, p, _)| !(s == session && p == path));
    }
}

impl State {
    /// A click on a fold's bar in the open file's Diff.
    pub fn open_diff_fold(&mut self, path: &str, first: usize) {
        let session = self.canvas_session();
        self.file_folds.open(&session, path, first);
    }
}
