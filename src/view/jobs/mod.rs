//! Jobs (component 23): `[jobs-list][jobs-detail]`. Ports `components/pages/JobsPage.vue` (the off notice or the screen) and
//! the context's layout. The list (`JobsNavPanel`) and the page's body come with later pieces.
use crate::State;
use crate::fleet::labs::Feature;
use crate::theme::Theme;
use crate::view::labs_frame;

pub fn page(s: &State, t: &Theme) -> Vec<rune::El> {
    labs_frame::page(s, t, Feature::Jobs, "jobs", labs_frame::panel(s, "jobs-list", "Jobs"), labs_frame::empty_page(t, "jobs-detail"))
}
