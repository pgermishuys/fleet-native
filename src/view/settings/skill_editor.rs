//! The skill Editor dialog (`skills/SkillEditorDialog.vue`): the skill's text in the code box, "Show changes" and "Start from
//! Fleet's version", what changed as a diff, a note, and Save as version N. Fleet's box is CodeMirror; here it is the shared code
//! box (`ui::code_box`: line numbers, Tab, Ctrl/⌘ S), the same one profiles use. Sizes are Fleet's Tailwind classes at the 14px root.
use crate::State;
use crate::fleet::settings::skill_editor::SkillEditor;
use crate::theme::{Theme, mono, sans};
use crate::ui::button::{self, Size, Variant, button, button_lead};
use crate::ui::code_box::code_box;
use crate::ui::confirm::{error_note, spinner};
use crate::ui::controls::input;
use crate::ui::dialog::{Dialog, dialog};
use crate::ui::rem;
use rune::{Cursor, El, col, row, text};

/// `sm:max-w-3xl`.
const WIDTH: f32 = 672.0;
/// `max-h-[88dvh]`.
const HEIGHT_OF_WINDOW: f32 = 0.88;

pub(super) fn window(s: &State) -> (f32, f32) {
    let (w, h) = s.ui.sel.window;
    if w > 0.0 && h > 0.0 { (w, h) } else { (1280.0, 760.0) }
}

/// A text link (`text-accent-text hover:underline`, half-faded when disabled) in `size` px.
pub(super) fn text_link(t: &Theme, key: String, words: &str, size: f32, off: bool, press: impl Fn(&mut State) + 'static) -> El {
    let l = row()
        .key(key)
        .role("button")
        .label(words.to_string())
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .child(text(words.to_string()).font(sans(size, 400.0, 1.5)).color(t.accent_text));
    if off { l.opacity(0.5).disabled(true).cursor(Cursor::Default) } else { l.on_press(press) }
}

fn title(t: &Theme, name: &str) -> El {
    row()
        .key("skill-editor-title")
        .flex_wrap()
        .gap(rem(0.5))
        .center_y()
        .child(text("Edit").font(sans(rem(1.125), 600.0, 1.0)).color(t.text))
        .child(text(name.to_string()).font(mono(rem(1.125), 600.0, 1.0)).color(t.text))
}

fn links(t: &Theme, e: &SkillEditor) -> El {
    let mut r = row().w_full().flex_wrap().center_y().gaps(4.0, 16.0).child(text_link(
        t,
        "skill-editor-show-changes".into(),
        if e.changes { "Hide changes" } else { "Show changes" },
        rem(0.875),
        !e.changed(),
        |s: &mut State| s.skill_editor_toggle_changes(),
    ));
    if e.can_start_from_fleets() {
        r = r.child(text_link(t, "skill-editor-from-fleets".into(), "Start from Fleet's version", rem(0.875), false, |s: &mut State| {
            s.skill_editor_start_from_fleets()
        }));
    }
    r
}

fn note(t: &Theme, e: &SkillEditor) -> El {
    col()
        .w_full()
        .gap(rem(0.5))
        .child(text("What did you change?").key("skill-editor-note-label").font(sans(rem(0.875), 500.0, 1.43)).color(t.text))
        .child(
            input(t, &e.note, "e.g. Don't report naming or style as findings")
                .key("skill-editor-note")
                .on_edit(|s: &mut State| &mut s.settings.editor.note),
        )
        .child(
            text("Optional. The skill's history shows it next to this version.")
                .font(sans(rem(0.75), 400.0, 1.33))
                .color(t.muted)
                .wrap()
                .w_full(),
        )
}

fn body(s: &State, t: &Theme, e: &SkillEditor) -> Vec<El> {
    let mut b = Vec::new();
    if e.loading {
        b.push(super::skills_tools::loading(t, "skill-editor-loading", "Loading the skill\u{2026}"));
    } else if let Some(d) = &e.detail {
        b.push(code_box(
            t,
            "skill-editor-text",
            &format!("{} SKILL.md", e.name),
            &e.draft,
            |s: &mut State| &mut s.settings.editor.draft,
            |s: &mut State| s.skill_editor_save(),
        ));
        b.push(links(t, e));
        if e.changes && e.changed() {
            b.push(crate::view::skill_diff::skill_diff(
                t,
                s,
                "skill-editor-diff",
                d.current(),
                &e.draft.text,
                &format!("{}/SKILL.md", e.name),
            ));
        }
        b.push(note(t, e));
    }
    if let Some(why) = &e.error {
        b.push(error_note(t, "skill-editor-error", why, None));
    }
    b
}

fn footer(t: &Theme, e: &SkillEditor) -> Vec<El> {
    let cancel = button(t, Variant::Outline, Size::Default, None, "Cancel").key("skill-editor-cancel");
    let cancel = if e.saving { button::disabled(cancel) } else { cancel.on_press(|s: &mut State| s.skill_editor_close()) };
    let label = format!("Save as version {}", e.next_version());
    let lead = e.saving.then(|| spinner(t.primary_fg));
    let save = button_lead(t, Variant::Default, Size::Default, lead, &label).key("skill-editor-save");
    let save = if e.saving || !e.changed() { button::disabled(save) } else { save.on_press(|s: &mut State| s.skill_editor_save()) };
    vec![cancel, save]
}

/// The dialog and its scrim; `None` while it is closed.
pub fn view(s: &State, t: &Theme) -> Option<Vec<El>> {
    let e = &s.settings.editor;
    if !e.open {
        return None;
    }
    let (w, h) = window(s);
    Some(dialog(
        t,
        Dialog {
            key: "skill-editor",
            title: format!("Edit {}", e.name),
            title_el: Some(title(t, &e.name)),
            description: Some(format!(
                "Saving makes this your version {}. Sessions you start afterwards use it in place of Fleet's, and Fleet's updates never overwrite it.",
                e.next_version()
            )),
            body: body(s, t, e),
            footer: footer(t, e),
            close_button: true,
            click_outside: !e.saving,
            dismiss: Some(|s: &mut State| s.skill_editor_close()),
            width: Some(WIDTH.min(w - 32.0)),
            max_height: Some(h * HEIGHT_OF_WINDOW),
            ..Default::default()
        },
    ))
}
