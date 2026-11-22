//! `WorkflowCreateDialog.vue`: New workflow, or Duplicate a built-in. A title and a line of what Fleet does, for New the two tabs
//! (Start blank, Describe it), a Name, the File it will make, the words for no repository or for a refusal, Cancel and
//! "Create and open" / "Duplicate and open". The Describe it tab's form (piece S): what it should do, the harness that is asked,
//! what it costs, Cancel and Draft it. State and the request: `fleet::workflows::create`.
use crate::State;
use crate::fleet::workflows::create::{Create, DESCRIBE_FIELD, Mode, NAME_FIELD};
use crate::fleet::workflows::draft::DRAFT_COST_NOTE;
use crate::icons::SPARKLES;
use crate::theme::{Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, button_lead, disabled};
use crate::ui::confirm::{error_note, spinner};
use crate::ui::controls::{grow, input, textarea_field};
use crate::ui::dialog::{Dialog, dialog};
use crate::ui::rem;
use crate::ui::select;
use rune::{Cursor, El, Key, KeyEvent, Span, Tx, col, icon, rich, row, text};
use std::rc::Rc;

/// `sm:max-w-md`.
const DIALOG_W: f32 = rem(28.0);

pub const NEEDS_REPOSITORY: &str = "Pick a repository in the Run box first: the file goes in that repository.";
const NEW_BLANK: &str = "Fleet creates the file in this repo's .weave/workflows/ folder with one agent step, so it's valid from the start, and opens it in the designer.";
/// The folder's name, set in mono in the description (`<code>`).
const FOLDER: &str = ".weave/workflows/";
const NEW_DESCRIBE: &str =
    "Say what it should do. The model drafts the steps, and the draft opens in the designer unsaved: nothing is written until you save it.";
const DUPLICATE: &str = "Fleet copies it into this repo's .weave/workflows/ folder and opens the copy in the designer. It's an ordinary file in your checkout: commit it when you're ready to share it.";

/// The dialog and its scrim; empty while it is closed.
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let Some(c) = s.workflows_nav.create.as_ref() else { return Vec::new() };
    let title = c.source.as_ref().map_or("New workflow".to_string(), |src| format!("Duplicate {}", src.name));
    let about = match (&c.source, c.mode) {
        (Some(_), _) => DUPLICATE,
        (None, Mode::Blank) => NEW_BLANK,
        (None, Mode::Describe) => NEW_DESCRIBE,
    };
    let mut body = Vec::new();
    if c.source.is_none() {
        body.push(tabs(t, c.mode));
    }
    let footer = match c.mode {
        Mode::Describe if c.source.is_none() => {
            body.push(describe(s, t, c));
            vec![cancel(t, c), draft_it(s, t)]
        }
        _ => {
            body.push(form(s, t, c));
            vec![cancel(t, c), submit(s, t, c)]
        }
    };
    let mut layer = dialog(
        t,
        Dialog {
            key: "workflow-create",
            title,
            description_el: Some(
                rich(about.to_string(), about.find(FOLDER).map(|at| Span::new(at, at + FOLDER.len()).mono()).into_iter().collect())
                    .font(sans(rem(0.875), 400.0, 1.4))
                    .color(t.muted)
                    .wrap()
                    .w_full(),
            ),
            body,
            footer,
            close_button: true,
            click_outside: true,
            dismiss: Some(|s: &mut State| s.workflow_create_close()),
            width: Some(DIALOG_W),
            ..Dialog::default()
        },
    );
    if c.picking_harness && c.mode == Mode::Describe {
        layer.extend(harness_list(s, t));
    }
    layer
}

/// `.wf-create-modes`: a group on an 8% wash, the picked tab on the panel. It is `inline-flex` in a grid, so it spans the dialog.
fn tabs(t: &Theme, mode: Mode) -> El {
    let tab = |key: &str, label: &str, glyph: bool, on: bool, next: Mode| {
        let mut b = row()
            .key(key.to_string())
            .role("tab")
            .label(label.to_string())
            .checked(on)
            .gap(5.0)
            .center_y()
            .px(10.0)
            .py(3.0)
            .radius(6.0)
            .cursor(Cursor::Pointer)
            .focus_ring(2.0, 0.0, t.accent);
        if glyph {
            b = b.child(icon(&SPARKLES, 12.0).color(if on { t.text } else { t.muted }));
        }
        let b = b.child(text(label.to_string()).font(sans(12.5, 400.0, 1.5)).color(if on { t.text } else { t.muted }));
        let b = if on { b.fill(t.panel_bg) } else { b };
        b.on_press_tx(move |s: &mut State, tx: &mut Tx| s.workflow_create_set_mode(next, tx))
    };
    row()
        .key("workflow-create-modes")
        .role("tablist")
        .label("How to start")
        .w_full()
        .padding(2.0)
        .radius(8.0)
        .fill(t.text.alpha(0.08))
        .child(tab("workflow-create-blank", "Start blank", false, mode == Mode::Blank, Mode::Blank))
        .child(tab("workflow-create-describe", "Describe it", true, mode == Mode::Describe, Mode::Describe))
}

fn label(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(rem(0.875), 500.0, 1.4)).color(t.text)
}

/// The Name field, the File it names, and the words under them.
fn form(s: &State, t: &Theme, c: &Create) -> El {
    let repository = s.workflows_nav.library.as_ref().and_then(|l| l.repository_name.as_deref());
    let mut name = input(t, &c.name, "e.g. Tidy up a flaky test").key(NAME_FIELD);
    name = if c.creating {
        name.opacity(0.5).disabled(true)
    } else {
        name.on_edit(|s: &mut State| &mut s.workflows_nav.create.as_mut().expect("the dialog is open").name).on_input_key(
            |s: &mut State, ev: &KeyEvent, _: &mut Tx| {
                if ev.key == Key::Enter {
                    s.workflow_create_submit();
                    return true;
                }
                s.workflow_create_edited();
                false
            },
        )
    };
    let file = row()
        .key("workflow-create-file")
        .w_full()
        .px(8.0)
        .py(6.0)
        .radius(6.0)
        .border(1.0, t.border)
        .border_layout()
        .child(text(c.file(repository)).font(mono(rem(0.75), 400.0, 1.33)).color(t.muted).truncate().min_w(0.0));
    let mut form = col()
        .w_full()
        .gap(rem(1.0))
        .child(col().w_full().gap(rem(0.5)).child(label(t, "Name")).child(name))
        .child(col().w_full().gap(rem(0.5)).child(label(t, "File")).child(file));
    if s.workflows_nav.repository_path().is_none() {
        form = form.child(
            text(NEEDS_REPOSITORY)
                .key("workflow-create-needs-repository")
                .font(sans(rem(0.875), 400.0, 1.43))
                .color(t.muted)
                .wrap()
                .w_full(),
        );
    }
    if let Some(why) = &c.error {
        form = form.child(error_note(t, "workflow-create-error", why, None));
    }
    form
}

fn cancel(t: &Theme, c: &Create) -> El {
    let b = button(t, Variant::Outline, Size::Default, None, "Cancel").key("workflow-create-cancel");
    if c.creating { disabled(b) } else { b.on_press(|s: &mut State| s.workflow_create_close()) }
}

fn submit(s: &State, t: &Theme, c: &Create) -> El {
    let words = if c.source.is_some() { "Duplicate and open" } else { "Create and open" };
    let b = if c.creating {
        button_lead(t, Variant::Default, Size::Default, Some(spinner(t.primary_fg)), words).busy(true)
    } else {
        button(t, Variant::Default, Size::Default, None, words)
    }
    .key("workflow-create-submit");
    if s.workflow_can_create() { b.on_press(|s: &mut State| s.workflow_create_submit()) } else { disabled(b) }
}

/// `DialogContent`'s padding takes 2 × 1.5rem off its width.
const INNER: f32 = DIALOG_W - 2.0 * rem(1.5);
const NEEDS_HARNESS: &str = "Describe it needs a harness workflows run on: OpenCode or OpenCode 2.";
const HARNESS_TRIGGER: &str = "workflow-describe-harness";

fn small(t: &Theme, key: &str, words: String) -> El {
    text(words).key(key.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().w_full()
}

fn form_mut(s: &mut State) -> &mut Create {
    s.workflows_nav.create.as_mut().expect("the dialog is open")
}

/// `workflow-describe-form`: What should it do? (four rows), Asks (the harness), the model it will use, the cost, and the
/// words for no repository.
fn describe(s: &State, t: &Theme, c: &Create) -> El {
    let ask = grow(
        textarea_field(t, &c.description, "e.g. Triage a bug report: reproduce it, find the cause, and fix it after I approve the plan.")
            .key(DESCRIBE_FIELD)
            .on_edit(|s: &mut State| &mut form_mut(s).description),
        8,
    );
    let list = s.workflow_describe_harnesses();
    let harness = s.workflow_describe_harness();
    let name = harness.as_ref().and_then(|h| list.iter().find(|(kind, _)| kind == h)).map(|(_, n)| n.clone()).or(harness.clone());
    let mut trigger = select::trigger(t, name.as_deref().unwrap_or(""), false, false, INNER).key(HARNESS_TRIGGER);
    trigger = if list.len() < 2 {
        trigger.opacity(0.5).disabled(true)
    } else {
        trigger.on_press(|s: &mut State| form_mut(s).picking_harness = true)
    };
    let note = match &harness {
        Some(h) => format!("On your Standard model: {}.", s.workflow_describe_model(h)),
        None => NEEDS_HARNESS.to_string(),
    };
    let mut form = col()
        .key("workflow-describe-form")
        .w_full()
        .gap(rem(1.0))
        .child(col().w_full().gap(rem(0.5)).child(label(t, "What should it do?")).child(ask))
        .child(col().w_full().gap(rem(0.5)).child(label(t, "Asks")).child(trigger).child(small(t, "workflow-describe-model", note)))
        .child(small(t, "workflow-describe-cost", DRAFT_COST_NOTE.to_string()));
    if s.workflows_nav.repository_path().is_none() {
        form = form.child(
            text(NEEDS_REPOSITORY)
                .key("workflow-create-needs-repository")
                .font(sans(rem(0.875), 400.0, 1.43))
                .color(t.muted)
                .wrap()
                .w_full(),
        );
    }
    form
}

fn harness_list(s: &State, t: &Theme) -> Vec<El> {
    let list = s.workflow_describe_harnesses();
    let options: Vec<(&str, &str)> = list.iter().map(|(kind, name)| (kind.as_str(), name.as_str())).collect();
    let close: Rc<dyn Fn(&mut State)> = Rc::new(|s: &mut State| form_mut(s).picking_harness = false);
    select::content_in(
        t,
        "workflow-describe-harness-list",
        HARNESS_TRIGGER,
        INNER,
        &options,
        s.workflow_describe_harness().as_deref(),
        Rc::new(|s: &mut State, v: &str| form_mut(s).harness_choice = Some(v.to_string())),
        close,
        0.0,
    )
}

/// Draft it, with the Sparkles before it; off until there is something to say, a repository and a harness.
fn draft_it(s: &State, t: &Theme) -> El {
    let b = button(t, Variant::Default, Size::Default, Some(&SPARKLES), "Draft it").key("workflow-describe-submit");
    if s.workflow_can_describe() { b.on_press(|s: &mut State| s.workflow_describe_submit()) } else { disabled(b) }
}
