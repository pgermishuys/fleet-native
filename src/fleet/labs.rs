//! The frame the three Labs views share (component 23, piece A): which view an address names (`lib/fleet-views.ts`,
//! `lib/jobs.ts` `jobPath`/`jobIdFromPath`), whether each switch is on (`use-mission-control-feature.ts`,
//! `use-jobs-feature.ts`, `use-board-feature.ts`) and Turn on. This app has no address bar, so the address a view is at is
//! kept on `State` and opened from an address string, as Workflows' and Analytics' nav do.
use super::rails::{BOARD_KEY, FLEET_KEY, JOBS_KEY};
use super::settings::HOME_MACHINE_KEY;
use crate::conv::export_history::encode_component;
use crate::{Rail, State};

/// A view of Fleet (mission control): Needs you, or one machine's sessions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FleetView {
    #[default]
    NeedsYou,
    Machine(String),
}

/// `fleetViewPath`: Needs you is the bare `/fleet`.
pub fn fleet_view_path(view: &FleetView) -> String {
    match view {
        FleetView::NeedsYou => "/fleet".into(),
        FleetView::Machine(id) => format!("/fleet/machines/{}", encode_component(id)),
    }
}

/// `decodeURIComponent`; `None` where it would throw (a bad escape, or bytes that aren't UTF-8).
pub fn decode_component(text: &str) -> Option<String> {
    let raw = text.as_bytes();
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' {
            let hex = std::str::from_utf8(raw.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok().filter(|_| hex.bytes().all(|b| b.is_ascii_hexdigit()))?);
            i += 3;
        } else {
            out.push(raw[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The part of `path` at `index` once trailing slashes are dropped (`replace(/\/+$/, "").split("/")`).
fn segment(path: &str, index: usize) -> Option<&str> {
    path.trim_end_matches('/').split('/').nth(index)
}

/// `fleetViewFromPath`: anything that isn't a machine's view (a stale link, a bad escape) shows Needs you.
pub fn fleet_view_from_path(path: &str) -> FleetView {
    match (segment(path, 2), segment(path, 3)) {
        (Some("machines"), Some(id)) if !id.is_empty() => decode_component(id).map_or(FleetView::NeedsYou, FleetView::Machine),
        _ => FleetView::NeedsYou,
    }
}

/// `isSameFleetView`.
pub fn is_same_fleet_view(a: &FleetView, b: &FleetView) -> bool {
    a == b
}

/// `jobPath`: the bare `/jobs` shows none selected.
pub fn job_path(id: &str) -> String {
    format!("/jobs/{}", encode_component(id))
}

/// `jobIdFromPath`: the job an address names, or none on `/jobs`. A bad escape names none (Fleet's throws).
pub fn job_id_from_path(path: &str) -> Option<String> {
    segment(path, 2).filter(|s| !s.is_empty()).and_then(decode_component)
}

/// A Labs view the frame shows, off or on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    Fleet,
    Jobs,
    Board,
}

impl Feature {
    /// The preference that holds the switch, on the live machine.
    pub fn key(self) -> &'static str {
        match self {
            Feature::Fleet => FLEET_KEY,
            Feature::Jobs => JOBS_KEY,
            Feature::Board => BOARD_KEY,
        }
    }

    /// The title of the off notice (`MissionControlPage`, `JobsPage`, `routes/board.tsx`).
    pub fn title(self) -> &'static str {
        match self {
            Feature::Fleet => "Mission control",
            Feature::Jobs => "Jobs",
            Feature::Board => "Board",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Feature::Fleet => {
                "One screen for every running agent on every machine: what needs you, what each is doing right now, and a nudge box."
            }
            Feature::Jobs => "Post jobs to one queue. Fleet starts each on whichever of your machines has a free seat.",
            Feature::Board => {
                "The board lays your sessions out in columns by status, so you can see what's running, waiting or done at a glance."
            }
        }
    }

    pub fn rail(self) -> Rail {
        match self {
            Feature::Fleet => Rail::Fleet,
            Feature::Jobs => Rail::Jobs,
            Feature::Board => Rail::Board,
        }
    }

    fn slot(self) -> usize {
        self as usize
    }
}

/// Where the three views are, and Turn on's saves in flight.
#[derive(Debug, Default)]
pub struct Labs {
    /// The view of Fleet the address names (`useFleetView`).
    pub fleet_view: FleetView,
    /// The job the address names (`/jobs/<id>`); none on the bare `/jobs`.
    pub job: Option<String>,
    turning_on: [bool; 3],
}

impl Labs {
    pub fn turning_on(&self, feature: Feature) -> bool {
        self.turning_on[feature.slot()]
    }
}

impl State {
    /// Whether a Labs view is on; `None` until the preferences have loaded, so a view that is on never flashes "is off".
    /// With no Fleet to ask (scenes, tests) the preference is whatever is kept here.
    pub fn labs_feature(&self, feature: Feature) -> Option<bool> {
        if !self.pickers.prefs.fetched && self.link.is_some() {
            return None;
        }
        Some(self.pickers.prefs.get(feature.key()) == Some("true"))
    }

    /// Turn on (`setMissionControlEnabled(true)` and the others): saves the preference on the live machine.
    pub fn labs_turn_on(&mut self, feature: Feature) {
        if self.labs.turning_on[feature.slot()] {
            return;
        }
        self.labs.turning_on[feature.slot()] = true;
        self.save_pref_on_machine(HOME_MACHINE_KEY, feature.key(), "true", move |s, _| s.labs.turning_on[feature.slot()] = false);
    }

    /// The Fleet rail opened at an address: `/fleet`, or `/fleet/machines/<id>`.
    pub fn open_fleet_address(&mut self, path: &str) {
        self.rail = Rail::Fleet;
        self.labs.fleet_view = fleet_view_from_path(path);
    }

    /// The Jobs rail opened at an address: `/jobs`, or `/jobs/<id>`.
    pub fn open_jobs_address(&mut self, path: &str) {
        self.rail = Rail::Jobs;
        self.labs.job = job_id_from_path(path);
    }

    /// A rail button, the palette or an Open link: each opens the view's bare address (`to`).
    pub fn enter_labs(&mut self, rail: Rail) {
        match rail {
            Rail::Fleet => self.open_fleet_address("/fleet"),
            Rail::Jobs => self.open_jobs_address("/jobs"),
            _ => self.rail = rail,
        }
    }
}
