//! Home (`/`) on the state: the landing rule's two moments. The pure rule is `dashboard::Landing`.
use super::dashboard::MISSION_CONTROL_KEY;
use super::store::Route;
use crate::State;

impl State {
    /// Whether the dashboard is what the window shows.
    pub fn on_home(&self) -> bool {
        self.route == Route::Home
    }

    /// Asks the landing rule, on arriving at Home and when the preferences come. Mission control is component 23: until it
    /// exists the rule only records that it would have landed (`landing.landed`).
    pub(crate) fn landing_check(&mut self) {
        let on = self.pickers.prefs.get(MISSION_CONTROL_KEY) == Some("true");
        if self.on_home() {
            self.landing.lands(self.pickers.prefs.fetched, on);
        }
    }
}
