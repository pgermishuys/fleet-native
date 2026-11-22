//! `WorkflowEditor.vue`: the frame around a repository's workflow file, shown in the detail's place while the workflow is open
//! (`WorkflowDetailPanel`'s `showEditor`). The header with its Designer | File segment, Try it and Save; the banners; the
//! problems strip; the toast; and the two alert dialogs (remove comments, changed on disk). The body is a stand-in until
//! pieces N (the File view) and O (the canvas). State and requests: `fleet::workflows::{editor, frame}`.
use crate::State;
use crate::fleet::workflows::editor::{Editor, EditorView, Source};
use crate::fleet::workflows::helpers::file_name;
use crate::icons::{CIRCLE_ALERT, CIRCLE_CHECK, CODE_XML, PLAY, SAVE, TRIANGLE_ALERT, WORKFLOW};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, button_lead, disabled};
use crate::ui::dialog::{Dialog, dialog};
use crate::ui::{confirm, rem};
use rune::{Color, Cursor, El, ScrollSpec, Span, col, el, icon, rich, row, text};

/// The whole editor, to fill the detail sheet.
pub fn view(s: &State, t: &Theme) -> El {
    let ed = &s.workflows_nav.editor;
    let mut c =
        col().key("workflow-editor").label("Workflow designer").grow(1.0).min_w(0.0).min_h(0.0).w_full().h_full().child(header(s, t, ed));
    if !ed.comments.is_empty() && ed.view == EditorView::Designer {
        c = c.child(comments_banner(t, ed));
    }
    if let Some(why) = &ed.designer_blocked {
        c = c.child(banner(t, "workflow-designer-blocked", &CIRCLE_ALERT, t.error, t.error.alpha(0.10), "alert", vec![why.clone()], None));
    }
    c = c.child(body(s, t, ed));
    if let Some((words, _)) = &s.workflows_nav.frame.notice {
        c = c.child(toast(t, words));
    }
    c
}

/// The two alert dialogs, over the window while they are asked.
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let ed = &s.workflows_nav.editor;
    if !s.workflow_editor_shown() {
        return Vec::new();
    }
    if ed.canvas.add_at.is_some() && ed.view == EditorView::Designer {
        return super::canvas::overlay(s, t);
    }
    if ed.asking_to_remove_comments {
        return comments_dialog(t, ed);
    }
    if ed.conflict.is_some() {
        return conflict_dialog(t, ed);
    }
    Vec::new()
}

fn short_name(ed: &Editor) -> String {
    file_name(&ed.file_name()).to_string()
}

fn header(s: &State, t: &Theme, ed: &Editor) -> El {
    let title = ed.draft.as_ref().map(|d| d.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| short_name(ed));
    let mark = row().size(24.0, 24.0).center().radius(6.0).fill(t.wash(8.0)).child(icon(&WORKFLOW, 14.0).color(t.muted));
    let mut head = row()
        .key("workflow-editor-header")
        .w_full()
        .center_y()
        .gap(10.0)
        .pt(12.0)
        .pr(16.0)
        .pb(12.0)
        .pl(20.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .border_layout()
        .child(mark)
        .child(text(title).key("workflow-editor-title").font(sans(14.0, 600.0, 1.4)).color(t.text))
        .child(
            text(ed.file_name())
                .key("workflow-file-name")
                .font(mono(12.0, 400.0, 1.5))
                .color(t.muted)
                .truncate()
                .shrink()
                .min_w(0.0)
                .max_w(280.0),
        );
    if ed.is_dirty() {
        head = head.child(
            row()
                .key("workflow-unsaved")
                .gap(5.0)
                .center_y()
                .child(el().size(6.0, 6.0).radius(999.0).fill(t.idle))
                .child(text("Unsaved").font(sans(11.5, 400.0, 1.4)).color(t.idle_text)),
        );
    }
    let try_it = button(t, Variant::Outline, Size::Sm, Some(&PLAY), "Try it")
        .key("workflow-try")
        .on_press_tx(|s: &mut State, tx: &mut crate::Tx| s.workflow_try_it(tx));
    let lead = if ed.is_saving { Some(confirm::spinner(t.primary_fg)) } else { Some(icon(&SAVE, 14.0).color(t.primary_fg)) };
    let save = button_lead(t, Variant::Default, Size::Sm, lead, "Save").key("workflow-save");
    let save = if ed.can_save() { save.on_press(|s: &mut State| s.workflow_save_pressed()) } else { disabled(save) };
    let _ = s;
    head.child(row().grow(1.0)).child(row().gap(8.0).center_y().child(segment(t, ed)).child(try_it).child(save))
}

/// `.wf-seg`.
fn segment(t: &Theme, ed: &Editor) -> El {
    let one = |key: &str, glyph: &'static rune::Icon, label: &str, view: EditorView| {
        let on = ed.view == view;
        let mut b = row()
            .key(key.to_string())
            .role("button")
            .label(label.to_string())
            .checked(on)
            .gap(5.0)
            .center_y()
            .px(10.0)
            .py(3.0)
            .radius(6.0)
            .cursor(Cursor::Pointer)
            .focus_ring(2.0, 0.0, t.accent)
            .child(icon(glyph, 12.0).color(if on { t.text } else { t.muted }))
            .child(text(label.to_string()).font(sans(12.0, 400.0, 1.5)).color(if on { t.text } else { t.muted }))
            .on_press(move |s: &mut State| s.workflow_editor_set_view(view));
        if on {
            b = b.fill(t.panel_bg);
        }
        b
    };
    row()
        .key("workflow-view")
        .role("group")
        .label("View")
        .padding(2.0)
        .radius(RADIUS_BTN)
        .fill(t.wash(8.0))
        .child(one("workflow-view-designer", &WORKFLOW, "Designer", EditorView::Designer))
        .child(one("workflow-view-file", &CODE_XML, "File", EditorView::File))
}

/// `.wf-editor__banner`: padding 10/16, 12.5 px, a rule under it, the icon 14 px at 2 px down.
#[allow(clippy::too_many_arguments)]
fn banner(
    t: &Theme,
    key: &str,
    glyph: &'static rune::Icon,
    ink: Color,
    fill: Color,
    role: &'static str,
    words: Vec<String>,
    action: Option<El>,
) -> El {
    let font = sans(12.5, 400.0, 1.5);
    let mut b = row()
        .key(key.to_string())
        .role(role)
        .w_full()
        .gap(10.0)
        .items_start()
        .px(16.0)
        .py(10.0)
        .fill(fill)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .border_layout()
        .child(icon(glyph, 14.0).color(ink).mt(2.0))
        .child(text(words.concat()).font(font).color(t.text).wrap().grow(1.0).shrink().min_w(0.0));
    if let Some(a) = action {
        b = b.child(a);
    }
    b
}

/// "**This file has N comments.** Saving from the designer…" and the button that goes to the text with them.
fn comments_banner(t: &Theme, ed: &Editor) -> El {
    let n = ed.comments.len();
    let lead = format!("This file has {n} comment{}.", plural(n));
    let words = format!(
        "{lead} Saving from the designer writes the file in Fleet's layout, which removes them. Edit in the File view to keep them."
    );
    let font = sans(12.5, 400.0, 1.5);
    let said = rich(words, vec![Span::new(0, lead.len()).weight(700.0)]).font(font).color(t.text).wrap().grow(1.0).shrink().min_w(0.0);
    let go = button(t, Variant::Outline, Size::Sm, None, "Edit in File view")
        .key("workflow-comments-file-view")
        .on_press(|s: &mut State| s.workflow_editor_keep_comments());
    row()
        .key("workflow-comments-banner")
        .role("note")
        .w_full()
        .gap(10.0)
        .items_start()
        .px(16.0)
        .py(10.0)
        .fill(t.idle.alpha(0.10))
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .border_layout()
        .child(icon(&TRIANGLE_ALERT, 14.0).color(t.idle).mt(2.0))
        .child(said)
        .child(go)
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// The body above the problems strip: the File view's field, or the designer's workflow (piece O puts its canvas here).
fn body(s: &State, t: &Theme, ed: &Editor) -> El {
    let main = if ed.view == EditorView::File { file_field(s, t) } else { super::canvas::view(s, t, ed) };
    let main = col().grow(1.0).min_h(0.0).w_full().child(main).child(problems(s, t, ed));
    row().w_full().grow(1.0).min_h(0.0).child(main.min_w(0.0))
}

/// `WorkflowFileEditor`: the file as it is, in component 11's code editor over a buffer of its own (`fleet::workflows::file_field`).
/// No wrapping, as CodeMirror's has none; the lines with a parser error are tinted (`.wf-file__error-line`).
fn file_field(s: &State, t: &Theme) -> El {
    let panel = |inner: El| {
        // `.cm-scroller`'s 10 px above and below the lines (outside the scroll here).
        col().key("workflow-file-editor").label("Workflow file").w_full().grow(1.0).min_h(0.0).py(10.0).fill(t.panel_bg).child(inner)
    };
    match s.workflow_field_target() {
        Some((m, session, path)) => panel(crate::view::code_view::code_view(s, t, &m, &session, &path, false)),
        None => panel(el()),
    }
}

/// `.wf-problems`: padding 8/16, 12 px, at most 120 high; each error "line N · message", or the OK line; the runs note while
/// dirty; the save error.
fn problems(s: &State, t: &Theme, ed: &Editor) -> El {
    let font = sans(12.0, 400.0, 1.5);
    let mut c =
        col().key("workflow-problems").w_full().gap(4.0).px(16.0).py(8.0).border_sides([1.0, 0.0, 0.0, 0.0], t.border).border_layout();
    let line = |key: &str, ink: Color, glyph: &'static rune::Icon, glyph_ink: Color, words: String| {
        row()
            .key(key.to_string())
            .gap(8.0)
            .center_y()
            .child(icon(glyph, 12.0).color(glyph_ink))
            .child(text(words).font(font).color(ink).wrap().shrink().min_w(0.0))
    };
    if !ed.errors().is_empty() {
        for e in ed.errors() {
            let at = if e.line > 0 { format!("line {} \u{b7} ", e.line) } else { String::new() };
            c = c.child(line("workflow-problem", t.text, &CIRCLE_ALERT, t.error, format!("{at}{}", e.message)));
        }
    } else if ed.check.is_some() {
        let words = if ed.view == EditorView::File {
            if !ed.comments.is_empty() && ed.source != Some(Source::Draft) {
                "No problems. The File view saves your text as it is, comments included."
            } else {
                "No problems. This is exactly what Save writes."
            }
        } else {
            "No problems. Checked by the same parser runs use."
        };
        c = c.child(line("workflow-problems-ok", t.muted, &CIRCLE_CHECK, t.running, words.into()));
    }
    if let Some(note) = s.workflow_runs_note().filter(|_| ed.is_dirty()) {
        c = c.child(text(note).key("workflow-runs-note").font(font).color(t.muted).wrap());
    }
    if let Some(why) = &ed.save_error {
        c = c.child(line("workflow-save-error", t.text, &CIRCLE_ALERT, t.error, why.clone()).role("alert"));
    }
    c.max_h(120.0).scroll(ScrollSpec { stick_bottom: false })
}

/// `.wf-editor__toast`: bottom 56, centred, the text colour as its fill.
fn toast(t: &Theme, words: &str) -> El {
    let pill = row()
        .key("workflow-notice")
        .role("status")
        .gap(8.0)
        .center_y()
        .px(14.0)
        .py(8.0)
        .max_w(rem(40.0))
        .radius(RADIUS_BTN)
        .fill(t.text)
        .child(icon(&CIRCLE_CHECK, 14.0).color(t.panel_bg))
        .child(text(words.to_string()).font(sans(12.5, 400.0, 1.5)).color(t.panel_bg).wrap().shrink().min_w(0.0));
    row().pin(Some(24.0), None, Some(24.0), Some(56.0)).justify_center().child(pill)
}

/// The three footer buttons of both dialogs: Cancel (ghost), then the outline and the default.
fn footer(
    t: &Theme,
    key: &str,
    cancel: fn(&mut State),
    outline: (&str, &str, fn(&mut State)),
    primary: (&str, &str, fn(&mut State)),
) -> Vec<El> {
    vec![
        button(t, Variant::Ghost, Size::Default, None, "Cancel").key(format!("{key}-cancel")).on_press(cancel),
        button(t, Variant::Outline, Size::Default, None, outline.0).key(outline.1.to_string()).on_press(outline.2),
        button(t, Variant::Default, Size::Default, None, primary.0).key(primary.1.to_string()).on_press(primary.2),
    ]
}

/// Escape does nothing here: Fleet's dialogs are driven by `:open` alone and ignore the close request.
fn stay(_: &mut State) {}

/// "Save and remove N comments?": the comments it would remove, and the way to keep them.
fn comments_dialog(t: &Theme, ed: &Editor) -> Vec<El> {
    let n = ed.comments.len();
    let list = col().key("workflow-comments-list").w_full().gap(0.0).pl(18.0).children(
        ed.comments.iter().map(|c| text(format!("# {}", c.text)).font(mono(12.0, 400.0, 1.5)).color(t.muted).truncate().w_full()),
    );
    let note = text("To keep them, make this change in the File view instead.").font(sans(13.0, 400.0, 1.5)).color(t.muted).wrap().w_full();
    dialog(
        t,
        Dialog {
            key: "workflow-comments-confirm",
            title: format!("Save and remove {n} comment{}?", plural(n)),
            description: Some("The designer writes the whole file in Fleet's layout. These comments would be removed:".into()),
            body: vec![list, note],
            footer: footer(
                t,
                "workflow-comments-confirm",
                |s| s.workflows_nav.editor.asking_to_remove_comments = false,
                ("Edit in File view", "workflow-comments-confirm-file", |s| s.workflow_editor_keep_comments()),
                ("Save and remove them", "workflow-comments-confirm-save", |s| s.workflow_editor_confirm_remove_comments()),
            ),
            dismiss: Some(stay),
            ..Default::default()
        },
    )
}

/// "<file> changed on disk": Reload takes theirs, Keep mine saves over it.
fn conflict_dialog(t: &Theme, ed: &Editor) -> Vec<El> {
    dialog(
        t,
        Dialog {
            key: "workflow-conflict",
            title: format!("{} changed on disk", short_name(ed)),
            description: Some(
                "Someone or something changed the file since you opened it, for example an edit in another editor or a git checkout. Reload to see their version and lose your edits, or keep yours and save over theirs."
                    .into(),
            ),
            footer: footer(
                t,
                "workflow-conflict",
                |s| s.workflows_nav.editor.conflict = None,
                ("Reload", "workflow-conflict-reload", |s| s.workflow_editor_reload()),
                ("Keep mine", "workflow-conflict-keep", |s| s.workflow_editor_keep_mine()),
            ),
            dismiss: Some(stay),
            ..Default::default()
        },
    )
}
