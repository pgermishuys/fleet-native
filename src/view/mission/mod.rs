//! Fleet, mission control (component 23): `[fleet-menu][fleet-detail]`. Ports `components/pages/MissionControlPage.vue` (the
//! off notice or the screen) and the context's layout. The menu (`FleetNavPanel`) and the page's body come with later pieces.
use crate::State;
use crate::fleet::labs::Feature;
use crate::theme::Theme;
use crate::view::labs_frame;

pub fn page(s: &State, t: &Theme) -> Vec<rune::El> {
    labs_frame::page(
        s,
        t,
        Feature::Fleet,
        "fleet",
        labs_frame::panel(s, "fleet-menu", "Fleet navigation"),
        labs_frame::empty_page(t, "fleet-detail"),
    )
}
