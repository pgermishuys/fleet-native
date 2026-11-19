//! Settings → Skills & tools → Mods' calls and what they change (`ModsSection.vue`, `ModRow.vue`, `ModsShowcase.vue`,
//! `stores/mods.ts`). Every call goes to the machine Settings shows. Nothing here blocks: a thread sends and the answer comes
//! back to the UI thread. With no Fleet to ask, the tab reads the invented mods in `mock::mods`.
use super::mods_kept::{self as kept, KeptMod, ModsTab, Refused, Switch, View};
use super::skills::Tab;
use crate::fleet::events::{Event, Kind};
use crate::fleet::state::Topic;
use crate::{State, Tx};
use serde_json::Value;

/// What the first read brings back: the switch (none when it can't be read), and the list when the switch was on.
struct Read {
    switch: Option<Switch>,
    kept: Option<Result<Option<View>, Refused>>,
}

/// `load`: the switch, then the list if the switch is on. A 404 on the list means Mods are off, so nothing to show.
fn read_first(base: &str, preference: Option<&str>) -> Read {
    let switch = kept::read_switch(base);
    if !kept::switched_on(preference, switch) {
        return Read { switch, kept: None };
    }
    let list = match kept::read_view(base) {
        Ok(v) => Ok(Some(v)),
        Err(r) if r.status == 404 => Ok(None),
        Err(r) => Err(r),
    };
    Read { switch, kept: Some(list) }
}

impl State {
    /// Called from `keep_skills`: reads the tab when it is shown and hasn't read for this machine; forgets everything when it
    /// isn't shown, so coming back reads again.
    pub(super) fn keep_mods_tab(&mut self, key: &str) {
        if self.settings.skills.tab != Tab::Mods {
            if self.settings.skills.mods != ModsTab::default() {
                self.settings.skills.mods = ModsTab::default();
            }
            return;
        }
        if self.settings.skills.mods.loaded_for.as_deref() != Some(key) {
            self.settings.skills.mods.loaded_for = Some(key.to_string());
            self.load_mods_tab();
        }
    }

    /// `load` (on mount and Try again): the spinner replaces the list while it reads.
    pub fn load_mods_tab(&mut self) {
        let preference = self.pickers.prefs.get(super::skills::MODS_PREFERENCE_KEY).map(str::to_string);
        let Some(link) = self.skills_link() else {
            // No Fleet: the invented mods, so the tab, the scenes and the tests come out the same every time.
            let tab = &mut self.settings.skills.mods;
            tab.loading = false;
            tab.switch = Some(Switch { on: true, safe_mode: false });
            tab.kept = Some(crate::mock::mods::kept());
            return;
        };
        let tab = &mut self.settings.skills.mods;
        tab.seq += 1;
        let seq = tab.seq;
        tab.loading = true;
        tab.error = None;
        link.off_thread(
            move |base| read_first(base, preference.as_deref()),
            move |s: &mut State, _tx: &mut Tx, got: Read| {
                let tab = &mut s.settings.skills.mods;
                if tab.seq != seq {
                    return;
                }
                tab.loading = false;
                tab.switch = got.switch;
                match got.kept {
                    Some(Ok(view)) => tab.kept = view,
                    Some(Err(refused)) => tab.error = Some(refused.message),
                    None => {}
                }
            },
        );
    }

    /// Try again.
    pub fn retry_mods(&mut self) {
        self.load_mods_tab();
    }

    /// `loadKept` after a `mods.changed`: no spinner, and a failure leaves what is shown.
    fn reload_mods_kept(&mut self) {
        let Some(link) = self.skills_link() else { return };
        let seq = self.settings.skills.mods.seq;
        link.off_thread(kept::read_view, move |s: &mut State, _tx: &mut Tx, got: Result<View, Refused>| {
            let tab = &mut s.settings.skills.mods;
            if tab.seq != seq {
                return;
            }
            match got {
                Ok(view) => tab.kept = Some(view),
                Err(r) if r.status == 404 => tab.kept = None,
                Err(_) => {}
            }
        });
    }

    fn reload_mods_switch(&mut self) {
        let Some(link) = self.skills_link() else { return };
        let seq = self.settings.skills.mods.seq;
        link.off_thread(kept::read_switch, move |s: &mut State, _tx: &mut Tx, got: Option<Switch>| {
            if s.settings.skills.mods.seq != seq {
                return;
            }
            s.settings.skills.mods.switch = got;
            s.settings.skills.mods_server = got.map(|g| g.on);
        });
    }

    /// `mods.changed` on the `sessions` topic: `switch` reads the switch; `safe-mode` the switch and the list; a kept-list
    /// reason the list, but only once it was read once.
    pub(super) fn on_mods_kept_event(&mut self, topic: &Topic, event: &Event) {
        let Event::Other(Kind::ModsChanged, payload) = event else { return };
        if *topic != Topic::Sessions || self.settings.skills.tab != Tab::Mods || self.settings.skills.mods.loaded_for.is_none() {
            return;
        }
        match payload.get("reason").and_then(Value::as_str).unwrap_or_default() {
            "switch" => {
                self.load_preferences();
                self.reload_mods_switch();
            }
            "safe-mode" => {
                self.reload_mods_switch();
                if self.settings.skills.mods.kept.is_some() {
                    self.reload_mods_kept();
                }
            }
            reason if kept::is_kept_reason(reason) && self.settings.skills.mods.kept.is_some() => self.reload_mods_kept(),
            _ => {}
        }
    }

    /// A row's switch or its "Turn on again": `POST /api/mods/{name}/on` or `/off`; the row takes the answer.
    pub fn set_mod_on(&mut self, name: &str, on: bool) {
        self.run_mod(name, "on", if on { "/on" } else { "/off" });
    }

    /// Undo: `POST /api/mods/{name}/undo`, unless the server's rule says there is nothing to undo.
    pub fn undo_mod(&mut self, name: &str) {
        if self.settings.skills.mods.mods().iter().find(|m| m.name == name).is_some_and(|m| m.undo_refusal().is_some()) {
            return;
        }
        self.run_mod(name, "undo", "/undo");
    }

    /// `run`: one request per row at a time; a failure shows under the row.
    fn run_mod(&mut self, name: &str, what: &'static str, tail: &'static str) {
        let tab = &mut self.settings.skills.mods;
        if tab.row_busy(name) {
            return;
        }
        tab.busy.insert(name.to_string(), what);
        tab.row_errors.remove(name);
        let Some(link) = self.skills_link() else {
            // No Fleet: the invented mod changes the way the server's rule says.
            let tab = &mut self.settings.skills.mods;
            tab.busy.remove(name);
            if let Some(m) = tab.kept.as_mut().and_then(|k| k.mods.iter_mut().find(|m| m.name == name)) {
                apply_locally(m, what, tail);
            }
            return;
        };
        let (seq, owned) = (self.settings.skills.mods.seq, name.to_string());
        let sent = owned.clone();
        link.off_thread(
            move |base| kept::post_mod(base, &sent, tail),
            move |s: &mut State, _tx: &mut Tx, got: Result<KeptMod, Refused>| {
                let tab = &mut s.settings.skills.mods;
                if tab.seq != seq {
                    return;
                }
                tab.busy.remove(&owned);
                match got {
                    Ok(m) => tab.replace_mod(m),
                    Err(r) => {
                        tab.row_errors.insert(owned, r.message);
                    }
                }
            },
        );
    }

    /// "Turn mods back on" in the safe-mode callout: `PUT /api/mods/safe-mode` `{on: false}`, which also moves the switch.
    pub fn turn_mods_back_on(&mut self) {
        let tab = &mut self.settings.skills.mods;
        if tab.saving {
            return;
        }
        tab.saving = true;
        tab.error = None;
        let Some(link) = self.skills_link() else {
            let tab = &mut self.settings.skills.mods;
            tab.saving = false;
            if let Some(k) = tab.kept.as_mut() {
                k.safe_mode = false;
            }
            return;
        };
        let seq = self.settings.skills.mods.seq;
        link.off_thread(
            |base| kept::put_safe_mode(base, false),
            move |s: &mut State, _tx: &mut Tx, got: Result<View, Refused>| {
                let tab = &mut s.settings.skills.mods;
                if tab.seq != seq {
                    return;
                }
                tab.saving = false;
                match got {
                    Ok(view) => {
                        if let Some(sw) = tab.switch.as_mut() {
                            sw.safe_mode = view.safe_mode;
                        }
                        tab.kept = Some(view);
                    }
                    Err(r) => tab.error = Some(r.message),
                }
            },
        );
    }

    /// A sample's Add: `POST /api/mods/samples/{name}`. Nothing changes locally on success; the list reloads from the event.
    pub fn add_mod_sample(&mut self, name: &str) {
        let tab = &mut self.settings.skills.mods;
        if tab.adding.is_some() {
            return;
        }
        tab.adding = Some(name.to_string());
        tab.sample_error = None;
        let Some(link) = self.skills_link() else {
            // No Fleet: the sample is kept at once, as a version 1 on.
            let tab = &mut self.settings.skills.mods;
            tab.adding = None;
            if let Some(k) = tab.kept.as_mut() {
                k.mods.retain(|m| m.name != name);
                k.mods.push(KeptMod { name: name.into(), active: Some(1), active_version: Some("1.0.0".into()), ..Default::default() });
            }
            return;
        };
        let (seq, owned) = (self.settings.skills.mods.seq, name.to_string());
        let sent = owned.clone();
        link.off_thread(
            move |base| kept::add_sample(base, &sent),
            move |s: &mut State, _tx: &mut Tx, got: Result<(), String>| {
                let tab = &mut s.settings.skills.mods;
                if tab.seq != seq {
                    return;
                }
                tab.adding = None;
                if let Err(text) = got {
                    tab.sample_error = Some((owned, text));
                }
            },
        );
    }

    /// The link to "Plugins › Mods" (`setActiveSection('plugin:mods')`).
    pub fn open_mods_plugin(&mut self) {
        self.open_settings("plugin:mods", None);
    }
}

/// The server's rule, for the invented mods: Undo on vN (N > 1) brings back the previous version and turns the mod on; on v1
/// it turns the mod off.
fn apply_locally(m: &mut KeptMod, what: &str, tail: &str) {
    match what {
        "undo" => match m.active {
            Some(n) if n > 1 => {
                m.active = Some(n - 1);
                m.off = None;
            }
            _ => m.off = Some(super::mods_kept::Off { by: "user".into(), error: None }),
        },
        _ => m.off = if tail == "/on" { None } else { Some(super::mods_kept::Off { by: "user".into(), error: None }) },
    }
}
