//! Component 18, piece O: the designer's canvas. The canvas's halves of Fleet's `WorkflowEditor.test.ts` under their names ("adds
//! an agent step and a You decide step with +, and greys out what arrives later", and the node half of "shows the parser's
//! error on the step…"), then what the native canvas adds: a click on a node opens the step, Escape closes the menu, the loops
//! are drawn from where the nodes were laid out, and a step that's gone leaves the settings open. They run on the scene's
//! invented Fix a bug copy and need no fake Fleet: nothing here may leave a thread behind (the macOS runner's limit is near).
use crate::State;
use crate::fleet::workflows::draft::{WorkflowCheck, WorkflowProblem, remove_step};
use crate::fleet::workflows::editor::Editor;
use crate::mock::workflow_canvas::Case;
use crate::tests::{FRAME, app_tall, click, press_key};
use crate::tests_workflow_editor::MISSING_MAX;
use rune::{Key, Mods, Runtime};

struct Canvas {
    rt: Runtime<State>,
}

fn mounted() -> Canvas {
    let mut rt = app_tall();
    rt.update(|s, _| s.show_workflow_canvas(true, Case::Add));
    rt.update(|s, _| s.workflows_nav.editor.canvas.add_at = None);
    let mut c = Canvas { rt };
    c.frames();
    c
}

impl Canvas {
    fn frames(&mut self) {
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    fn ed(&self) -> &Editor {
        &self.rt.state.workflows_nav.editor
    }

    fn has(&self, key: &str) -> bool {
        self.rt.find(key).is_some()
    }

    fn ids(&self) -> Vec<String> {
        self.ed().draft.as_ref().expect("a draft").steps.iter().map(|s| s.id.clone()).collect()
    }

    /// The text of an element and of everything inside it, in order, joined by a space.
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

    fn adopt(&mut self) {
        self.rt.update(|s, _| s.show_workflow_canvas(true, Case::Add));
        self.rt.update(|s, _| s.workflows_nav.editor.canvas.add_at = None);
        self.frames();
    }
}

#[test]
fn adds_an_agent_step_and_a_you_decide_step_with_plus_and_greys_out_what_arrives_later() {
    let mut r = mounted();
    let before = r.ids().len();

    click(&mut r.rt, "workflow-add-7");
    // The menu fades in; greyed means still dim after it has.
    r.rt.frame(0.5);
    let menu = r.words("workflow-add-menu");
    assert!(menu.contains("Arrives with Stage 2"), "{menu}");
    assert!(menu.contains("Arrives with Stage 3"), "{menu}");
    let disabled = r.rt.elements().iter().filter(|e| e.role == "menuitem" && e.opacity < 0.9).count();
    assert_eq!(disabled, 2, "the two that arrive later are off");
    click(&mut r.rt, "workflow-add-parallel");
    assert_eq!(r.ids().len(), before, "a greyed item adds nothing");
    click(&mut r.rt, "workflow-add-agent");
    assert!(!r.has("workflow-add-menu"), "a choice closes the menu");
    click(&mut r.rt, "workflow-add-7");
    click(&mut r.rt, "workflow-add-you");

    let ids = r.ids();
    assert_eq!(ids[before - 1..], ["open-pr", "ask", "step"]);
    assert_eq!(r.ed().draft.as_ref().unwrap().steps[before].choices[0].to, "step");
    // The inspector is piece P's: the new step is the one open in it.
    assert_eq!(r.ed().canvas.selected, Some(before));
    assert!(r.has("workflow-node-ask") && r.has("workflow-node-step"));
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
    assert!(!r.words("workflow-node-fix").contains(MISSING_MAX));
    r.rt.update(|s, _| s.workflows_nav.editor.check = None);
    r.frames();
    assert!(!r.has("workflow-node-error-0"));
}

#[test]
fn a_click_opens_the_step_escape_closes_the_menu_and_the_loops_follow_the_nodes() {
    // One runtime for all of it: each one leaves threads behind, and the macOS runner is near its limit.
    let mut r = mounted();
    // the new step is what a save would write
    {
        click(&mut r.rt, "workflow-add-0");
        click(&mut r.rt, "workflow-add-agent");
        let step = &r.ed().draft.as_ref().unwrap().steps[0];
        assert_eq!((step.id.as_str(), step.title.as_str(), step.model.as_deref()), ("step", "New step", Some("standard")));
        assert_eq!(step.prompt.as_deref(), Some("The request: {{request}}\n\n{{previous.summary}}"));
        assert_eq!(r.ed().canvas.selected, Some(0));
        assert!(r.ed().is_dirty());
    }
    r.adopt();
    // a click on a node opens the step and the settings box opens the workflow
    {
        assert_eq!(r.ed().canvas.selected, None, "the workflow's own settings are open first");
        click(&mut r.rt, "workflow-node-fix");
        assert_eq!(r.ed().canvas.selected, Some(2));
        click(&mut r.rt, "workflow-node-investigate");
        assert_eq!(r.ed().canvas.selected, Some(0));
        click(&mut r.rt, "workflow-canvas-settings");
        assert_eq!(r.ed().canvas.selected, None);
    }
    r.adopt();
    // escape closes the menu and a second click on the plus does too
    {
        let before = r.ids().len();
        click(&mut r.rt, "workflow-add-2");
        assert!(r.has("workflow-add-menu"));
        press_key(&mut r.rt, Key::Escape, Mods::default());
        r.frames();
        assert!(!r.has("workflow-add-menu"));
        assert_eq!(r.ed().canvas.add_at, None);
        assert_eq!(r.ids().len(), before, "nothing was added");

        click(&mut r.rt, "workflow-add-2");
        assert_eq!(r.ed().canvas.add_at, Some(2));
        r.rt.update(|s, _| s.workflows_nav.editor.canvas.toggle_add(2));
        assert_eq!(r.ed().canvas.add_at, None);
    }
    r.adopt();
    // a loop is drawn from where the nodes are and says how often
    {
        assert_eq!(r.words("workflow-loop-label"), "changes \u{b7} at most 2\u{d7}");
        assert!(r.words("workflow-node-review").contains("changes \u{2192} Fix \u{2264}2"));

        r.rt.update(|s, _| {
            let e = &mut s.workflows_nav.editor;
            let mut next = e.draft.clone().unwrap();
            next.steps[3].max_loops = None;
            e.edit_draft(next);
        });
        r.frames();
        assert_eq!(r.words("workflow-loop-label"), "changes \u{b7} needs a max");

        // Another file opens with the same layout: Rune reports no move, so the places must have been kept.
        r.adopt();
        assert_eq!(r.words("workflow-loop-label"), "changes \u{b7} at most 2\u{d7}");
    }
    r.adopt();
    // a step that is gone leaves the settings open
    {
        click(&mut r.rt, "workflow-node-open-pr");
        assert_eq!(r.ed().canvas.selected, Some(6));
        r.rt.update(|s, _| {
            let e = &mut s.workflows_nav.editor;
            let next = remove_step(e.draft.as_ref().unwrap(), 6);
            e.edit_draft(next);
        });
        r.frames();
        assert_eq!(r.ed().canvas.selected, None);
    }
    r.adopt();
}
