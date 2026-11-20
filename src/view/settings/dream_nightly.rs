//! Settings → Memory's "Dream each night" row (`memory/DreamNightly.vue`): the switch, and with it on the time, the harness, the
//! model, the budget, a link to the automation that runs it, and whether it is paused. With no answer from the server the
//! row is not drawn; its failed read is an error with Try again where the row would be (weave-fleet #1051).
//! The time is a text box that takes `HH:MM`: Rune has no time field (`docs/RUNE.md`).
use super::dreaming_panel::small;
use super::kit::{self, SwitchState};
use crate::State;
use crate::fleet::settings::dream_nightly::{self, Nightly};
use crate::icons::{CHEVRON_DOWN, CIRCLE_ALERT};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, sans};
use crate::ui::button::{self, Size, Variant};
use crate::ui::text_menu::text_input;
use crate::ui::{Open, rem, select};
use rune::{Cursor, El, col, icon, row, text};
use std::rc::Rc;

pub const HARNESS_SELECT: &str = "dream-nightly-harness";
pub const MODEL_SELECT: &str = "dream-nightly-model";
pub const BUDGET_SELECT: &str = "dream-budget";

/// A select as the row draws it: `rounded-btn border border-border bg-main-bg px-2 py-1 text-sm`, the chevron at the end.
fn field(s: &State, t: &Theme, key: &'static str, label: &str, shown: &str, can_open: bool) -> El {
    boxed_field(s, t, key, label, shown, can_open, true)
}

/// `bare` leaves out the border and the fill: Fleet's model picker is the composer's, a quiet text button.
fn boxed_field(s: &State, t: &Theme, key: &'static str, label: &str, shown: &str, can_open: bool, boxed: bool) -> El {
    let (edge, fill) = match (boxed, s.ui.open == Some(Open::Select(key))) {
        (true, true) => (t.accent, t.main_bg),
        (true, false) => (t.border, t.main_bg),
        (false, _) => (rune::Color::TRANSPARENT, rune::Color::TRANSPARENT),
    };
    let ink = if boxed { t.text } else { t.muted };
    let e = row()
        .key(key)
        .gap(7.0)
        .px(7.0)
        .py(3.5)
        .center_y()
        .radius(RADIUS_BTN)
        .border(1.0, edge)
        .border_layout()
        .fill(fill)
        .role("combobox")
        .label(label.to_string())
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(text(shown.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(ink))
        .child(icon(&CHEVRON_DOWN, 12.0).color(t.muted));
    if can_open { e.cursor(Cursor::Pointer).on_press(move |s: &mut State| s.ui.open = Some(Open::Select(key))) } else { e }
}

fn time_box(s: &State, t: &Theme) -> El {
    let edit = &s.settings.memory.nightly.time;
    text_input(edit, sans(rem(0.875), 400.0, 1.43))
        .key("dream-time")
        .label("Time to dream")
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .on_edit(|s: &mut State| &mut s.settings.memory.nightly.time)
        .on_submit(|s: &mut State, _| s.nightly_commit_time())
        .w(63.0)
        .px(7.0)
        .py(3.5)
        .radius(RADIUS_BTN)
        .border(1.0, if edit.focused { t.accent } else { t.border })
        .border_layout()
        .fill(t.main_bg)
}

fn model_label(s: &State) -> String {
    let key = s.nightly_model_key();
    let found = s.nightly_models().and_then(|c| c.models.iter().find(|m| m.selection_key == key));
    match (found, s.settings.memory.nightly.nightly.as_ref().and_then(|n| n.model.clone())) {
        (Some(m), _) => m.name.clone(),
        (None, Some(path)) => path,
        (None, None) => "Default model".into(),
    }
}

fn controls(s: &State, t: &Theme, n: &Nightly) -> El {
    let page = &s.settings.memory.nightly;
    let choices = s.nightly_harness_choices();
    let shown = s.nightly_harness_shown();
    let name = choices.iter().find(|(k, _)| *k == shown).map_or(String::new(), |(_, display)| display.clone());
    let mut c = row().key("dream-nightly-controls").w_full().mt(10.5).flex_wrap().center_y().gap(7.0).child(time_box(s, t)).child(field(
        s,
        t,
        HARNESS_SELECT,
        "Harness for the nightly dream",
        &name,
        !choices.is_empty() && !page.saving,
    ));
    if s.nightly_models().is_some() {
        c = c.child(boxed_field(s, t, MODEL_SELECT, "Model for the nightly dream", &model_label(s), !page.saving, false));
    }
    c = c.child(field(s, t, BUDGET_SELECT, "Tokens a night", &dream_nightly::budget_label(n.budget_tokens), !page.saving));
    // Another machine's automation has no page to open from here: the link is the live machine's.
    if n.automation_id.is_some() && s.settings_is_live() {
        c = c.child(
            text("Runs as the built-in automation Dreaming")
                .key("dream-automation")
                .font(sans(rem(0.75), 400.0, 1.33))
                .color(t.accent_text)
                .role("link")
                .label("Runs as the built-in automation Dreaming")
                .focusable(true)
                .cursor(Cursor::Pointer)
                .on_press(|s: &mut State| s.open_dream_automation()),
        );
    }
    if n.paused {
        c = c.child(text("Paused in Automations").key("dream-paused").font(sans(rem(0.75), 400.0, 1.33)).color(t.idle_text));
    }
    c
}

fn experimental(t: &Theme) -> El {
    row()
        .px(7.0)
        .py(1.75)
        .radius(999.0)
        .border(1.0, t.border)
        .border_layout()
        .child(text("EXPERIMENTAL").font(sans(10.0, 600.0, 1.4).tracking(0.5)).color(t.muted))
}

fn row_card(s: &State, t: &Theme, n: &Nightly) -> El {
    let page = &s.settings.memory.nightly;
    let mut words = col()
        .shrink()
        .grow(1.0)
        .min_w(0.0)
        .child(
            row()
                .center_y()
                .gap(7.0)
                .child(text("Dream each night").font(sans(rem(0.875), 500.0, 1.43)).color(t.text))
                .child(experimental(t)),
        )
        .child(row().mt(3.5).child(
            small(t, "Fleet reads the day's sessions in each repository you worked in and updates its notes. Every change says why, and you can undo it. Notes you wrote yourself are never rewritten or removed.")
                .wrap()
                .shrink()
                .min_w(0.0),
        ));
    if n.enabled {
        words = words.child(controls(s, t, n));
    }
    if let Some(message) = s.nightly_time_error().filter(|_| n.enabled) {
        words = words.child(kit::error_note(t, "dream-time-error", message));
    }
    if let Some(message) = &page.error {
        words = words.child(kit::error_note(t, "dream-nightly-save-error", message));
    }
    let switch = kit::setting_switch(
        t,
        "dream-nightly-switch",
        "Dream each night",
        n.enabled,
        SwitchState { saving: page.saving, disabled: page.saving, pending: false },
        |s: &mut State| s.nightly_toggle(),
    );
    row()
        .key("dream-nightly")
        .w_full()
        .mt(17.5)
        .items_start()
        .justify_between()
        .gap(14.0)
        .px(14.0)
        .py(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(words)
        .child(row().center_y().gap(7.0).child(switch))
}

/// `ErrorNote` with Try again, in the row's place.
fn load_error(t: &Theme, message: &str) -> El {
    let retry = button::button(t, Variant::Outline, Size::Sm, None, "Try again")
        .key("dream-nightly-retry")
        .on_press(|s: &mut State| s.nightly_retry())
        .mt(7.0);
    row()
        .key("dream-nightly-error")
        .role("alert")
        .w_full()
        .mt(17.5)
        .gap(7.0)
        .items_start()
        .px(10.5)
        .py(7.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.1))
        .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0))
        .child(
            col()
                .shrink()
                .min_w(0.0)
                .child(text(message.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.error_text).wrap())
                .child(row().child(retry)),
        )
}

/// The row, the error that stands in for it, or nothing (Memory off, or no answer yet).
pub fn nightly(s: &State, t: &Theme) -> Option<El> {
    let page = &s.settings.memory.nightly;
    match (&page.nightly, &page.error) {
        (Some(n), _) => Some(row_card(s, t, n)),
        (None, Some(message)) => Some(load_error(t, message)),
        (None, None) => None,
    }
}

pub fn harness_list(s: &State, t: &Theme) -> Vec<El> {
    let choices = s.nightly_harness_choices();
    let options: Vec<(&str, &str)> = choices.iter().map(|(k, n)| (k.as_str(), n.as_str())).collect();
    let shown = s.nightly_harness_shown();
    let choose: select::Choose = Rc::new(|s: &mut State, kind: &str| {
        s.ui.open = None;
        s.nightly_pick_harness(kind);
    });
    select::content(t, "dream-nightly-harness-list", HARNESS_SELECT, 160.0, &options, Some(shown.as_str()), choose)
}

pub fn model_list(s: &State, t: &Theme) -> Vec<El> {
    let Some(catalog) = s.nightly_models() else { return vec![] };
    let mut options: Vec<(&str, &str)> = vec![("", "Default model")];
    options.extend(catalog.models.iter().map(|m| (m.selection_key.as_str(), m.name.as_str())));
    let current = s.nightly_model_key();
    let choose: select::Choose = Rc::new(|s: &mut State, key: &str| {
        s.ui.open = None;
        s.nightly_pick_model(key);
    });
    select::content(t, "dream-nightly-model-list", MODEL_SELECT, 220.0, &options, Some(current.as_str()), choose)
}

pub fn budget_list(s: &State, t: &Theme) -> Vec<El> {
    let labels: Vec<(String, String)> = dream_nightly::BUDGETS.iter().map(|b| (b.to_string(), dream_nightly::budget_label(*b))).collect();
    let options: Vec<(&str, &str)> = labels.iter().map(|(v, l)| (v.as_str(), l.as_str())).collect();
    let current = s.settings.memory.nightly.nightly.as_ref().map(|n| n.budget_tokens.to_string());
    let choose: select::Choose = Rc::new(|s: &mut State, value: &str| {
        s.ui.open = None;
        if let Ok(budget) = value.parse() {
            s.nightly_pick_budget(budget);
        }
    });
    select::content(t, "dream-budget-list", BUDGET_SELECT, 200.0, &options, current.as_deref(), choose)
}
