//! `WorkflowRunBox.vue`: the box under a workflow's recent runs. What the run should do in the composer's frame with a Run
//! button, the chips under it (folder, New worktree, base, optional steps, Check with me, harness, Project, Models) and the one
//! line under them: a warning, or the foot. The state is `fleet::workflows::run_box`; the folder, base and harness menus are
//! the New session page's.
use crate::fleet::workflows::run_box::{REQUEST_KEY, RUN_KEY, Warning, workflow_harnesses};
use crate::fleet::workflows::{Role, StepKind, Workflow};
use crate::icons::{BOT, GIT_BRANCH, PLAY, USER_ROUND};
use crate::theme::{RADIUS_PANEL, Theme, sans};
use crate::ui::chip::Chip;
use crate::ui::rem;
use crate::view::{base_picker, folder_menu, harness_picker, ring_popover};
use crate::{State, Tx};
use rune::{Color, Cursor, El, Shadow, Span, auto_grow, col, icon, rich, row, spacer, text};

use super::project_picker;

pub fn view(s: &State, t: &Theme, w: &Workflow) -> El {
    col().key("workflow-run-box").w_full().gap(6.0).child(frame(s, t, w)).child(strip(s, t, w)).child(note(s, t, w))
}

/// `ComposerFrame`: the field above and the toolbar below (760 wide at most, the radius of the panel plus 2).
fn frame(s: &State, t: &Theme, w: &Workflow) -> El {
    let run_box = &s.workflows_nav.run_box;
    let starting = run_box.starting;
    let placeholder = w.placeholder.clone().filter(|p| !p.is_empty()).unwrap_or_else(|| "What should it do?".into());
    let mut field = crate::ui::text_menu::text_input(&run_box.request, sans(14.0, 400.0, 1.5))
        .key(REQUEST_KEY)
        .label("What the run should do")
        .placeholder(placeholder, t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .pt(14.0)
        .px(16.0)
        .pb(6.0)
        .min_h(52.0)
        .disabled(starting);
    if !starting {
        field = field.on_edit(|s: &mut State| &mut s.workflows_nav.run_box.request).on_submit(|s: &mut State, tx| s.run_box_run(tx));
    }
    let toolbar =
        row().key("workflow-run-toolbar").w_full().center_y().gap(2.0).pt(4.0).px(8.0).pb(8.0).child(spacer()).child(run_button(s, t, w));
    col()
        .key("workflow-run-frame")
        .w_full()
        .max_w(760.0)
        .self_center()
        .radius(RADIUS_PANEL + 2.0)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .box_shadow(Shadow { x: 0.0, y: 10.0, blur: 28.0, spread: -18.0, color: Color::BLACK.alpha(0.5), inset: false })
        .focus_within_ring(1.0, -1.0, t.accent_border(55.0))
        // `rows=2` and `min-height: 52px` on the field: Chrome lays out `rows=2` plus its padding at 66.
        .child(auto_grow(field, 2, 7).min_h(66.0))
        .child(toolbar)
}

/// `.composer-frame__send` as `.wf-run__go` makes it: 30 high, a pill, Play (a spinner while it starts) and "Run".
fn run_button(s: &State, t: &Theme, w: &Workflow) -> El {
    let starting = s.workflows_nav.run_box.starting;
    let can = s.run_box_can_run(w);
    let lead = if starting {
        icon(&crate::icons::LOADER_CIRCLE, 14.0).color(t.primary_fg).cycle(ring_popover::spinner())
    } else {
        icon(&PLAY, 14.0).color(t.primary_fg)
    };
    let b = row()
        .key(RUN_KEY)
        .role("button")
        .label("Run")
        .h(30.0)
        .px(12.0)
        .gap(6.0)
        .center()
        .radius(999.0)
        .fill(t.accent)
        .focus_ring(2.0, 2.0, t.accent)
        .opacity(if can || starting { 1.0 } else { 0.5 })
        .child(lead)
        .child(text("Run").font(sans(rem(0.875), 500.0, 1.4)).color(t.primary_fg));
    if can {
        b.hover(Color::BLACK.alpha(0.1)).cursor(Cursor::Pointer).on_press_tx(|s: &mut State, tx: &mut Tx| s.run_box_run(tx))
    } else {
        b.disabled(true)
    }
}

/// The roles this workflow's enabled steps ask for, in Strong, Standard, Fast order (`rolesUsed`).
pub(super) fn roles_used(s: &State, w: &Workflow) -> Vec<Role> {
    let on = &s.workflows_nav.run_box.optional_on;
    crate::fleet::workflows::ROLES
        .into_iter()
        .filter(|r| {
            w.steps
                .iter()
                .any(|st| st.kind == StepKind::Agent && st.model.as_deref() == Some(r.key()) && (!st.optional || on.contains(&st.id)))
        })
        .collect()
}

/// `.wf-run__strip`: wraps, 2 apart.
fn strip(s: &State, t: &Theme, w: &Workflow) -> El {
    let run_box = &s.workflows_nav.run_box;
    let starting = run_box.starting;
    let mut r = row().key("workflow-run-strip").w_full().flex_wrap().center_y().gap(2.0);
    r = r.child(folder_menu::trigger(s, t));
    r = r.child(
        Chip::new("workflow-new-worktree", "New worktree")
            .glyph(&GIT_BRANCH)
            .flat()
            .still()
            .help(Some("Each run gets its own worktree and branch; every step works in it.".into()))
            .view(t, |_: &mut State, _: &mut Tx| {}),
    );
    if s.workflows_nav.repository_path().is_some() {
        r = r.child(base_picker::chip(s, t));
    }
    for step in w.steps.iter().filter(|st| st.optional) {
        let on = run_box.optional_on.contains(&step.id);
        let id = step.id.clone();
        r = r.child(
            Chip::new(format!("workflow-optional-{}", step.id), if on { "on" } else { "off" })
                .head(format!("{}: ", step.title))
                .flat()
                .disabled(starting)
                .help(step.optional_hint.clone())
                .view(t, move |s: &mut State, _: &mut Tx| s.workflows_nav.run_box.toggle_optional(&id)),
        );
    }
    r = r.child(
        Chip::new("workflow-check-with-me-start", if run_box.check_with_me { "on" } else { "off" })
            .glyph(&USER_ROUND)
            .head("Check with me: ")
            .flat()
            .disabled(starting)
            .help(Some(
                "Check with me after each step: every step waits for you to move it on, and you can talk to the agent in between. You can change it later from the run's header."
                    .into(),
            ))
            .view(t, |s: &mut State, _: &mut Tx| s.workflows_nav.run_box.check_with_me ^= true),
    );
    if workflow_harnesses(&s.run_box_harnesses()).len() > 1 {
        r = r.child(harness_picker::chip(s, t));
    }
    r = r.child(project_picker::chip(s, t));
    let changed = run_box.overrides.len();
    let models = if changed > 0 { format!("Models \u{b7} {changed} changed") } else { "Models".to_string() };
    r.child(
        Chip::new("workflow-models", models)
            .glyph(&BOT)
            .open(s.models_popover_open())
            .disabled(starting || roles_used(s, w).is_empty())
            .view(t, super::roles::open_popover),
    )
}

/// The first that holds under the strip (`role=alert`): the file's errors, no harness, a skill that's off, the start error;
/// else the foot.
fn note(s: &State, t: &Theme, w: &Workflow) -> El {
    let link = |key: &str, words: &str, go: fn(&mut State)| {
        let spans = vec![Span::new(0, words.len()).underline(t.accent_text)];
        row()
            .key(key.to_string())
            .role("button")
            .label(words.to_string())
            .cursor(Cursor::Pointer)
            .focus_ring(2.0, 0.0, t.accent)
            .on_press(go)
            .child(rich(words.to_string(), spans).font(sans(12.0, 400.0, 1.5)).color(t.accent_text))
    };
    let words = |w: String, color: Color| text(w).font(sans(12.0, 400.0, 1.5)).color(color).wrap();
    let line = || row().key("workflow-run-note").w_full().px(8.0).flex_wrap().gap(4.0);
    match s.run_box_warning(w) {
        Some(Warning::FixFile) => line().role("alert").child(words("Fix the workflow file first.".into(), t.error_text)),
        Some(Warning::NoHarness(why)) => line().role("alert").key("workflow-no-harness").child(words(why, t.error_text)).child(link(
            "workflow-no-harness-link",
            "Set one up in Settings \u{2192} Harnesses",
            |s| s.open_settings("harnesses", None),
        )),
        Some(Warning::SkillOff { step, skill }) => {
            line().role("alert").key("workflow-skill-off").child(words(format!("{step} uses {skill}, which is off."), t.error_text)).child(
                link("workflow-skill-off-link", "Turn it on in Settings \u{2192} Skills & tools", |s| s.open_settings("skills", None)),
            )
        }
        Some(Warning::StartError(why)) => line().role("alert").key("workflow-start-error").child(words(why, t.error_text)),
        None => {
            let foot = "Runs as a group in Sessions. It stops at each You decide step and shows it under Needs you.";
            let at = foot.find("You decide").unwrap_or(0);
            let spans = vec![Span::new(at, at + "You decide".len()).weight(500.0).color(t.idle_text)];
            line().child(rich(foot.to_string(), spans).font(sans(12.0, 400.0, 1.5)).color(t.muted).wrap())
        }
    }
}
