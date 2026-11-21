//! The nightly row's calls and what they change (`DreamNightly.vue`: `load`, `save`, the picks). Every call goes to the machine
//! Settings shows. None blocks: a thread sends and the answer comes back to the UI thread. With no Fleet to ask the row shows
//! the invented answer in `mock::dreaming`, and a change is applied to it here.
use super::dream_nightly::{self, BAD_TIME, Change, Nightly, parse_nightly};
use super::memory;
use crate::composer::choice::{key_for_model, model_from_key, model_from_path, model_to_path};
use crate::fleet::http;
use crate::fleet::link::Link;
use crate::fleet::new_session::feeds::{Catalog, parse_catalog};
use crate::fleet::settings_nav::Page;
use crate::{State, Tx};

impl State {
    /// Harnesses the row offers, `(type, display name)`.
    pub fn nightly_harness_choices(&self) -> Vec<(String, String)> {
        dream_nightly::harness_choices(&self.dreaming_harnesses())
    }

    /// The harness the select shows.
    pub fn nightly_harness_shown(&self) -> String {
        let saved = self.settings.memory.nightly.nightly.as_ref().and_then(|n| n.harness_type.clone());
        dream_nightly::shown_harness(saved.as_deref(), &self.nightly_harness_choices())
    }

    /// The models of the harness shown, when its catalog is `supported`; else none and no picker.
    pub fn nightly_models(&self) -> Option<&Catalog> {
        self.settings.memory.nightly.catalog.as_ref().filter(|c| c.supported)
    }

    /// The key of the model saved (`provider/model` → selection key), empty for the harness's default.
    pub fn nightly_model_key(&self) -> String {
        let model = self.settings.memory.nightly.nightly.as_ref().and_then(|n| n.model.as_deref());
        model_from_path(model).map(|m| key_for_model(&m)).unwrap_or_default()
    }

    /// Called from `poll`: reads the row's answer when Memory is on and the page is read, drops it when Memory goes off, keeps
    /// the time box and its commit, and asks for the models of the harness shown. True when something on screen changed.
    pub fn keep_nightly(&mut self, tx: &mut Tx) -> bool {
        if self.rail != crate::Rail::Settings || self.settings.page != Page::Memory {
            return false;
        }
        if !self.settings.memory.enabled() {
            let had = self.settings.memory.nightly != Default::default();
            self.settings.memory.nightly = Default::default();
            return had;
        }
        let request = self.settings.memory.request;
        if self.settings.memory.nightly.asked != Some(request) {
            self.settings.memory.nightly.asked = Some(request);
            self.read_nightly();
            return true;
        }
        self.sync_nightly_harness();
        let mut changed = self.keep_nightly_time();
        changed |= self.keep_nightly_models(tx);
        changed
    }

    /// The time box shows the saved time until the user types; leaving it commits what was typed (a change event, as the
    /// browser's `type=time` sends one).
    fn keep_nightly_time(&mut self) -> bool {
        let page = &mut self.settings.memory.nightly;
        let Some(saved) = page.nightly.as_ref().map(|n| n.time.clone()) else { return false };
        let focused = page.time.focused;
        let left = page.time_was_focused && !focused;
        page.time_was_focused = focused;
        if left {
            self.nightly_commit_time();
            return true;
        }
        let page = &mut self.settings.memory.nightly;
        if !focused && page.time_shown != saved {
            page.time_shown = saved.clone();
            page.time.replace_text(saved);
            page.time_error = false;
            return true;
        }
        false
    }

    /// Asks for the models of the harness shown when it is not the one asked about.
    fn keep_nightly_models(&mut self, _tx: &mut Tx) -> bool {
        let on = self.settings.memory.nightly.nightly.as_ref().is_some_and(|n| n.enabled);
        let shown = self.nightly_harness_shown();
        let page = &mut self.settings.memory.nightly;
        if !on {
            return page.catalog_for.take().is_some() | page.catalog.take().is_some();
        }
        // While the harness list is being read again nothing is shown; the models stay until it says which harness it is.
        let harness = shown;
        if harness.is_empty() {
            return false;
        }
        if page.catalog_for.as_deref() == Some(harness.as_str()) {
            return false;
        }
        page.catalog_for = Some(harness.clone());
        page.catalog = None;
        let Some(link) = self.dreaming_link() else { return true };
        let request = self.settings.memory.request;
        let asked = harness.clone();
        link.off_thread(
            move |base| parse_catalog(http::harness_catalog(base, &asked, None, None)),
            move |s: &mut State, _, got: Result<Catalog, String>| {
                let page = &mut s.settings.memory.nightly;
                if s.settings.memory.request != request || page.catalog_for.as_deref() != Some(harness.as_str()) {
                    return;
                }
                // A harness that can't list its models offers no picker, as an unsupported catalog does.
                page.catalog = got.ok();
            },
        );
        true
    }

    /// `load`: the nightly answer. An answer for a page since read again, or for a machine no longer shown, is dropped.
    fn read_nightly(&mut self) {
        let Some(link) = self.dreaming_link() else {
            self.settings.memory.nightly.nightly = Some(crate::mock::dreaming::nightly());
            return;
        };
        let request = self.settings.memory.request;
        self.settings.memory.nightly.reading = true;
        let call = dream_nightly::get_nightly();
        self.nightly_call(&link, call, request, false);
    }

    /// Sends `call` and takes its answer as the row's view.
    fn nightly_call(&mut self, link: &Link, call: memory::Request, request: u64, saving: bool) {
        link.off_thread(
            move |base| {
                let body = memory::send(base, &call)?;
                parse_nightly(&body).ok_or_else(|| call.fallback.clone())
            },
            move |s: &mut State, _, got: Result<Nightly, String>| {
                if s.settings.memory.request != request {
                    return;
                }
                let page = &mut s.settings.memory.nightly;
                if saving {
                    page.saving = false;
                } else {
                    page.reading = false;
                }
                match got {
                    Ok(view) => {
                        page.nightly = Some(view);
                        page.error = None;
                    }
                    Err(e) => page.error = Some(if e.is_empty() { "Something went wrong.".into() } else { e }),
                }
            },
        );
    }

    /// Try again on the error that stands where the row would be (`@retry="load"`).
    pub fn nightly_retry(&mut self) {
        if self.settings.memory.nightly.reading {
            return;
        }
        self.settings.memory.nightly.error = None;
        self.read_nightly();
    }

    /// `save`: one change at a time; the zone goes with a switch or a time (`timeZone: localTimeZone()`).
    fn nightly_save(&mut self, mut change: Change, with_zone: bool) {
        let page = &mut self.settings.memory.nightly;
        if page.saving || page.nightly.is_none() {
            return;
        }
        page.error = None;
        let Some(link) = self.dreaming_link() else {
            apply_locally(self.settings.memory.nightly.nightly.as_mut().expect("an answer"), &change);
            return;
        };
        self.settings.memory.nightly.saving = true;
        let request = self.settings.memory.request;
        link.off_thread(
            move |base| {
                if with_zone {
                    change.time_zone = dream_nightly::local_time_zone();
                }
                let call = dream_nightly::set_nightly(&change);
                let body = memory::send(base, &call)?;
                parse_nightly(&body).ok_or(call.fallback)
            },
            move |s: &mut State, _, got: Result<Nightly, String>| {
                if s.settings.memory.request != request {
                    return;
                }
                let page = &mut s.settings.memory.nightly;
                page.saving = false;
                match got {
                    Ok(view) => page.nightly = Some(view),
                    Err(e) => page.error = Some(if e.is_empty() { "Something went wrong.".into() } else { e }),
                }
            },
        );
    }

    /// The switch (`toggle`).
    pub fn nightly_toggle(&mut self) {
        let on = self.settings.memory.nightly.nightly.as_ref().is_some_and(|n| n.enabled);
        self.nightly_save(Change { enabled: Some(!on), ..Default::default() }, true);
    }

    /// The time box was left or Enter pressed (`pickTime`): an empty box does nothing, a bad time is said, a good one is saved.
    pub fn nightly_commit_time(&mut self) {
        let page = &mut self.settings.memory.nightly;
        let typed = page.time.text.trim().to_string();
        let Some(saved) = page.nightly.as_ref().map(|n| n.time.clone()) else { return };
        if typed.is_empty() {
            page.time.replace_text(saved);
            page.time_error = false;
            return;
        }
        match dream_nightly::parse_time(&typed) {
            None => page.time_error = true,
            Some(time) => {
                page.time_error = false;
                page.time.replace_text(time.clone());
                if time != saved {
                    self.nightly_save(Change { time: Some(time), ..Default::default() }, true);
                }
            }
        }
    }

    /// The words under the time box, when what was typed is not a time.
    pub fn nightly_time_error(&self) -> Option<&'static str> {
        self.settings.memory.nightly.time_error.then_some(BAD_TIME)
    }

    /// A model belongs to its harness, so changing the harness clears it (`pickHarness`).
    pub fn nightly_pick_harness(&mut self, kind: &str) {
        self.nightly_save(Change { harness_type: Some(kind.to_string()), model: Some(None), ..Default::default() }, false);
    }

    /// `pickModel`: a selection key back to `provider/model`; "Default" (no key) clears it.
    pub fn nightly_pick_model(&mut self, key: &str) {
        let model = model_to_path(model_from_key(key).as_ref());
        self.nightly_save(Change { model: Some(model), ..Default::default() }, false);
    }

    pub fn nightly_pick_budget(&mut self, budget: i64) {
        self.nightly_save(Change { budget_tokens: Some(budget), ..Default::default() }, false);
    }

    /// Whether Settings shows the machine this app is on: the automation link is shown only there.
    pub fn settings_is_live(&self) -> bool {
        !self.settings.choice.is_other(&self.machines)
    }

    /// "Runs as the built-in automation Dreaming" (`openAutomation`): selects the automation the row names, then the Automations
    /// page opens on it (the link is the live machine's, and the page starts on it).
    pub fn open_dream_automation(&mut self) {
        let id = self.settings.memory.nightly.nightly.as_ref().and_then(|n| n.automation_id.clone());
        self.press_rail(crate::Rail::Automations);
        if let Some(id) = id {
            self.set_active_automation(&id);
        }
    }

    /// The harness Dream now follows: the row's, while the row is switched on (`emit("harness")`).
    pub fn sync_nightly_harness(&mut self) {
        let choices = self.nightly_harness_choices();
        let harness = self.settings.memory.nightly.harness(&choices).filter(|_| self.shows_dreaming());
        self.settings.memory.dreaming.nightly_harness = harness;
    }
}

/// With no Fleet, a change shows on the invented answer and goes nowhere.
fn apply_locally(view: &mut Nightly, change: &Change) {
    if let Some(on) = change.enabled {
        view.enabled = on;
    }
    if let Some(time) = &change.time {
        view.time = time.clone();
    }
    if let Some(harness) = &change.harness_type {
        view.harness_type = Some(harness.clone());
    }
    if let Some(model) = &change.model {
        view.model = model.clone();
    }
    if let Some(budget) = change.budget_tokens {
        view.budget_tokens = budget;
    }
}
