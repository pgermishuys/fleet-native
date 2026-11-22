//! Component 18, piece Q: the inspector's agent-step form. The last three of Fleet's `WorkflowInspector.test.ts` under their names
//! (a loop's max, who finishes the step and a variable chip; two loops sharing the max; pointing an outcome at another step), then
//! what the native form adds. They run on the canvas scene's invented Fix a bug copy, whose step 3 is Review (`pass`, and `changes`
//! back to Fix with a max of 2).
use crate::State;
use crate::fleet::workflows::inspector_agent::PROMPT_FIELD;
use crate::tests::click;
use crate::tests_workflow_inspector::{Rig, mounted};

const REVIEW: usize = 3;

fn route(r: &Rig, outcome: &str) -> Option<String> {
    r.draft().steps[REVIEW].routes.iter().find(|(k, _)| k == outcome).map(|(_, to)| to.clone())
}

fn pick(r: &mut Rig, trigger: &str, list: &str, at: usize) {
    click(&mut r.rt, trigger);
    r.frames();
    click(&mut r.rt, &format!("{list}-opt{at}"));
    r.frames();
}

#[test]
fn edits_a_loops_max_who_finishes_the_step_and_inserts_a_variable_chip() {
    let mut r = mounted(Some(REVIEW));

    r.type_in("workflow-outcome-max-1", "4");
    assert_eq!(r.draft().steps[REVIEW].max_loops, Some(4));

    click(&mut r.rt, "workflow-finish-agent");
    r.frames();
    let step = &r.draft().steps[REVIEW];
    assert!(step.finish_agent && !step.finish_you);

    click(&mut r.rt, "workflow-var-run.base");
    r.frames();
    assert!(r.draft().steps[REVIEW].prompt.as_deref().unwrap_or_default().contains("{{run.base}}"));
}

#[test]
fn says_when_two_loops_share_the_steps_max() {
    let mut r = mounted(Some(REVIEW));
    assert!(!r.has("workflow-shared-max"), "one loop shares nothing");
    r.set_draft(|d| {
        let step = &mut d.steps[REVIEW];
        step.outcomes = vec!["pass".into(), "changes".into(), "redo".into()];
        step.routes = vec![("changes".into(), "fix".into()), ("redo".into(), "investigate".into())];
    });

    assert!(r.has("workflow-outcome-max-1") && r.has("workflow-outcome-max-2"));
    assert!(!r.has("workflow-outcome-max-0"), "only a loop has a max box");
    assert_eq!(r.words("workflow-shared-max"), "At most 2 for this step's loops: changes and redo share it.");
}

#[test]
fn points_an_outcome_at_another_step_and_a_max_goes_once_theres_no_loop() {
    let mut r = mounted(Some(REVIEW));

    // Next step, the seven steps, then the end.
    pick(&mut r, "workflow-outcome-to-1", "workflow-outcome-to-list", 8);

    assert_eq!(route(&r, "changes").as_deref(), Some("end"));
    assert_eq!(r.draft().steps[REVIEW].max_loops, None);
    assert!(!r.has("workflow-outcome-max-1"), "no loop, no max box");
}

#[test]
fn a_chip_goes_in_at_the_caret_and_the_prompt_keeps_focus() {
    let mut r = mounted(Some(REVIEW));
    r.type_in(PROMPT_FIELD, "AB");
    r.rt.update(|s: &mut State, _| s.workflows_nav.editor.inspector.agent.prompt.set_selection(1, 1));

    click(&mut r.rt, "workflow-var-request");
    r.frames();

    assert_eq!(r.draft().steps[REVIEW].prompt.as_deref(), Some("A{{request}}B"));
    let mut focused = None;
    r.rt.update(|_, tx| focused = tx.focused().map(str::to_string));
    assert_eq!(focused.as_deref(), Some(PROMPT_FIELD));
    assert_eq!(r.ed().inspector.agent.prompt.text, "A{{request}}B");
}

#[test]
fn chips_list_the_runs_variables_then_each_earlier_steps_summary_and_files() {
    let r = mounted(Some(REVIEW));
    assert!(r.has("workflow-var-request"));
    assert!(r.has("workflow-var-steps.investigate.summary"));
    assert!(r.has("workflow-var-steps.investigate.files"), "Find the cause declares a file");
    assert!(r.has("workflow-var-steps.fix.summary"));
    assert!(!r.has("workflow-var-steps.fix.files"), "Fix writes nothing");
    assert!(!r.has("workflow-var-steps.review.summary"), "not itself, nor a later step");
}

#[test]
fn the_instructions_agent_skill_and_effort_go_into_the_draft() {
    let mut r = mounted(Some(REVIEW));
    r.type_in(PROMPT_FIELD, "Look at {{request}}.");
    r.type_in("workflow-step-agent", "plan");
    r.type_in("workflow-step-skill", "fleet-run");
    r.type_in("workflow-step-effort", "high");
    let step = &r.draft().steps[REVIEW];
    assert_eq!(step.prompt.as_deref(), Some("Look at {{request}}."));
    assert_eq!((step.agent.as_deref(), step.skill.as_deref(), step.effort.as_deref()), (Some("plan"), Some("fleet-run"), Some("high")));

    r.type_in("workflow-step-agent", "");
    assert_eq!(r.draft().steps[REVIEW].agent, None, "an empty agent is the default one");
}

#[test]
fn picks_a_role_or_a_pinned_model() {
    let mut r = mounted(Some(REVIEW));
    assert!(!r.has("workflow-step-pinned"));

    pick(&mut r, "workflow-step-model", "workflow-step-model-list", 2);
    assert_eq!(r.draft().steps[REVIEW].model.as_deref(), Some("fast"));

    pick(&mut r, "workflow-step-model", "workflow-step-model-list", 3);
    assert!(r.has("workflow-step-pinned"), "Pinned model… shows its field");
    assert_eq!(r.draft().steps[REVIEW].model.as_deref(), Some(""));
    r.type_in("workflow-step-pinned", "acme/model-1");
    assert_eq!(r.draft().steps[REVIEW].model.as_deref(), Some("acme/model-1"));

    pick(&mut r, "workflow-step-model", "workflow-step-model-list", 0);
    assert_eq!(r.draft().steps[REVIEW].model.as_deref(), Some("strong"));
    assert!(!r.has("workflow-step-pinned"));
}

#[test]
fn who_finishes_it_is_one_of_three_and_the_default_clears_both() {
    let mut r = mounted(Some(REVIEW));
    click(&mut r.rt, "workflow-finish-you");
    r.frames();
    let step = &r.draft().steps[REVIEW];
    assert!(step.finish_you && !step.finish_agent);
    click(&mut r.rt, "workflow-finish-default");
    r.frames();
    let step = &r.draft().steps[REVIEW];
    assert!(!step.finish_you && !step.finish_agent);
}

#[test]
fn renaming_an_outcome_carries_its_route_once_the_name_is_entered() {
    let mut r = mounted(Some(REVIEW));
    r.type_in("workflow-outcome-name-1", "rework");
    assert_eq!(r.draft().steps[REVIEW].outcomes[1], "changes", "nothing changes until the name is entered");
    r.enter();

    let step = &r.draft().steps[REVIEW];
    assert_eq!(step.outcomes, ["pass", "rework"]);
    assert_eq!(route(&r, "rework").as_deref(), Some("fix"));
    assert_eq!(route(&r, "changes"), None);
    assert_eq!(step.max_loops, Some(2));
    assert_eq!(r.ed().inspector.agent.outcomes[1].text, "rework");
}

#[test]
fn adds_and_removes_an_outcome() {
    let mut r = mounted(Some(REVIEW));
    click(&mut r.rt, "workflow-outcome-add");
    r.frames();
    assert_eq!(r.draft().steps[REVIEW].outcomes, ["pass", "changes", "done"]);
    click(&mut r.rt, "workflow-outcome-add");
    r.frames();
    assert_eq!(r.draft().steps[REVIEW].outcomes[3], "outcome-2");

    click(&mut r.rt, "workflow-outcome-remove-1");
    r.frames();
    let step = &r.draft().steps[REVIEW];
    assert_eq!(step.outcomes, ["pass", "done", "outcome-2"]);
    assert_eq!(route(&r, "changes"), None, "its route went with it");
    assert_eq!(step.max_loops, None, "and so did the max");
}

#[test]
fn a_new_loop_leaves_the_max_empty_and_the_step_says_it_needs_one() {
    let mut r = mounted(Some(REVIEW));
    r.set_draft(|d| d.steps[REVIEW].max_loops = None);
    // "pass" back to Fix is a loop now.
    pick(&mut r, "workflow-outcome-to-0", "workflow-outcome-to-list", 3);
    assert_eq!(route(&r, "pass").as_deref(), Some("fix"));
    assert!(r.has("workflow-outcome-max-0"));
    assert_eq!(r.words("workflow-shared-max"), "The max is for this step's loops: pass and changes share it.");
    assert_eq!(r.ed().inspector.agent.maxes[0].text, "");
}

#[test]
fn a_max_that_is_not_a_number_is_none_and_digits_win() {
    let mut r = mounted(Some(REVIEW));
    r.type_in("workflow-outcome-max-1", "0");
    assert_eq!(r.draft().steps[REVIEW].max_loops, None);
    r.type_in("workflow-outcome-max-1", "3x");
    assert_eq!(r.draft().steps[REVIEW].max_loops, Some(3));
}

#[test]
fn writes_are_added_edited_and_removed() {
    let mut r = mounted(Some(REVIEW));
    assert_eq!(r.words("workflow-writes-empty"), "No declared files.");
    click(&mut r.rt, "workflow-write-add");
    r.frames();
    assert_eq!(r.draft().steps[REVIEW].writes, ["docs/{{slug}}.md"]);
    assert!(!r.has("workflow-writes-empty"));

    r.type_in("workflow-write-0", "notes/{{slug}}.md");
    assert_eq!(r.draft().steps[REVIEW].writes, ["notes/{{slug}}.md"]);
    click(&mut r.rt, "workflow-write-remove-0");
    r.frames();
    assert!(r.draft().steps[REVIEW].writes.is_empty());
    assert!(r.has("workflow-writes-empty"));
}

#[test]
fn optional_shows_its_hint_and_always_runs_forgets_it() {
    let mut r = mounted(Some(REVIEW));
    assert!(!r.has("workflow-step-hint"));
    click(&mut r.rt, "workflow-step-optional");
    r.frames();
    assert!(r.draft().steps[REVIEW].optional);
    r.type_in("workflow-step-hint", "For UI changes");
    assert_eq!(r.draft().steps[REVIEW].optional_hint.as_deref(), Some("For UI changes"));

    click(&mut r.rt, "workflow-step-always");
    r.frames();
    let step = &r.draft().steps[REVIEW];
    assert!(!step.optional);
    assert_eq!(step.optional_hint, None);
    assert!(!r.has("workflow-step-hint"));
}

#[test]
fn a_you_decide_step_has_none_of_the_agent_form() {
    let r = mounted(Some(1));
    for key in ["workflow-step-prompt", "workflow-finish", "workflow-outcomes", "workflow-writes", "workflow-optional"] {
        assert!(!r.has(key), "{key}");
    }
}

#[test]
fn opening_a_step_loads_its_fields_and_asks_for_a_new_frame() {
    let mut r = mounted(None);
    r.rt.update(|s: &mut State, _| s.workflows_nav.editor.canvas.selected = Some(REVIEW));
    let mut redraw = false;
    r.rt.update(|s: &mut State, _| redraw = s.workflows_nav.editor.sync_inspector());
    assert!(redraw, "the fields changed under the view, so it must draw again");
    assert_eq!(r.ed().inspector.title.text, "Review");
    assert_eq!(r.ed().inspector.agent.skill.text, "fleet-code-review");
}
