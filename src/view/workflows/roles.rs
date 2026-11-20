//! `WorkflowModelRoles.vue`: the Model roles dialog (opened from the Library's "Model roles"), and the Models popover of the Run
//! box (`WorkflowRunBox.vue`) that shares its model trigger. Three roles, each with a model menu and an effort menu, and the
//! steps that use it. The menus are the composer's selector menu (`view::selector`) pointed at a role; the state is
//! `fleet::workflows::roles`.
use crate::composer::selector as rules;
use crate::fleet::workflows::roles::{EFFORT_MENU, MODEL_MENU, MODELS_CHIP, Place, RoleMenu};
use crate::fleet::workflows::{ROLES, Role};
use crate::icons::{CHECK, CHEVRON_DOWN};
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::ui::chip::{pop, pop_label, pop_note};
use crate::ui::dialog::{Dialog, dialog};
use crate::ui::rem;
use crate::{State, Tx};
use rune::{Align, Cursor, El, Side, col, el, icon, row, text};

/// `sm:max-w-2xl`.
const DIALOG_W: f32 = rem(42.0);

/// What the dialog and its menus add to the window, in order: the dialog, then a role menu over it.
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let mut layer = Vec::new();
    if s.workflows_nav.roles_open {
        layer.extend(dialog_view(s, t));
    }
    if let Some(menu) = s.workflows_nav.roles.menu
        && menu.place == Place::Dialog
        && s.workflows_nav.roles_open
    {
        layer.extend(menu_view(s, t, menu));
    }
    layer
}

/// The role menu of the Run box's popover: the popover stays up under it.
pub fn run_box_menu(s: &State, t: &Theme, menu: RoleMenu) -> Vec<El> {
    let mut layer = popover(s, t);
    layer.extend(menu_view(s, t, menu));
    layer
}

fn trigger_key(menu: RoleMenu) -> String {
    match (menu.place, menu.effort) {
        (Place::Dialog, false) => format!("workflows-role-model-{}", menu.role.key()),
        (Place::Dialog, true) => format!("workflows-role-effort-{}", menu.role.key()),
        (Place::RunBox, _) => format!("workflow-model-{}", menu.role.key()),
    }
}

/// The selector menu above (or, with no room, below) its trigger, over a scrim that closes it.
fn menu_view(s: &State, t: &Theme, menu: RoleMenu) -> Vec<El> {
    let name = if menu.effort { EFFORT_MENU } else { MODEL_MENU };
    let anchor = trigger_key(menu);
    let panel = crate::view::selector::panel(s, t, name).focus_return(anchor.clone()).modal(true, |s: &mut State| s.close_selector());
    // In the dialog the menu slides left so it stays inside the dialog, with 8 px to spare (`shiftLeft`); the dialog is centred.
    let panel = if menu.place == Place::Dialog {
        let (window_w, _) = s.ui.sel.window;
        let clip_right = (window_w + DIALOG_W.min(window_w)) / 2.0;
        panel.offset(-rules::shift_left(s.workflows_nav.roles.trigger_x, clip_right), 0.0)
    } else {
        panel
    };
    vec![
        crate::ui::overlay::scrim("workflows-role-menu-scrim", |s: &mut State| s.close_selector()),
        crate::ui::overlay::anchored(panel, &anchor, Side::Top, Align::Start, rules::GAP),
    ]
}

/// `.selector-btn`: 28 high, the label at 80% and a chevron that turns over while its menu is open.
fn trigger(s: &State, t: &Theme, key: String, label: &str, menu: RoleMenu, disabled: bool) -> El {
    let open = s.workflows_nav.roles.menu == Some(menu);
    let b = row()
        .key(key)
        .role("button")
        .label(if menu.effort { "Effort selector" } else { "Model selector" })
        .h(28.0)
        .px(8.0)
        .gap(5.0)
        .center_y()
        .radius(RADIUS_BTN)
        .focus_ring(2.0, 2.0, t.accent)
        .child(text(label.to_string()).font(sans(12.0, 400.0, 1.2)).color(t.text.alpha(0.8)).truncate().shrink().min_w(0.0))
        .child(icon(&CHEVRON_DOWN, 13.0).color(t.muted).rotate(if open { std::f32::consts::PI } else { 0.0 }))
        .on_layout(move |s: &mut State, [x, ..]| {
            if open && (s.workflows_nav.roles.trigger_x - x).abs() > 0.5 {
                s.workflows_nav.roles.trigger_x = x;
            }
        });
    if disabled {
        return b.opacity(0.6).disabled(true);
    }
    let b = if open { b.fill(t.wash(6.0)) } else { b.hover(t.wash(6.0)) };
    b.cursor(Cursor::Pointer).on_press(move |s: &mut State| s.open_role_menu(menu.place, menu.role, menu.effort))
}

// ─── The dialog ────────────────────────────────────────────────────────────────────────────────────────────────

fn dialog_view(s: &State, t: &Theme) -> Vec<El> {
    let harnesses = s.roles_harnesses();
    let all_known = !s.run_box_harnesses().all.is_empty();
    let mut body = Vec::new();
    if harnesses.len() > 1 {
        body.push(segment(s, t, &harnesses));
    }
    if harnesses.is_empty() && all_known {
        body.push(
            text(s.roles_harness_note())
                .key("workflows-roles-note")
                .font(sans(rem(0.875), 400.0, 1.43))
                .color(t.muted)
                .wrap()
                .w_full()
                .mt(14.0),
        );
    } else if !harnesses.is_empty() {
        for role in ROLES {
            body.push(role_block(s, t, role));
        }
        body.push(note(t, "workflows-roles-checked", true, "Checked when a run starts. If a role's model isn't available, for example because its provider signed out, the run doesn't start and names the step."));
        body.push(note(
            t,
            "workflows-roles-plain",
            false,
            "A run can use different models from the Models menu in its Run box without changing these.",
        ));
    }
    let window_h = s.ui.sel.window.1;
    dialog(
        t,
        Dialog {
            key: "workflows-roles",
            title: "Model roles".into(),
            description: Some(
                "Workflow steps ask for a role, not a model, so a workflow file someone shares works on your providers. Pick which of your models does each kind of work. Kept in Fleet for you, not in the repo."
                    .into(),
            ),
            body,
            close_button: true,
            click_outside: true,
            dismiss: Some(|s: &mut State| s.close_roles()),
            width: Some(DIALOG_W),
            max_height: (window_h > 0.0).then_some(window_h * 0.9),
            ..Dialog::default()
        },
    )
}

/// `.wf-seg`: a bordered group; the pressed harness is on a 9% wash and heavier.
fn segment(s: &State, t: &Theme, harnesses: &[(String, String)]) -> El {
    let current = s.roles_harness_type();
    let mut seg = row()
        .key("workflows-roles-harness")
        .role("group")
        .label("Harness")
        .self_start()
        .padding(2.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout();
    for (kind, name) in harnesses {
        let on = *kind == current;
        let pick = kind.clone();
        let mut b = row()
            .key(format!("workflows-roles-harness-{kind}"))
            .role("button")
            .label(name.clone())
            .checked(on)
            .px(10.0)
            .py(3.0)
            .radius(RADIUS_BTN - 2.0)
            .cursor(Cursor::Pointer)
            .focus_ring(2.0, 0.0, t.accent)
            .child(text(name.clone()).font(sans(12.0, if on { 500.0 } else { 400.0 }, 1.5)).color(if on { t.text } else { t.muted }))
            .on_press(move |s: &mut State| s.pick_roles_harness(&pick));
        if on {
            b = b.fill(t.text.alpha(0.09));
        }
        seg = seg.child(b);
    }
    seg
}

/// `.wf-role`: a rule above, the name and what it is for at the left, the two menus at the right, then who uses it.
fn role_block(s: &State, t: &Theme, role: Role) -> El {
    let model_menu = RoleMenu { place: Place::Dialog, role, effort: false };
    let effort_menu = RoleMenu { place: Place::Dialog, role, effort: true };
    let model = trigger(
        s,
        t,
        trigger_key(model_menu),
        &s.role_model_label(Place::Dialog, role),
        model_menu,
        !s.role_models_supported() || s.workflows_nav.roles.saving == Some(role),
    );
    let effort = trigger(s, t, trigger_key(effort_menu), &s.role_effort_label(role), effort_menu, s.role_effort_items(role).len() < 2);
    let head = row()
        .w_full()
        .center_y()
        .justify_between()
        .gap(16.0)
        .child(
            col()
                .shrink()
                .min_w(0.0)
                .child(text(role.name()).font(sans(rem(0.875), 500.0, 1.43)).color(t.text))
                .child(text(role.description()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap()),
        )
        .child(row().center_y().gap(4.0).child(model).child(effort));
    let mut used = row().w_full().flex_wrap().center_y().gap(6.0).child(text("Used by").font(sans(12.0, 400.0, 1.5)).color(t.muted));
    let titles = s.role_used_by(role);
    if titles.is_empty() {
        used = used.child(text("no steps yet").font(sans(12.0, 400.0, 1.5)).color(t.muted));
    }
    for title in titles {
        used = used.child(
            row()
                .px(7.0)
                .py(1.0)
                .radius(999.0)
                .border(1.0, t.border)
                .border_layout()
                .child(text(title).font(sans(11.5, 400.0, 1.5)).color(t.text)),
        );
    }
    col()
        .key(format!("workflows-role-{}", role.key()))
        .w_full()
        .mt(14.0)
        .gap(14.0)
        .child(el().h(1.0).w_full().fill(t.border))
        .child(col().w_full().gap(6.0).child(head).child(used))
}

/// `.wf-ok`: 12 px muted; the first has the check in the running colour, the second lines up with its words.
fn note(t: &Theme, key: &str, check: bool, words: &str) -> El {
    let mut r = row().key(key.to_string()).w_full().items_start().gap(6.0).mt(if check { 16.0 } else { 4.0 });
    if check {
        r = r.child(icon(&CHECK, 14.0).color(t.running).mt(1.0));
    } else {
        r = r.pl(20.0);
    }
    r.child(text(words.to_string()).font(sans(12.0, 400.0, 1.5)).color(t.muted).wrap().shrink().min_w(0.0))
}

// ─── The Run box's Models popover ──────────────────────────────────────────────────────────────────────────────

/// `Models for this run`: a row per role this workflow's enabled steps use, with its model menu (or the fixed model when the
/// harness lists none), and the note that it is only for this run.
pub fn popover(s: &State, t: &Theme) -> Vec<El> {
    let Some(w) = s.workflows_nav.active_workflow() else { return Vec::new() };
    let roles = super::run_box::roles_used(s, w);
    let mut body = vec![pop_label(t, "Models for this run")];
    for role in roles {
        let menu = RoleMenu { place: Place::RunBox, role, effort: false };
        let end = if s.role_models_supported() {
            trigger(s, t, trigger_key(menu), &s.role_model_label(Place::RunBox, role), menu, false)
        } else {
            text(s.role_fixed_label(role)).key(format!("workflow-model-{}", role.key())).font(sans(12.5, 400.0, 1.5)).color(t.muted)
        };
        body.push(
            row()
                .key(format!("workflow-model-row-{}", role.key()))
                .w_full()
                .center_y()
                .justify_between()
                .gap(10.0)
                .px(8.0)
                .py(4.0)
                .child(text(role.name()).font(sans(12.5, 500.0, 1.5)).color(t.text))
                .child(end),
        );
    }
    body.push(pop_note(t, "Only for this run. The roles themselves are set under Model roles, in the Workflows list."));
    let panel =
        pop(t, "workflow-models-pop", 360.0, body).focus_return(MODELS_CHIP.to_string()).modal(true, |s: &mut State| s.ui.open = None);
    vec![
        crate::ui::overlay::scrim("workflow-models-scrim", |s: &mut State| s.ui.open = None),
        crate::ui::overlay::anchored(panel, MODELS_CHIP, Side::Top, Align::Start, 6.0),
    ]
}

/// The chip's press (the strip opens the popover).
pub fn open_popover(s: &mut State, _tx: &mut Tx) {
    s.open_models_popover();
}
