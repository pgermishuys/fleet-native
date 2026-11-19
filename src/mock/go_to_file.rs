//! Go to file's scenes (component 11, piece L): the dialog open over the invented project, in its four states.
use crate::State;
use crate::fleet::finder::{Scope, Step};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// Nothing typed: the files open in the session's tabs.
    Open,
    /// "pag": the files the invented project answers with.
    Typed,
    /// "zzq": nothing matches.
    None,
    /// "pag", and the search failed with nothing to show.
    Failed,
}

impl State {
    pub fn show_go_to_file(&mut self, case: Case, dark: bool) {
        self.show_file_tabs(dark);
        let id = self.canvas_session();
        self.go_to_file.show(&id);
        self.go_to = Default::default();
        self.ui.open = Some(crate::ui::Open::GoToFile);
        let typed = match case {
            Case::Open => return,
            Case::Typed | Case::Failed => "pag",
            Case::None => "zzq",
        };
        self.go_to.query = rune::TextEdit::new(typed);
        let scope = Scope::Session(id);
        // The first look starts the 80 ms wait; a later one sends, and the invented project answers.
        self.go_to.search.want(Some(&scope), Some(typed), 0.0);
        if let Step::Send { generation, query } = self.go_to.search.want(Some(&scope), Some(typed), 1.0) {
            let answer = if case == Case::Failed { Err("HTTP 500".to_string()) } else { Ok(crate::mock::files_tab::find(&query)) };
            self.go_to.search.answered(generation, &query, answer);
        }
    }
}
