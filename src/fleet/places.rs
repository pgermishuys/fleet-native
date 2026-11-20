//! The places core offers and the plugins that fill them (`plugins/registry.ts`, `plugins/slots.ts`, `plugins/types.ts`).
//!
//! Fleet's core never imports a built-in plugin except in `main.ts`, which registers each manifest. Native holds to that
//! more strictly: a plugin is an `Entry` in `State.places`, made by `crate::plugins::register`, the only line outside
//! `src/plugins/` that names one. Core reads the entries: the rail asks for the rail items, the rail's context draws the
//! panel and the page, Settings › Plugins lists the manifests and draws a plugin's own page, `State::poll` runs each `keep`.
//! A plugin that is switched off (`plugins.disabled`) adds nothing: every question here takes the preferences and skips it.
//! A plugin's own state is a `Box<dyn Any + Send>` in its entry, so core never names its type.
use super::prefs::Prefs;
use super::rails::Placement;
use super::settings::plugins::{self, Plugin};
use crate::theme::Theme;
use crate::{State, Tx};
use rune::{El, Icon};
use std::any::Any;

/// A panel, a page or a settings body: drawn from the state like every other view.
pub type View = fn(&State, &Theme) -> El;
/// Run from `State::poll`; true when something drawn changed.
pub type Keep = fn(&mut State, f64, &mut Tx) -> bool;

/// A plugin's rail button (`sidebarItems`: `viewId`, `label`, `icon`, `defaultPath`, `order`).
#[derive(Clone, Copy)]
pub struct RailItem {
    pub label: &'static str,
    pub icon: &'static Icon,
    pub path: &'static str,
    pub order: i32,
    pub placement: Placement,
}

impl std::fmt::Debug for RailItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RailItem({:?}, {})", self.label, self.path)
    }
}

/// What one plugin adds.
pub struct Entry {
    pub manifest: Plugin,
    pub rail: Option<RailItem>,
    /// The rail's panel (`sidebarPanels`) and its page, drawn right of it.
    pub panel: Option<View>,
    pub page: Option<View>,
    /// The body of the plugin's page under Settings › Plugins (`configPage`).
    pub settings: Option<View>,
    pub keep: Option<Keep>,
    pub state: Box<dyn Any + Send>,
}

impl Entry {
    pub fn new(manifest: Plugin, state: impl Any + Send) -> Entry {
        Entry { manifest, rail: None, panel: None, page: None, settings: None, keep: None, state: Box::new(state) }
    }
}

/// Every registered plugin, in registration order.
#[derive(Default)]
pub struct Places {
    entries: Vec<Entry>,
}

impl Places {
    pub fn register(&mut self, entry: Entry) {
        self.entries.push(entry);
    }

    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.manifest.id == id)
    }

    pub fn state<T: Any>(&self, id: &str) -> Option<&T> {
        self.entry(id)?.state.downcast_ref()
    }

    pub fn state_mut<T: Any>(&mut self, id: &str) -> Option<&mut T> {
        self.entries.iter_mut().find(|e| e.manifest.id == id)?.state.downcast_mut()
    }

    /// The entries that are on, for the questions below.
    fn on<'a>(&'a self, prefs: &'a Prefs) -> impl Iterator<Item = &'a Entry> {
        self.entries.iter().filter(|e| e.manifest.enabled(prefs))
    }

    /// The rail buttons of the plugins that are on: id and item.
    pub fn rail_items(&self, prefs: &Prefs) -> Vec<(&'static str, RailItem)> {
        self.on(prefs).filter_map(|e| Some((e.manifest.id, e.rail?))).collect()
    }

    /// A plugin's rail button, whether the plugin is on or not (a rail that went away still has words for its notice).
    pub fn rail_item(&self, id: &str) -> Option<RailItem> {
        self.entry(id)?.rail
    }

    /// The panel and the page a plugin's rail draws, `None` while it is off or has no panel of its own.
    pub fn rail_views(&self, id: &str, prefs: &Prefs) -> Option<(Option<View>, View)> {
        let e = self.on(prefs).find(|e| e.manifest.id == id)?;
        Some((e.panel, e.page?))
    }

    /// The body of a plugin's page in Settings, `None` while it is off.
    pub fn settings_body(&self, id: &str, prefs: &Prefs) -> Option<View> {
        self.on(prefs).find(|e| e.manifest.id == id)?.settings
    }

    /// The `keep` of every plugin that is on.
    pub fn keeps(&self, prefs: &Prefs) -> Vec<Keep> {
        self.on(prefs).filter_map(|e| e.keep).collect()
    }

    /// The manifests Settings › Plugins lists, in `main.ts`' order: the registered plugins, then core's own.
    pub fn manifests(&self) -> Vec<Plugin> {
        self.entries.iter().map(|e| e.manifest).chain(plugins::CORE.iter().copied()).collect()
    }

    pub fn manifest(&self, id: &str) -> Option<Plugin> {
        self.manifests().into_iter().find(|p| p.id == id)
    }
}

impl State {
    /// Runs each plugin's `keep` (from `poll`). True when any changed what is drawn.
    pub fn keep_places(&mut self, now: f64, tx: &mut Tx) -> bool {
        let mut changed = false;
        for keep in self.places.keeps(&self.pickers.prefs) {
            changed |= keep(self, now, tx);
        }
        changed
    }
}
