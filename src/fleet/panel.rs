//! What the session list says when it has no rows to show (`SessionsPanel.vue`, rows 1.65-1.73). Pure: no Rune.

/// The error the panel shows: the sessions' failure, else the projects' (`errorMessage`). Fleet's "Can't reach
/// `<machine>`" for a machine that stopped answering is component 17's.
pub fn error_message(sessions: Option<&str>, projects: Option<&str>) -> Option<String> {
    sessions.or(projects).map(str::to_string)
}

/// The banner above rows that are on screen from an earlier read.
pub fn banner_copy(error: &str) -> String {
    format!("Showing cached sessions. Refresh failed: {error}")
}

/// The empty state's title and line (`EmptyState compact`): a filter, the Archived view, or no sessions at all.
pub fn empty_words(filtering: bool, archived: bool) -> (&'static str, &'static str) {
    if filtering {
        ("No sessions found", "Try a different search term or clear the filter.")
    } else if archived {
        ("No archived sessions", "Sessions you archive show up here.")
    } else {
        ("No sessions yet", "Start one with New session above.")
    }
}

/// What fills the list when it has nothing to draw, in Fleet's order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Feedback {
    /// Rows are drawn (a stale list may carry a banner).
    None,
    Loading,
    /// The error text.
    Error(String),
    Empty,
}

/// `isLoading && !hasSessions`, then `errorMessage && !hasSessions`, then the empty state when no group is left.
pub fn feedback(loading: bool, error: Option<&str>, has_rows: bool, groups: usize) -> Feedback {
    if loading && !has_rows {
        Feedback::Loading
    } else if let (Some(e), false) = (error, has_rows) {
        Feedback::Error(e.to_string())
    } else if !loading && error.is_none() && groups == 0 {
        Feedback::Empty
    } else {
        Feedback::None
    }
}
