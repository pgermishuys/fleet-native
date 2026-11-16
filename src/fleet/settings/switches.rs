//! The preference switches of the Settings pages (`composables/use-preference-switch.ts`, `stores/preferences.ts`): which key
//! a switch holds, what an unset key means, and what turning it writes. Plain Rust; the requests are `State::save_setting`.
use super::super::prefs::Prefs;
use crate::{State, Tx};

/// One preference kept as an on/off switch. `default_on` is what the key means until the user chooses: on unless it is
/// "false", or off unless it is "true" (row 2.33).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Switch {
    pub key: &'static str,
    pub default_on: bool,
}

/// On unless turned off: the new-session page offers a few sentences to start from.
pub const STARTERS: Switch = Switch { key: "NewSessionStarters", default_on: true };
/// Off unless turned on: Fleet may raise a notification when a session needs you or finishes.
pub const DESKTOP_NOTIFICATIONS: Switch = Switch { key: "DesktopNotifications", default_on: false };
/// Off unless turned on.
pub const SESSION_RECAP: Switch = Switch { key: "SessionRecap", default_on: false };
/// Off unless turned on: it sends a message the user didn't type.
pub const RESTART_CONTINUE_AUTO: Switch = Switch { key: "RestartContinueAuto", default_on: false };
/// On unless turned off: a turn a model provider's limit stopped carries on by itself.
pub const RETRY_AFTER_LIMITS: Switch = Switch { key: "RetryAfterLimits", default_on: true };
/// On unless turned off.
pub const USAGE_LIMITS_CHIP: Switch = Switch { key: "UsageLimitsChip", default_on: true };
/// Off unless turned on.
pub const BOARD: Switch = Switch { key: "features.board.enabled", default_on: false };

impl Switch {
    pub fn enabled(&self, prefs: &Prefs) -> bool {
        match (prefs.get(self.key), self.default_on) {
            (None, on) => on,
            (Some(value), true) => value != "false",
            (Some(value), false) => value == "true",
        }
    }

    /// What turning the switch writes: the opposite of what it shows.
    pub fn toggled(&self, prefs: &Prefs) -> &'static str {
        if self.enabled(prefs) { "false" } else { "true" }
    }
}

/// `LARGE_PASTES_PREFERENCE_KEY`'s choices and words, in the select's order (`GeneralSection.vue`).
pub const LARGE_PASTES: [(&str, &str); 3] = [("ask", "Ask"), ("attach", "Always attach"), ("text", "Paste as text")];

impl State {
    /// A page asks for the preferences when it opens, so it shows what the phone or the browser changed meanwhile
    /// (`ensureLoaded`, row 2.32). Nothing happens with no server.
    pub fn refresh_preferences(&self) {
        self.load_preferences();
    }

    /// Whether the preferences are still on their way: a page's switches wait for them (`preferencesStore.isLoading`).
    pub fn preferences_loading(&self) -> bool {
        self.link.is_some() && !self.pickers.prefs.fetched
    }

    pub fn setting_saving(&self, key: &str) -> bool {
        self.settings.saving.contains(key)
    }

    /// `preferencesStore.set` with the saving spinner a switch shows: the value shows now, the key counts as saving until
    /// the server answers, and a refusal puts the old value back and reads them all again.
    pub fn save_setting(&mut self, key: &str, value: &str) {
        let before = self.pickers.prefs.get(key).map(str::to_string);
        self.pickers.prefs.set(key, value);
        self.settle_rail();
        let Some(link) = &self.link else { return };
        self.settings.saving.insert(key.to_string());
        let (k, v) = (key.to_string(), value.to_string());
        link.off_thread(
            {
                let (k, v) = (k.clone(), v.clone());
                move |base| crate::fleet::http::put_preference(base, &k, &v)
            },
            move |s: &mut State, tx: &mut Tx, result| {
                s.settings.saving.remove(&k);
                if result.is_ok() {
                    return;
                }
                s.pickers.prefs.roll_back(&k, &v, before);
                s.load_preferences();
                s.settle_pickers(tx);
            },
        );
    }

    /// A switch's press (`usePreferenceSwitch.toggle`): ignored while the page waits or the key is saving.
    pub fn toggle_setting(&mut self, switch: Switch) {
        if self.preferences_loading() || self.setting_saving(switch.key) {
            return;
        }
        let value = switch.toggled(&self.pickers.prefs);
        self.save_setting(switch.key, value);
    }
}
