//! Home (`/`) on the state: the landing rule's two moments. The pure rule is `dashboard::Landing`.
use super::dashboard::MISSION_CONTROL_KEY;
use super::store::Route;
use crate::State;

impl State {
    /// Whether the dashboard is what the window shows.
    pub fn on_home(&self) -> bool {
        self.route == Route::Home
    }

    /// Asks the landing rule, on arriving at Home and when the preferences come: with Mission control on, the first `/` of a
    /// start opens Fleet's Needs you (a replace: Home is not left behind to go back to).
    pub(crate) fn landing_check(&mut self) {
        let on = self.pickers.prefs.get(MISSION_CONTROL_KEY) == Some("true");
        if self.on_home() && self.landing.lands(self.pickers.prefs.fetched, on) {
            self.open_fleet_address("/fleet");
        }
    }
}

/// The states of the home page worth a picture.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scene {
    /// The invented sessions: one needs the user, one works, the rest are quiet.
    Mock,
    /// Three sessions, one in each section, as the scratch Fleet shows them.
    Three,
    Empty,
    Loading,
    Error,
    /// No harness is ready: the banner with Fleet's reason.
    Harness,
    /// The three of `Three` and ten more quiet ones: Recent has eleven, shows eight and "Show 3 more".
    Many,
    /// The "Show:" menu open over the three sections.
    Filter,
    /// The window works on an invented machine "falcon": its sessions, read-only rows.
    Machine,
}

impl State {
    /// A scene of the home page, at 9 in the morning wherever the machine is.
    pub fn show_home_scene(&mut self, scene: Scene, dark: bool) {
        self.show_home(dark);
        match scene {
            Scene::Mock => {}
            Scene::Three => {
                let mut kept: Vec<_> = self.projects[0].sessions.drain(..).take(3).collect();
                for (session, title) in kept.iter_mut().zip(["Rebuild the index", "Pick a page size", "Fix pagination"]) {
                    session.title = title.into();
                    session.branch.clear();
                    session.listing.project_name = Some("Scratch".into());
                }
                // The third is quiet: invented ages are in seconds.
                kept[2].status = crate::mock::Status::Idle;
                kept[2].ago_secs = Some(1);
                self.projects[0].name = "Scratch".into();
                self.projects[0].sessions = kept;
                self.projects.truncate(1);
            }
            Scene::Empty | Scene::Loading | Scene::Error => {
                self.projects.clear();
                self.store.is_loading = scene == Scene::Loading;
                if scene == Scene::Error {
                    self.store.error = Some("The server didn't answer (503).".into());
                }
            }
            Scene::Many => {
                self.show_home_scene(Scene::Three, dark);
                let titles = [
                    "Rename the tide table",
                    "Sort the harbour log",
                    "Tune the cache sizes",
                    "Write the tide parser",
                    "Check the lamp timer",
                    "Draft the release notes",
                    "Label the chart legend",
                    "Trim the image sizes",
                    "Name the map layers",
                    "Rerun the nightly import",
                ];
                for (n, title) in titles.iter().enumerate() {
                    let mut x = crate::mock::machines::listed(&format!("q{n}"), title, crate::mock::Status::Idle, "1h", None, |_| {});
                    x.ago_secs = Some(3600 * (n as i64 + 2));
                    x.row.updated = 0;
                    x.listing.project_name = Some("Scratch".into());
                    x.branch.clear();
                    self.projects[0].sessions.push(x);
                }
            }
            Scene::Filter => self.ui.open = Some(crate::ui::Open::Menu("retention")),
            Scene::Machine => {
                use crate::mock::machines::{Case, listed};
                self.show_machines_only(Case::Several);
                self.now_ms = Some(crate::mock::rows::NOW);
                let rows = vec![
                    listed(
                        "f-fence",
                        "Paint the fence",
                        crate::mock::Status::NeedsInput,
                        "3h",
                        Some(("p-lighthouse", "Lighthouse")),
                        |_| {},
                    ),
                    listed(
                        "f-fork",
                        "Try the other lens model",
                        crate::mock::Status::Working,
                        "now",
                        Some(("p-lighthouse", "Lighthouse")),
                        |_| {},
                    ),
                    listed(
                        "f-plan",
                        "Plan the lamp rewiring",
                        crate::mock::Status::Idle,
                        "12m",
                        Some(("p-lighthouse", "Lighthouse")),
                        |_| {},
                    ),
                    listed("f-tomato", "Water the tomatoes", crate::mock::Status::Idle, "2d", Some(("p-garden", "Garden")), |_| {}),
                ];
                self.machines.set_current("m-falcon");
                self.dash.other.key = Some("m-falcon".into());
                self.dash.other.sessions = rows;
            }
            Scene::Harness => {
                let mut off = super::new_session::harness_choice::HarnessEntry::ready("opencode");
                off.available = false;
                off.reason =
                    Some("OpenCode isn't installed: Fleet couldn't find opencode on PATH or in the folders its installer uses.".into());
                if let Some(feed) = self.feeds.harnesses.get_mut(super::settings::HOME_MACHINE_KEY) {
                    feed.harnesses.all = vec![off];
                }
                self.settle_choices();
            }
        }
    }
}
