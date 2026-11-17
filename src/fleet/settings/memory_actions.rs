//! Settings → Memory's calls and what they change (`MemorySection.vue`: `load`, `toggle`, `run`). Every call goes to the machine
//! Settings shows (`State::settings_target`), never home by default. Nothing here blocks: a thread sends, and the answer comes
//! back to the UI thread. With no Fleet to ask the page reads the invented overview in `mock::memory` and the switch only
//! changes it.
use super::memory::{self, MemoryPage, Notes, Overview, Request};
use crate::fleet::settings_nav::Page;
use crate::{State, Tx};

/// What one read of the page brings back: whatever of the overview and the notes arrived, and why the rest didn't.
struct Read {
    overview: Option<Overview>,
    repository: Option<String>,
    notes: Option<Notes>,
    error: Option<String>,
}

fn ask<T>(base: &str, req: &Request, parse: impl Fn(&serde_json::Value) -> Option<T>) -> Result<T, String> {
    let body = memory::send(base, req)?;
    parse(&body).ok_or_else(|| req.fallback.clone())
}

/// The repository the page opens on: the one asked for if the machine lists it, else the first (`repository.value ??= …`).
fn choose(overview: &Overview, wanted: Option<String>) -> Option<String> {
    wanted.filter(|w| overview.repositories.iter().any(|r| &r.path == w)).or_else(|| overview.repositories.first().map(|r| r.path.clone()))
}

/// `load`: the overview, then the notes of the repository wanted (the one asked for by `openMemory` if the machine has notes
/// about it, else the first listed).
fn read(base: &str, wanted: Option<String>) -> Read {
    let overview = match ask(base, &memory::get_memory(), memory::parse_overview) {
        Ok(o) => o,
        Err(error) => return Read { overview: None, repository: None, notes: None, error: Some(error) },
    };
    let repository = choose(&overview, wanted);
    match ask(base, &memory::list_notes(repository.as_deref()), memory::parse_notes) {
        Ok(notes) => Read { overview: Some(overview), repository, notes: Some(notes), error: None },
        Err(error) => Read { overview: Some(overview), repository, notes: None, error: Some(error) },
    }
}

impl State {
    /// Called from `poll`: while Memory is open, asks the machine Settings shows when the page hasn't, or when the switch
    /// has moved to another machine; leaving the page forgets what it read, so coming back reads again (`onMounted(load)`).
    pub fn keep_memory(&mut self, _tx: &mut Tx) -> bool {
        if self.rail != crate::Rail::Settings || self.settings.page != Page::Memory {
            let had = self.settings.memory != MemoryPage::default() || self.settings.memory_ui != Default::default();
            self.settings.memory = MemoryPage::default();
            self.settings.memory_ui = Default::default();
            return had;
        }
        let cut = self.limit_memory_fields();
        let key = self.settings.choice.key(&self.machines).to_string();
        if self.settings.memory.loaded_for.as_deref() == Some(key.as_str()) {
            return cut;
        }
        self.load_memory(key);
        true
    }

    fn load_memory(&mut self, key: String) {
        let wanted = self.settings.repository.take();
        self.settings.memory_ui = Default::default();
        let page = &mut self.settings.memory;
        *page = MemoryPage { loaded_for: Some(key.clone()), request: page.request + 1, loading: true, ..MemoryPage::default() };
        let request = page.request;
        let Some(link) = self.link_for_machine(&key).filter(|_| self.settings_target().is_some()) else {
            // No Fleet: the invented notes, so the page, the scenes and the tests come out the same every time.
            let page = &mut self.settings.memory;
            page.loading = false;
            let overview = crate::mock::memory::overview();
            page.repository = choose(&overview, wanted);
            page.overview = Some(overview);
            page.notes = Some(crate::mock::memory::notes());
            return;
        };
        link.off_thread(
            move |base| read(base, wanted),
            move |s: &mut State, _tx: &mut Tx, got: Read| {
                let page = &mut s.settings.memory;
                if page.request != request {
                    return;
                }
                page.loading = false;
                page.overview = got.overview;
                page.repository = got.repository;
                page.notes = got.notes;
                page.error = got.error;
            },
        );
    }

    /// Starts one action (`run`): none while another is under way; clears the last failure.
    pub fn begin_memory_action(&mut self) -> bool {
        let page = &mut self.settings.memory;
        if page.saving {
            return false;
        }
        page.saving = true;
        page.error = None;
        true
    }

    /// The switch (`toggle`): `PUT /api/memory`, which replaces the overview. Memory is a preference on that machine, so
    /// when it is home the preferences the rest of the app reads follow (Dreaming shows whenever Memory is on).
    pub fn toggle_memory(&mut self) {
        let enabled = !self.settings.memory.enabled();
        if self.settings.memory.overview.is_none() || !self.begin_memory_action() {
            return;
        }
        let key = self.settings.choice.key(&self.machines).to_string();
        let request = self.settings.memory.request;
        let Some(link) = self.link_for_machine(&key).filter(|_| self.settings_target().is_some()) else {
            let page = &mut self.settings.memory;
            page.saving = false;
            if let Some(o) = page.overview.as_mut() {
                o.enabled = enabled;
            }
            self.settings.memory_ui.confirming_clear = false;
            return;
        };
        let home = self.settings_target().is_some_and(|t| t.is_home());
        link.off_thread(
            move |base| ask(base, &memory::set_enabled(enabled), memory::parse_overview),
            move |s: &mut State, _tx: &mut Tx, got: Result<Overview, String>| {
                if s.settings.memory.request != request {
                    return;
                }
                s.settings.memory.saving = false;
                match got {
                    Ok(overview) => {
                        s.settings.memory_ui.confirming_clear = false;
                        if home {
                            s.pickers.prefs.set(memory::MEMORY_PREFERENCE_KEY, if overview.enabled { "true" } else { "false" });
                        }
                        s.settings.memory.overview = Some(overview);
                    }
                    Err(e) => s.settings.memory.error = Some(if e.is_empty() { "Something went wrong.".into() } else { e }),
                }
            },
        );
    }

    /// `useDreamingFeature`: Dreaming shows whenever the Memory preference is on; it has no switch of its own, and a leftover
    /// `Dreaming` preference changes nothing.
    pub fn dreaming_enabled(&self) -> bool {
        super::switches::MEMORY.enabled(&self.pickers.prefs)
    }
}
