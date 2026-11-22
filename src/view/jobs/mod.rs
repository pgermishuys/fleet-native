//! Jobs (component 23): `[jobs-list][jobs-detail]`. Ports `components/pages/JobsPage.vue` (the off notice or the screen) and
//! the context's layout. `list` is `JobsNavPanel`, `page` is `JobsScreen`, `detail` is `JobDetail`; `agents` is `JobsAgents` and `new_job` is `NewJobDialog`.
use crate::State;
use crate::fleet::labs::Feature;
use crate::theme::Theme;
use crate::view::labs_frame;

pub mod agents;
pub mod detail;
pub mod list;
pub mod new_job;
pub mod page;

pub fn page(s: &State, t: &Theme) -> Vec<rune::El> {
    if s.labs_feature(Feature::Jobs) != Some(true) {
        return labs_frame::page(
            s,
            t,
            Feature::Jobs,
            "jobs",
            labs_frame::panel(s, "jobs-list", "Jobs"),
            labs_frame::empty_page(t, "jobs-detail"),
        );
    }
    labs_frame::page(s, t, Feature::Jobs, "jobs", list::panel(s, t), page::screen(s, t))
}
