//! The Library list, `[workflows-list]` (`WorkflowsNavPanel.vue`): the header, the machine switch, New workflow and Model
//! roles, the built-ins, and the files of the repository the Run box has picked.
use crate::State;
use crate::fleet::workflows::Workflow;
use crate::fleet::workflows::nav::{MetaTone, latest_run, row_meta};
use crate::icons::{CIRCLE_ALERT, GIT_PULL_REQUEST, MESSAGE_SQUARE_TEXT, PLUS, SLIDERS_HORIZONTAL, TRIANGLE_ALERT};
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::ui::{empty_state, rem};
use crate::view::{files_tab, machine_switch, parts};
use rune::{Cursor, El, Icon, ScrollSpec, col, el, icon, row, text};

/// The switch under the header: "Workflows for machine".
pub const SWITCH: machine_switch::Switch =
    machine_switch::Switch { label: "Workflows", testid: "workflows", choice: |s| &mut s.workflows_nav.choice };

/// The switch's open menu, over the window.
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    machine_switch::overlay(s, t, SWITCH, &s.workflows_nav.choice)
}

pub fn panel(s: &State, t: &Theme) -> El {
    let nav = &s.workflows_nav;
    let header = row()
        .pt(4.0)
        .child(row().px(16.0).pt(14.0).pb(10.0).child(text("WORKFLOWS").font(sans(rem(0.71429), 600.0, 1.5).tracking(0.5)).color(t.muted)));
    let machine = machine_switch::chip(s, t, SWITCH, &nav.choice).map(|chip| row().px(12.0).pb(6.0).child(chip));
    let can_create = nav.library.as_ref().is_some_and(|l| l.repository.is_some());
    let create = dashed(t, "workflow-new", &PLUS, "New workflow", !can_create);
    let create = if can_create {
        create.on_press_tx(|s: &mut State, tx: &mut rune::Tx| s.workflow_create_open(None, tx))
    } else {
        create.help("Pick a repository in the Run box first")
    };
    let roles = dashed(t, "workflow-model-roles", &SLIDERS_HORIZONTAL, "Model roles", false)
        .on_press(|s: &mut State| s.workflows_nav.roles_open = true);
    let body = if nav.library.is_some() {
        library(s, t)
    } else if nav.loading_library {
        col()
            .key("workflows-loading")
            .w_full()
            .grow(1.0)
            .center()
            .px(12.0)
            .py(24.0)
            .label("Loading workflows…")
            .child(files_tab::spinner(t.muted, rem(1.4286)))
    } else if let Some(error) = &nav.library_error {
        error_note(t, error)
    } else {
        el()
    };
    col()
        .key("workflows-list")
        .label("Workflows navigation")
        .w(s.sidebar_w)
        .h_full()
        .min_h(0.0)
        .child(header)
        .children(machine)
        .child(create)
        .child(roles)
        .child(col().w_full().grow(1.0).shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body))
}

/// `.new-workflow`, `.model-roles`: a dashed button, 13 px muted, the accent mixed into the border on hover.
fn dashed(t: &Theme, key: &str, glyph: &'static Icon, label: &str, off: bool) -> El {
    let e = row()
        .key(key.to_string())
        .role("button")
        .label(label.to_string())
        .gap(6.0)
        .center_y()
        .mt(4.0)
        .mx(8.0)
        .mb(8.0)
        .px(10.0)
        .py(6.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_dash(4.0, 3.0)
        .border_layout()
        .cursor(if off { Cursor::Default } else { Cursor::Pointer })
        .child(icon(glyph, 14.0).color(t.muted).group_color(t.text))
        .child(text(label.to_string()).font(sans(rem(0.92857), 400.0, 1.5)).color(t.muted).group_color(t.text));
    // Fleet's hover also mixes the accent into the border; Rune has no hover border (docs/RUNE.md).
    if off { e.opacity(0.6).disabled(true) } else { e.hover_group() }
}

/// `.group-label`: the name and its count, 11 px 500 muted.
fn group_label(t: &Theme, key: &str, name: &str, count: Option<usize>) -> El {
    row()
        .key(key.to_string())
        .w_full()
        .gap(8.0)
        .px(10.0)
        .pt(12.0)
        .pb(6.0)
        .child(text(name.to_string()).font(sans(rem(0.78571), 500.0, 1.5)).color(t.muted).grow(1.0).truncate())
        .children(count.map(|n| text(n.to_string()).font(sans(rem(0.78571), 500.0, 1.5)).color(t.muted)))
}

fn library(s: &State, t: &Theme) -> El {
    let nav = &s.workflows_nav;
    let machine = s.workflows_machine();
    let runs = s.workflow_runs(&machine);
    let loading = nav.is_loading_repository();
    let lib = nav.library.as_ref().expect("a library");
    let repo_label = match (&lib.repository_name, loading) {
        (Some(name), false) => format!("{name} · .weave/workflows"),
        _ => "This repo · .weave/workflows".to_string(),
    };
    let row_of = |w: &Workflow| {
        let (label, tone) = row_meta(w, latest_run(runs, w), nav.unsaved.as_deref() == Some(w.id.as_str()));
        item(s, t, w, &label, tone)
    };
    let built_ins = nav.built_ins();
    let files = nav.repo_workflows();
    let mut list = col().w_full().gap(1.0).px(8.0).pb(12.0).child(group_label(
        t,
        "workflows-group-built-in",
        "Built into Fleet",
        Some(built_ins.len()),
    ));
    list = list.children(built_ins.into_iter().map(row_of));
    list = list.child(group_label(t, "workflows-group-repo", &repo_label, (!loading).then_some(files.len())));
    if loading {
        list = list.child(
            row()
                .key("workflows-repo-loading")
                .label("Loading this repo's workflows…")
                .w_full()
                .center()
                .px(12.0)
                .py(10.0)
                .child(files_tab::spinner(t.muted, 14.0)),
        );
    } else {
        list = list.children(files.iter().map(|w| row_of(w)));
        if files.is_empty() {
            let words = if lib.repository.is_some() {
                "No workflow files in this repo yet"
            } else {
                "Pick a repository in the Run box to see its workflows"
            };
            list = list.child(empty_state::compact_bare(t, "workflows-empty", words));
        }
    }
    list
}

/// `.workflow-row`: 32 high, a glyph, the title and a word at the end.
fn item(s: &State, t: &Theme, w: &Workflow, label: &str, tone: MetaTone) -> El {
    let active = s.workflows_nav.active_id() == w.id;
    let lead: El = if tone == MetaTone::Working {
        row().w(13.0).center().child(parts::status_glyph(t, "active", false))
    } else if !w.built_in && !w.errors.is_empty() {
        icon(&TRIANGLE_ALERT, 13.0).color(t.error)
    } else if w.built_in {
        icon(&MESSAGE_SQUARE_TEXT, 13.0).color(t.muted)
    } else {
        icon(&GIT_PULL_REQUEST, 13.0).color(t.muted)
    };
    let ink = match tone {
        MetaTone::Plain => t.muted.alpha(0.8),
        MetaTone::Working => t.running_text,
        MetaTone::Wait => t.idle_text,
        MetaTone::With => t.accent_text,
        MetaTone::Error => t.error_text,
    };
    let id = w.id.clone();
    let mut e = row()
        .key(format!("workflow-row-{}", w.id))
        .role("button")
        .label(w.name.clone())
        .w_full()
        .min_w(0.0)
        .min_h(32.0)
        .gap(9.0)
        .center_y()
        .px(10.0)
        .radius(RADIUS_BTN)
        .cursor(Cursor::Pointer)
        .hover(t.wash(5.0))
        .focus_ring(2.0, -2.0, t.accent)
        .child(lead)
        .child(
            text(w.name.clone())
                .font(sans(rem(0.92857), if active { 500.0 } else { 400.0 }, 1.3))
                .color(if active { t.text } else { t.text.alpha(0.86) })
                .grow(1.0)
                .shrink()
                .min_w(0.0)
                .truncate(),
        )
        .child(text(label.to_string()).font(sans(rem(0.85714), 400.0, 1.4)).color(ink))
        .on_press(move |s: &mut State| s.set_active_workflow(&id));
    if active {
        e = e.fill(t.wash(9.0));
    }
    e
}

/// `ErrorNote` in the list: margin 12, the server's words.
fn error_note(t: &Theme, message: &str) -> El {
    row()
        .key("workflows-list-error")
        .role("alert")
        .mx(12.0)
        .my(12.0)
        .gap(8.0)
        .items_start()
        .px(12.0)
        .py(8.0)
        .radius(crate::theme::RADIUS_CARD)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.1))
        .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0))
        .child(text(message.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.error_text).wrap().shrink().min_w(0.0))
}
