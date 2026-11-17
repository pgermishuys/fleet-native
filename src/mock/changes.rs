//! Invented states for the Changes body's scenes and tests (component 12, piece D): what the body shows before there is a review.
use crate::State;
use crate::fleet::diffs::{Base, Refusal};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// The first answer hasn't come.
    Loading,
    /// `/diffs` answered HTTP 500.
    Failed,
    /// Git won't read the folder.
    Refused,
    /// A folder off any branch, nothing changed.
    EmptySession,
    /// A branch compared with where it left main, nothing changed.
    EmptyBranch,
}

pub const FIX: &str = "git config --global --add safe.directory /srv/demo-app";

impl State {
    /// The panel open on Changes with `case` held.
    pub fn show_changes_scene(&mut self, case: Case, dark: bool) {
        self.show_tabs(super::canvas_host::Case::Fresh, dark);
        self.content_open = true;
        self.right.review = false;
        let id = self.canvas_session();
        self.diffs.reset(&id);
        let d = &mut self.diffs;
        match case {
            Case::Loading => return,
            Case::Failed => d.error = Some("HTTP 500".into()),
            Case::Refused => {
                d.refusal =
                    Some(Refusal { message: "Git won't read this folder because another user owns it.".into(), fix_command: FIX.into() })
            }
            Case::EmptySession => d.base = Some(Base::Session),
            Case::EmptyBranch => d.base = Some(Base::Branch { branch: Some("main".into()), commit: Some("8a61fdd7c0ffee".into()) }),
        }
        d.has_loaded = true;
    }
}
