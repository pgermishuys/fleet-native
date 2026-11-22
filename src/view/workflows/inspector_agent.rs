//! `WorkflowInspector.vue`, an agent step: Runs as (agent, model, skill, effort), Who finishes it, Instructions with the variable
//! chips, Outcomes with their loops and max, Writes and Optional. The step's head and Remove are `inspector`; the fields and what
//! the buttons do are `fleet::workflows::inspector_agent`.
use super::canvas::model_label;
use super::inspector::{help, label, labelled, mini, mono_mini, option, raise, select_box, targets};
use crate::State;
use crate::fleet::workflows::draft::{WorkflowDraft, variables_for};
use crate::fleet::workflows::editor::Editor;
use crate::fleet::workflows::inspector_agent::{AgentMenu, PROMPT_FIELD};
use crate::fleet::workflows::{ROLES, Step};
use crate::icons::{PLUS, TRASH_2};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::controls::grow;
use crate::ui::select;
use crate::ui::text_menu::text_input;
use rune::{Cursor, El, Key, KeyEvent, Tx, col, icon, row, text};
use std::rc::Rc;

const PINNED: &str = "pinned";

/// The model list: the three roles with what they map to, then "Pinned model…".
fn model_options(s: &State) -> Vec<(String, String)> {
    let machine = s.workflows_machine();
    let roles = s.workflow_model_roles(&machine);
    let harness = s.workflow_default_harness(&machine);
    let mut all: Vec<(String, String)> =
        ROLES.iter().map(|r| (r.key().to_string(), model_label(&roles, &harness, Some(r.key())))).collect();
    all.push((PINNED.to_string(), "Pinned model\u{2026}".to_string()));
    all
}

/// The value the model box shows: a role, "pinned", or none.
fn model_value(ed: &Editor, step: &Step) -> String {
    let model = step.model.clone().unwrap_or_default();
    if ed.inspector.agent.pinning || (!model.is_empty() && !crate::fleet::workflows::inspector_agent::is_role(Some(&model))) {
        PINNED.to_string()
    } else {
        model
    }
}

pub fn form(s: &State, t: &Theme, ed: &Editor, draft: &WorkflowDraft, step: &Step, index: usize) -> Vec<El> {
    vec![
        runs_as(s, t, ed, step),
        finisher(t, step),
        instructions(t, ed, draft, index),
        outcomes(t, ed, draft, step, index),
        writes(t, ed, step),
        optional(t, ed, step),
    ]
}

/// The open list: the model's, or an outcome's "Leads to".
pub fn overlay(s: &State, t: &Theme, ed: &Editor) -> Vec<El> {
    let (Some(draft), Some(i), Some(menu)) = (ed.draft.as_ref(), ed.canvas.selected, ed.inspector.agent.menu) else { return Vec::new() };
    let Some(step) = draft.steps.get(i) else { return Vec::new() };
    let close: Rc<dyn Fn(&mut State)> = Rc::new(|s: &mut State| s.workflow_agent_menu(None));
    match menu {
        AgentMenu::Model => {
            let labels = model_options(s);
            let options: Vec<(&str, &str)> = labels.iter().map(|(v, l)| (v.as_str(), l.as_str())).collect();
            let choose: select::Choose = Rc::new(|s: &mut State, to: &str| s.workflows_nav.editor.inspector_pick_model(to));
            let current = model_value(ed, step);
            select::content_in(t, "workflow-step-model-list", "workflow-step-model", 176.0, &options, Some(&current), choose, close, 0.0)
        }
        AgentMenu::Route(at) => {
            let Some(outcome) = step.outcomes.get(at) else { return Vec::new() };
            let mut labels = vec![(String::new(), "\u{2192} Next step".to_string())];
            labels.extend(targets(draft, i, step.kind));
            let options: Vec<(&str, &str)> = labels.iter().map(|(v, l)| (v.as_str(), l.as_str())).collect();
            let current = step.routes.iter().find(|(k, _)| k == outcome).map(|(_, to)| to.as_str()).unwrap_or("");
            let choose: select::Choose = Rc::new(move |s: &mut State, to: &str| s.workflows_nav.editor.inspector_pick_route(at, to));
            select::content_in(
                t,
                "workflow-outcome-to-list",
                &format!("workflow-outcome-to-{at}"),
                131.0,
                &options,
                Some(current),
                choose,
                close,
                0.0,
            )
        }
    }
}

// ---- Runs as ------------------------------------------------------------------------------------------------------------

fn runs_as(s: &State, t: &Theme, ed: &Editor, step: &Step) -> El {
    let a = &ed.inspector.agent;
    let agent = mini(t, &a.agent, "Agent")
        .key("workflow-step-agent")
        .placeholder("Default agent".to_string(), t.muted)
        .w(0.0)
        .grow(1.0)
        .min_w(0.0)
        .on_edit(|s: &mut State| &mut s.workflows_nav.editor.inspector.agent.agent);
    let value = model_value(ed, step);
    let words = model_options(s).into_iter().find(|(v, _)| *v == value).map_or_else(|| "No model".to_string(), |(_, l)| l);
    let open = a.menu == Some(AgentMenu::Model);
    let model = select_box(t, &words, open)
        .key("workflow-step-model")
        .label("Model")
        .w(0.0)
        .grow(1.2)
        .min_w(0.0)
        .on_press(move |s: &mut State| s.workflow_agent_menu(if open { None } else { Some(AgentMenu::Model) }));
    let skill = mono_mini(t, &a.skill, "Skill")
        .key("workflow-step-skill")
        .placeholder("No skill".to_string(), t.muted)
        .w(0.0)
        .grow(1.0)
        .min_w(0.0)
        .on_edit(|s: &mut State| &mut s.workflows_nav.editor.inspector.agent.skill);
    let effort = mini(t, &a.effort, "Effort")
        .key("workflow-step-effort")
        .placeholder("Effort: default".to_string(), t.muted)
        .w(0.0)
        .grow(1.0)
        .min_w(0.0)
        .on_edit(|s: &mut State| &mut s.workflows_nav.editor.inspector.agent.effort);
    let mut body = col().w_full().gap(6.0).child(row().w_full().gap(6.0).child(agent).child(model));
    if a.pinning {
        body = body.child(
            mono_mini(t, &a.pinned, "Pinned model")
                .key("workflow-step-pinned")
                .placeholder("provider/model, e.g. github-copilot/gpt-5.4-mini".to_string(), t.muted)
                .w_full()
                .on_edit(|s: &mut State| &mut s.workflows_nav.editor.inspector.agent.pinned),
        );
    }
    body = body.child(row().w_full().gap(6.0).child(skill).child(effort));
    col().w_full().child(label(t, "Runs as")).child(body).child(help(
        t,
        "workflow-runs-as-help",
        "Roles map to your models under Model roles, in the Workflows list. A pinned model has to be one everyone who runs this has."
            .into(),
        false,
    ))
}

// ---- Who finishes it ----------------------------------------------------------------------------------------------------

fn radio(t: &Theme, key: &str, on: bool, words: &str, detail: &str, press: impl Fn(&mut State) + 'static) -> El {
    option(t, key, on, words, detail)
        .opacity(1.0)
        .role("radio")
        .checked(on)
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 0.0, t.accent)
        .on_press(press)
}

fn finisher(t: &Theme, step: &Step) -> El {
    let now = if step.finish_you {
        "you"
    } else if step.finish_agent {
        "agent"
    } else {
        "default"
    };
    let one = |value: &'static str, words: &str, detail: &str| {
        radio(t, &format!("workflow-finish-{value}"), now == value, words, detail, move |s: &mut State| {
            s.workflows_nav.editor.inspector_pick_finisher(value == "you", value == "agent")
        })
    };
    let group = col()
        .key("workflow-finish")
        .role("radiogroup")
        .label("Who finishes it")
        .w_full()
        .gap(4.0)
        .child(one(
            "default",
            "The agent, unless Check with me is on",
            "It calls fleet_step_done. Check with me turns it into a step you finish.",
        ))
        .child(one("you", "You", "Only you can move it on. The agent has no fleet_step_done here."))
        .child(one("agent", "Always the agent", "Ignores Check with me, for steps like pushing a branch."));
    labelled(t, "Who finishes it", group)
}

// ---- Instructions -------------------------------------------------------------------------------------------------------

fn instructions(t: &Theme, ed: &Editor, draft: &WorkflowDraft, index: usize) -> El {
    let prompt = grow(
        text_input(&ed.inspector.agent.prompt, mono(11.5, 400.0, 1.55))
            .key(PROMPT_FIELD)
            .on_edit(|s: &mut State| &mut s.workflows_nav.editor.inspector.agent.prompt)
            .label("Instructions")
            .color(t.text)
            .caret(t.text, t.accent.alpha(0.35))
            .w_full()
            .px(8.0)
            .py(6.0)
            .radius(RADIUS_BTN)
            .fill(raise(t))
            .border(1.0, t.border)
            .border_layout()
            .min_h(110.0)
            .focus_ring(2.0, 0.0, t.accent),
        14,
    );
    let mut chips = row().key("workflow-variables").w_full().flex_wrap().gap(4.0).mt(6.0);
    for variable in variables_for(draft, index) {
        let chip = variable.clone();
        chips = chips.child(
            row()
                .key(format!("workflow-var-{variable}"))
                .role("button")
                .label(format!("Insert {{{{{variable}}}}}"))
                .px(6.0)
                .py(1.0)
                .radius(4.0)
                .fill(t.accent_dim)
                .cursor(Cursor::Pointer)
                .focus_ring(2.0, 0.0, t.accent)
                .child(text(format!("{{{{{variable}}}}}")).font(mono(10.5, 400.0, 1.5)).color(t.accent_text))
                .on_press_tx(move |s: &mut State, tx: &mut Tx| {
                    s.workflows_nav.editor.inspector_insert_variable(&chip);
                    tx.focus(PROMPT_FIELD, false);
                }),
        );
    }
    col().w_full().child(label(t, "Instructions")).child(prompt).child(chips).child(help(
        t,
        "workflow-prompt-help",
        "Include {{request}} so the step sees any constraints in it. A line whose variables are all empty is left out.".into(),
        false,
    ))
}

// ---- Outcomes -----------------------------------------------------------------------------------------------------------

fn trash(t: &Theme, key: String, name: &str, press: impl Fn(&mut State) + 'static) -> El {
    row()
        .key(key)
        .role("button")
        .label(name.to_string())
        .size(24.0, 24.0)
        .center()
        .radius(6.0)
        .cursor(Cursor::Pointer)
        .hover(t.wash(5.0))
        .focus_ring(2.0, 0.0, t.accent)
        .child(icon(&TRASH_2, 12.0).color(t.muted))
        .on_press(press)
}

fn add(t: &Theme, key: &str, words: &str, press: impl Fn(&mut State) + 'static) -> El {
    row()
        .key(key.to_string())
        .role("button")
        .gap(4.0)
        .center_y()
        .px(4.0)
        .py(2.0)
        .mt(6.0)
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 0.0, t.accent)
        .child(icon(&PLUS, 12.0).color(t.accent_text))
        .child(text(words.to_string()).font(sans(12.0, 400.0, 1.5)).color(t.accent_text))
        .on_press(press)
}

fn outcomes(t: &Theme, ed: &Editor, draft: &WorkflowDraft, step: &Step, index: usize) -> El {
    let a = &ed.inspector.agent;
    let loops = crate::fleet::workflows::draft::loop_outcomes(draft, index);
    let mut list = col().key("workflow-outcomes").w_full().gap(4.0);
    for (at, outcome) in step.outcomes.iter().enumerate() {
        let (Some(name), Some(max)) = (a.outcomes.get(at), a.maxes.get(at)) else { continue };
        let input = mono_mini(t, name, "Outcome")
            .key(format!("workflow-outcome-name-{at}"))
            .w(0.0)
            .grow(1.0)
            .min_w(0.0)
            .on_edit(move |s: &mut State| &mut s.workflows_nav.editor.inspector.agent.outcomes[at])
            .on_input_key(move |s: &mut State, ev: &KeyEvent, _: &mut Tx| {
                if ev.key == Key::Enter {
                    s.workflow_outcome_commit(at);
                    return true;
                }
                false
            });
        let to = step.routes.iter().find(|(k, _)| k == outcome).map(|(_, to)| to.as_str());
        let open = a.menu == Some(AgentMenu::Route(at));
        let leads = select_box(t, &format!("\u{2192} {}", crate::fleet::workflows::draft::target_title(draft, to)), open)
            .key(format!("workflow-outcome-to-{at}"))
            .label("Leads to")
            .w(0.0)
            .grow(1.2)
            .min_w(0.0)
            .on_press(move |s: &mut State| s.workflow_agent_menu(if open { None } else { Some(AgentMenu::Route(at)) }));
        let mut line = row().key(format!("workflow-outcome-{outcome}")).w_full().gap(6.0).center_y().child(input).child(leads);
        if loops.contains(outcome) {
            let bad = step.max_loops.is_none();
            let num = text_input(max, sans(12.0, 400.0, 1.4))
                .key(format!("workflow-outcome-max-{at}"))
                .single_line()
                .label("Maximum times")
                .color(t.text)
                .caret(t.text, t.accent.alpha(0.35))
                .w(44.0)
                .px(8.0)
                .py(4.0)
                .radius(6.0)
                .fill(raise(t))
                .border(1.0, if bad { t.error } else { t.border })
                .border_layout()
                .on_edit(move |s: &mut State| &mut s.workflows_nav.editor.inspector.agent.maxes[at])
                .focus_ring(2.0, 0.0, t.accent);
            line = line.child(row().gap(4.0).center_y().child(text("max").font(sans(11.5, 400.0, 1.5)).color(t.muted)).child(num));
        }
        line = line.child(trash(t, format!("workflow-outcome-remove-{at}"), "Remove the outcome", move |s: &mut State| {
            s.workflows_nav.editor.inspector_remove_outcome(at)
        }));
        list = list.child(line);
    }
    let mut out = col().w_full().child(label(t, "Outcomes")).child(list);
    if loops.len() > 1 {
        let max = step.max_loops.map_or_else(|| "The max is".to_string(), |n| format!("At most {n}"));
        out = out.child(help(t, "workflow-shared-max", format!("{max} for this step's loops: {} share it.", loops.join(" and ")), false));
    }
    out.child(row().child(add(t, "workflow-outcome-add", "Add an outcome", |s: &mut State| s.workflows_nav.editor.inspector_add_outcome())))
        .child(help(
            t,
            "workflow-outcomes-help",
            "An outcome that goes back to an earlier step is a loop. It needs a max; the next time goes to you.".into(),
            false,
        ))
}

// ---- Writes and Optional ------------------------------------------------------------------------------------------------

fn writes(t: &Theme, ed: &Editor, step: &Step) -> El {
    let mut list = col().key("workflow-writes").w_full().gap(4.0);
    for at in 0..step.writes.len() {
        let Some(edit) = ed.inspector.agent.writes.get(at) else { continue };
        let input = mono_mini(t, edit, "Declared file")
            .key(format!("workflow-write-{at}"))
            .w(0.0)
            .grow(1.0)
            .min_w(0.0)
            .on_edit(move |s: &mut State| &mut s.workflows_nav.editor.inspector.agent.writes[at]);
        list = list.child(row().w_full().gap(6.0).center_y().child(input).child(trash(
            t,
            format!("workflow-write-remove-{at}"),
            "Remove the file",
            move |s: &mut State| s.workflows_nav.editor.inspector_remove_write(at),
        )));
    }
    let mut out = col().w_full().child(label(t, "Writes")).child(list);
    if step.writes.is_empty() {
        out = out.child(help(t, "workflow-writes-empty", "No declared files.".into(), false).mt(0.0));
    }
    out.child(row().child(add(t, "workflow-write-add", "Add a file", |s: &mut State| s.workflows_nav.editor.inspector_add_write())))
        .child(help(
        t,
        "workflow-writes-help",
        "Paths in the run's worktree. Fleet checks they exist before the next step and commits them; a path git ignores stays uncommitted."
            .into(),
        false,
    ))
}

fn optional(t: &Theme, ed: &Editor, step: &Step) -> El {
    let group = col()
        .key("workflow-optional")
        .role("radiogroup")
        .label("Optional")
        .w_full()
        .gap(4.0)
        .child(radio(t, "workflow-step-always", !step.optional, "Always runs", "", |s: &mut State| {
            s.workflows_nav.editor.inspector_pick_optional(false)
        }))
        .child(radio(
            t,
            "workflow-step-optional",
            step.optional,
            "Off unless switched on for a run",
            "Shows as a switch in the Run box",
            |s: &mut State| s.workflows_nav.editor.inspector_pick_optional(true),
        ));
    let mut out = col().w_full().gap(6.0).child(label(t, "Optional")).child(group);
    if step.optional {
        out = out.child(
            mini(t, &ed.inspector.agent.hint, "When to switch it on")
                .key("workflow-step-hint")
                .placeholder("When to switch it on, e.g. For UI and new features".to_string(), t.muted)
                .w_full()
                .on_edit(|s: &mut State| &mut s.workflows_nav.editor.inspector.agent.hint),
        );
    }
    out
}
