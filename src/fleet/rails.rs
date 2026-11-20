//! The rails (`lib/rails.ts`, `components/layout/core-rails.ts`, `IconRail.vue`, `use-context-panel.ts`): which rail
//! buttons there are, where each sits, which route belongs to which rail, and which rail has a context panel. Pure: no
//! Rune and no I/O (the plugin poll in `plugins` hands its answer over with `Post::update`).
//!
//! Fleet's four Labs rails (Fleet, Board, Workflows, Jobs) leave the rail while their preference is off; their address
//! still opens, and the page's notice then fills the whole context (`noPanel`). GitHub is a plugin's rail: it is there
//! while the plugin isn't switched off in the `plugins.disabled` preference.
use super::prefs::Prefs;
use crate::Rail;
use serde_json::Value;

/// `MISSION_CONTROL_PREFERENCE_KEY`, `BOARD_FEATURE_ENABLED_KEY`, `WORKFLOWS_PREFERENCE_KEY`, `JOBS_PREFERENCE_KEY`. The
/// two older ones are bare names on the server; they are the keys it stores.
pub const FLEET_KEY: &str = "features.mission-control.enabled";
pub const BOARD_KEY: &str = "features.board.enabled";
pub const WORKFLOWS_KEY: &str = "Workflows";
pub const JOBS_KEY: &str = "Jobs";
/// `DISABLED_PLUGINS_PREFERENCE_KEY`: a JSON list of the plugin ids that are switched off.
pub const DISABLED_PLUGINS_KEY: &str = "plugins.disabled";

/// Where a rail's icon sits (`RailPlacement`): the top group, the group under the first divider, the plugins' (above
/// Settings, in the bottom group), and Fleet's own bottom rails.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Placement {
    Top,
    Secondary,
    Plugin,
    Bottom,
}

/// One rail (`RailDefinition`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RailDef {
    pub rail: Rail,
    pub label: &'static str,
    /// The address it opens.
    pub to: &'static str,
    pub placement: Placement,
    /// Within a placement, lower comes first.
    pub order: i32,
    /// Left out of the icon rail (a Labs feature that is off).
    pub hidden: bool,
    /// No panel: the page's notice fills the whole context (a Labs feature that is off).
    pub no_panel: bool,
    /// Drawn dimmed with a muted dot and "<label> · switched off" (a plugin's rail that is off; none of Fleet's own).
    pub off: bool,
    /// Whether the rail has a panel of its own when it is on (Sessions' list, Settings' menu, ...).
    pub has_panel: bool,
}

/// Which Labs features are on: the switches in Settings → Labs, all off until the preference says `"true"`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Features {
    pub fleet: bool,
    pub board: bool,
    pub workflows: bool,
    pub jobs: bool,
}

impl Features {
    pub fn from_prefs(prefs: &Prefs) -> Features {
        let on = |key: &str| prefs.get(key) == Some("true");
        Features { fleet: on(FLEET_KEY), board: on(BOARD_KEY), workflows: on(WORKFLOWS_KEY), jobs: on(JOBS_KEY) }
    }
}

/// What decides the list: the Labs switches, whether the GitHub plugin is there, and which rails are marked off.
#[derive(Clone, Debug, Default)]
pub struct Inputs {
    pub features: Features,
    pub github_off: bool,
    /// Rails to draw dimmed (`RailDefinition.off`). Fleet's own never are; kept so the marker has a test.
    pub marked_off: Vec<Rail>,
}

impl Inputs {
    pub fn from_prefs(prefs: &Prefs) -> Inputs {
        Inputs { features: Features::from_prefs(prefs), github_off: plugin_is_off(prefs, "github"), marked_off: Vec::new() }
    }
}

/// `parseDisabledPlugins`: whether `plugin` is in the `plugins.disabled` list (anything that isn't a list of text is none).
pub fn plugin_is_off(prefs: &Prefs, plugin: &str) -> bool {
    let list: Value = serde_json::from_str(prefs.get(DISABLED_PLUGINS_KEY).unwrap_or("[]")).unwrap_or(Value::Null);
    list.as_array().is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(plugin)))
}

/// `core-rails.ts` and the GitHub plugin's sidebar item, sorted by `order` (stable, so equal orders keep this order),
/// with a rail left out of the list altogether when its plugin is off.
pub fn rails(inputs: &Inputs) -> Vec<RailDef> {
    let f = inputs.features;
    let def = |rail, label, to, placement, order, labs: Option<bool>, has_panel| RailDef {
        rail,
        label,
        to,
        placement,
        order,
        hidden: labs == Some(false),
        no_panel: labs == Some(false),
        off: inputs.marked_off.contains(&rail),
        has_panel,
    };
    let mut list = vec![
        def(Rail::Fleet, "Fleet", "/fleet", Placement::Top, -10, Some(f.fleet), true),
        def(Rail::Sessions, "Sessions", "/", Placement::Top, 0, None, true),
        def(Rail::Board, "Board", "/board", Placement::Top, 10, Some(f.board), true),
        def(Rail::Workflows, "Workflows", "/workflows", Placement::Top, 20, Some(f.workflows), true),
        def(Rail::Jobs, "Jobs", "/jobs", Placement::Top, 25, Some(f.jobs), true),
        def(Rail::Automations, "Automations", "/automations", Placement::Top, 30, None, true),
        def(Rail::Analytics, "Analytics", "/analytics", Placement::Secondary, 0, None, true),
        def(Rail::Settings, "Settings", "/settings", Placement::Bottom, 40, None, true),
    ];
    if !inputs.github_off {
        // The plugin's own panel; the placeholder one it shows until the plugin supplies a panel counts as a panel.
        list.push(def(Rail::GitHub, "GitHub", "/github", Placement::Plugin, 100, None, true));
    }
    list.sort_by_key(|r| r.order);
    list
}

/// The icons, in the order the rail draws them: top, then secondary, then the bottom group (plugins before Settings).
/// A rail that leaves the rail is gone from each.
pub fn top(list: &[RailDef]) -> Vec<RailDef> {
    group(list, &[Placement::Top])
}

pub fn secondary(list: &[RailDef]) -> Vec<RailDef> {
    group(list, &[Placement::Secondary])
}

pub fn bottom(list: &[RailDef]) -> Vec<RailDef> {
    group(list, &[Placement::Plugin, Placement::Bottom])
}

fn group(list: &[RailDef], placements: &[Placement]) -> Vec<RailDef> {
    placements.iter().flat_map(|p| list.iter().filter(move |r| r.placement == *p && !r.hidden).copied()).collect()
}

/// The tooltip (`RailButton.vue`): the label, and "· switched off" for a rail marked off.
pub fn tooltip(def: &RailDef) -> String {
    if def.off { format!("{} · switched off", def.label) } else { def.label.to_string() }
}

/// `currentRouteRail`: the rail an address belongs to, or `None` when no rail owns it (`/welcome`, `/repositories`,
/// anything unknown: the page then fills the context). Session pages are the Sessions rail. A plugin's address is its
/// rail, a plugin's own sub-addresses too.
pub fn rail_for_path(path: &str, list: &[RailDef]) -> Option<Rail> {
    let under = |base: &str| path == base || path.strip_prefix(base).is_some_and(|rest| rest.starts_with('/'));
    if path == "/board" {
        return Some(Rail::Board);
    }
    for (base, rail) in [
        ("/analytics", Rail::Analytics),
        ("/automations", Rail::Automations),
        ("/jobs", Rail::Jobs),
        ("/fleet", Rail::Fleet),
        // Settings and its sub-addresses (`/settings/plugins/...`) are all the Settings rail.
        ("/settings", Rail::Settings),
    ] {
        if under(base) {
            return Some(rail);
        }
    }
    if path == "/workflows" {
        return Some(Rail::Workflows);
    }
    if path == "/" || path.starts_with("/sessions/") {
        return Some(Rail::Sessions);
    }
    list.iter().find(|r| r.placement == Placement::Plugin && under(r.to)).map(|r| r.rail)
}

/// `railPanel`: whether `rail` draws a panel left of its page. `None` (no rail), a rail that is off, and a rail nobody
/// contributes have none: no view ever borrows another's panel.
pub fn has_panel(rail: Option<Rail>, list: &[RailDef]) -> bool {
    rail.and_then(|r| list.iter().find(|d| d.rail == r)).is_some_and(|d| d.has_panel && !d.no_panel)
}

/// `useStaleRailRedirect`: the rail to fall back to when the active one is nobody's any more (a plugin that went away):
/// Sessions, which also goes to `/`. `None` for a known rail and for no rail.
pub fn stale_redirect(active: Option<Rail>, list: &[RailDef]) -> Option<Rail> {
    let rail = active?;
    (!list.iter().any(|d| d.rail == rail)).then_some(Rail::Sessions)
}

/// `GET /api/plugins` (`PluginCatalogResponse`): what the server says about each plugin. Kept for the badge Fleet wires
/// and hides (`showPluginRailBadges = false`), so nothing draws it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PluginStatuses {
    /// The plugin ids and their connection status, in the server's order.
    pub statuses: Vec<(String, String)>,
    /// The last request failed (`HTTP <status>`, or why nothing answered); the last answer stays.
    pub error: Option<String>,
}

impl PluginStatuses {
    pub fn adopt(&mut self, answer: Result<&Value, String>) {
        match answer {
            Ok(body) => {
                self.statuses = body["statuses"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|s| Some((s["pluginId"].as_str()?.to_string(), s["status"].as_str().unwrap_or("disconnected").to_string())))
                    .collect();
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }
}

impl crate::State {
    /// What the rail list is made from now: the preferences as last read and the marker for rails that are off.
    pub fn rail_inputs(&self) -> Inputs {
        Inputs { marked_off: self.rail_marked_off.clone(), ..Inputs::from_prefs(&self.pickers.prefs) }
    }

    pub fn rail_list(&self) -> Vec<RailDef> {
        rails(&self.rail_inputs())
    }

    /// The lit rail, `None` on a page no rail owns (`NO_RAIL`).
    pub fn active_rail(&self) -> Option<Rail> {
        (!self.on_bare_page()).then_some(self.rail)
    }

    /// Whether the context panel is drawn: the active rail has one (not while its Labs feature is off) and Ctrl/Cmd B
    /// hasn't collapsed it. Any rail's panel collapses, and the choice is remembered (`weave:left-collapsed`).
    pub fn context_panel_shown(&self) -> bool {
        self.sidebar_open && has_panel(self.active_rail(), &self.rail_list())
    }

    /// `useStaleRailRedirect`: a rail that is nobody's now (its plugin was switched off) opens Sessions, at `/`.
    pub fn settle_rail(&mut self) {
        if let Some(to) = stale_redirect(self.active_rail(), &self.rail_list()) {
            self.rail = to;
            self.go(super::store::Route::Home);
        }
    }

    /// A press on a rail button: it becomes the rail, and its address takes the window back from `/welcome` and
    /// `/repositories`. A shortcut into Settings leaves the rail the route sets, which is Settings here too.
    pub fn press_rail(&mut self, rail: Rail) {
        if rail == Rail::Settings {
            self.enter_settings();
        }
        if rail == Rail::Workflows {
            self.enter_workflows();
        }
        self.rail = rail;
        // Sessions' address is `/`: home, with nothing selected (piece C).
        if self.on_bare_page() || rail == Rail::Sessions {
            self.go(super::store::Route::Home);
        }
    }
}
