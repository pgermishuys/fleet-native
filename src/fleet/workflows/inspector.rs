//! The designer's inspector state (component 18, piece P; `WorkflowInspector.vue`): the fields of the workflow's settings and of
//! a step's head and You decide form, and the edits they make. Every edit sends the whole new draft to the editor (`editDraft`).
//! Plain state; `view::workflows::inspector` draws it.
//!
//! A text field has no change hook that sees the draft, so `Editor::sync_inspector` runs once a frame and compares each field with
//! what it last agreed with (`Mirror`): a field that moved is the user's edit and goes into the draft; a draft that moved under an
//! untouched field (a reload, a rename) goes into the field.
use super::draft::{END, StepPatch, WorkflowDraft, drop_unused_max, remove_step, rename_step_id, update_choice, update_step};
use super::editor::Editor;
use super::{Choice, StepKind};
use crate::State;
use rune::TextEdit;

pub const NAME_FIELD: &str = "workflow-name";
pub const DESCRIPTION_FIELD: &str = "workflow-description";
pub const HINT_FIELD: &str = "workflow-hint";
pub const TITLE_FIELD: &str = "workflow-step-title";
pub const ID_FIELD: &str = "workflow-step-id";
pub const ASK_FIELD: &str = "workflow-step-ask";

pub const ID_NEEDED: &str = "A step needs an id.";
pub const ID_IS_END: &str = "\"end\" is where a run finishes; pick another id.";
pub const ID_TAKEN: &str = "Another step has that id.";

/// What the fields last agreed with the draft about.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Mirror {
    name: String,
    description: String,
    hint: String,
    title: String,
    ask: String,
    labels: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Inspector {
    pub name: TextEdit,
    pub description: TextEdit,
    pub hint: TextEdit,
    pub title: TextEdit,
    pub id: TextEdit,
    pub ask: TextEdit,
    pub labels: Vec<TextEdit>,
    /// Why the id as typed was refused; cleared when another step opens or the id changes.
    pub id_error: Option<String>,
    /// The choice whose "Leads to" list is open.
    pub menu: Option<usize>,
    mirror: Mirror,
    /// What the fields were loaded for: the open step and its id.
    scope: Option<(Option<usize>, String)>,
    id_was_focused: bool,
}

fn set(field: &mut TextEdit, words: &str) {
    if field.text != words {
        field.replace_text(words);
    }
}

/// One field against its mirror: `Some(text)` when the user changed it, and the field follows the draft when only that moved.
fn two_way(field: &mut TextEdit, mirror: &mut String, from_draft: &str) -> Option<String> {
    if field.text != *mirror {
        *mirror = field.text.clone();
        return Some(field.text.clone());
    }
    if from_draft != mirror {
        *mirror = from_draft.to_string();
        set(field, from_draft);
    }
    None
}

impl Inspector {
    fn load(&mut self, draft: &WorkflowDraft, selected: Option<usize>) {
        let step = selected.and_then(|i| draft.steps.get(i));
        self.mirror = Mirror {
            name: draft.name.clone(),
            description: draft.description.clone().unwrap_or_default(),
            hint: draft.placeholder.clone().unwrap_or_default(),
            title: step.map(|s| s.title.clone()).unwrap_or_default(),
            ask: step.and_then(|s| s.ask.clone()).unwrap_or_default(),
            labels: step.map(|s| s.choices.iter().map(|c| c.label.clone()).collect()).unwrap_or_default(),
        };
        let m = self.mirror.clone();
        for (field, words) in [
            (&mut self.name, &m.name),
            (&mut self.description, &m.description),
            (&mut self.hint, &m.hint),
            (&mut self.title, &m.title),
            (&mut self.ask, &m.ask),
        ] {
            set(field, words);
        }
        set(&mut self.id, step.map_or("", |s| s.id.as_str()));
        self.labels = m.labels.iter().map(|l| TextEdit::new(l.clone())).collect();
    }
}

impl Editor {
    /// Once a frame: what the user typed goes into the draft, what the draft changed goes into the fields. True when the draft
    /// changed.
    pub fn sync_inspector(&mut self) -> bool {
        let Some(draft) = self.draft.clone() else { return false };
        let selected = self.canvas.selected.filter(|i| *i < draft.steps.len());
        let scope = (selected, selected.map(|i| draft.steps[i].id.clone()).unwrap_or_default());
        let mut insp = std::mem::take(&mut self.inspector);
        // The id field is committed when it is left (the field's `change`), before another step takes the panel.
        let left = insp.id_was_focused && !insp.id.focused;
        insp.id_was_focused = insp.id.focused;
        if left && insp.scope.as_ref() == Some(&scope) {
            self.inspector = insp;
            self.commit_inspector_id();
            return true;
        }
        if insp.scope.as_ref() != Some(&scope) {
            insp.load(&draft, selected);
            insp.scope = Some(scope);
            insp.id_error = None;
            insp.menu = None;
            self.inspector = insp;
            return false;
        }
        let mut next = draft.clone();
        let m = &mut insp.mirror;
        if let Some(words) = two_way(&mut insp.name, &mut m.name, &draft.name) {
            next.name = words;
        }
        if let Some(words) = two_way(&mut insp.description, &mut m.description, draft.description.as_deref().unwrap_or("")) {
            next.description = Some(words).filter(|w| !w.is_empty());
        }
        if let Some(words) = two_way(&mut insp.hint, &mut m.hint, draft.placeholder.as_deref().unwrap_or("")) {
            next.placeholder = Some(words).filter(|w| !w.is_empty());
        }
        if let Some(i) = selected {
            let step = &draft.steps[i];
            if let Some(words) = two_way(&mut insp.title, &mut m.title, &step.title) {
                next = update_step(&next, i, &StepPatch { title: Some(words), ..StepPatch::default() });
            }
            if let Some(words) = two_way(&mut insp.ask, &mut m.ask, step.ask.as_deref().unwrap_or("")) {
                next = update_step(&next, i, &StepPatch { ask: Some(Some(words)), ..StepPatch::default() });
            }
            let now: Vec<String> = step.choices.iter().map(|c| c.label.clone()).collect();
            if insp.labels.len() != now.len() {
                // A choice came or went: every label is read again.
                insp.labels = now.iter().map(|l| TextEdit::new(l.clone())).collect();
                m.labels = now;
            } else {
                for (at, field) in insp.labels.iter_mut().enumerate() {
                    if let Some(words) = two_way(field, &mut m.labels[at], &now[at]) {
                        let patch = update_choice(&next.steps[i], at, |c| c.label = words.clone());
                        next = update_step(&next, i, &patch);
                    }
                }
            }
        }
        self.inspector = insp;
        if next != draft {
            self.edit_draft(next);
            return true;
        }
        false
    }

    /// `commitId`: the id as typed becomes the step's, and every reference follows; or the field says why not.
    pub fn commit_inspector_id(&mut self) {
        let Some(draft) = self.draft.clone() else { return };
        let Some(i) = self.canvas.selected.filter(|i| *i < draft.steps.len()) else { return };
        let current = draft.steps[i].id.clone();
        let next = self.inspector.id.text.trim().to_string();
        if next == current {
            self.inspector.id_error = None;
            set(&mut self.inspector.id, &next);
            return;
        }
        let refused = if next.is_empty() {
            Some(ID_NEEDED)
        } else if next == END {
            Some(ID_IS_END)
        } else if draft.steps.iter().any(|other| other.id == next) {
            Some(ID_TAKEN)
        } else {
            None
        };
        if let Some(why) = refused {
            self.inspector.id_error = Some(why.into());
            return;
        }
        self.inspector.id_error = None;
        self.edit_draft(rename_step_id(&draft, &current, &next));
    }

    fn open_step_index(&self) -> Option<usize> {
        let n = self.draft.as_ref()?.steps.len();
        self.canvas.selected.filter(|i| *i < n)
    }

    /// `set(patch)` for the open step: the patch, then a max with no loop left goes.
    pub fn inspector_patch(&mut self, patch: &StepPatch) {
        let (Some(draft), Some(i)) = (self.draft.clone(), self.open_step_index()) else { return };
        self.edit_draft(drop_unused_max(&update_step(&draft, i, patch), i));
    }

    /// The "Leads to" of a You decide step's choice: a step's id, or `end`.
    pub fn inspector_set_choice_target(&mut self, at: usize, target: &str) {
        let (Some(draft), Some(i)) = (self.draft.as_ref(), self.open_step_index()) else { return };
        let patch = update_choice(&draft.steps[i], at, |c| c.to = target.to_string());
        self.inspector_patch(&patch);
    }

    pub fn inspector_toggle_choice_note(&mut self, at: usize) {
        let (Some(draft), Some(i)) = (self.draft.as_ref(), self.open_step_index()) else { return };
        let patch = update_choice(&draft.steps[i], at, |c| c.note = !c.note);
        self.inspector_patch(&patch);
    }

    /// "Add a choice": New choice, leading to the end.
    pub fn inspector_add_choice(&mut self) {
        let (Some(draft), Some(i)) = (self.draft.as_ref(), self.open_step_index()) else { return };
        let mut choices = draft.steps[i].choices.clone();
        choices.push(Choice { label: "New choice".into(), to: END.into(), note: false });
        self.inspector_patch(&StepPatch { choices: Some(choices), ..StepPatch::default() });
    }

    pub fn inspector_remove_choice(&mut self, at: usize) {
        let (Some(draft), Some(i)) = (self.draft.as_ref(), self.open_step_index()) else { return };
        let choices: Vec<Choice> = draft.steps[i].choices.iter().enumerate().filter(|(n, _)| *n != at).map(|(_, c)| c.clone()).collect();
        self.inspector.menu = None;
        self.inspector_patch(&StepPatch { choices: Some(choices), ..StepPatch::default() });
    }

    /// `remove(index)`: the step goes and the workflow's settings open.
    pub fn inspector_remove_step(&mut self) {
        let (Some(draft), Some(i)) = (self.draft.clone(), self.open_step_index()) else { return };
        self.canvas.selected = None;
        self.edit_draft(remove_step(&draft, i));
    }

    /// The step open in the inspector is a You decide step.
    pub fn inspector_step_is_you(&self) -> bool {
        self.open_step_index().and_then(|i| self.draft.as_ref().map(|d| d.steps[i].kind == StepKind::You)).unwrap_or(false)
    }
}

impl State {
    pub fn workflow_inspector_commit_id(&mut self) {
        self.workflows_nav.editor.commit_inspector_id();
    }

    pub fn workflow_inspector_menu(&mut self, at: Option<usize>) {
        self.workflows_nav.editor.inspector.menu = at;
    }
}
