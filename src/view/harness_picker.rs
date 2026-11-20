//! Ports `NSD/HarnessPicker.vue`: the chip that names the harness a session starts on, and the menu above it (rows B7.1-B7.6).
//! The chip shows only with more than one harness enabled; what decides that is `fleet::new_session::harness`.
use crate::fleet::new_session::harness::{HARNESS_CHIP, HARNESS_POPOVER};
use crate::theme::Theme;
use crate::ui::chip::{Chip, Option_, option, pop, pop_label};
use crate::{State, Tx};
use rune::{Align, El, Side};

const WIDTH: f32 = 220.0;

fn row_key(kind: &str) -> String {
    format!("{HARNESS_CHIP}-{kind}")
}

/// B7.2: no mark, the display name, a chevron.
pub fn chip(s: &State, t: &Theme) -> El {
    let disabled = if s.run_box_active() { s.workflows_nav.run_box.starting } else { s.new_draft.draft().is_some_and(|d| d.is_starting) };
    Chip::new(HARNESS_CHIP, s.harness_chip_label()).open(s.harness_menu_open()).disabled(disabled).view(t, |s: &mut State, tx: &mut Tx| {
        s.open_harness_menu();
        let selected = s.resolved_harness();
        tx.focus(row_key(&selected), false);
    })
}

/// B7.3-B7.5: `Harness`, then one row per enabled harness with a check on the one in use.
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let current =
        if s.run_box_active() { s.run_box_harness_type() } else { s.new_draft.draft().map(|d| d.harness_type.clone()).unwrap_or_default() };
    let mut body = vec![pop_label(t, "Harness")];
    for (kind, name) in s.harness_names() {
        let spec = Option_ { checked: kind == current, no_mark: true, ..Option_::new(row_key(&kind), name) };
        body.push(option(t, spec, None, move |s, _| s.choose_harness(&kind)));
    }
    let panel = pop(t, "new-session-harness-pop", WIDTH, body)
        .focus_return(HARNESS_CHIP.to_string())
        .modal(true, |s: &mut State| s.close_chip_menu());
    debug_assert_eq!(HARNESS_POPOVER, "harness");
    vec![
        crate::ui::overlay::scrim("new-session-harness-scrim", |s: &mut State| s.close_chip_menu()),
        crate::ui::overlay::anchored(panel, HARNESS_CHIP, Side::Top, Align::Start, 6.0),
    ]
}
