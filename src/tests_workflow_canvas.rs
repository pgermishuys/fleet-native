//! Component 18, piece O: the designer's canvas. The canvas's halves of Fleet's `WorkflowEditor.test.ts` under their names ("adds
//! an agent step and a You decide step with +, and greys out what arrives later", and the node half of "shows the parser's error
//! on the step…"), then what the native canvas adds: a click on a node opens the step, Escape closes the menu, the loops are
//! drawn from where the nodes were laid out, and a step that's gone leaves the settings open. A fake Fleet answers.
use crate::fleet::workflows::draft::{WorkflowCheck, WorkflowProblem};
use crate::tests::{click, press_key};
use crate::tests_workflow_editor::{MISSING_MAX, Rig, rig};
use rune::{Key, Mods};

fn mounted() -> Rig {
    let mut r = rig();
    r.mount(false);
    r
}

impl Rig {
    fn ids(&self) -> Vec<String> {
        self.ed().draft.as_ref().expect("a draft").steps.iter().map(|s| s.id.clone()).collect()
    }
}

#[test]
fn adds_an_agent_step_and_a_you_decide_step_with_plus_and_greys_out_what_arrives_later() {
    let mut r = mounted();

    click(&mut r.rt, "workflow-add-4");
    // The menu fades in; greyed means still dim after it has.
    r.rt.frame(0.5);
    let menu = r.words("workflow-add-menu");
    assert!(menu.contains("Arrives with Stage 2"), "{menu}");
    assert!(menu.contains("Arrives with Stage 3"), "{menu}");
    let disabled = r.rt.elements().iter().filter(|e| e.role == "menuitem" && e.opacity < 0.9).count();
    assert_eq!(disabled, 2, "the two that arrive later are off");
    click(&mut r.rt, "workflow-add-parallel");
    assert_eq!(r.ids().len(), 4, "a greyed item adds nothing");
    click(&mut r.rt, "workflow-add-agent");
    assert!(!r.has("workflow-add-menu"), "a choice closes the menu");
    click(&mut r.rt, "workflow-add-4");
    click(&mut r.rt, "workflow-add-you");

    assert_eq!(r.ids(), ["plan", "ok-plan", "implement", "review", "ask", "step"]);
    assert_eq!(r.ed().draft.as_ref().unwrap().steps[4].choices[0].to, "step");
    // The inspector is piece P's: the new step is the one open in it.
    assert_eq!(r.ed().canvas.selected, Some(4));
    assert!(r.has("workflow-node-ask") && r.has("workflow-node-step"));
}

#[test]
fn the_new_step_is_what_a_save_would_write() {
    let mut r = mounted();
    click(&mut r.rt, "workflow-add-0");
    click(&mut r.rt, "workflow-add-agent");
    let step = &r.ed().draft.as_ref().unwrap().steps[0];
    assert_eq!((step.id.as_str(), step.title.as_str(), step.model.as_deref()), ("step", "New step", Some("standard")));
    assert_eq!(step.prompt.as_deref(), Some("The request: {{request}}\n\n{{previous.summary}}"));
    assert_eq!(r.ed().canvas.selected, Some(0));
    assert!(r.rt.state.workflows_nav.editor.is_dirty());
}

#[test]
fn shows_the_parser_s_error_on_the_step() {
    let mut r = mounted();
    assert!(!r.has("workflow-node-error-0"), "nothing wrong yet");
    r.rt.update(|s, _| {
        let e = &mut s.workflows_nav.editor;
        e.check = Some(WorkflowCheck {
            errors: vec![WorkflowProblem { line: 38, message: MISSING_MAX.into(), step: Some(3) }],
            ..e.check.clone().unwrap_or_default()
        });
    });
    r.frames();
    assert_eq!(r.words("workflow-node-review").matches(MISSING_MAX).count(), 1, "on the step it is about");
    assert!(!r.words("workflow-node-plan").contains(MISSING_MAX));
    r.rt.update(|s, _| s.workflows_nav.editor.check = None);
    r.frames();
    assert!(!r.has("workflow-node-error-0"));
}

#[test]
fn a_click_on_a_node_opens_the_step_and_the_settings_box_opens_the_workflow() {
    let mut r = mounted();
    assert_eq!(r.ed().canvas.selected, None, "the workflow's own settings are open first");
    click(&mut r.rt, "workflow-node-implement");
    assert_eq!(r.ed().canvas.selected, Some(2));
    click(&mut r.rt, "workflow-node-plan");
    assert_eq!(r.ed().canvas.selected, Some(0));
    click(&mut r.rt, "workflow-canvas-settings");
    assert_eq!(r.ed().canvas.selected, None);
}

#[test]
fn escape_closes_the_menu_and_a_second_click_on_the_plus_does_too() {
    let mut r = mounted();
    click(&mut r.rt, "workflow-add-2");
    assert!(r.has("workflow-add-menu"));
    press_key(&mut r.rt, Key::Escape, Mods::default());
    r.frames();
    assert!(!r.has("workflow-add-menu"));
    assert_eq!(r.ed().canvas.add_at, None);
    assert_eq!(r.ids().len(), 4, "nothing was added");

    click(&mut r.rt, "workflow-add-2");
    assert_eq!(r.ed().canvas.add_at, Some(2));
    r.rt.update(|s, _| s.workflows_nav.editor.canvas.toggle_add(2));
    assert_eq!(r.ed().canvas.add_at, None);
}

#[test]
fn a_loop_is_drawn_from_where_the_nodes_are_and_says_how_often() {
    let mut r = mounted();
    r.frames();
    assert_eq!(r.words("workflow-loop-label"), "changes \u{b7} at most 2\u{d7}");
    assert!(r.words("workflow-node-review").contains("changes \u{2192} Implement \u{2264}2"));

    r.edit_draft(|d| d.steps[3].max_loops = None);
    r.frames();
    assert_eq!(r.words("workflow-loop-label"), "changes \u{b7} needs a max");
}

#[test]
fn a_step_that_is_gone_leaves_the_settings_open() {
    let mut r = mounted();
    click(&mut r.rt, "workflow-node-review");
    assert_eq!(r.ed().canvas.selected, Some(3));
    r.rt.update(|s, _| {
        let e = &mut s.workflows_nav.editor;
        let next = crate::fleet::workflows::draft::remove_step(e.draft.as_ref().unwrap(), 3);
        e.edit_draft(next);
    });
    r.frames();
    assert_eq!(r.ed().canvas.selected, None);
}
