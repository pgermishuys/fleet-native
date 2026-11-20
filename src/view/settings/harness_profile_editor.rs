//! A profile's editor (`HarnessProfileEditor.vue`): the Name box, the config in the code box, the harness's note, the result
//! card, and Duplicate, Delete, Cancel, Test and Save. It sits in the row of the profile, or under "New profile". Sizes are
//! Fleet's Tailwind classes at the 14px root.
use crate::State;
use crate::fleet::new_session::feeds::Profile;
use crate::fleet::settings::harness_profiles::{Busy, Check, Draft, Editing};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{self, Size, Variant, button_lead};
use crate::ui::code_box::code_box;
use crate::ui::text_menu::text_input;
use crate::ui::{confirm::spinner, rem};
use rune::{El, Span, col, icon, rich, row, text};

/// `splitCode`: `text` split at backticks; the odd pieces are code.
pub fn split_code(words: &str) -> Vec<(&str, bool)> {
    words.split('`').enumerate().map(|(i, p)| (p, i % 2 == 1)).filter(|(p, _)| !p.is_empty()).collect()
}

/// The harness's note with its backtick pieces in mono `--text`.
fn note(t: &Theme, words: &str) -> El {
    let (mut plain, mut spans) = (String::new(), Vec::new());
    for (piece, code) in split_code(words) {
        let at = plain.len();
        plain.push_str(piece);
        if code {
            spans.push(Span::new(at, plain.len()).mono().color(t.text));
        }
    }
    row().key("harness-profile-note").child(rich(plain, spans).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().shrink().min_w(0.0))
}

/// `<label class="flex items-center gap-3 text-xs text-muted">` with a 48px label and an `h-8 max-w-xs` input.
fn name_row(t: &Theme, d: &Draft) -> El {
    let input = text_input(&d.name, sans(rem(0.875), 400.0, 1.3))
        .key("harness-profile-name")
        .single_line()
        .placeholder("Work, Local models, Reviewer\u{2026}".to_string(), t.muted)
        .label("Name".to_string())
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .h(rem(2.0))
        .px(rem(0.625))
        .py((rem(2.0) - rem(0.875) * 1.3) / 2.0 - 1.0)
        .radius(RADIUS_BTN)
        .border(1.0, if d.name.focused { t.accent } else { t.border })
        .fill(t.main_bg)
        .on_edit(|s: &mut State| &mut s.settings.harnesses.profiles.draft.get_or_insert_with(Draft::default).name);
    row()
        .w_full()
        .center_y()
        .gap(10.5)
        .child(row().w(rem(3.0)).child(text("Name").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted)))
        .child(row().w_full().max_w(320.0).child(input))
}

/// The result card (`role="status"`): running-green when the harness loads the profile, error-red with its reason otherwise.
fn result_card(t: &Theme, harness: &str, r: &Check) -> El {
    let (edge, fill, ink) = if r.ok {
        (t.running.alpha(0.3), t.running.alpha(0.1), t.running_text)
    } else {
        (t.error.alpha(0.3), t.error.alpha(0.1), t.error_text)
    };
    let words = if r.ok { format!("{harness} loads this profile.") } else { r.error.clone().unwrap_or_default() };
    let mut card = col()
        .key("harness-profile-result")
        .role("status")
        .w_full()
        .px(10.5)
        .py(7.0)
        .radius(RADIUS_CARD)
        .border(1.0, edge)
        .border_layout()
        .fill(fill)
        .child(
            row()
                .gap(5.25)
                .center_y()
                .child(icon(if r.ok { &CHECK } else { &X }, 13.0).color(ink))
                .child(text(words).font(sans(rem(0.75), 500.0, 1.33)).color(ink).wrap().shrink().min_w(0.0)),
        );
    if let (false, Some(details)) = (r.ok, r.details.as_deref().filter(|d| !d.is_empty())) {
        let mut lines = col().mt(5.25).scroll_xy().max_w(f32::MAX);
        for line in details.split('\n') {
            lines = lines.child(text(line.to_string()).font(mono(11.0, 400.0, 1.5)).color(t.text));
        }
        card = card.child(lines.key("harness-profile-details"));
    }
    card
}

fn busy_button(t: &Theme, v: Variant, key: &str, label: &str, lead: El) -> El {
    button_lead(t, v, Size::Sm, Some(lead), label).key(key.to_string())
}

/// Buttons of the editor: Duplicate and Delete on the left (an existing profile), Cancel, Test and Save on the right.
fn buttons(t: &Theme, kind: &str, harness: &str, d: &Draft, profile: Option<&Profile>) -> El {
    let busy = d.busy.is_some();
    let off = |b: El| if busy { button::disabled(b) } else { b };
    let mut left = row().gap(3.5);
    if let Some(p) = profile {
        let name = p.name.clone();
        left = left.child(
            button::button(t, Variant::Ghost, Size::Sm, None, "Duplicate")
                .key("harness-profile-duplicate")
                .on_press(move |s: &mut State| s.duplicate_profile(&name)),
        );
        let (name, is_default) = (p.name.clone(), p.is_default);
        let label = if d.confirming_delete { format!("Delete {}?", p.name) } else { "Delete".to_string() };
        let delete = button::button_ink(t, Variant::Ghost, Size::Sm, None, &label, Some(t.error_text))
            .key("harness-profile-delete")
            .on_press(move |s: &mut State| s.delete_profile_asked(&name, is_default));
        left = left.child(off(delete));
    }
    let shut = kind.to_string();
    let cancel = button::button(t, Variant::Ghost, Size::Sm, None, "Cancel")
        .key("harness-profile-cancel")
        .on_press(move |s: &mut State| s.close_profile_editor(&shut, None));
    let test_lead = if d.busy == Some(Busy::Test) { spinner(t.text) } else { icon(&PLAY, rem(1.0)).color(t.text) };
    let test = off(busy_button(t, Variant::Outline, "harness-profile-test", "Test", test_lead).on_press(|s: &mut State| s.test_profile()));
    let saving = d.busy == Some(Busy::Save);
    let save = if saving {
        busy_button(t, Variant::Default, "harness-profile-save", &format!("Checking with {harness}\u{2026}"), spinner(t.primary_fg))
    } else {
        button::button(t, Variant::Default, Size::Sm, None, "Save").key("harness-profile-save")
    };
    let save = off(save.on_press(|s: &mut State| s.save_profile()));
    row().w_full().flex_wrap().center_y().justify_between().gap(7.0).child(left).child(row().gap(3.5).child(cancel).child(test).child(save))
}

/// The editor for `editing` (a profile of the list, or a new one).
pub fn editor(s: &State, t: &Theme, kind: &str, harness: &str, profile_note: Option<&str>, editing: &Editing) -> El {
    let Some(d) = s.settings.harnesses.profiles.draft.as_ref() else { return col() };
    let profile = match editing {
        Editing::Edit(id) => s.harness_profiles(kind).into_iter().find(|p| &p.id == id),
        Editing::New(_) => None,
    };
    let mut e = col()
        .key(if matches!(editing, Editing::New(_)) { "harness-profile-editor-new" } else { "harness-profile-editor" })
        .w_full()
        .gap(10.5)
        .child(name_row(t, d))
        .child(code_box(
            t,
            "harness-profile-config",
            &d.config_label(),
            &d.content,
            |s: &mut State| &mut s.settings.harnesses.profiles.draft.get_or_insert_with(Draft::default).content,
            |s: &mut State| s.save_profile(),
        ));
    if let Some(words) = profile_note.filter(|n| !n.is_empty()) {
        e = e.child(note(t, words));
    }
    if let Some(problem) = d.syntax_problem() {
        e = e.child(text(problem).key("harness-profile-syntax").font(sans(rem(0.75), 400.0, 1.33)).color(t.idle_text));
    }
    if let Some(r) = &d.result {
        e = e.child(result_card(t, harness, r));
    }
    e.child(buttons(t, kind, harness, d, profile.as_ref()))
}
