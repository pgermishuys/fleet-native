//! `NewProjectDialog.vue`: "New project", a name, the machine it goes on (only when there are other machines), a description,
//! an error note, Cancel and "Create project". Drawn on `ui::dialog`; the state is `fleet::new_project`.
use crate::fleet::new_project::{DESCRIPTION_FIELD, NAME_FIELD, NewProject};
use crate::icons::{CIRCLE_ALERT, PLUS};
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::button::{Size, Variant, button, button_lead, disabled};
use crate::ui::chip::{Chip, Option_, option, pop, pop_label, pop_separator};
use crate::ui::confirm::spinner;
use crate::ui::controls::{grow, input, textarea_field};
use crate::ui::dialog::{Dialog, dialog};
use crate::ui::rem;
use crate::{State, Tx};
use rune::{Align, El, Key, KeyEvent, Side, col, el, icon, row, text};

pub const DESCRIPTION: &str = "Create a project to organize related sessions.";
const MACHINE_CHIP: &str = "new-project-machine";
const MACHINE_HINT: &str = "The project and its sessions live on this machine.";

/// `ErrorNote`: the words in a red box.
fn error_note(t: &Theme, message: &str) -> El {
    row()
        .key("new-project-error")
        .role("alert")
        .w_full()
        .gap(rem(0.5))
        .items_start()
        .px(rem(0.75))
        .py(rem(0.5))
        .radius(RADIUS_CARD)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.1))
        .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0))
        .child(col().shrink().min_w(0.0).child(text(message.to_string()).font(sans(rem(0.875), 400.0, 1.4)).color(t.error_text).wrap()))
}

fn label(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(rem(0.875), 500.0, 1.4)).color(t.text)
}

fn machine_name(s: &State, key: &str) -> String {
    s.machines.entries().into_iter().find(|e| e.key == key).map_or_else(|| "This machine".to_string(), |e| e.name)
}

/// The "Create on" chip, 32 high and framed like the fields (`.new-project-machine`).
fn machine_field(s: &State, t: &Theme, d: &NewProject) -> El {
    let reachable = s.machines.is_answering(&d.machine);
    let chip = Chip::new(MACHINE_CHIP, machine_name(s, &d.machine))
        .dot(if reachable { t.running } else { t.error })
        .label_font(sans(13.0, 500.0, 1.2))
        .open(d.picking)
        .disabled(d.creating)
        .view(t, |s: &mut State, _: &mut Tx| s.toggle_project_machine_menu())
        .h(32.0)
        .px(10.0)
        .border(1.0, t.border);
    col()
        .w_full()
        .gap(rem(0.5))
        .child(label(t, "Create on"))
        .child(row().role("group").label("Create on").child(chip))
        .child(text(MACHINE_HINT).font(sans(rem(0.75), 400.0, 1.4)).color(t.muted))
}

/// The menu under the chip: every machine, and Add a machine…. An unreachable one is off.
fn machine_menu(s: &State, t: &Theme, d: &NewProject) -> Vec<El> {
    let mut body = vec![pop_label(t, "Create on")];
    for r in s.machine_rows() {
        let key = r.key.clone();
        let dot = el().size(7.0, 7.0).radius(999.0).fill(if r.reachable { t.running } else { t.error });
        let spec = Option_ {
            detail: Some(r.detail),
            checked: r.key == d.machine,
            disabled: !r.reachable,
            mono_title: true,
            ..Option_::new(format!("{MACHINE_CHIP}-{}", r.key), r.name)
        };
        body.push(option(t, spec, Some(dot), move |s, _| s.pick_project_machine(&key)));
    }
    body.push(pop_separator(t));
    body.push(option(
        t,
        Option_ {
            icon: Some(&PLUS),
            detail: Some("Settings → Machines".into()),
            ..Option_::new("new-project-machine-add", "Add a machine…")
        },
        None,
        |s, tx| s.add_machine_from_project(tx),
    ));
    let panel = pop(t, "new-project-machine-pop", 300.0, body)
        .focus_return(MACHINE_CHIP.to_string())
        .modal(true, |s: &mut State| s.close_new_project());
    vec![
        crate::ui::overlay::scrim("new-project-machine-scrim", |s: &mut State| s.close_new_project()),
        crate::ui::overlay::anchored(panel, MACHINE_CHIP, Side::Bottom, Align::Start, 6.0),
    ]
}

fn body(s: &State, t: &Theme, d: &NewProject) -> Vec<El> {
    let mut name = input(t, &d.name, "Project name").key(NAME_FIELD);
    let mut about = textarea_field(t, &d.description, "Optional description").key(DESCRIPTION_FIELD);
    if d.creating {
        name = name.opacity(0.5).disabled(true);
        about = about.opacity(0.5).disabled(true);
    } else {
        name = name.on_edit(|s: &mut State| &mut s.proj.new.as_mut().expect("the dialog is open").name).on_input_key(
            |s: &mut State, ev: &KeyEvent, tx: &mut Tx| {
                if ev.key == Key::Enter {
                    s.new_project_submit(tx);
                    return true;
                }
                s.new_project_edited();
                false
            },
        );
        about = about.on_edit(|s: &mut State| &mut s.proj.new.as_mut().expect("the dialog is open").description).on_input_key(
            |s: &mut State, _: &KeyEvent, _: &mut Tx| {
                s.new_project_edited();
                false
            },
        );
    }
    let about = grow(about, 8);
    let field = |words: &str, input: El| col().w_full().gap(rem(0.5)).child(label(t, words)).child(input);
    let mut form = col().w_full().gap(rem(1.25)).child(field("Name", name));
    if s.machines.has_machines() {
        form = form.child(machine_field(s, t, d));
    }
    form = form.child(field("Description", about));
    if let Some(why) = &d.error {
        form = form.child(error_note(t, why));
    }
    vec![form]
}

/// The dialog and its scrim, to mount over the window; `None` while it is closed.
pub fn view(s: &State, t: &Theme) -> Option<Vec<El>> {
    let d = s.proj.new.as_ref()?;
    let cancel = button(t, Variant::Outline, Size::Default, None, "Cancel").key("new-project-cancel");
    let cancel = if d.creating { disabled(cancel) } else { cancel.on_press(|s: &mut State| s.close_new_project()) };
    let submit = if d.creating {
        button_lead(t, Variant::Default, Size::Default, Some(spinner(t.primary_fg)), "Creating\u{2026}")
    } else {
        button(t, Variant::Default, Size::Default, None, "Create project")
    }
    .key("new-project-submit");
    let submit = if d.can_submit() { submit.on_press_tx(|s: &mut State, tx: &mut Tx| s.new_project_submit(tx)) } else { disabled(submit) };
    let mut layer = dialog(
        t,
        Dialog {
            key: "new-project",
            title: "New project".into(),
            description: Some(DESCRIPTION.into()),
            body: body(s, t, d),
            footer: vec![cancel, submit],
            close_button: true,
            click_outside: true,
            dismiss: Some(|s: &mut State| s.close_new_project()),
            ..Default::default()
        },
    );
    if d.picking {
        layer.extend(machine_menu(s, t, d));
    }
    Some(layer)
}
