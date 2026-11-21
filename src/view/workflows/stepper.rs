//! `WorkflowStepper.vue` and `WorkflowStartedBy.vue`: every step of the run the open session is a step of, under the session's
//! header. The run line ("Step of <title> · <workflow>", the branch, who started it, the Check with me switch), the note and
//! the error under it, and a pill per step that runs. The state is `fleet::workflows::stepper`.
use crate::fleet::workflows::helpers::{check_with_me_note, ordinal};
use crate::fleet::workflows::stepper::finished;
use crate::fleet::workflows::{Run, RunStep, StartedBy, StepState};
use crate::icons::{CHECK, USER_ROUND, WORKFLOW, ZAP};
use crate::theme::{Theme, mono, sans};
use crate::view::parts::status_glyph;
use crate::{State, Tx};
use rune::{Color, Cursor, El, col, el, icon, row, spacer, text};

/// Nothing at all for a session that is not a step of a run the machine's store knows.
pub fn stepper(s: &State, t: &Theme) -> Option<El> {
    let run = s.stepper_run()?;
    let session = s.session().id.as_str();
    let mine = run.sessions.iter().find(|x| x.session_id == session).map(|x| x.step_id.as_str());
    let over = finished(run);
    let mut line = row().key("workflow-stepper-run").w_full().min_w(0.0).center_y().gap(6.0);
    line = line.child(icon(&WORKFLOW, 12.0).color(t.accent)).child(title(t, run));
    if let Some(branch) = run.branch.as_deref().filter(|b| !b.is_empty()) {
        line = line.child(text(branch.to_string()).font(mono(11.0, 400.0, 1.4)).color(t.muted));
    }
    if let Some(by) = &run.started_by {
        line = line.child(started_by(t, by));
    }
    if !over {
        line = line.child(spacer()).child(check_switch(s, t, run));
    }
    let mut stepper =
        col().key("workflow-stepper").w_full().gap(6.0).pt(8.0).px(16.0).pb(10.0).border_sides([0.0, 0.0, 1.0, 0.0], t.border);
    stepper = stepper.child(line);
    if !over && let Some(note) = check_with_me_note(run) {
        stepper = stepper.child(text(note).key("workflow-check-with-me-note").font(sans(12.0, 400.0, 1.4)).color(t.muted).wrap().w_full());
    }
    if let Some(error) = s.stepper_error() {
        stepper = stepper.child(
            text(error.to_string())
                .key("workflow-stepper-error")
                .role("alert")
                .font(sans(12.0, 400.0, 1.4))
                .color(t.error_text)
                .wrap()
                .w_full(),
        );
    }
    let steps: Vec<&RunStep> = run.steps.iter().filter(|x| x.enabled).collect();
    let mut list = row().key("workflow-stepper-steps").w_full().flex_wrap().items_center().gaps(4.0, 0.0);
    for (i, step) in steps.iter().enumerate() {
        list = list.child(item(s, t, step, i > 0, mine == Some(step.id.as_str()), session));
    }
    Some(stepper.child(list))
}

/// "Step of **<title>** · <workflow>": the title in the text colour.
fn title(t: &Theme, run: &Run) -> El {
    let plain = sans(12.0, 400.0, 1.4);
    row()
        .min_w(0.0)
        .shrink()
        .child(text("Step of ").font(plain).color(t.muted))
        .child(text(run.title.clone()).font(sans(12.0, 500.0, 1.4)).color(t.text).truncate().shrink().min_w(0.0))
        .child(text(format!(" \u{b7} {}", run.workflow_name)).font(plain).color(t.muted).truncate().shrink().min_w(0.0))
}

/// `WorkflowStartedBy`: a link that opens the automation (Zap, "Started by <name>", 11.5 px).
pub(super) fn started_by(t: &Theme, by: &StartedBy) -> El {
    let id = by.automation_id.clone();
    row()
        .key("workflow-started-by")
        .role("button")
        .label(format!("Started by {}", by.automation_name))
        .gap(4.0)
        .center_y()
        .min_w(0.0)
        .shrink()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, t.accent)
        .help(format!("Open the {} automation", by.automation_name))
        .child(icon(&ZAP, 12.0).color(t.muted))
        .child(text("Started by").font(sans(11.5, 400.0, 1.4)).color(t.muted))
        .child(text(by.automation_name.clone()).font(sans(11.5, 500.0, 1.4)).color(t.text).truncate().shrink().min_w(0.0))
        .on_press(move |s: &mut State| s.open_started_by(&id))
}

/// `.wf-check`: a pill with a person, "Check with me" and a 26 × 15 track. Disabled (70 %) while the change is on its way.
fn check_switch(s: &State, t: &Theme, run: &Run) -> El {
    let switching = s.workflows_nav.stepper.switching;
    let on = run.check_with_me;
    let track = if on { t.accent } else { t.text.alpha(0.18) };
    let thumb = el().size(11.0, 11.0).radius(5.5).fill(t.panel_bg).ml(if on { 13.0 } else { 2.0 });
    let b = row()
        .key("workflow-check-with-me")
        .role("switch")
        .checked(on)
        .label("Check with me")
        .gap(5.0)
        .center_y()
        .pl(8.0)
        .pr(4.0)
        .py(2.0)
        .radius(999.0)
        .border(1.0, t.border)
        .border_layout()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 1.0, t.accent)
        .opacity(if switching { 0.7 } else { 1.0 })
        .help("Check with me after each step. Applies to the steps that haven't finished.")
        .child(icon(&USER_ROUND, 12.0).color(t.accent))
        .child(text("Check with me").font(sans(12.0, 500.0, 1.4)).color(t.text))
        .child(row().size(26.0, 15.0).center_y().radius(7.5).fill(track).child(thumb));
    if switching { b.disabled(true) } else { b.on_press_tx(|s: &mut State, tx: &mut Tx| s.stepper_toggle_check(tx)) }
}

/// One `<li>`: the 14 px rule that joins it to the step before, and the pill.
fn item(s: &State, t: &Theme, step: &RunStep, joined: bool, current: bool, session: &str) -> El {
    let _ = s;
    let mut li = row().center_y();
    if joined {
        li = li.child(el().w(14.0).h(1.0).mx(2.0).fill(t.border));
    }
    let ink = match step.state {
        StepState::Pending => t.muted,
        StepState::Waiting => t.idle_text,
        _ => t.text,
    };
    let lead = match step.state {
        StepState::Done => icon(&CHECK, 12.0).color(t.running_text),
        StepState::Running => status_glyph(t, "active", false),
        StepState::Waiting => el().size(7.0, 7.0).radius(3.5).fill(t.idle),
        _ => el().size(7.0, 7.0).radius(3.5).border(1.5, ink.alpha(ink.a * 0.6)).border_layout(),
    };
    let extra = if step.visits > 1 {
        Some(ordinal(step.visits))
    } else {
        step.max_loops.filter(|_| step.state == StepState::Pending).map(|n| format!("\u{2264}{n}\u{d7}"))
    };
    let target = step.session_id.clone().filter(|x| !x.is_empty() && x != session);
    let tip = match &step.outcome {
        Some(outcome) => format!("{}: {outcome}", step.title),
        None => step.title.clone(),
    };
    let mut pill = row()
        .key(format!("workflow-step-{}", step.id))
        .role("button")
        .label(step.title.clone())
        .gap(5.0)
        .center_y()
        .px(8.0)
        .py(3.0)
        .radius(999.0)
        .border(1.0, if current { t.border } else { Color::TRANSPARENT })
        .border_layout()
        .focus_ring(2.0, 1.0, t.accent)
        .help(tip)
        .child(lead)
        .child(text(step.title.clone()).font(sans(12.0, if current { 500.0 } else { 400.0 }, 1.4)).color(ink));
    if current {
        pill = pill.fill(t.wash(6.0));
    }
    if let Some(extra) = extra {
        pill = pill.child(text(extra).font(sans(10.5, 400.0, 1.4)).color(t.muted));
    }
    if step.with_you && !matches!(step.state, StepState::Done | StepState::Skipped) {
        pill = pill.child(
            icon(&USER_ROUND, 12.0).color(t.accent_text).key(format!("workflow-step-with-you-{}", step.id)).label("You finish this step"),
        );
    }
    pill = match target {
        Some(id) => {
            pill.cursor(Cursor::Pointer).hover(t.wash(6.0)).on_press_tx(move |s: &mut State, tx: &mut Tx| s.stepper_open_step(&id, tx))
        }
        None => pill.disabled(true),
    };
    li.child(pill)
}
