//! `WorkflowCreateDialog.vue`: New workflow, or Duplicate a built-in. A title and a line of what Fleet does, for New the two tabs
//! (Start blank, Describe it), a Name, the File it will make, the words for no repository or for a refusal, Cancel and
//! "Create and open" / "Duplicate and open". The Describe it tab is drawn and picked; its form is piece S's. State and the
//! request: `fleet::workflows::create`.
use crate::State;
use crate::fleet::workflows::create::{Create, Mode, NAME_FIELD};
use crate::icons::SPARKLES;
use crate::theme::{Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, button_lead, disabled};
use crate::ui::confirm::{error_note, spinner};
use crate::ui::controls::input;
use crate::ui::dialog::{Dialog, dialog};
use crate::ui::rem;
use rune::{Cursor, El, Key, KeyEvent, Span, Tx, col, icon, rich, row, text};

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
        // Piece S: the question, the harness and Draft it.
        Mode::Describe if c.source.is_none() => vec![cancel(t, c)],
        _ => {
            body.push(form(s, t, c));
            vec![cancel(t, c), submit(s, t, c)]
        }
    };
    dialog(
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
    )
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
        b.on_press(move |s: &mut State| s.workflow_create_set_mode(next))
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
