//! Component 18, piece P: the designer's inspector. The first five of Fleet's `WorkflowInspector.test.ts` under their names (the
//! workflow's settings, a step's title and id, an id another step has, a You decide step's question and choices), then what the
//! native inspector adds: an id committed by leaving the field, a choice added and removed, and Remove. They run on the canvas
//! scene's invented Fix a bug copy and need no fake Fleet.
use crate::State;
use crate::fleet::workflows::draft::WorkflowDraft;
use crate::fleet::workflows::editor::Editor;
use crate::mock::workflow_canvas::Case;
use crate::tests::{FRAME, app_tall, click, press_key};
use rune::{Key, Mods, Runtime};

struct Rig {
    rt: Runtime<State>,
}

fn mounted(selected: Option<usize>) -> Rig {
    let mut rt = app_tall();
    rt.update(|s, _| s.show_workflow_canvas(true, Case::Picked));
    rt.update(move |s, _| s.workflows_nav.editor.canvas.selected = selected);
    let mut r = Rig { rt };
    r.frames();
    r
}

impl Rig {
    fn frames(&mut self) {
        for _ in 0..3 {
            self.rt.frame(FRAME);
        }
    }

    fn ed(&self) -> &Editor {
        &self.rt.state.workflows_nav.editor
    }

    fn draft(&self) -> &WorkflowDraft {
        self.ed().draft.as_ref().expect("a draft")
    }

    fn has(&self, key: &str) -> bool {
        self.rt.find(key).is_some()
    }

    /// The text of an element and of everything inside it, joined by a space.
    fn words(&self, key: &str) -> String {
        let all = self.rt.elements();
        let Some(root) = all.iter().find(|e| e.key.as_deref() == Some(key)) else { return String::new() };
        let inside = format!("{}/", root.path);
        all.iter()
            .filter(|e| e.path == root.path || e.path.starts_with(&inside))
            .filter_map(|e| e.text.clone())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Clicks a field, selects what is in it and types `words` over it.
    fn type_in(&mut self, key: &str, words: &str) {
        click(&mut self.rt, key);
        press_key(&mut self.rt, Key::Command('a'), Mods::default());
        let key = if words.is_empty() { Key::Backspace } else { Key::Text(words.into()) };
        press_key(&mut self.rt, key, Mods::default());
        self.frames();
    }

    fn enter(&mut self) {
        press_key(&mut self.rt, Key::Enter, Mods::default());
        self.frames();
    }

    fn set_draft(&mut self, edit: impl FnOnce(&mut WorkflowDraft)) {
        self.rt.update(|s, _| edit(s.workflows_nav.editor.draft.as_mut().expect("a draft")));
        self.frames();
    }
}

#[test]
fn shows_the_workflows_settings_when_no_step_is_selected_with_what_arrives_later_greyed_out() {
    let mut r = mounted(None);

    let panel = r.words("workflow-inspector");
    assert!(panel.contains("fix-a-bug-our-way.yaml"), "{panel}");
    assert!(panel.contains("A sentence you type"), "{panel}");
    assert!(panel.contains("Arrives with Stage 2"), "{panel}");
    assert!(panel.contains("7 steps"), "{panel}");
    r.type_in("workflow-name", "Ours");
    assert_eq!(r.draft().name, "Ours");
    r.type_in("workflow-hint", "What should it fix?");
    assert_eq!(r.draft().placeholder.as_deref(), Some("What should it fix?"));
    r.type_in("workflow-description", "Our way.");
    assert_eq!(r.draft().description.as_deref(), Some("Our way."));
    assert!(r.ed().is_dirty(), "an edit is something to save");
}

#[test]
fn renames_a_steps_title_without_touching_its_id() {
    let mut r = mounted(Some(0));

    r.type_in("workflow-step-title", "Outline");

    let draft = r.draft();
    assert_eq!((draft.steps[0].id.as_str(), draft.steps[0].title.as_str()), ("investigate", "Outline"));
    assert_eq!(draft.steps[1].choices[1].to, "investigate");
    assert_eq!(r.rt.state.workflows_nav.editor.inspector.id.text, "investigate");
}

#[test]
fn changes_a_steps_id_on_purpose_and_every_reference_follows() {
    let mut r = mounted(Some(0));
    r.set_draft(|d| {
        d.steps[2].prompt = Some("Fix {{steps.investigate.files}}.".into());
        d.steps[3].prompt = Some("Check {{ steps.investigate.summary }}.".into());
    });

    r.type_in("workflow-step-id", "locate");
    assert_eq!(r.draft().steps[0].id, "investigate", "nothing changes until the field is left");
    r.enter();

    let draft = r.draft();
    assert_eq!(draft.steps[0].id, "locate");
    assert_eq!(draft.steps[1].choices.iter().map(|c| c.to.as_str()).collect::<Vec<_>>(), ["fix", "locate", "end"]);
    assert_eq!(draft.steps[2].prompt.as_deref(), Some("Fix {{steps.locate.files}}."));
    assert_eq!(draft.steps[3].prompt.as_deref(), Some("Check {{ steps.locate.summary }}."));
    assert_eq!(r.ed().canvas.selected, Some(0), "the same step stays open");
    assert_eq!(r.words("workflow-inspector-tag"), "id: locate");
}

#[test]
fn refuses_an_id_another_step_has() {
    let mut r = mounted(Some(0));
    let before = r.draft().clone();

    r.type_in("workflow-step-id", "review");
    r.enter();

    assert_eq!(r.draft(), &before, "nothing was sent");
    assert!(r.words("workflow-step-id-error").contains("Another step has that id."));
    assert!(!r.has("workflow-step-id-help"), "the error takes the help's place");

    r.type_in("workflow-step-id", "end");
    r.enter();
    assert_eq!(r.words("workflow-step-id-error"), "\"end\" is where a run finishes; pick another id.");
    r.type_in("workflow-step-id", "");
    r.enter();
    assert_eq!(r.words("workflow-step-id-error"), "A step needs an id.");
    assert_eq!(r.draft(), &before);
}

#[test]
fn edits_a_you_decide_steps_question_and_choices() {
    let mut r = mounted(Some(1));

    assert!(r.words("workflow-inspector").contains("You decide"));
    r.type_in("workflow-step-ask", "Go ahead?");
    assert_eq!(r.draft().steps[1].ask.as_deref(), Some("Go ahead?"));

    click(&mut r.rt, "workflow-choice-to-1");
    r.frames();
    // "Leads to" lists every other step, then the end: investigate, fix, review, ….
    click(&mut r.rt, "workflow-choice-to-list-opt2");
    r.frames();
    let choice = &r.draft().steps[1].choices[1];
    assert_eq!((choice.label.as_str(), choice.to.as_str(), choice.note), ("Send back with a note", "review", true));
    assert!(r.words("workflow-choice-1").contains("Review"), "{}", r.words("workflow-choice-1"));

    r.type_in("workflow-choice-label-0", "Yes");
    assert_eq!(r.draft().steps[1].choices[0].label, "Yes");
    click(&mut r.rt, "workflow-choice-note-0");
    assert!(r.draft().steps[1].choices[0].note);
}

#[test]
fn an_id_is_committed_when_the_field_is_left() {
    let mut r = mounted(Some(0));
    r.type_in("workflow-step-id", "locate");
    assert_eq!(r.draft().steps[0].id, "investigate");

    click(&mut r.rt, "workflow-step-title");
    r.frames();

    assert_eq!(r.draft().steps[0].id, "locate");
    assert_eq!(r.draft().steps[1].choices[1].to, "locate");
}

#[test]
fn adds_and_removes_a_choice_and_removes_the_step() {
    let mut r = mounted(Some(1));
    click(&mut r.rt, "workflow-choice-add");
    r.frames();
    let choices = &r.draft().steps[1].choices;
    assert_eq!((choices.len(), choices[3].label.as_str(), choices[3].to.as_str()), (4, "New choice", "end"));
    assert!(r.has("workflow-choice-3"));

    click(&mut r.rt, "workflow-choice-remove-0");
    r.frames();
    assert_eq!(
        r.draft().steps[1].choices.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(),
        ["Send back with a note", "Stop here", "New choice"]
    );
    assert!(!r.has("workflow-choice-3"));

    click(&mut r.rt, "workflow-remove-step");
    r.frames();
    assert_eq!(r.draft().steps.len(), 6);
    assert_eq!(r.ed().canvas.selected, None, "the workflow's settings open");
    assert!(r.has("workflow-name"));
}

#[test]
fn a_reloaded_draft_reaches_the_fields() {
    let mut r = mounted(None);
    r.set_draft(|d| d.name = "Renamed elsewhere".into());
    assert_eq!(r.ed().inspector.name.text, "Renamed elsewhere");
}
