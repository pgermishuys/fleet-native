//! What the app frame, the composer and the status bar know of Mods (component 22, piece K): the Mods switch and safe mode
//! behind the banner and the palette command (`stores/mods.ts` as the banner and `use-start-without-mods.ts` read it), and the
//! band and chip views (`composerBands`, `statusChips` of `lib/mods/points.ts`). Nothing here draws. The switch is the home
//! machine's, as in Fleet; Settings keeps its own copy for the machine it shows (`settings/mods_kept_actions.rs`).
use super::points::{ComposerBandView, HasTree, StatusChipView, Views};
use super::resolve::Resolved;
use super::rows::holds;
use super::types::{Action, Site, Surface};
use crate::fleet::events::{Event, Kind};
use crate::fleet::notices::{IconRef, Notice, Tone};
use crate::fleet::settings::mods_kept::{self as kept, Refused, Switch, View, switched_on};
use crate::fleet::settings::skills::MODS_PREFERENCE_KEY;
use crate::fleet::state::Topic;
use crate::{State, Tx};
use serde_json::Value;

/// The band and chip views of every session, and the safe-mode state.
#[derive(Default)]
pub struct ModSites {
    pub bands: Views<ComposerBandView>,
    pub chips: Views<StatusChipView>,
    /// `modsSwitch`: none until read (or when it can't be).
    pub switch: Option<Switch>,
    /// `kept.safeMode`, once the kept list was read or safe mode was set: it outranks the switch's word.
    pub kept_safe: Option<bool>,
    /// The switch has been asked for (`onMounted`'s `loadSwitch`, once while it is null).
    pub asked: bool,
    /// The banner's "Turn mods back on" is running, and what it was refused with.
    pub pending: bool,
    pub error: Option<String>,
}

/// The control keys the band and the chip draw their trees under.
pub const BAND_KEY: &str = "mod-composer-band-tree";
pub const CHIP_KEY: &str = "mod-status-chip-tree";

impl State {
    /// `safeMode`.
    pub fn mods_safe_mode(&self) -> bool {
        self.mod_sites.kept_safe.or(self.mod_sites.switch.map(|s| s.safe_mode)).unwrap_or(false)
    }

    /// `isSwitchedOn`: the user's choice as soon as it is made, else the switch's answer, else off.
    pub fn home_mods_on(&self) -> bool {
        switched_on(self.pickers.prefs.get(MODS_PREFERENCE_KEY), self.mod_sites.switch)
    }

    /// The banner shows while the switch is on and safe mode is on.
    pub fn mods_banner_shown(&self) -> bool {
        self.home_mods_on() && self.mods_safe_mode()
    }

    /// Called from `poll`: reads the switch once, when there is a server and nothing has (`onMounted`).
    pub fn keep_mods_sites(&mut self) -> bool {
        if self.mod_sites.asked || self.mod_sites.switch.is_some() {
            return false;
        }
        let Some(link) = self.link.clone() else { return false };
        self.mod_sites.asked = true;
        link.off_thread(kept::read_switch, |s: &mut State, _tx: &mut Tx, got: Option<Switch>| s.mod_sites.switch = got);
        true
    }

    /// `mods.changed` on the home machine's `sessions` topic: `switch` reads the switch again; `safe-mode` the switch and, when the
    /// kept list was read, its word on safe mode.
    pub(crate) fn on_mods_sites_event(&mut self, topic: &Topic, event: &Event) {
        let Event::Other(Kind::ModsChanged, payload) = event else { return };
        if *topic != Topic::Sessions {
            return;
        }
        let Some(link) = self.link.clone() else { return };
        match payload.get("reason").and_then(Value::as_str).unwrap_or_default() {
            "switch" => {
                self.load_preferences();
                link.off_thread(kept::read_switch, |s: &mut State, _tx: &mut Tx, got: Option<Switch>| s.mod_sites.switch = got);
            }
            "safe-mode" => {
                let had_kept = self.mod_sites.kept_safe.is_some();
                link.off_thread(
                    move |base| (kept::read_switch(base), had_kept.then(|| kept::read_view(base).ok()).flatten()),
                    |s: &mut State, _tx: &mut Tx, (switch, view): (Option<Switch>, Option<View>)| {
                        s.mod_sites.switch = switch;
                        if let Some(view) = view {
                            s.mod_sites.kept_safe = Some(view.safe_mode);
                        }
                    },
                );
            }
            _ => {}
        }
    }

    /// `setSafeMode`: `PUT /api/mods/safe-mode`; the answer is what the banner and the command follow. `banner` is the banner's
    /// own button, which keeps its state (running, refused) in the banner; the command says a refusal in a notice instead.
    pub fn set_mods_safe_mode(&mut self, on: bool, banner: bool) {
        if banner {
            if self.mod_sites.pending {
                return;
            }
            self.mod_sites.pending = true;
            self.mod_sites.error = None;
        }
        let Some(link) = self.link.clone() else {
            // No Fleet: the invented state flips at once.
            self.mod_sites.pending = false;
            self.mod_sites.kept_safe = Some(on);
            if let Some(sw) = self.mod_sites.switch.as_mut() {
                sw.safe_mode = on;
            }
            return;
        };
        link.off_thread(
            move |base| kept::put_safe_mode(base, on),
            move |s: &mut State, _tx: &mut Tx, got: Result<View, Refused>| {
                if banner {
                    s.mod_sites.pending = false;
                }
                match got {
                    Ok(view) => {
                        s.mod_sites.kept_safe = Some(view.safe_mode);
                        if let Some(sw) = s.mod_sites.switch.as_mut() {
                            sw.safe_mode = view.safe_mode;
                        }
                    }
                    Err(r) if banner => {
                        s.mod_sites.error = Some(if r.message.is_empty() { "Couldn't turn mods back on.".into() } else { r.message })
                    }
                    Err(r) => {
                        let body = if r.message.is_empty() { "Try again in a moment.".to_string() } else { r.message };
                        let id = format!("mods-safe-mode-failed-{}", (s.clock * 1000.0) as u64);
                        let mut n = Notice::new(id, "Couldn't change mods").expires(20.0);
                        n.body = Some(body);
                        n.icon = Some(IconRef(&crate::icons::POWER_OFF));
                        n.tone = Tone::Warn;
                        s.notices.post(n, s.clock);
                    }
                }
            },
        );
    }

    /// The palette command and the Help menu's item: flips safe mode.
    pub fn toggle_start_without_mods(&mut self) {
        let on = !self.mods_safe_mode();
        self.set_mods_safe_mode(on, false);
    }

    /// The session whose band and chip show: the open one (a scene's invented session has its own id).
    pub fn mod_site_session(&self) -> String {
        self.mod_row_session()
    }

    /// The band to draw above the composer: its own session's, with a valid tree that isn't `null`.
    pub fn composer_band(&self) -> Option<(std::sync::Arc<Value>, bool)> {
        let view = self.mod_sites.bands.get(&self.mod_site_session())?;
        match self.mod_resolver.resolve(Some(&view), Site::ComposerBand) {
            Resolved::Draws { tree, draft, .. } if !tree.is_null() => Some((tree, draft)),
            _ => None,
        }
    }

    /// The chip for the status bar: only while a session is on screen (the Sessions rail on a session's own route).
    pub fn status_chip(&self) -> Option<(std::sync::Arc<Value>, bool)> {
        if self.rail != crate::Rail::Sessions || !matches!(self.route, crate::fleet::store::Route::Session(_)) {
            return None;
        }
        let view = self.mod_sites.chips.get(&self.mod_site_session())?;
        match self.mod_resolver.resolve(Some(&view), Site::StatusChip) {
            Resolved::Draws { tree, draft, .. } if !tree.is_null() => Some((tree, draft)),
            _ => None,
        }
    }

    /// The controls of the band and the chip know their trees (`ModControls::sync`), as a row's body's do.
    pub fn sync_site_controls(&mut self) -> bool {
        let mut moved = false;
        for (key, tree) in [(BAND_KEY, self.composer_band()), (CHIP_KEY, self.status_chip())] {
            let seen = self.mods_ui.row_seen.get(key).cloned();
            match tree {
                Some((tree, _)) if seen.as_ref().is_some_and(|s| std::sync::Arc::ptr_eq(s, &tree)) => {}
                Some((tree, _)) => {
                    self.mods_ui.sync(key, &tree);
                    self.mods_ui.row_seen.insert(key.to_string(), tree);
                    moved = true;
                }
                None if seen.is_some() => {
                    self.mods_ui.clear(key);
                    self.mods_ui.row_seen.remove(key);
                    moved = true;
                }
                None => {}
            }
        }
        moved
    }

    /// A control of a band or a chip was used: its view's `on_action` hears it as `desktop`. False when no band or chip holds the handle.
    pub fn deliver_site_action(&self, action: &Action) -> bool {
        let find = |v: &super::points::ModTreeView| holds(&v.tree, &action.handle).then(|| v.on_action.clone());
        let sink = self
            .mod_sites
            .bands
            .items()
            .iter()
            .find_map(|v| find(v.base()))
            .or_else(|| self.mod_sites.chips.items().iter().find_map(|v| find(v.base())));
        match sink {
            Some(sink) => {
                if let Some(sink) = sink {
                    sink(action, Surface::Desktop);
                }
                true
            }
            None => false,
        }
    }
}
