//! The designer's inspector for the scenes (component 18, piece P): on the canvas scene's Fix a bug copy, the workflow's own
//! settings, a You decide step, and a step id the inspector refused.
use crate::State;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// Nothing picked: the workflow's settings.
    Settings,
    /// "Agree on the cause" picked: its question and its choices.
    You,
    /// "Agree on the cause" picked, with an id another step has typed in.
    IdError,
}

impl State {
    pub fn show_workflow_inspector(&mut self, dark: bool, case: Case) {
        self.show_workflow_canvas(dark, super::workflow_canvas::Case::Picked);
        let ed = &mut self.workflows_nav.editor;
        ed.canvas.selected = match case {
            Case::Settings => None,
            Case::You => Some(1),
            Case::IdError => Some(1),
        };
        // The fields load for the open step on the next poll; the scene has none, so load them here.
        ed.sync_inspector();
        if case == Case::IdError {
            ed.inspector.id.replace_text("fix");
            ed.commit_inspector_id();
        }
    }
}
