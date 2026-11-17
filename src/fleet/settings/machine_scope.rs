//! Which machine a Settings page is about (`SettingsPage.vue`: `MACHINE_PAGES`, `BROWSER_PAGES`, `comesLater`). The menu's
//! switch (`State.settings.choice`) names a machine; some pages ask that machine, some belong to this app whatever it says,
//! and the rest say they come in a later step rather than show the live machine's settings as the other's.
use crate::State;
use crate::fleet::machine::Target;
use crate::fleet::machines::Entry;
use crate::fleet::settings_nav::Page;

/// What a page does with the machine the switch names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Follows {
    /// Asks the machine shown (`MACHINE_PAGES`).
    Machine,
    /// Belongs to this app, not to a machine (`BROWSER_PAGES`, Plugins and every plugin's page).
    App,
    /// Not ported to ask another machine yet: says so (General and Labs in Fleet today).
    Later,
}

pub fn follows(page: &Page) -> Follows {
    match page {
        Page::Harnesses | Page::Folders | Page::Permissions | Page::Skills | Page::Memory | Page::Updates | Page::Credentials => {
            Follows::Machine
        }
        Page::Appearance | Page::Machines | Page::Plugins | Page::Plugin(_) => Follows::App,
        Page::General | Page::Labs => Follows::Later,
    }
}

/// The chip beside the title (`.settings-machine-chip`): the machine's name, or the app's own word on a page no machine owns.
/// Fleet says "This browser"; the app says "This app" (decision 2 of docs/parity/16-settings.md).
pub const APP_CHIP: &str = "This app";

/// "Show <live>'s <Title>".
pub fn show_live_words(live: &str, title: &str) -> String {
    format!("Show {live}'s {title}")
}

impl State {
    /// The switch is there only when another machine is listed.
    pub fn settings_switch_on(&self) -> bool {
        self.settings.choice.enabled(&self.machines)
    }

    /// The machine Settings shows (`settingsMachine.entry`).
    pub fn settings_machine(&self) -> Entry {
        self.settings.choice.entry(&self.machines)
    }

    /// Where a page's requests go: the machine shown, never home by default. `None` while the app has no Fleet to ask.
    pub fn settings_target(&self) -> Option<Target> {
        self.target_of_key(self.settings.choice.key(&self.machines))
    }

    /// `comesLater`: another machine is shown, on a page that neither belongs to the app nor asks a machine.
    pub fn settings_comes_later(&self) -> bool {
        self.settings_switch_on() && self.settings.choice.is_other(&self.machines) && follows(&self.settings.page) == Follows::Later
    }

    /// The machine "Show <name>'s <page>" goes back to (`machines.live`): home, the Fleet the app is connected to.
    pub fn live_machine(&self) -> Entry {
        self.machines.entries().into_iter().next().expect("home is always listed")
    }
}
