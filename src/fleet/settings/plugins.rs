//! Settings → Plugins (`plugins/registry.ts`, `plugins/slots.ts`, `plugins/use-plugin-switch.ts`, the three built-in manifests):
//! the add-ons a Fleet has built in, what each adds, whether it is on, and what a switch writes. Native has no plugin registry
//! to fill at run time (decision of docs/parity/16-settings.md, Part 11): the three manifests are a fixed table in plain Rust,
//! and core never names one in a view; the views read this table. Component 21 fills GitHub's places.
use super::super::prefs::Prefs;
use super::super::rails::{DISABLED_PLUGINS_KEY, plugin_is_off};
use crate::{State, Tx};
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::Mutex;

/// `MODS_PREFERENCE_KEY`: the Mods plugin's own switch (`"true"` / `"false"`), not the list of plugins that are off.
pub const MODS_KEY: &str = "Mods";

/// Where a plugin adds something (`PluginAddition.area`), in the order Fleet lists the areas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Area {
    Rail,
    Settings,
    FolderPicker,
    Board,
}

impl Area {
    pub fn label(self) -> &'static str {
        match self {
            Area::Rail => "Rail",
            Area::Settings => "Settings",
            Area::FolderPicker => "Folder picker",
            Area::Board => "Board",
        }
    }
}

/// One thing a plugin adds (`PluginAddition`): `point` names the contribution point it comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Added {
    pub area: Area,
    pub detail: &'static str,
    pub point: &'static str,
}

/// A manifest (`FleetPluginManifest`): the descriptor, whether it has its own switch, and what it contributes. Every built-in
/// plugin has a settings page, so `page_title` and `nav_label` are always there.
#[derive(Clone, Copy, Debug)]
pub struct Plugin {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// `ownSwitch`: the plugin's switch is its own preference (Mods), not an entry in `plugins.disabled`.
    pub own_switch: bool,
    /// `switchless`: no switch and never off; the row has an Open button.
    pub switchless: bool,
    /// The config page's title, and its label in the menu (`navLabel`, else the title).
    pub page_title: &'static str,
    pub nav_label: &'static str,
    /// What it adds, in `describeContributions`' order: rail, settings, folder picker, board.
    pub adds: &'static [Added],
}

/// In the order Fleet registers them (`main.ts`).
pub const PLUGINS: [Plugin; 3] = [
    Plugin {
        id: "github",
        name: "GitHub",
        description: "Pull requests, issues and checks beside your sessions.",
        own_switch: false,
        switchless: false,
        page_title: "GitHub integration",
        nav_label: "GitHub",
        adds: &[
            Added { area: Area::Rail, detail: "GitHub (opens /github)", point: "sidebarItems" },
            Added { area: Area::Settings, detail: "GitHub integration", point: "configPage" },
            Added { area: Area::FolderPicker, detail: "Suggests your repositories to clone (github)", point: "repositorySources" },
            Added {
                area: Area::Board,
                detail: "Add GitHub issue sources from your bookmarks, optionally narrowing sync to matching labels.",
                point: "boardSources",
            },
        ],
    },
    Plugin {
        id: "mods",
        name: "Mods",
        description: "Agents write small add-ons for Fleet; you review each one.",
        own_switch: true,
        switchless: false,
        page_title: "Mods",
        nav_label: "Mods",
        adds: &[Added { area: Area::Settings, detail: "Mods", point: "configPage" }],
    },
    Plugin {
        id: "built-in-skills",
        name: "Built-in skills",
        description: "Skills that come with Fleet: review, simplify, run, mockups and more. Each has its own switch.",
        own_switch: false,
        switchless: true,
        page_title: "Built-in skills",
        nav_label: "Built-in skills",
        adds: &[Added { area: Area::Settings, detail: "Built-in skills", point: "configPage" }],
    },
];

pub fn find(id: &str) -> Option<&'static Plugin> {
    PLUGINS.iter().find(|p| p.id == id)
}

impl Plugin {
    /// `isPluginEnabled`: the plugin's own switch if it has one; a switchless plugin is always on; else it is on unless it is
    /// in the list of plugins that are off (a plugin added later starts on).
    pub fn enabled(&self, prefs: &Prefs) -> bool {
        if self.own_switch {
            return prefs.get(MODS_KEY) == Some("true");
        }
        self.switchless || !plugin_is_off(prefs, self.id)
    }

    /// The areas it adds to, once each, in Fleet's order: what turning it on would add, so shown whether it is on or off.
    pub fn areas(&self) -> Vec<Area> {
        let set: BTreeSet<Area> = self.adds.iter().map(|a| a.area).collect();
        set.into_iter().collect()
    }

    /// What `GET /api/plugins` says about it, in the words of the page header: only while it is on.
    pub fn status_word(status: Option<&str>) -> Option<&'static str> {
        status.map(|s| match s {
            "connected" => "Connected",
            "error" => "Needs attention",
            _ => "Not connected",
        })
    }
}

/// The count beside Plugins in the menu: the plugins that are on (none of the three is `alwaysOn`).
pub fn count_on(prefs: &Prefs) -> usize {
    PLUGINS.iter().filter(|p| p.enabled(prefs)).count()
}

/// The menu's children under Plugins: one per plugin that is on and has a settings page.
pub fn children(prefs: &Prefs) -> Vec<&'static Plugin> {
    PLUGINS.iter().filter(|p| p.enabled(prefs)).collect()
}

/// `parseDisabledPlugins`: the ids in the stored list; anything that isn't a list of text is none.
pub fn parse_disabled(raw: Option<&str>) -> BTreeSet<String> {
    let list: Value = serde_json::from_str(raw.unwrap_or("[]")).unwrap_or(Value::Null);
    list.as_array().into_iter().flatten().filter_map(|id| id.as_str().map(str::to_string)).collect()
}

/// What `plugins.disabled` holds for these ids, as text.
pub fn disabled_json(ids: &BTreeSet<String>) -> String {
    serde_json::to_string(&ids.iter().collect::<Vec<_>>()).unwrap_or_else(|_| "[]".into())
}

/// The list with one plugin switched on or off and nothing else changed.
pub fn with_plugin(ids: &BTreeSet<String>, plugin: &str, enabled: bool) -> BTreeSet<String> {
    let mut next = ids.clone();
    if enabled {
        next.remove(plugin);
    } else {
        next.insert(plugin.to_string());
    }
    next
}

/// Two quick presses must not overwrite each other: each read-change-save runs after the one before it.
static WRITE_ORDER: Mutex<()> = Mutex::new(());

impl State {
    /// Whether a plugin's own page and rail are on now, as the preferences stand.
    pub fn plugin_enabled(&self, id: &str) -> bool {
        find(id).is_some_and(|p| p.enabled(&self.pickers.prefs))
    }

    /// A press on a plugin's switch: the opposite of what it shows. A switchless plugin has none.
    pub fn plugin_press(&mut self, id: &str) {
        let Some(plugin) = find(id) else { return };
        if plugin.switchless {
            return;
        }
        let enabled = plugin.enabled(&self.pickers.prefs);
        self.set_plugin_enabled(id, !enabled);
    }

    /// `setPluginEnabledEverywhere`. The Mods switch is its own preference; the others change the list of plugins that are off.
    /// The list is one preference, so saving this copy would undo what another device changed since it was read: the new list
    /// is made from what the server has now (Fleet #1052), changing only this plugin. The page changes at once.
    pub fn set_plugin_enabled(&mut self, id: &str, enabled: bool) {
        let Some(plugin) = find(id) else { return };
        if plugin.own_switch {
            // Turning Mods on asks about Bun first (`modsOwnSwitch.set`).
            self.mods_switch(enabled);
            return;
        }
        if plugin.switchless {
            return;
        }
        let key = DISABLED_PLUGINS_KEY;
        let local = parse_disabled(self.pickers.prefs.get(key));
        let optimistic = disabled_json(&with_plugin(&local, id, enabled));
        let before = self.pickers.prefs.set(key, &optimistic);
        self.settle_rail();
        let Some(link) = &self.link else { return };
        let (plugin_id, local_list) = (id.to_string(), local);
        link.off_thread(
            move |base| {
                let _turn = WRITE_ORDER.lock().unwrap_or_else(|e| e.into_inner());
                // A failed read leaves this copy to build from, as Fleet's `refresh()` does.
                let latest = super::super::http::preferences(base).ok();
                let served = latest.as_ref().and_then(|b| b.get(key)).and_then(Value::as_str).map(str::to_string);
                let base_ids = served.map_or(local_list, |raw| parse_disabled(Some(&raw)));
                let next = disabled_json(&with_plugin(&base_ids, &plugin_id, enabled));
                super::super::http::put_preference(base, key, &next).map(|()| next)
            },
            move |s: &mut State, tx: &mut Tx, result| {
                match result {
                    Ok(next) => {
                        s.pickers.prefs.set(key, &next);
                    }
                    Err(_) => {
                        s.pickers.prefs.roll_back(key, &optimistic, before);
                        s.load_preferences();
                    }
                }
                s.settle_rail();
                s.settle_pickers(tx);
            },
        );
    }

    /// The fold button on Plugins in the menu (`toggleFold`): folded or unfolded by hand, from anywhere.
    pub fn toggle_plugins_fold(&mut self) {
        let open = self.plugins_unfolded();
        self.settings.plugins_folded = Some(open);
    }

    /// Unfolded while one of its pages is open, unless folded by hand (`unfolded`).
    pub fn plugins_unfolded(&self) -> bool {
        use crate::fleet::settings_nav::Page;
        match self.settings.plugins_folded {
            Some(folded) => !folded,
            None => matches!(self.settings.page, Page::Plugins | Page::Plugin(_)),
        }
    }
}
