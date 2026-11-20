//! Settings → API keys (`components/settings/CredentialsSection.vue`): one card with the Add API key button, the add form and a
//! grid of the keys the machine Settings shows stores, each with Update key and Remove. Sizes are the component's Tailwind
//! classes at the 14 px root. A key's secret is typed into secure boxes (bullets only) and never drawn.
use super::credentials_form::{self as form, Look, button};
use super::skills_tools::{empty_state, error_box};
use crate::State;
use crate::fleet::settings::credentials::{Credential, CredentialsPage};
use crate::icons::{CHECK, KEY_ROUND, PENCIL, PLUS, TRASH_2, X};
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use crate::ui::confirm::spinner;
use crate::ui::rem;
use rune::{El, col, icon, row, text};

/// The "Stored" pill: `rounded-full border border-running/30 bg-running/10 px-2 py-1 text-[10px] font-medium text-running-text`.
fn stored(t: &Theme) -> El {
    row()
        .px(7.0)
        .py(3.5)
        .radius(1000.0)
        .border(1.0, t.running.alpha(0.3))
        .border_layout()
        .fill(t.running.alpha(0.1))
        .child(text("Stored").font(sans(10.0, 500.0, 1.5)).color(t.running_text))
}

/// The inline edit of one key: the new value (bullets), its error, Save and Cancel.
fn editing(page: &CredentialsPage, t: &Theme) -> El {
    let Some(edit) = page.editing.as_ref() else { return col() };
    let mut c = col().key("credential-edit").w_full().mt(14.0).gap(10.5);
    c = c.child(form::field(
        t,
        "credential-edit-value",
        "New value",
        "Enter updated API key value",
        &edit.value,
        edit.updating,
        |s| &mut s.settings.credentials.editing.as_mut().expect("an edit is open").value,
        |s| s.update_credential(),
    ));
    if let Some(e) = &page.action_error {
        c = c.child(error_box(t, "credential-error", e));
    }
    let save = button(
        t,
        "credential-save",
        Look::Primary,
        Some(if edit.updating { spinner(t.primary_fg) } else { icon(&CHECK, 16.0).color(t.primary_fg) }),
        if edit.updating { "Saving\u{2026}" } else { "Save" },
        !edit.updating,
    )
    .on_press(|s: &mut State| s.update_credential());
    let cancel = button(t, "credential-cancel", Look::Secondary, Some(icon(&X, 16.0).color(t.text)), "Cancel", true)
        .on_press(|s: &mut State| s.cancel_credential_edit());
    c.child(row().flex_wrap().gap(7.0).child(save).child(cancel))
}

/// "Update key" and "Remove", both waiting while that key is being removed.
fn actions(t: &Theme, key: &Credential, removing: bool) -> El {
    let id = key.id.clone();
    let edit_id = key.id.clone();
    let update =
        button(t, &format!("credential-update-{id}"), Look::Secondary, Some(icon(&PENCIL, 16.0).color(t.text)), "Update key", !removing)
            .on_press(move |s: &mut State| s.begin_credential_edit(&edit_id));
    let remove = button(
        t,
        &format!("credential-remove-{id}"),
        Look::Danger,
        Some(if removing { spinner(t.error_text) } else { icon(&TRASH_2, 16.0).color(t.error_text) }),
        if removing { "Removing\u{2026}" } else { "Remove" },
        !removing,
    )
    .on_press(move |s: &mut State| s.remove_credential(&id));
    row().flex_wrap().gap(7.0).mt(14.0).child(update).child(remove)
}

/// One stored key (`article`).
fn key_card(s: &State, t: &Theme, key: &Credential) -> El {
    let page = &s.settings.credentials;
    let head =
        row()
            .w_full()
            .justify_between()
            .items_start()
            .gap(10.5)
            .child(
                col()
                    .shrink()
                    .min_w(0.0)
                    .child(
                        row().gap(7.0).center_y().min_w(0.0).child(icon(&KEY_ROUND, 16.0).color(t.muted)).child(
                            text(key.label.clone()).font(sans(rem(0.875), 600.0, 1.43)).color(t.text).truncate().shrink().min_w(0.0),
                        ),
                    )
                    .child(
                        row()
                            .mt(3.5)
                            .min_w(0.0)
                            .child(text(key.line()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).truncate().shrink().min_w(0.0)),
                    ),
            )
            .child(stored(t));
    let mut c = col()
        .key(format!("credential-{}", key.id))
        .w_full()
        .padding(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(head)
        .child(
            row()
                .key(format!("credential-hint-{}", key.id))
                .mt(14.0)
                .child(text(key.hint_text()).font(mono(rem(0.75), 400.0, 1.33)).color(t.muted)),
        );
    let editing_this = page.editing.as_ref().is_some_and(|e| e.id == key.id);
    if editing_this {
        c = c.child(editing(page, t));
    } else {
        c = c.child(actions(t, key, page.deleting.as_deref() == Some(key.id.as_str())));
    }
    c
}

/// `xl:grid-cols-2`: Tailwind's `xl` is 1280px of the 16px browser rem, so the window's own width decides (as the Repositories
/// page does); below it one card to a row.
const TWO_COLUMNS_FROM: f32 = 1280.0;

fn grid(s: &State, t: &Theme) -> El {
    let columns = if s.ui.sel.window.0 >= TWO_COLUMNS_FROM { 2 } else { 1 };
    let mut cards = s.settings.credentials.keys.iter().map(|k| key_card(s, t, k));
    let mut g = col().key("credential-grid").w_full().mt(17.5).gap(10.5);
    loop {
        let mut line = row().w_full().gap(10.5).items_start();
        let mut filled = 0;
        for card in cards.by_ref().take(columns) {
            line = line.child(col().w(0.0).grow(1.0).min_w(0.0).child(card));
            filled += 1;
        }
        if filled == 0 {
            return g;
        }
        for _ in filled..columns {
            line = line.child(rune::el().grow(1.0).w(0.0));
        }
        g = g.child(line);
    }
}

fn head(s: &State, t: &Theme) -> El {
    let words = col().gap(3.5).shrink().min_w(0.0).child(text("Credentials").font(sans(rem(1.125), 600.0, 1.56)).color(t.text)).child(
        text("Store encrypted provider credentials for sessions that require authenticated access.")
            .font(sans(rem(0.875), 400.0, 1.43))
            .color(t.muted)
            .wrap(),
    );
    let mut r = row().w_full().justify_between().items_start().gap(10.5).child(words);
    if !s.settings.credentials.form_open {
        r = r.child(
            button(t, "credential-add", Look::Primary, Some(icon(&PLUS, 16.0).color(t.primary_fg)), "Add API key", true)
                .on_press(|s: &mut State| s.open_credential_form()),
        );
    }
    r
}

/// The card (`CredentialsSection`).
pub fn card(s: &State, t: &Theme) -> El {
    let page = &s.settings.credentials;
    let mut c = col()
        .key("credentials-card")
        .w_full()
        .padding(21.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(head(s, t));
    if page.form_open {
        c = c.child(form::add_form(s, t));
    }
    if page.loading {
        c = c.child(
            row()
                .key("credentials-loading")
                .mt(17.5)
                .gap(7.0)
                .center_y()
                .child(spinner(t.muted))
                .child(text("Loading credentials\u{2026}").font(sans(rem(0.875), 400.0, 1.43)).color(t.muted)),
        );
    } else if let Some(e) = &page.load_error {
        c = c.child(col().w_full().mt(17.5).child(error_box(t, "credentials-load-error", e)));
    } else if page.keys.is_empty() {
        if !page.form_open {
            c = c.child(col().w_full().mt(17.5).child(empty_state(
                t,
                "credentials-empty",
                &KEY_ROUND,
                "No API keys stored",
                "Add a credential to unlock provider-backed sessions and integrations.",
            )));
        }
    } else {
        c = c.child(grid(s, t));
    }
    if let Some(e) = page.action_error.as_ref().filter(|_| page.editing.is_none()) {
        c = c.child(col().w_full().mt(14.0).child(error_box(t, "credential-error", e)));
    }
    c
}

pub fn content(s: &State, t: &Theme) -> El {
    col()
        .key("credentials-section")
        .w_full()
        .gap(21.0)
        .child(super::super::settings::page_header(s, t, &crate::fleet::settings_nav::Page::Credentials))
        .child(card(s, t))
}
