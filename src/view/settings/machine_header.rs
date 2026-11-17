//! The machine part of Settings' chrome (`SettingsNavPanel.vue`'s `.panel-machine`, `SettingsPage.vue`'s chip and
//! "comes in a later step" note, `SettingsMachineSwitch.vue`): the switch under the menu's header, the chip beside a page's
//! title, and what another machine's page says when it can't show it yet.
use crate::State;
use crate::fleet::settings::machine_scope::{APP_CHIP, Follows, follows, show_live_words};
use crate::fleet::settings_nav::Page;
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::rem;
use crate::view::machine_switch::{self, Switch};
use rune::{El, row, text};

/// "Settings for" in the menu: the choice is `State.settings.choice`, shared by the switch and the pages.
pub const SETTINGS: Switch = Switch { label: "Settings", testid: "settings", choice: |s| &mut s.settings.choice };

/// `.panel-machine` (`padding: 0 12px 6px`): the switch's chip, only when another machine is listed.
pub fn menu_switch(s: &State, t: &Theme) -> Option<El> {
    machine_switch::chip(s, t, SETTINGS, &s.settings.choice).map(|chip| row().px(12.0).pb(6.0).child(chip))
}

/// The switch's open menu, over the window.
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    machine_switch::overlay(s, t, SETTINGS, &s.settings.choice)
}

/// `.settings-machine-chip`: mono 11 px, 600, padding 1 6, radius 4. The machine's name in coral, or this app's word on a page
/// no machine owns, muted on a faint wash.
pub fn title_chip(s: &State, t: &Theme, page: &Page) -> Option<El> {
    if !s.settings_switch_on() {
        return None;
    }
    let app = follows(page) == Follows::App;
    let (words, ink, ground) = if app {
        (APP_CHIP.to_string(), t.muted, t.text.alpha(0.08))
    } else {
        (s.settings_machine().name, t.coral_text, t.coral.alpha(0.14))
    };
    Some(
        row()
            .key("settings-machine-chip")
            .label(words.clone())
            .px(6.0)
            .py(1.0)
            .radius(4.0)
            .fill(ground)
            .child(text(words).font(mono(rem(0.78571), 600.0, 1.5)).color(ink)),
    )
}

/// `data-testid="settings-machine-later"`: the page's body for another machine, in place of the page.
pub fn later(s: &State, t: &Theme, page: &Page) -> El {
    let title = page.title();
    let shown = s.settings_machine().name;
    let live = s.live_machine().name;
    let note = row()
        .shrink()
        .min_w(0.0)
        .wrap()
        .child(text(format!("{title} on ")).font(sans(rem(0.875), 400.0, 1.5)).color(t.muted))
        .child(text(shown).font(mono(rem(0.875), 600.0, 1.5)).color(t.coral))
        .child(text(" comes in a later step.").font(sans(rem(0.875), 400.0, 1.5)).color(t.muted));
    row()
        .key("settings-machine-later")
        .w_full()
        .wrap()
        .center_y()
        .gap(12.0)
        .px(14.0)
        .py(10.5)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .child(note)
        .child(
            button(t, Variant::Outline, Size::Sm, None, &show_live_words(&live, title))
                .key("settings-machine-show-live")
                .on_press(|s: &mut State| s.settings.choice.pick(crate::fleet::settings::HOME_MACHINE_KEY)),
        )
}
