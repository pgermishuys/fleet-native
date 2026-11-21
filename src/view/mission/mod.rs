//! Fleet, mission control (component 23): `[fleet-menu][fleet-detail]`. Ports `components/pages/MissionControlPage.vue` (the
//! off notice or the screen) and the context's layout. `menu` is `FleetNavPanel`, `page` is `MissionControlScreen`'s header and
//! the view switch, `needs_you` is `NeedsYouColumn`; a machine's view and the peek come with piece E.
use crate::State;
use crate::fleet::labs::Feature;
use crate::theme::Theme;
use crate::view::labs_frame;

mod menu;
mod needs_you;
mod page;

pub fn page(s: &State, t: &Theme) -> Vec<rune::El> {
    // Off, nothing is read: the wall is only worked out for the view that is on.
    if s.labs_feature(Feature::Fleet) != Some(true) {
        return labs_frame::page(
            s,
            t,
            Feature::Fleet,
            "fleet",
            labs_frame::panel(s, "fleet-menu", "Fleet navigation"),
            labs_frame::empty_page(t, "fleet-detail"),
        );
    }
    let wall = s.mission_wall();
    labs_frame::page(s, t, Feature::Fleet, "fleet", menu::panel(s, t, &wall), page::screen(s, t, &wall))
}
