//! Settings → Folders → Locations' calls and what they change (`WorkspaceSection.vue`). Every call goes to the machine
//! Settings shows (`State::settings_target`). Nothing here blocks: a thread sends, and the answer comes back to the UI thread.
//! With no Fleet to ask the page works on the invented list in `mock::locations`.
use super::locations::{self, Browse, LocationsPage, Op, Outcome};
use crate::fleet::settings_nav::Page;
use crate::ui::Open;
use crate::{State, Tx};

/// The key the Browse button and its popover share.
pub const BROWSE_KEY: &str = "locations-browse";

impl State {
    /// Called from `poll`: while Folders is open, reads the list when the page hasn't, or when the switch moved to another
    /// machine; leaving the page forgets it, so coming back reads again (`onMounted`).
    pub fn keep_locations(&mut self, _tx: &mut Tx) -> bool {
        if self.rail != crate::Rail::Settings || self.settings.page != Page::Folders {
            let had = self.settings.locations != LocationsPage::default();
            self.settings.locations = LocationsPage::default();
            if self.ui.open == Some(Open::Popover(BROWSE_KEY)) {
                self.ui.open = None;
            }
            return had;
        }
        let key = self.settings.choice.key(&self.machines).to_string();
        if self.settings.locations.loaded_for.as_deref() == Some(key.as_str()) {
            return false;
        }
        self.settings.locations =
            LocationsPage { loaded_for: Some(key), request: self.settings.locations.request + 1, ..LocationsPage::default() };
        self.read_locations(Op::Load);
        true
    }

    /// Sends `op` to the machine shown, or applies it to the invented list when there is none.
    fn read_locations(&mut self, op: Op) {
        let key = self.settings.choice.key(&self.machines).to_string();
        let request = self.settings.locations.request;
        self.settings.locations.loading = matches!(op, Op::Load);
        let Some(link) = self.link_for_machine(&key).filter(|_| self.settings_target().is_some()) else {
            let page = &mut self.settings.locations;
            if matches!(op, Op::Load) && !page.refreshing {
                page.roots = crate::mock::locations::roots();
            }
            match &op {
                Op::Load => {}
                Op::Add(path) => page.roots.push(locations::Root {
                    id: Some(format!("loc-{}", page.roots.len() + 1)),
                    path: path.clone(),
                    env: false,
                    exists: true,
                }),
                Op::Remove(id) => page.roots.retain(|r| r.id.as_deref() != Some(id)),
            }
            let outcome = Outcome { refused: None, roots: Some(Ok(page.roots.clone())) };
            self.apply_locations(&op, outcome);
            return;
        };
        let sent = op.clone();
        link.off_thread(
            move |base| locations::run(base, &sent),
            move |s: &mut State, _tx: &mut Tx, got: Outcome| {
                if s.settings.locations.request == request {
                    s.apply_locations(&op, got);
                }
            },
        );
    }

    fn apply_locations(&mut self, op: &Op, got: Outcome) {
        let page = &mut self.settings.locations;
        page.loading = false;
        page.refreshing = false;
        match op {
            Op::Load => {}
            Op::Add(_) => {
                page.adding = false;
                match &got.refused {
                    Some(why) => page.add_error = Some(why.clone()),
                    None => page.path.clear(),
                }
            }
            Op::Remove(_) => {
                page.removing = None;
                if let Some(why) = &got.refused {
                    page.error = Some(why.clone());
                }
            }
        }
        match got.roots {
            Some(Ok(roots)) => {
                page.roots = roots;
                page.error = None;
            }
            Some(Err(why)) => page.error = Some(why),
            None => {}
        }
    }

    /// Refresh: reads the list again (it doesn't scan; adding and removing do).
    pub fn refresh_locations(&mut self) {
        let page = &mut self.settings.locations;
        if page.loading || page.refreshing {
            return;
        }
        page.refreshing = true;
        self.read_locations(Op::Load);
    }

    /// Add location: an empty box says so; else the trimmed path is added.
    pub fn add_location(&mut self) {
        let page = &mut self.settings.locations;
        if page.adding {
            return;
        }
        let path = page.path.text.trim().to_string();
        if path.is_empty() {
            page.add_error = Some(locations::EMPTY_PATH.into());
            return;
        }
        page.adding = true;
        page.add_error = None;
        self.read_locations(Op::Add(path));
    }

    pub fn remove_location(&mut self, id: &str) {
        let page = &mut self.settings.locations;
        if page.removing.is_some() {
            return;
        }
        page.removing = Some(id.to_string());
        self.read_locations(Op::Remove(id.to_string()));
    }

    /// Browse: opens the popover on the machine's roots.
    pub fn open_location_browser(&mut self) {
        self.ui.open = Some(Open::Popover(BROWSE_KEY));
        self.browse_locations(None);
    }

    /// Lists the folders in `path` (the roots for none). An answer to an older ask is dropped.
    pub fn browse_locations(&mut self, path: Option<String>) {
        let page = &mut self.settings.locations;
        let search = page.browse.take().map(|b| b.search).unwrap_or_default();
        let request = page.request;
        let mut b = Browse { loading: true, ..Browse::default() };
        b.search = search;
        b.search.clear();
        b.current = path.clone();
        page.browse = Some(b);
        let key = self.settings.choice.key(&self.machines).to_string();
        let Some(link) = self.link_for_machine(&key).filter(|_| self.settings_target().is_some()) else {
            let shown = crate::mock::locations::browse(path.as_deref());
            if let Some(b) = self.settings.locations.browse.as_mut() {
                b.loading = false;
                b.current = shown.current;
                b.entries = shown.entries;
                b.roots = shown.roots;
            }
            return;
        };
        link.off_thread(
            move |base| locations::browse(base, path.as_deref()),
            move |s: &mut State, _tx: &mut Tx, got: Result<Browse, String>| {
                let page = &mut s.settings.locations;
                if page.request != request {
                    return;
                }
                let Some(b) = page.browse.as_mut() else { return };
                b.loading = false;
                match got {
                    Ok(shown) => {
                        b.current = shown.current;
                        b.entries = shown.entries;
                        b.roots = shown.roots;
                        b.error = None;
                    }
                    Err(why) => b.error = Some(why),
                }
            },
        );
    }

    /// Select, or Use this directory: the path goes in the box and the popover closes.
    pub fn pick_location_folder(&mut self, path: &str) {
        self.settings.locations.path.replace_text(path);
        self.settings.locations.browse = None;
        self.ui.open = None;
    }
}
