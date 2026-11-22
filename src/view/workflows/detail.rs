//! A workflow's detail (`WorkflowDetailPanel.vue`, the read view) and `WorkflowStartedBy.vue`: the header with its actions,
//! the blurb, the file's errors, the steps strip, the loops, the facts and the recent runs. The Run box (piece D) goes under
//! the runs; the designer for a repository's workflow (piece M) will take the pane for one while it is edited.
use crate::State;
use crate::conv::time::{format_timestamp, local_offset_secs};
use crate::fleet::work::parse_iso_ms;
use crate::fleet::workflows::detail::{RECENT, models, run_text, skills};
use crate::fleet::workflows::helpers::{Tone, loop_notes};
use crate::fleet::workflows::{Run, Step, StepKind, Workflow};
use crate::icons::{BOT, CALENDAR_CLOCK, CHECK, COPY, PENCIL, REPEAT, TRIANGLE_ALERT, USER_ROUND, WORKFLOW, ZAP};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::{empty_state, rem};
use crate::view::{files_tab, parts};
use rune::{Cursor, El, col, el, icon, row, text};

/// "Pick a repository in the Run box first": the title of a button that needs one.
const NEEDS_REPOSITORY: &str = "Pick a repository in the Run box first";

/// The whole read view: a column at most 760 wide, centred, gap 20, padding 28/24/40.
pub fn read(s: &State, t: &Theme, w: &Workflow) -> El {
    let nav = &s.workflows_nav;
    let machine = s.workflows_machine();
    let roles = s.workflow_model_roles(&machine);
    let harness = s.workflow_default_harness(&machine);
    let mut c = col().key("workflow-detail").w_full().max_w(760.0).self_center().gap(20.0).pt(28.0).px(24.0).pb(40.0);
    c = c.child(header(t, w, nav.repository_path().is_some()));
    if w.built_in {
        // `margin: -8px 0 0` under a gap of 20.
        c = c.child(
            text("Built-ins can't be edited: duplicate this one into your repo to change it, and the copy is yours to share.")
                .key("workflow-hint")
                .font(sans(12.5, 400.0, 1.5))
                .color(t.muted)
                .wrap()
                .mt(-8.0),
        );
    }
    if let Some(blurb) = w.description.as_deref().filter(|d| !d.is_empty()) {
        c = c.child(text(blurb.to_string()).key("workflow-blurb").font(sans(14.0, 400.0, 1.55)).color(t.text.alpha(0.85)).wrap());
    }
    if !w.errors.is_empty() {
        c = c.child(errors(t, &w.errors));
    }
    if !w.steps.is_empty() {
        c = c.child(steps(t, w));
    }
    c = c.child(facts(t, &models(w, &roles, &harness), &skills(w)));
    c.child(recent_runs(s, t, w, &machine)).child(super::run_box::view(s, t, w))
}

/// `PageHeader`: the leading icon box, the title and the pill on the left; the actions on the right.
fn header(t: &Theme, w: &Workflow, has_repository: bool) -> El {
    let (pill, ink, edge) = match (&w.file, w.built_in) {
        (Some(file), false) => (file.clone(), t.accent_text, t.mix(t.accent, t.border, 0.45)),
        _ => ("Built into Fleet".to_string(), t.muted, t.border),
    };
    let mark = row()
        .key("workflow-icon")
        .size(28.0, 28.0)
        .center()
        .radius(RADIUS_BTN)
        .fill(t.accent.alpha(0.14))
        .child(icon(&WORKFLOW, 15.0).color(t.accent));
    let mut pill = row()
        .key("workflow-pill")
        .px(8.0)
        .py(2.0)
        .radius(999.0)
        .border(1.0, edge)
        .border_layout()
        .child(text(pill).font(if w.built_in { sans(11.0, 400.0, 1.5) } else { mono(11.0, 400.0, 1.5) }).color(ink).truncate());
    pill = pill.shrink().min_w(0.0);
    let title = text(w.name.clone()).font(sans(rem(1.5), 600.0, 1.25).tracking(-0.525)).color(t.text).wrap().shrink().min_w(0.0);
    let lead = row()
        .key("workflow-header")
        .grow(1.0)
        .shrink()
        .min_w(rem(16.0))
        .flex_wrap()
        .center_y()
        .gap(10.0)
        .child(mark)
        .child(title)
        .child(pill);
    // Repeat on a schedule… waits for a repository and a file without errors; it opens a new automation that runs this.
    let can_repeat = has_repository && w.errors.is_empty();
    let mut repeat = button(t, Variant::Outline, Size::Sm, Some(&CALENDAR_CLOCK), "Repeat on a schedule…")
        .key("workflow-repeat-on-schedule")
        .help(if has_repository { "Run this workflow on a schedule, as an automation" } else { NEEDS_REPOSITORY });
    repeat = if can_repeat {
        repeat.on_press_tx(|s: &mut State, tx: &mut rune::Tx| s.repeat_workflow_on_schedule(tx))
    } else {
        disabled(repeat)
    };
    let mut actions = row().gap(8.0).center_y().flex_wrap().justify_end().child(repeat);
    if w.built_in {
        let source = crate::fleet::workflows::create::Source { id: w.id.clone(), name: w.name.clone() };
        let mut dup = button(t, Variant::Default, Size::Sm, Some(&COPY), "Duplicate to this repo").key("workflow-duplicate");
        dup = if has_repository {
            dup.on_press_tx(move |s: &mut State, tx: &mut rune::Tx| s.workflow_create_open(Some(source.clone()), tx))
        } else {
            disabled(dup.help(NEEDS_REPOSITORY))
        };
        actions = actions.child(dup);
    } else {
        // Back to the designer (`nav.showRuns(false)`).
        let edit = button(t, Variant::Outline, Size::Sm, Some(&PENCIL), "Edit").key("workflow-edit");
        actions = actions.child(edit.on_press(|s: &mut State| s.workflows_nav.showing_runs = false));
    }
    row().w_full().flex_wrap().center_y().justify_between().gap(8.0).child(lead).child(actions)
}

/// `.wf-detail__errors`: a bordered box, one line each, in the error colour.
fn errors(t: &Theme, errors: &[String]) -> El {
    col()
        .key("workflow-errors")
        .role("alert")
        .w_full()
        .gap(6.0)
        .px(12.0)
        .py(10.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.mix(t.error, t.border, 0.4))
        .border_layout()
        .children(errors.iter().map(|e| {
            row()
                .w_full()
                .gap(8.0)
                .items_start()
                .child(icon(&TRIANGLE_ALERT, 13.0).color(t.error_text).mt(2.0))
                .child(text(e.clone()).font(mono(12.0, 400.0, 1.5)).color(t.error_text).wrap().shrink().min_w(0.0))
        }))
}

/// `.wf-sec-label`: 11 px 600 uppercase, muted.
fn section_label(t: &Theme, label: &str) -> El {
    text(label.to_uppercase()).font(sans(11.0, 600.0, 1.5).tracking(0.55)).color(t.muted).mb(8.0)
}

/// The steps strip: a chip per step with an arrow between, wrapping; then the loops.
fn steps(t: &Theme, w: &Workflow) -> El {
    let mut strip = row().key("workflow-steps").w_full().flex_wrap().center_y().gap(6.0);
    for (i, step) in w.steps.iter().enumerate() {
        if i > 0 {
            strip = strip.child(text("\u{2192}").font(sans(12.0, 400.0, 1.5)).color(t.muted));
        }
        strip = strip.child(chip(t, step, i));
    }
    let mut c = col().w_full().child(section_label(t, "Steps")).child(strip);
    let loops = loop_notes(w);
    if !loops.is_empty() {
        c = c.child(row().key("workflow-loops").w_full().flex_wrap().gap(14.0).mt(8.0).children(loops.into_iter().map(|note| {
            row()
                .gap(5.0)
                .center_y()
                .child(icon(&REPEAT, 12.0).color(t.muted))
                .child(text(note).font(sans(12.0, 400.0, 1.5)).color(t.muted))
        })));
    }
    c
}

fn chip(t: &Theme, step: &Step, index: usize) -> El {
    let you = step.kind == StepKind::You;
    let (edge, ink) = if you { (t.mix(t.idle, t.border, 0.45), t.idle) } else { (t.border, t.muted) };
    let mut e = row()
        .key(format!("workflow-step-{index}"))
        .gap(6.0)
        .center_y()
        .px(9.0)
        .py(4.0)
        .radius(RADIUS_BTN)
        .fill(t.card_bg)
        .border(1.0, edge)
        .border_layout()
        .child(icon(if you { &USER_ROUND } else { &BOT }, 12.0).color(ink))
        .child(text(step.title.clone()).font(sans(12.5, 400.0, 1.4)).color(t.text));
    if step.optional {
        e = e.border_dash(4.0, 3.0).child(text("(optional)").font(sans(12.0, 400.0, 1.4)).color(t.muted));
    }
    e
}

/// `.wf-facts`: auto-fit columns of at least 180.
fn facts(t: &Theme, models: &str, skills: &[&str]) -> El {
    let fact = |key: &str, label: String, body: El| {
        col()
            .key(key.to_string())
            .min_w(180.0)
            .grow(1.0)
            .gap(3.0)
            .child(text(label).font(sans(11.5, 400.0, 1.4)).color(t.muted).wrap().shrink().min_w(0.0))
            .child(body)
    };
    let dd = |words: String| text(words).font(sans(13.0, 400.0, 1.45)).color(t.text).wrap();
    let mut third = col().gap(2.0).child(dd(models.to_string()));
    if !skills.is_empty() {
        third = third.child(text(skills.join(", ")).font(mono(11.0, 400.0, 1.5)).color(t.muted).wrap());
    }
    let label = format!("Models{} (roles are under Model roles)", if skills.is_empty() { "" } else { " \u{b7} skills" });
    row()
        .key("workflow-facts")
        .w_full()
        .flex_wrap()
        .gap(12.0)
        .child(fact("workflow-fact-starts", "Starts from".into(), dd("A sentence".into())))
        .child(fact("workflow-fact-runs", "Runs in".into(), dd("A new worktree from the base branch".into())))
        .child(fact("workflow-fact-models", label, third))
}

/// `Recent runs`: the newest six of this workflow, a spinner until the runs are read, or "No runs yet".
fn recent_runs(s: &State, t: &Theme, w: &Workflow, machine: &str) -> El {
    let store = s.workflow_runs(machine);
    let runs: Vec<&Run> = store.ordered().into_iter().filter(|r| r.workflow_id == w.id).take(RECENT).collect();
    let body = if !runs.is_empty() {
        let last = runs.len() - 1;
        col()
            .key("workflow-runs")
            .w_full()
            .radius(RADIUS_CARD)
            .clip_rounded(RADIUS_CARD)
            .border(1.0, t.border)
            .border_layout()
            .children(runs.iter().enumerate().map(|(i, r)| run_row(s, t, r, i < last)))
    } else if store.loading && !store.loaded {
        row()
            .key("workflow-runs-loading")
            .label("Loading recent runs…")
            .w_full()
            .center()
            .px(12.0)
            .py(12.0)
            .child(files_tab::spinner(t.muted, 14.0))
    } else {
        empty_box(t)
    };
    col().w_full().child(section_label(t, "Recent runs")).child(body)
}

/// `EmptyState` bordered and compact: a dashed card.
fn empty_box(t: &Theme) -> El {
    empty_state::compact_bare(t, "workflow-runs-empty", "No runs yet")
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_dash(4.0, 3.0)
        .border_layout()
}

/// `.wf-runs__row`: 34 high; a glyph, the title, who started it, when, and where it is.
fn run_row(s: &State, t: &Theme, r: &Run, rule: bool) -> El {
    let glyph: El = match r.status {
        crate::fleet::workflows::RunStatus::Running => parts::status_glyph(t, "active", false),
        crate::fleet::workflows::RunStatus::Waiting => el().size(8.0, 8.0).radius(999.0).fill(t.idle),
        crate::fleet::workflows::RunStatus::Failed => icon(&TRIANGLE_ALERT, 13.0).color(t.error_text),
        _ => icon(&CHECK, 14.0).color(t.running_text),
    };
    let (words, tone) = run_text(r);
    let ink = match tone {
        Tone::Wait => t.idle_text,
        Tone::Run => t.running_text,
        Tone::With => t.accent_text,
        Tone::Fail => t.error_text,
        Tone::Done => t.muted,
    };
    let now = crate::view::message::now_ms(s);
    let when = format_timestamp(parse_iso_ms(&r.created_at).unwrap_or(0), now, local_offset_secs());
    let id = r.id.clone();
    let mut e = row()
        .key(format!("workflow-recent-run-{}", r.id))
        .role("button")
        .label(r.title.clone())
        .w_full()
        .min_h(34.0)
        .gap(10.0)
        .center_y()
        .px(12.0)
        .cursor(Cursor::Pointer)
        .hover(t.wash(5.0))
        .focus_ring(2.0, -2.0, t.accent)
        .child(row().w(16.0).center().child(glyph))
        .child(text(r.title.clone()).font(sans(13.0, 400.0, 1.4)).color(t.text).grow(1.0).shrink().min_w(0.0).truncate());
    if let Some(by) = &r.started_by {
        e = e.child(started_by_plain(t, &by.automation_name));
    }
    e = e
        .child(text(when).font(sans(12.0, 400.0, 1.4)).color(t.muted))
        .child(text(words).font(sans(12.0, 400.0, 1.4)).color(ink))
        .on_press_tx(move |s: &mut State, tx: &mut rune::Tx| s.open_workflow_run(&id, tx));
    if rule { e.border_sides([0.0, 0.0, 1.0, 0.0], t.border) } else { e }
}

/// `WorkflowStartedBy` in its plain form: inside something that is already a button, so no link.
pub fn started_by_plain(t: &Theme, name: &str) -> El {
    row()
        .key("workflow-started-by")
        .gap(4.0)
        .center_y()
        .min_w(0.0)
        .shrink()
        .child(icon(&ZAP, 12.0).color(t.muted))
        .child(text("Started by").font(sans(11.5, 400.0, 1.4)).color(t.muted))
        .child(text(name.to_string()).font(sans(11.5, 500.0, 1.4)).color(t.text).truncate().shrink().min_w(0.0))
}
