//! `WorkflowInspector.vue`: the designer's inspector, 360 px on the right of the canvas. With no step open it is the workflow's
//! own settings; with one open, the step's head (kind, id tag, Title, Step id) and, for a You decide step, its Question and
//! Choices, then Remove. An agent step's form is `inspector_agent` (piece Q). State: `fleet::workflows::inspector`.
use crate::State;
use crate::fleet::workflows::draft::{END, WorkflowDraft};
use crate::fleet::workflows::editor::Editor;
use crate::fleet::workflows::inspector::{ASK_FIELD, DESCRIPTION_FIELD, HINT_FIELD, ID_FIELD, Inspector, NAME_FIELD, TITLE_FIELD};
use crate::fleet::workflows::{Step, StepKind};
use crate::icons::{BOT, CHECK, CHEVRON_DOWN, PLUS, TRASH_2, USER_ROUND, WORKFLOW};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::controls::grow;
use crate::ui::select;
use crate::ui::text_menu::text_input;
use rune::{Cursor, El, Icon, Key, KeyEvent, ScrollSpec, Tx, col, el, icon, row, text};
use std::rc::Rc;

pub const WIDTH: f32 = 360.0;

/// `--wf-raise`: the fill of every field.
pub(super) fn raise(t: &Theme) -> rune::Color {
    t.mix(t.text, t.card_bg, 0.03)
}

/// `--wf-border-strong`.
pub(super) fn strong(t: &Theme) -> rune::Color {
    t.text.alpha(0.16)
}

pub fn view(s: &State, t: &Theme, ed: &Editor) -> Option<El> {
    let draft = ed.draft.as_ref()?;
    let step = ed.canvas.selected.and_then(|i| draft.steps.get(i));
    let body = match step {
        None => workflow(t, ed, draft),
        Some(step) => step_form(s, t, ed, draft, step, ed.canvas.selected.unwrap_or_default()),
    };
    Some(
        col()
            .key("workflow-inspector")
            .role("complementary")
            .label("Inspector")
            .w(WIDTH)
            .h_full()
            .min_h(0.0)
            .gap(14.0)
            .pt(14.0)
            .pr(16.0)
            .pb(24.0)
            .pl(16.0)
            .border_sides([0.0, 0.0, 0.0, 1.0], t.border)
            .border_layout()
            .children(body)
            .scroll(ScrollSpec { stick_bottom: false }),
    )
}

/// The overlay of the open "Leads to" list.
pub fn overlay(s: &State, t: &Theme, ed: &Editor) -> Vec<El> {
    if ed.inspector.agent.menu.is_some() {
        return super::inspector_agent::overlay(s, t, ed);
    }
    let (Some(draft), Some(i), Some(at)) = (ed.draft.as_ref(), ed.canvas.selected, ed.inspector.menu) else { return Vec::new() };
    let Some(step) = draft.steps.get(i) else { return Vec::new() };
    let Some(choice) = step.choices.get(at) else { return Vec::new() };
    let labels: Vec<(String, String)> = targets(draft, i, step.kind);
    let options: Vec<(&str, &str)> = labels.iter().map(|(v, l)| (v.as_str(), l.as_str())).collect();
    let close: Rc<dyn Fn(&mut State)> = Rc::new(|s: &mut State| s.workflow_inspector_menu(None));
    let choose: select::Choose = Rc::new(move |s: &mut State, to: &str| s.workflows_nav.editor.inspector_set_choice_target(at, to));
    select::content_in(
        t,
        "workflow-choice-to-list",
        &format!("workflow-choice-to-{at}"),
        131.0,
        &options,
        Some(&choice.to),
        choose,
        close,
        0.0,
    )
}

/// Where a choice can lead from step `from`: every step (not itself, for a You decide step), then the end.
pub(super) fn targets(draft: &WorkflowDraft, from: usize, kind: StepKind) -> Vec<(String, String)> {
    let mut all: Vec<(String, String)> = draft
        .steps
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != from || kind == StepKind::Agent)
        .map(|(_, other)| (other.id.clone(), format!("\u{2192} {}", if other.title.is_empty() { &other.id } else { &other.title })))
        .collect();
    all.push((END.to_string(), "\u{2192} End the run".to_string()));
    all
}

// ---- pieces -------------------------------------------------------------------------------------------------------------

/// `h3`: the icon box, the words, and the tag at the right.
fn head(t: &Theme, glyph: &'static Icon, you: bool, words: &str, tag: &str) -> El {
    let (fill, ink) = if you { (t.idle.alpha(0.10), t.idle) } else { (t.wash(8.0), t.muted) };
    row()
        .key("workflow-inspector-head")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(row().size(24.0, 24.0).center().radius(6.0).fill(fill).child(icon(glyph, 14.0).color(ink)))
        .child(text(words.to_string()).font(sans(13.0, 600.0, 1.5)).color(t.text))
        .child(row().grow(1.0).min_w(0.0))
        .child(
            text(tag.to_string()).key("workflow-inspector-tag").font(mono(11.0, 400.0, 1.5)).color(t.muted).truncate().shrink().min_w(0.0),
        )
}

/// `.wf-field label` and `.wf-field__label`: 11 px at 500, 4 px above the field.
pub(super) fn label(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(11.0, 500.0, 1.5)).color(t.muted).mb(4.0)
}

/// `.wf-help`.
pub(super) fn help(t: &Theme, key: &str, words: String, error: bool) -> El {
    let ink = if error { t.error_text } else { t.muted };
    text(words).key(key.to_string()).font(sans(11.0, 400.0, 1.5)).color(ink).wrap().w_full().mt(4.0)
}

/// `.wf-field > input`: padding 6/8, a 1 px border, the button radius, the raised fill, 12.5 px.
pub(super) fn field(t: &Theme, edit: &rune::TextEdit, placeholder: &str, mono_face: bool, bad: bool) -> El {
    let font = if mono_face { mono(12.5, 400.0, 1.4) } else { sans(12.5, 400.0, 1.4) };
    text_input(edit, font)
        .single_line()
        .placeholder(placeholder.to_string(), t.muted)
        .label(placeholder.to_string())
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .px(8.0)
        .py(6.0)
        .radius(RADIUS_BTN)
        .fill(raise(t))
        .border(1.0, if bad { t.error } else { t.border })
        .border_layout()
        .focus_ring(2.0, 0.0, t.accent)
}

/// `.wf-mini`: padding 4/8, radius 6, 12 px.
pub(super) fn mini(t: &Theme, edit: &rune::TextEdit, name: &str) -> El {
    mini_in(t, edit, name, sans(12.0, 400.0, 1.4))
}

/// `.wf-mini.wf-mono`: 11.5 px in the mono face.
pub(super) fn mono_mini(t: &Theme, edit: &rune::TextEdit, name: &str) -> El {
    mini_in(t, edit, name, mono(11.5, 400.0, 1.4))
}

fn mini_in(t: &Theme, edit: &rune::TextEdit, name: &str, font: rune::TextStyle) -> El {
    text_input(edit, font)
        .single_line()
        .label(name.to_string())
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .px(8.0)
        .py(4.0)
        .radius(6.0)
        .fill(raise(t))
        .border(1.0, t.border)
        .border_layout()
        .focus_ring(2.0, 0.0, t.accent)
}

pub(super) fn labelled(t: &Theme, name: &str, input: El) -> El {
    col().w_full().child(label(t, name)).child(input)
}

/// `.wf-radio__option`: the dot, the words and the line under them; the one that is on is ringed in the accent, the ones that
/// arrive later are dimmed.
pub(super) fn option(t: &Theme, key: &str, on: bool, words: &str, detail: &str) -> El {
    let dot = if on {
        el().size(12.0, 12.0).radius(999.0).border(4.0, t.accent).border_layout()
    } else {
        el().size(12.0, 12.0).radius(999.0).border(1.5, strong(t)).border_layout()
    };
    let mut o = row()
        .key(key.to_string())
        .w_full()
        .gap(8.0)
        .items_start()
        .px(8.0)
        .py(6.0)
        .radius(RADIUS_BTN)
        .border(1.0, if on { t.accent } else { t.border })
        .border_layout()
        .child(dot.mt(2.0))
        .child(
            col()
                .shrink()
                .min_w(0.0)
                .child(text(words.to_string()).font(sans(12.0, 400.0, 1.5)).color(t.text).wrap())
                .children((!detail.is_empty()).then(|| text(detail.to_string()).font(sans(11.0, 400.0, 1.5)).color(t.muted).wrap())),
        );
    if on {
        o = o.fill(t.accent_dim);
    } else {
        o = o.opacity(0.55);
    }
    o
}

pub(super) fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

// ---- the workflow's settings --------------------------------------------------------------------------------------------

fn workflow(t: &Theme, ed: &Editor, draft: &WorkflowDraft) -> Vec<El> {
    let insp = &ed.inspector;
    let name = field(t, &insp.name, "", false, false)
        .key(NAME_FIELD)
        .label("Name")
        .on_edit(|s: &mut State| &mut s.workflows_nav.editor.inspector.name);
    let description = grow(
        text_input(&insp.description, sans(12.5, 400.0, 1.55))
            .key(DESCRIPTION_FIELD)
            .on_edit(|s: &mut State| &mut s.workflows_nav.editor.inspector.description)
            .placeholder("What it does, in a sentence or two. Shown in the Library.".to_string(), t.muted)
            .label("Description")
            .color(t.text)
            .caret(t.text, t.accent.alpha(0.35))
            .w_full()
            .px(8.0)
            .py(6.0)
            .radius(RADIUS_BTN)
            .fill(raise(t))
            .border(1.0, t.border)
            .border_layout()
            .min_h(56.0)
            .focus_ring(2.0, 0.0, t.accent),
        8,
    );
    let hint = field(t, &insp.hint, "e.g. What should it fix?", false, false)
        .key(HINT_FIELD)
        .label("Run box hint")
        .on_edit(|s: &mut State| &mut s.workflows_nav.editor.inspector.hint);
    let n = draft.steps.len();
    let steps = row()
        .key("workflow-steps-count")
        .w_full()
        .gap(6.0)
        .center_y()
        .px(8.0)
        .py(5.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .child(text(format!("{n} step{}", plural(n))).font(sans(12.0, 400.0, 1.5)).color(t.text))
        .child(row().grow(1.0))
        .child(text("click one to edit it, or add one with +").font(sans(12.0, 400.0, 1.5)).color(t.muted));
    let radios = |key: &str, on: (&str, &str), later: (&str, &str)| {
        col().key(key.to_string()).w_full().gap(4.0).child(option(t, &format!("{key}-on"), true, on.0, on.1)).child(option(
            t,
            &format!("{key}-later"),
            false,
            later.0,
            later.1,
        ))
    };
    vec![
        head(t, &WORKFLOW, false, "Workflow", &file_name(ed)),
        col().w_full().child(label(t, "Name")).child(name).child(help(
            t,
            "workflow-name-help",
            "Shown in the Library and on the run. The file name stays as it was created.".into(),
            false,
        )),
        labelled(t, "Description", description),
        col().w_full().child(label(t, "Run box hint")).child(hint).child(help(
            t,
            "workflow-hint-help",
            "The placeholder in the Run box, where the request is typed.".into(),
            false,
        )),
        labelled(
            t,
            "Starts from",
            radios(
                "workflow-starts",
                ("A sentence you type", "It becomes {{request}}."),
                ("A GitHub issue or pull request", "Arrives with Stage 2"),
            ),
        ),
        labelled(
            t,
            "Runs in",
            radios(
                "workflow-runs-in",
                ("A new worktree", "From the base branch picked in the Run box. Every step works in it."),
                ("The pull request's branch", "Arrives with Stage 2"),
            ),
        ),
        labelled(t, "Steps", steps),
    ]
}

fn file_name(ed: &Editor) -> String {
    crate::fleet::workflows::helpers::file_name(&ed.file_name()).to_string()
}

// ---- a step -------------------------------------------------------------------------------------------------------------

fn step_form(s: &State, t: &Theme, ed: &Editor, draft: &WorkflowDraft, step: &Step, index: usize) -> Vec<El> {
    let insp = &ed.inspector;
    let you = step.kind == StepKind::You;
    let kind = if you { "You decide" } else { "Agent step" };
    let title = field(t, &insp.title, "", false, false)
        .key(TITLE_FIELD)
        .label("Title")
        .on_edit(|s: &mut State| &mut s.workflows_nav.editor.inspector.title);
    let id = field(t, &insp.id, "", true, insp.id_error.is_some())
        .key(ID_FIELD)
        .label("Step id")
        .on_edit(|s: &mut State| &mut s.workflows_nav.editor.inspector.id)
        .on_input_key(|s: &mut State, ev: &KeyEvent, _: &mut Tx| {
            if ev.key == Key::Enter {
                s.workflow_inspector_commit_id();
                return true;
            }
            false
        });
    let id_help = match &insp.id_error {
        Some(why) => help(t, "workflow-step-id-error", why.clone(), true).role("alert"),
        None => help(
            t,
            "workflow-step-id-help",
            format!(
                "Lowercase letters, digits and dashes. Changing it changes every loop and choice that leads here, and {{{{steps.{}.summary}}}} and .files in the prompts.",
                step.id
            ),
            false,
        ),
    };
    let mut out = vec![
        head(t, if you { &USER_ROUND } else { &BOT }, you, kind, &format!("id: {}", step.id)),
        col().w_full().child(label(t, "Title")).child(title).child(help(
            t,
            "workflow-step-title-help",
            "Loops and choices point at the id, so renaming is safe.".into(),
            false,
        )),
        col().w_full().child(label(t, "Step id")).child(id).child(id_help),
    ];
    if you {
        out.extend(you_form(t, insp, draft, step, index));
    } else {
        out.extend(super::inspector_agent::form(s, t, ed, draft, step, index));
    }
    out.push(remove(t, step));
    out
}

fn you_form(t: &Theme, insp: &Inspector, draft: &WorkflowDraft, step: &Step, index: usize) -> Vec<El> {
    let ask = field(t, &insp.ask, "", false, false)
        .key(ASK_FIELD)
        .label("Question")
        .on_edit(|s: &mut State| &mut s.workflows_nav.editor.inspector.ask);
    let mut choices = col().key("workflow-choices").w_full().gap(4.0);
    for (at, choice) in step.choices.iter().enumerate() {
        let Some(edit) = insp.labels.get(at) else { continue };
        let name = mini(t, edit, "Choice")
            .key(format!("workflow-choice-label-{at}"))
            .w(80.0)
            .grow(1.0)
            .min_w(0.0)
            .on_edit(move |s: &mut State| &mut s.workflows_nav.editor.inspector.labels[at]);
        let leads = select_box(t, &choice_target_label(draft, index, &choice.to), insp.menu == Some(at))
            .key(format!("workflow-choice-to-{at}"))
            .w(131.0)
            .label("Leads to")
            .on_press(move |s: &mut State| {
                let open = s.workflows_nav.editor.inspector.menu == Some(at);
                s.workflow_inspector_menu(if open { None } else { Some(at) });
            });
        let note = check_box(t, choice.note)
            .key(format!("workflow-choice-note-{at}"))
            .on_press(move |s: &mut State| s.workflows_nav.editor.inspector_toggle_choice_note(at));
        let gone = row()
            .key(format!("workflow-choice-remove-{at}"))
            .role("button")
            .label("Remove the choice")
            .size(24.0, 24.0)
            .center()
            .radius(6.0)
            .cursor(Cursor::Pointer)
            .hover(t.wash(5.0))
            .focus_ring(2.0, 0.0, t.accent)
            .child(icon(&TRASH_2, 12.0).color(t.muted))
            .on_press(move |s: &mut State| s.workflows_nav.editor.inspector_remove_choice(at));
        choices = choices.child(
            row().key(format!("workflow-choice-{at}")).w_full().gap(6.0).center_y().child(name).child(leads).child(note).child(gone),
        );
    }
    let add = row()
        .key("workflow-choice-add")
        .role("button")
        .gap(4.0)
        .center_y()
        .px(4.0)
        .py(2.0)
        .mt(6.0)
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 0.0, t.accent)
        .child(icon(&PLUS, 12.0).color(t.accent_text))
        .child(text("Add a choice").font(sans(12.0, 400.0, 1.5)).color(t.accent_text))
        .on_press(|s: &mut State| s.workflows_nav.editor.inspector_add_choice());
    vec![
        labelled(t, "Question", ask),
        col().w_full().child(label(t, "Choices")).child(choices).child(row().child(add)).child(help(
            t,
            "workflow-choices-help",
            "A choice with a note asks you what should change, and the step it leads to gets your note. A forward choice with two or more agent steps after it also offers \"check with me after each step\".".into(),
            false,
        )),
    ]
}

/// The words in a "Leads to" box: where the choice goes, or the id when no step has it any more.
fn choice_target_label(draft: &WorkflowDraft, from: usize, to: &str) -> String {
    let kind = draft.steps.get(from).map_or(StepKind::You, |s| s.kind);
    targets(draft, from, kind).into_iter().find(|(value, _)| value == to).map_or_else(|| to.to_string(), |(_, words)| words)
}

/// `.wf-mini--select`: a box with the words and a chevron.
pub(super) fn select_box(t: &Theme, words: &str, open: bool) -> El {
    row()
        .role("combobox")
        .gap(4.0)
        .center_y()
        .px(8.0)
        .py(4.0)
        .radius(6.0)
        .fill(raise(t))
        .border(1.0, if open { t.accent } else { t.border })
        .border_layout()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 0.0, t.accent)
        .child(text(words.to_string()).font(sans(12.0, 400.0, 1.4)).color(t.text).truncate().grow(1.0).shrink().min_w(0.0))
        .child(icon(&CHEVRON_DOWN, 12.0).color(t.muted))
}

/// `.wf-check`: a checkbox with "note".
pub(super) fn check_box(t: &Theme, on: bool) -> El {
    let boxed = row()
        .size(13.0, 13.0)
        .center()
        .radius(3.0)
        .border(1.0, if on { t.accent } else { strong(t) })
        .border_layout()
        .fill(if on { t.accent } else { rune::Color::TRANSPARENT })
        .child(icon(&CHECK, 10.0).color(t.primary_fg).opacity(if on { 1.0 } else { 0.0 }));
    row()
        .role("checkbox")
        .label("note")
        .checked(on)
        .gap(4.0)
        .center_y()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 0.0, t.accent)
        .child(boxed)
        .child(text("note").font(sans(11.5, 400.0, 1.5)).color(t.muted))
}

/// `.wf-remove`: "Remove <title>" in the error colour, with its own width.
fn remove(t: &Theme, step: &Step) -> El {
    let words = format!("Remove {}", if step.title.is_empty() { "this step" } else { step.title.as_str() });
    row().w_full().child(
        row()
            .key("workflow-remove-step")
            .role("button")
            .label(words.clone())
            .gap(6.0)
            .center_y()
            .px(8.0)
            .py(4.0)
            .radius(RADIUS_BTN)
            .border(1.0, t.border)
            .border_layout()
            .cursor(Cursor::Pointer)
            .hover(t.wash(5.0))
            .focus_ring(2.0, 0.0, t.accent)
            .child(icon(&TRASH_2, 12.0).color(t.error_text))
            .child(text(words).font(sans(12.0, 400.0, 1.5)).color(t.error_text))
            .on_press(|s: &mut State| s.workflows_nav.editor.inspector_remove_step()),
    )
}
