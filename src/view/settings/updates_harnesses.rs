//! Updates & about › Harnesses (`HarnessUpdatesGroup.vue`): the "Check for updates" switch and "Check again", then a row per
//! harness Fleet lists with its version and, for one that is turned on, its update strip.
use super::kit::{self, Row, SwitchState};
use crate::State;
use crate::fleet::settings::updates_page::harness_version_line;
use crate::fleet::updates::HARNESS_UPDATE_CHECK;
use crate::icons::REFRESH_CW;
use crate::theme::Theme;
use crate::ui::button::{Size, Variant, button, button_lead, disabled};
use crate::ui::confirm::spinner;
use rune::{El, row};

pub fn group(s: &State, t: &Theme) -> El {
    let machine = s.harness_machine();
    let prefs = s.harness_prefs();
    let on = prefs.get(HARNESS_UPDATE_CHECK.key) != Some("false");
    let checking = s.harnesses_checking();
    let again = if checking {
        button_lead(t, Variant::Outline, Size::Sm, Some(spinner(t.muted)), "Check again")
    } else {
        button(t, Variant::Outline, Size::Sm, Some(&REFRESH_CW), "Check again")
    }
    .key("harnesses-check-again")
    .role("button")
    .label("Check again");
    let again = if checking { disabled(again) } else { again.on_press(|s: &mut State| s.check_harnesses_again()) };
    let switch = kit::setting_switch(
        t,
        "harnesses-update-checks",
        "Check for updates",
        on,
        SwitchState { saving: false, disabled: s.harness_prefs_loading(), pending: false },
        |s: &mut State| s.toggle_harness_update_checks(),
    );
    let mut rows = vec![
        Row::new("harness-update-checks", "Check for updates")
            .summary("Fleet looks up each harness's latest version on npm.")
            .control(row().center_y().gap(10.5).child(again).child(switch))
            .build(s, t),
    ];
    for h in s.harness_list().all {
        let mut r = Row::new(&format!("harness-version-{}", h.kind), &h.display_name).summary(&harness_version_line(&h));
        if let Some(strip) = s.harness_enabled(&h).then(|| super::harness_update::view(s, t, &h, &machine)).flatten() {
            r = r.below(row().w_full().mt(7.0).child(strip));
        }
        rows.push(r.build(s, t));
    }
    kit::group(t, "harnesses", "Harnesses", Some("Each harness's version and its update."), rows)
}
