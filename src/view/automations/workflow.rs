//! Ports `AutomationTargetPicker.vue` (the Runs chip, row 12.1), `AutomationWorkflowPicker.vue` (the workflow chip and its
//! menu, 12.4) and the workflow part of `AutomationComposer.vue`'s strip (12.3): the static New worktree chip, one switch
//! per optional step. The base and harness chips are New session's (`BasePicker`, `HarnessPicker`) on the composer.
use super::composer::MESSAGE;
use crate::fleet::automations::composer::TargetKind;
use crate::fleet::automations::workflow::{TARGET_CHIP, WORKFLOW_CHIP};
use crate::fleet::workflows::Workflow;
use crate::icons::{GIT_BRANCH, MESSAGE_SQUARE, TRIANGLE_ALERT, WORKFLOW};
use crate::theme::Theme;
use crate::ui::chip::{Chip, Option_, option, pop, pop_label, pop_note};
use crate::{State, Tx};
use rune::{Align, El, Side};

const POP_WIDTH: f32 = 344.0;

/// The Runs chip: Workflow or MessageSquare, "Runs" muted before what each run starts.
pub fn target_chip(s: &State, t: &Theme, workflow: bool, busy: bool) -> El {
    let (glyph, label) = if workflow { (&WORKFLOW, "a workflow") } else { (&MESSAGE_SQUARE, "a session") };
    // `ns-chip__hint` then `ns-chip__label`: the hint is muted at 400, the label at 500 in the text colour.
    Chip::new(TARGET_CHIP, label).glyph(glyph).head("Runs ").open(s.target_menu_open()).disabled(busy).view(
        t,
        |s: &mut State, tx: &mut Tx| {
            s.open_target_menu();
            tx.focus("automation-target-session", false);
        },
    )
}

/// The Runs menu above its chip: a session, and a workflow while Workflows are on or it already runs one.
pub fn target_overlay(s: &State, t: &Theme) -> Vec<El> {
    let workflow = s.automations_nav.composer(s.composer_editing()).is_workflow();
    let row_for = |key: &str, glyph, title: &str, detail: &str, checked: bool, kind: TargetKind| {
        let spec = Option_ { icon: Some(glyph), detail: Some(detail.to_string()), checked, ..Option_::new(key, title) };
        option(t, spec, None, move |s, _| s.automation_set_target_kind(kind))
    };
    let mut body = vec![row_for(
        "automation-target-session",
        &MESSAGE_SQUARE,
        "A session",
        "the agent gets your message",
        !workflow,
        TargetKind::Session,
    )];
    if s.automation_workflows_on() || workflow {
        body.push(row_for(
            "automation-target-workflow",
            &WORKFLOW,
            "A workflow",
            "a workflow from the Library, with your message as its request",
            workflow,
            TargetKind::Workflow,
        ));
    }
    anchored(pop(t, "automation-target-pop", POP_WIDTH, body), "automation-target", TARGET_CHIP)
}

/// `.automation-composer__static`: New worktree, nothing to press; its title says why.
pub fn new_worktree(t: &Theme) -> El {
    Chip::new("automation-new-worktree", "New worktree")
        .glyph(&GIT_BRANCH)
        .flat()
        .still()
        .help(Some("Each run gets its own worktree and branch; every step works in it.".into()))
        .view(t, |_: &mut State, _: &mut Tx| {})
}

/// The workflow chip: the workflow's name (its id while the Library hasn't it), else "Pick a workflow".
pub fn workflow_chip(s: &State, t: &Theme, busy: bool) -> El {
    Chip::new(WORKFLOW_CHIP, s.automation_workflow_label()).glyph(&WORKFLOW).open(s.workflow_menu_open()).disabled(busy).view(
        t,
        |s: &mut State, tx: &mut Tx| {
            s.open_workflow_menu();
            if let Some(id) = s.automations_nav.composer(s.composer_editing()).workflow_id.clone() {
                tx.focus(format!("automation-workflow-{id}"), false);
            }
        },
    )
}

/// One switch per optional step: "<title>: on/off", its hint as the title.
pub fn step_chips(s: &State, t: &Theme, busy: bool) -> Vec<El> {
    let on = &s.automations_nav.composer(s.composer_editing()).workflow_steps;
    s.automation_optional_steps()
        .into_iter()
        .map(|(id, title, hint)| {
            let checked = on.contains(&id);
            let state = if checked { "on" } else { "off" };
            let key = format!("automation-workflow-step-{id}");
            Chip::new(key, state)
                .head(format!("{title}: "))
                .flat()
                .help(hint)
                .disabled(busy)
                .view(t, move |s: &mut State, _: &mut Tx| s.automation_toggle_step(&id))
                .role("switch")
                .checked(checked)
                .label(format!("{title}: {state}"))
        })
        .collect()
}

/// The workflow menu above its chip: a note or "Loading the workflows…", then the built-ins and the repo's own files; a file
/// with errors can't be picked.
pub fn workflow_overlay(s: &State, t: &Theme) -> Vec<El> {
    let feed = s.automation_library();
    let chosen = s.automations_nav.composer(s.composer_editing()).workflow_id.clone();
    let mut body = Vec::new();
    if let Some(note) = &feed.note {
        body.push(pop_note(t, note));
    } else if feed.loading && feed.workflows.is_none() {
        body.push(pop_note(t, "Loading the workflows…"));
    }
    let all = feed.workflows.as_deref().unwrap_or(&[]);
    for (label, built_in) in [("Built into Fleet", true), ("This repo · .weave/workflows", false)] {
        let group: Vec<&Workflow> = all.iter().filter(|w| w.built_in == built_in).collect();
        if group.is_empty() {
            continue;
        }
        body.push(pop_label(t, label));
        for w in group {
            let broken = !w.errors.is_empty();
            let detail = if broken {
                "The file has errors; fix it in the Library first".to_string()
            } else {
                w.file.clone().or_else(|| w.description.clone()).unwrap_or_default()
            };
            let spec = Option_ {
                icon: Some(if broken { &TRIANGLE_ALERT } else { &WORKFLOW }),
                detail: Some(detail).filter(|d| !d.is_empty()),
                checked: chosen.as_deref() == Some(w.id.as_str()),
                disabled: broken,
                ..Option_::new(format!("automation-workflow-{}", w.id), w.name.clone())
            };
            let id = w.id.clone();
            body.push(option(t, spec, None, move |s, _| s.automation_set_workflow(&id)));
        }
    }
    anchored(pop(t, "automation-workflow-pop", POP_WIDTH, body), "automation-workflow", WORKFLOW_CHIP)
}

/// Above the chip, start-aligned, 6 px off; closing gives the box the keys (`returnFocusToMessage`).
fn anchored(panel: El, key: &str, chip: &str) -> Vec<El> {
    let panel = panel.focus_return(MESSAGE.to_string()).modal(true, |s: &mut State| s.close_strip_menus());
    vec![
        crate::ui::overlay::scrim(&format!("{key}-scrim"), |s: &mut State| s.close_strip_menus()),
        crate::ui::overlay::anchored(panel, chip, Side::Top, Align::Start, 6.0),
    ]
}
