//! The designer's inspector for the scenes (component 18, piece P): on the canvas scene's Fix a bug copy, the workflow's own
//! settings, a You decide step, a step id the inspector refused, and (piece Q) an agent step with its loops.
use crate::State;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// Nothing picked: the workflow's settings.
    Settings,
    /// "Agree on the cause" picked: its question and its choices.
    You,
    /// "Agree on the cause" picked, with an id another step has typed in.
    IdError,
    /// "Review" picked: an agent step with a loop (piece Q).
    Agent,
    /// "Review" picked with a second loop that shares its max, and the max not set yet.
    Loops,
}

impl State {
    pub fn show_workflow_inspector(&mut self, dark: bool, case: Case) {
        self.show_workflow_canvas(dark, super::workflow_canvas::Case::Picked);
        let ed = &mut self.workflows_nav.editor;
        ed.canvas.selected = match case {
            Case::Settings => None,
            Case::You => Some(1),
            Case::IdError => Some(1),
            Case::Agent | Case::Loops => Some(3),
        };
        if case == Case::Loops {
            let review = &mut ed.draft.as_mut().expect("a draft").steps[3];
            review.outcomes.push("redo".into());
            review.routes.push(("redo".into(), "investigate".into()));
        }
        // The fields load for the open step on the next poll; the scene has none, so load them here.
        ed.sync_inspector();
        if case == Case::IdError {
            ed.inspector.id.replace_text("fix");
            ed.commit_inspector_id();
        }
    }
}
