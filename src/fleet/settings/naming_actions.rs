//! Settings → Folders → Worktrees' calls and what they change (`WorktreeNamingSection.vue`). The read and the save go to the
//! machine Settings shows (`State::settings_target`); the preview is local and never asks the server. Nothing here blocks: a
//! thread sends, and the answer comes back to the UI thread. With no Fleet the page works on Fleet's own defaults.
use super::naming_draft::{Field, Loaded, NamingPage, SAMPLE, SAVED_SECS};
use crate::fleet::settings_nav::Page;
use crate::{State, Tx};

impl State {
    /// Called from `poll`: while Folders is open, reads the user's layer when the page hasn't, or when the switch moved to
    /// another machine; leaving the page forgets it, so coming back reads again (`onMounted(load)`). Also ends "Saved".
    pub fn keep_naming(&mut self, now: f64, tx: &mut Tx) -> bool {
        if self.rail != crate::Rail::Settings || self.settings.page != Page::Folders {
            let had = self.settings.naming != NamingPage::default();
            self.settings.naming = NamingPage::default();
            return had;
        }
        if let Some(until) = self.settings.naming.saved_until {
            if now >= until {
                self.settings.naming.saved_until = None;
                return true;
            }
            tx.wake_after(std::time::Duration::from_millis(100));
        }
        let key = self.settings.choice.key(&self.machines).to_string();
        if self.settings.naming.loaded_for.as_deref() == Some(key.as_str()) {
            return false;
        }
        self.read_user_naming(key);
        true
    }

    fn read_user_naming(&mut self, key: String) {
        let request = self.settings.naming.request + 1;
        let mut page = NamingPage { loaded_for: Some(key.clone()), request, loading: true, ..NamingPage::default() };
        page.sample.replace_text(SAMPLE);
        self.settings.naming = page;
        let Some(link) = self.link_for_machine(&key).filter(|_| self.settings_target().is_some()) else {
            let page = &mut self.settings.naming;
            page.loading = false;
            page.adopt(Loaded::default());
            return;
        };
        link.off_thread(super::naming_draft::load, move |s: &mut State, _tx: &mut Tx, got: Result<Loaded, String>| {
            let page = &mut s.settings.naming;
            if page.request != request {
                return;
            }
            page.loading = false;
            match got {
                Ok(loaded) => {
                    page.error = None;
                    page.adopt(loaded);
                }
                Err(why) => page.error = Some(why),
            }
        });
    }

    /// A chip appends its token to the end of its field.
    pub fn insert_naming_token(&mut self, field: Field, name: &str) {
        self.settings.naming.insert_token(field, name);
    }

    /// Save: the five fields go to the machine shown, an empty one as `null`; the answer replaces the layers.
    pub fn save_naming(&mut self) {
        let page = &mut self.settings.naming;
        if page.saving {
            return;
        }
        page.saving = true;
        page.error = None;
        page.saved_until = None;
        let body = page.save_body();
        let request = page.request;
        let key = self.settings.choice.key(&self.machines).to_string();
        let Some(link) = self.link_for_machine(&key).filter(|_| self.settings_target().is_some()) else {
            // No Fleet to keep it: the fields stand as the user's own layer.
            let clock = self.clock;
            let page = &mut self.settings.naming;
            page.saving = false;
            page.saved_until = Some(clock + SAVED_SECS);
            return;
        };
        link.off_thread(
            move |base| super::naming_draft::save(base, &body),
            move |s: &mut State, _tx: &mut Tx, got: Result<Loaded, String>| {
                let clock = s.clock;
                let page = &mut s.settings.naming;
                if page.request != request {
                    return;
                }
                page.saving = false;
                match got {
                    Ok(loaded) => {
                        page.adopt(loaded);
                        page.saved_until = Some(clock + SAVED_SECS);
                    }
                    Err(why) => page.error = Some(why),
                }
            },
        );
    }

    /// Use Fleet's defaults: every field emptied, then saved.
    pub fn reset_naming(&mut self) {
        if self.settings.naming.saving {
            return;
        }
        self.settings.naming.clear_draft();
        self.save_naming();
    }
}
