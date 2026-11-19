//! The add form of Settings → API keys and the pieces both it and the key cards draw: the three button looks and a labelled
//! field. The secret boxes are secure: one bullet per character, and the text is never drawn or copied.
use super::skills_tools::error_box;
use crate::State;
use crate::fleet::settings::credentials::PRESETS;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, sans};
use crate::ui::confirm::spinner;
use crate::ui::rem;
use rune::{Cursor, El, TextEdit, col, row, text, text_input};

#[derive(Clone, Copy, PartialEq)]
pub enum Look {
    /// `buttonPrimaryClass`.
    Primary,
    /// `buttonSecondaryClass`.
    Secondary,
    /// `buttonDangerClass`.
    Danger,
}

/// The component's three button classes: `gap-2 rounded-btn px-3 py-2 text-sm font-medium`, the secondary and the danger with a
/// 1px border; waiting buttons are drawn at 60% and don't press.
pub fn button(t: &Theme, key: &str, look: Look, lead: Option<El>, words: &str, on: bool) -> El {
    let (fill, edge, ink, hover) = match look {
        Look::Primary => (t.accent, None, t.primary_fg, t.accent.alpha(0.9)),
        Look::Secondary => (t.main_bg, Some(t.border), t.text, t.main_bg),
        Look::Danger => (t.error.alpha(0.1), Some(t.error.alpha(0.4)), t.error_text, t.error.alpha(0.2)),
    };
    let mut b = row().items_center().justify_center().gap(7.0).px(rem(0.75)).py(rem(0.5)).radius(RADIUS_BTN).fill(fill).hover(hover);
    if let Some(edge) = edge.filter(|_| look == Look::Danger) {
        b = b.border(1.0, edge).border_layout();
    }
    if let Some(lead) = lead {
        b = b.child(lead);
    }
    let b = b.child(text(words.to_string()).font(sans(rem(0.875), 500.0, 1.43)).color(ink));
    // The secondary button's border turns accent half way on hover and Rune has no hover for a border alone: the border is the
    // outer box's fill, one pixel showing around the inner one.
    let b = if look == Look::Secondary { ringed(t, b, RADIUS_BTN) } else { b };
    let b = b
        .key(key.to_string())
        .role("button")
        .label(words.to_string())
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent);
    if on { b } else { b.opacity(0.6).disabled(true).cursor(Cursor::Default) }
}

/// `border border-border hover:border-accent/50` over `inner`, which draws its own fill.
fn ringed(t: &Theme, inner: El, radius: f32) -> El {
    row().padding(1.0).radius(radius).fill(t.border).hover(t.accent_border(50.0)).child(inner.radius(radius - 1.0).grow(1.0))
}

/// `text-xs font-medium uppercase tracking-wide text-muted`.
fn caption(t: &Theme, words: &str) -> El {
    text(words.to_uppercase()).font(sans(rem(0.75), 500.0, 1.33).tracking(0.2625)).color(t.muted)
}

type Edit = fn(&mut State) -> &mut TextEdit;

/// A labelled input: `inputClass` (`rounded-btn border px-3 py-2 text-sm`, the border turns accent with the focus), at 60% and
/// not typeable while its request runs. A secure box is told by the edit it was made with.
#[allow(clippy::too_many_arguments)]
pub fn field(
    t: &Theme,
    key: &str,
    label: &str,
    placeholder: &str,
    value: &TextEdit,
    waiting: bool,
    edit: Edit,
    submit: fn(&mut State),
) -> El {
    let mut input = text_input(value, sans(rem(0.875), 400.0, 1.43))
        .key(key.to_string())
        .label(label.to_string())
        .single_line()
        .placeholder(placeholder.to_string(), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .px(rem(0.75))
        .py(rem(0.5))
        .radius(RADIUS_BTN)
        .border(1.0, if value.focused { t.accent } else { t.border })
        .border_layout()
        .fill(t.main_bg)
        .on_edit(edit)
        .on_submit(move |s: &mut State, _| submit(s));
    if waiting {
        input = input.disabled(true).opacity(0.6);
    }
    col().w_full().gap(3.5).child(caption(t, label)).child(input)
}

/// A quick-select pill: `rounded-full border px-3 py-1 text-xs`, the border turns accent half way on hover.
fn chip(t: &Theme, namespace: &'static str, label: &str) -> El {
    let inner = row().px(9.5).py(2.5).fill(t.main_bg).child(text(label.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.text));
    ringed(t, inner, 1000.0)
        .key(format!("credential-preset-{namespace}"))
        .role("button")
        .label(label.to_string())
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .on_press(move |s: &mut State| s.pick_preset(namespace))
}

/// The panel opened by Add API key.
pub fn add_form(s: &State, t: &Theme) -> El {
    let f = &s.settings.credentials.form;
    let heading = col().child(text("Add API key").font(sans(rem(0.875), 600.0, 1.43)).color(t.text)).child(
        row()
            .mt(3.5)
            .child(text("The secret value is never returned after it is saved.").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap()),
    );
    let quick = col()
        .gap(7.0)
        .child(caption(t, "Quick select"))
        .child(row().flex_wrap().gap(7.0).children(PRESETS.iter().map(|p| chip(t, p.namespace, p.label))));
    let submit: fn(&mut State) = |s| s.store_credential();
    let mut fields = col()
        .w_full()
        .gap(10.5)
        .child(field(
            t,
            "credential-label",
            "Label",
            "My Anthropic API Key",
            &f.label,
            f.saving,
            |s| &mut s.settings.credentials.form.label,
            submit,
        ))
        .child(
            row()
                .w_full()
                .gap(10.5)
                .child(col().w(0.0).grow(1.0).min_w(0.0).child(field(
                    t,
                    "credential-provider",
                    "Provider",
                    "anthropic",
                    &f.namespace,
                    f.saving,
                    |s| &mut s.settings.credentials.form.namespace,
                    submit,
                )))
                .child(col().w(0.0).grow(1.0).min_w(0.0).child(field(
                    t,
                    "credential-kind",
                    "Type",
                    "api-key",
                    &f.kind,
                    f.saving,
                    |s| &mut s.settings.credentials.form.kind,
                    submit,
                ))),
        )
        .child(field(
            t,
            "credential-secret",
            "Secret value",
            "Paste your API key",
            &f.value,
            f.saving,
            |s| &mut s.settings.credentials.form.value,
            submit,
        ));
    if let Some(e) = &f.error {
        fields = fields.child(error_box(t, "credential-form-error", e));
    }
    let save = button(
        t,
        "credential-store",
        Look::Primary,
        f.saving.then(|| spinner(t.primary_fg)),
        if f.saving { "Saving\u{2026}" } else { "Save API key" },
        !f.saving,
    )
    .on_press(|s: &mut State| s.store_credential());
    let cancel =
        button(t, "credential-form-cancel", Look::Secondary, None, "Cancel", !f.saving).on_press(|s: &mut State| s.close_credential_form());
    col()
        .key("credential-form")
        .w_full()
        .mt(17.5)
        .gap(14.0)
        .padding(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(heading)
        .child(quick)
        .child(fields.child(row().flex_wrap().gap(7.0).child(save).child(cancel)))
}
