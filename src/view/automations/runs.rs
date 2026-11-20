//! An automation's runs (`AutomationRunsList.vue`, and the empty state `AutomationDetailPanel.vue` shows before the first):
//! the label with how many finished, then a row per run, newest first. A run that started a session opens it on a click.
use crate::State;
use crate::fleet::automations::model::{Automation, AutomationRun, RunState};
use crate::fleet::automations::schedule::describe_date;
use crate::fleet::automations::words::{Tone, describe_run_state, describe_run_trigger};
use crate::fleet::work::parse_iso_ms;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::rem;
use crate::view::parts;
use rune::{Cursor, El, Tx, col, el, row, text};

/// The runs, or the empty state once they have been read and there are none; nothing before that.
pub fn section(s: &State, t: &Theme, a: &Automation) -> Option<El> {
    let runs = &s.automations_nav.runs;
    if runs.of.as_ref().map(|(_, id)| id.as_str()) != Some(a.id.as_str()) {
        return None;
    }
    if runs.loaded && runs.list.is_empty() {
        return Some(no_runs(t, &s.first_run_hint(a)));
    }
    (!runs.list.is_empty()).then(|| list(t, &runs.list))
}

/// `EmptyState` with `bordered`: a dashed card, 21 px of padding, no icon.
fn no_runs(t: &Theme, hint: &str) -> El {
    col()
        .key("automation-no-runs")
        .w_full()
        .items_center()
        .gap(rem(0.25))
        .padding(rem(1.5))
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_dash(4.0, 3.0)
        .border_layout()
        .child(text("No runs yet").font(sans(rem(1.0), 600.0, 1.5)).color(t.text).wrap())
        .child(text(hint.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().max_w(513.0).align(rune::Align::Center))
}

/// `.automation-runs`: 760 wide at most, centred; its label and a row per run.
fn list(t: &Theme, runs: &[AutomationRun]) -> El {
    let done = runs.iter().filter(|r| r.state == RunState::Done).count();
    let label = row()
        .key("automation-runs-label")
        .w_full()
        .items_end()
        .px(10.0)
        .pb(6.0)
        .child(text("RUNS").font(sans(11.0, 600.0, 1.3).tracking(0.55)).color(t.muted).grow(1.0))
        .child(text(format!("{done} of {} finished", runs.len())).key("automation-runs-count").font(sans(12.0, 400.0, 1.3)).color(t.muted));
    col().key("automation-runs").role("region").label("Runs").w_full().max_w(760.0).child(label).children(runs.iter().map(|r| item(t, r)))
}

/// The glyph before a run: working while it starts or goes, waiting, failed; nothing for the rest.
fn glyph(t: &Theme, run: &AutomationRun) -> El {
    let slot = row().w(10.0).h(10.0).center();
    match run.state {
        RunState::Starting | RunState::Running => slot.label(describe_run_state(run).0).child(parts::status_glyph(t, "active", false)),
        RunState::Waiting => slot.label("Needs you").child(parts::status_glyph(t, "waiting_input", false)),
        RunState::Failed => slot.label("Failed").child(parts::status_glyph(t, "error", false)),
        _ => slot,
    }
}

/// `describeDate(scheduledFor ?? startedAt)`.
fn run_time(run: &AutomationRun) -> String {
    let tz = |ms: i64| crate::conv::time::local_offset_at(ms);
    let at = run.scheduled_for.as_deref().unwrap_or(&run.started_at);
    parse_iso_ms(at).map(|ms| describe_date(ms, &tz)).unwrap_or_default()
}

/// What the run did, and why it failed unless that is already said (`runNotes`).
fn notes(run: &AutomationRun) -> Vec<String> {
    let why = run.error.clone().filter(|e| !e.is_empty() && (run.summary.is_none() || run.state == RunState::Failed));
    run.summary.clone().into_iter().chain(why).collect()
}

/// `.automation-run`: glyph, time with the trigger, and at the end the way in and the state; the notes under the time.
fn item(t: &Theme, run: &AutomationRun) -> El {
    let (state, tone) = describe_run_state(run);
    let ink = match tone {
        Tone::Working => t.running_text,
        Tone::Error => t.error_text,
        Tone::Warn => t.status_waiting_text,
        Tone::Quiet => t.muted.alpha(0.85),
    };
    let mut title = row().items_end().gap(6.0).child(
        text(run_time(run)).key("automation-run-time").font(sans(13.0, 400.0, 1.3).features("'tnum' 1")).group_color(t.text).color(t.text),
    );
    let trigger = describe_run_trigger(&run.trigger);
    if !trigger.is_empty() {
        title = title.child(text(trigger).font(sans(12.0, 400.0, 1.3)).color(t.muted));
    }
    if let Some(name) = run.machine_name.as_deref().filter(|n| !n.is_empty()) {
        title = title.child(text(format!("on {name}")).key("automation-run-machine").font(mono(11.5, 400.0, 1.3)).color(t.coral_text));
    }
    let mut end = row().items_center().gap(10.0);
    let openable = run.session_id.is_some();
    if openable {
        let word = if run.workflow_run_id.is_some() { "Open run →" } else { "Open session →" };
        end = end.child(text(word).key("automation-run-open").font(sans(12.0, 400.0, 1.3)).color(t.muted).group_opacity(0.0, 1.0));
    }
    end = end.child(text(state).key("automation-run-state").font(sans(12.0, 400.0, 1.3)).color(ink));
    let mut body = col().w_full().min_w(0.0).child(
        row().w_full().min_h(24.0).gap(9.0).items_center().child(glyph(t, run)).child(title.grow(1.0).shrink().min_w(0.0)).child(end),
    );
    for (n, note) in notes(run).into_iter().enumerate() {
        let words = text(note).key("automation-run-note").font(sans(12.0, 400.0, 1.4)).color(t.muted).wrap().pl(19.0).pb(2.0);
        body = body.child(row().key(format!("automation-run-{}-note-{n}", run.id)).w_full().child(words));
    }
    let id = run.id.clone();
    let shell = el().key(format!("automation-run-{}", run.id)).w_full().min_h(34.0).px(10.0).py(5.0).radius(RADIUS_BTN).child(body);
    if openable {
        shell
            .role("button")
            .label(format!("{} {}", run_time(run), describe_run_state(run).0))
            .cursor(Cursor::Pointer)
            .hover(t.wash(5.0))
            .hover_group()
            .focus_ring(2.0, -2.0, t.accent)
            .on_press_tx(move |s: &mut State, tx: &mut Tx| s.open_automation_run(&id, tx))
    } else {
        shell
    }
}
