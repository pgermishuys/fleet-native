//! `WorkflowRunGroup.vue`: a workflow run in the Sessions list. A head row with the run's title and status, "Started by" under
//! it when an automation started the run, and the run's step sessions under that, each named by its step.
use super::stepper::started_by;
use crate::fleet::workflows::helpers::{Tone, run_status_label};
use crate::fleet::workflows::{Run, RunStatus};
use crate::icons::WORKFLOW;
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::view::parts::status_glyph;
use crate::view::session_row::{RowClock, Slots, session_row_with};
use crate::{State, Tx};
use rune::{Cursor, El, col, el, icon, row, text};

/// What the head says when neither the run nor a step's title names it.
const UNNAMED: &str = "Workflow run";

/// The group: `steps` are the run's step sessions in the order they ran, as their places in `State::projects` and their labels.
pub fn run_group(s: &State, t: &Theme, clock: &RowClock, run_id: &str, run: Option<&Run>, steps: &[((usize, usize), String)]) -> El {
    let mut group = col().key(format!("workflow-run-group-{run_id}")).w_full().child(head(s, t, run_id, run, steps));
    if let Some(by) = run.and_then(|r| r.started_by.as_ref()) {
        group = group.child(row().w_full().min_w(0.0).pl(42.0).pr(10.0).pb(2.0).child(started_by(t, by)));
    }
    for ((p, i), label) in steps {
        let session = &s.projects[*p].sessions[*i];
        // "With you" on the step the user finishes, while it is open.
        let note = run.filter(|r| r.status == RunStatus::Running && r.with_you.as_ref().is_some_and(|w| w.session_id == session.id));
        let slots = Slots { label: Some(label), step: true, step_note: note.map(|_| "With you"), ..Slots::default() };
        group = group.child(col().w_full().pl(32.0).child(session_row_with(s, t, clock, *p, *i, session, slots)));
    }
    group
}

/// The run's title, else the first step session's title before " · ", else a plain name.
fn title(s: &State, run: Option<&Run>, steps: &[((usize, usize), String)]) -> String {
    if let Some(run) = run {
        return run.title.clone();
    }
    let first = steps.first().map(|((p, i), _)| s.projects[*p].sessions[*i].title.as_str());
    first.and_then(|t| t.split(" \u{b7} ").next()).filter(|t| !t.is_empty()).unwrap_or(UNNAMED).to_string()
}

fn head(s: &State, t: &Theme, run_id: &str, run: Option<&Run>, steps: &[((usize, usize), String)]) -> El {
    let title = title(s, run, steps);
    let status = run.map(run_status_label);
    let label = format!(
        "{title}, {}{}",
        run.map_or("workflow run", |r| r.workflow_name.as_str()),
        status.as_ref().map_or(String::new(), |(words, _)| format!(", {words}"))
    );
    // The session the run waits in, else its newest step.
    let waiting = run.and_then(|r| r.waiting.as_ref()?.session_id.as_deref());
    let target = waiting
        .and_then(|id| steps.iter().find(|((p, i), _)| s.projects[*p].sessions[*i].id == id))
        .or_else(|| steps.last())
        .map(|&((p, i), _)| (p, i, s.projects[p].sessions[i].id.clone()));
    // The 8 px glyph column holds the working quad or the 13 px mark, so the title never moves when the run starts or stops.
    let lead = if run.is_some_and(|r| r.status == RunStatus::Running) {
        status_glyph(t, "active", false)
    } else {
        icon(&WORKFLOW, 13.0).color(t.accent).mx(-2.5)
    };
    let mut head = row()
        .key(format!("workflow-run-head-{run_id}"))
        .role("button")
        .label(label)
        .w_full()
        .min_w(0.0)
        .min_h(32.0)
        .center_y()
        .gap(9.0)
        .pl(6.0)
        .pr(10.0)
        .radius(RADIUS_BTN)
        .hover(t.wash(5.0))
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        // The empty caret place a top-level row keeps, so the title lines up with the sessions'.
        .child(el().size(14.0, 14.0).mr(-4.0))
        .child(lead)
        .child(text(title).key(format!("workflow-run-title-{run_id}")).font(sans(13.0, 500.0, 1.3)).color(t.text).truncate().grow(1.0));
    if let Some((words, tone)) = status {
        let ink = match tone {
            Tone::Wait => t.idle_text,
            Tone::Run => t.running_text,
            Tone::With => t.accent_text,
            Tone::Fail => t.error_text,
            Tone::Done => t.muted.alpha(0.8),
        };
        head = head.child(text(words).key(format!("workflow-run-status-{run_id}")).font(sans(12.0, 400.0, 1.3)).color(ink));
    }
    if let Some((p, i, id)) = target {
        head = head.on_press_tx(move |s: &mut State, tx: &mut Tx| s.press_row(&id, None, move |s, tx| s.open_session(p, i, tx), tx));
    }
    head
}
