//! The session list's groups (component 6, piece A): projects in their own order, an empty project with its 0, and
//! the filter matching a project's name.
use crate::State;
use crate::fleet::groups::ProjectKind;
use crate::mock::Project;

impl State {
    /// The list with one more project that has no sessions and one more that comes first (position 0), and `filter` typed.
    pub fn show_groups(&mut self, filter: &str) {
        let project = |id: &str, name: &str, position: i64| Project {
            id: id.into(),
            kind: ProjectKind::User,
            position,
            name: name.into(),
            path: format!("~/code/{id}"),
            open: true,
            sessions: vec![],
        };
        self.projects.insert(0, project("mock-ledger", "Ledger", 0));
        self.projects.push(project("mock-docs", "Docs site", 5));
        self.projects.sort_by_key(|p| p.position);
        self.set_open((1, 1));
        self.filter = rune::TextEdit::new(filter);
    }
}

/// What the list shows besides rows (component 6, piece E).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// The first read, still going.
    Loading,
    /// The first read failed and nothing is held.
    Error,
    /// A refresh failed over rows that are on screen.
    Banner,
    /// A server with no project and no session.
    Empty,
    /// A filter nothing matches.
    NoMatch,
    /// The filter has the caret.
    Focused,
}

impl State {
    pub fn show_panel(&mut self, case: Panel) {
        if matches!(case, Panel::Loading | Panel::Error | Panel::Empty) {
            self.projects.clear();
            self.route = crate::fleet::store::Route::None;
        }
        match case {
            Panel::Loading => self.store.is_loading = true,
            Panel::Error => self.store.error = Some("Failed to fetch".into()),
            Panel::Banner => self.store.error = Some("Failed to fetch".into()),
            Panel::Empty => {}
            Panel::NoMatch => self.filter = rune::TextEdit::new("zzz"),
            Panel::Focused => {
                self.filter = rune::TextEdit::new("orch");
                self.filter.focused = true;
            }
        }
    }
}
