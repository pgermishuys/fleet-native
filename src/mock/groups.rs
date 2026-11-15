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
