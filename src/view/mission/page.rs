//! The page (`MissionControlScreen.vue`): the header "Fleet" with what is running, then the view the address names: Needs you,
//! or a machine's sessions.
use super::{machine, needs_you};
use crate::State;
use crate::fleet::labs::FleetView;
use crate::fleet::mission::wall::Wall;
use crate::theme::Theme;
use crate::ui::page_header;
use crate::view::workflows::sheet;
use rune::{El, col};

/// The count line under the title: plural words never change ("1 running", "0 helpers").
pub fn description(wall: &Wall) -> String {
    let n = wall.summary;
    format!("{} running · {} helpers · {} need you", n.running, n.helpers, n.need_you)
}

pub fn screen(s: &State, t: &Theme, wall: &Wall) -> El {
    let body = match &s.labs.fleet_view {
        FleetView::NeedsYou => col().key("fleet-needs-you-view").grow(1.0).min_h(0.0).w_full().child(needs_you::column(s, t, wall)),
        FleetView::Machine(id) => col().key("fleet-machine-body").grow(1.0).min_h(0.0).w_full().child(machine::view(s, t, wall, id)),
    };
    // `center__body`: the page sits in 24 px of padding all round inside the sheet.
    let page = col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .padding(24.0)
        .child(page_header::bar_described(t, "fleet-header", "Fleet", &description(wall)))
        .child(body);
    sheet(t, "fleet-detail").child(page)
}
