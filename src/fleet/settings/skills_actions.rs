//! Settings → Skills & tools' calls and what they change (`SkillsToolsPage.vue`, `InstalledSkillsTab.vue`, `use-skills.ts`).
//! Every call goes to the machine Settings shows (`State::settings_target`). Nothing here blocks: a thread sends and the
//! answer comes back to the UI thread. With no Fleet to ask, the page reads the invented skills in `mock::skills` and the
//! buttons change them.
use super::install_target::InstallTarget;
use super::skills::{self, InstalledSkill, SkillsPage, Tab};
use crate::fleet::http;
use crate::fleet::new_session::feeds::{self, Repository};
use crate::fleet::settings_nav::Page;
use crate::{State, Tx};
use serde_json::Value;

fn read_list(base: &str) -> Result<Vec<InstalledSkill>, String> {
    skills::parse_skills(&skills::send(base, &skills::list())?)
}

/// `fetchModsSwitch`: `{on}`; any failure is "no answer" (a 404 is Mods off).
fn read_switch(base: &str) -> Option<bool> {
    http::get(base, "/api/features/mods").ok().and_then(|v| v.get("on").and_then(Value::as_bool))
}

fn read_repositories(base: &str) -> Vec<Repository> {
    let answer = http::text_with("GET", base, "/api/repositories", None);
    feeds::parse_scan(answer).map(|scan| scan.repositories).unwrap_or_default()
}

impl State {
    /// The Mods preference this app holds (the user's choice in Labs): "true" or "false" once they have made it.
    fn mods_preference(&self) -> Option<String> {
        self.pickers.prefs.get(skills::MODS_PREFERENCE_KEY).map(str::to_string)
    }

    /// Whether the Mods tab exists.
    pub fn skills_mods_on(&self) -> bool {
        self.settings.skills.mods_on(self.mods_preference().as_deref())
    }

    /// Called from `poll`: while Skills & tools is open, asks the machine Settings shows when the page hasn't, or when the
    /// switch has moved to another machine; picks the tab from the anchor when that changes. Leaving the page forgets what
    /// it read (each tab reads on opening), so coming back starts on Skills and Installed again.
    pub fn keep_skills(&mut self, _tx: &mut Tx) -> bool {
        if self.rail != crate::Rail::Settings || self.settings.page != Page::Skills {
            let had = self.settings.skills != SkillsPage::default();
            self.settings.skills = SkillsPage::default();
            return had;
        }
        let before = self.settings.skills.clone();
        let (anchor, pref) = (self.settings.anchor.clone(), self.mods_preference());
        let page = &mut self.settings.skills;
        if page.anchor_seen != anchor {
            page.tab = page.tab_for(anchor.as_deref(), pref.as_deref());
            page.anchor_seen = anchor.clone();
        }
        page.settle_mods(anchor.as_deref(), pref.as_deref());
        let key = self.settings.choice.key(&self.machines).to_string();
        if self.settings.skills.loaded_for.as_deref() != Some(key.as_str()) {
            self.load_skills_page(key);
            return true;
        }
        self.settings.skills != before
    }

    /// The page's first read: the list, the repositories the picker offers and the Mods switch, each on its own thread.
    fn load_skills_page(&mut self, key: String) {
        let seq = self.settings.skills.seq + 1;
        let target = self.settings.skills.target.clone();
        let (tab, anchor_seen, mods_seen_on) = {
            let p = &self.settings.skills;
            (p.tab, p.anchor_seen.clone(), p.mods_seen_on)
        };
        self.settings.skills =
            SkillsPage { loaded_for: Some(key.clone()), tab, anchor_seen, mods_seen_on, seq, target, ..Default::default() };
        let Some(link) = self.link_for_machine(&key).filter(|_| self.settings_target().is_some()) else {
            // No Fleet: the invented skills, so the page, the scenes and the tests come out the same every time.
            let page = &mut self.settings.skills;
            page.skills = crate::mock::skills::installed();
            page.repositories = crate::mock::skills::repositories();
            return;
        };
        self.settings.skills.loading = true;
        self.settings.skills.repos_loading = true;
        self.read_skill_list(&link, seq);
        link.off_thread(read_repositories, move |s: &mut State, _tx: &mut Tx, repos: Vec<Repository>| {
            let page = &mut s.settings.skills;
            if page.seq != seq {
                return;
            }
            page.repos_loading = false;
            page.repositories = repos.into_iter().map(|r| (r.path, r.name)).collect();
        });
        link.off_thread(read_switch, move |s: &mut State, _tx: &mut Tx, on: Option<bool>| {
            if s.settings.skills.seq == seq {
                s.settings.skills.mods_server = on;
            }
        });
    }

    /// `fetchSkills`: the list again. The spinner replaces the list while it reads, as Fleet's does.
    fn read_skill_list(&mut self, link: &crate::fleet::link::Link, seq: u64) {
        let page = &mut self.settings.skills;
        page.loading = true;
        page.error = None;
        let read = page.reads + 1;
        page.reads = read;
        link.off_thread(read_list, move |s: &mut State, _tx: &mut Tx, got: Result<Vec<InstalledSkill>, String>| {
            let page = &mut s.settings.skills;
            if page.seq != seq || page.reads != read {
                return;
            }
            page.loading = false;
            match got {
                Ok(list) => page.skills = list,
                Err(e) => page.error = Some(e),
            }
        });
    }

    fn skills_link(&self) -> Option<crate::fleet::link::Link> {
        let key = self.settings.choice.key(&self.machines).to_string();
        self.link_for_machine(&key).filter(|_| self.settings_target().is_some())
    }

    /// A tab of the page (the Mods tab only while its switch is on).
    pub fn pick_skills_tab(&mut self, tab: Tab) {
        if tab == Tab::Mods && !self.skills_mods_on() {
            return;
        }
        self.settings.skills.tab = tab;
    }

    /// An inner tab of Skills. Installed reads the list each time it is opened, as Fleet's unmounted tab does.
    pub fn pick_skills_inner(&mut self, inner: skills::Inner) {
        let was = self.settings.skills.inner;
        self.settings.skills.inner = inner;
        self.settings.skills.confirm_remove = None;
        if inner == skills::Inner::Installed && was != inner {
            self.reload_skills();
        }
    }

    /// Reads the list again (the Installed tab opening).
    pub fn reload_skills(&mut self) {
        let seq = self.settings.skills.seq;
        // With no Fleet the invented list is already there and the buttons keep it up to date.
        if let Some(link) = self.skills_link() {
            self.read_skill_list(&link, seq);
        }
    }

    /// The picker's choice, shared by Catalog and Custom and kept while you move between them.
    pub fn choose_skills_target(&mut self, key: &str) {
        self.settings.skills.target = InstallTarget::from_key(key);
    }

    /// `handleCheckUpdate`: asks whether a GitHub install has an update. A failure puts its words on the card and under the list.
    pub fn check_skill_update(&mut self, key: &str) {
        let Some(skill) = self.settings.skills.skills.iter().find(|s| s.key() == key).cloned() else { return };
        let page = &mut self.settings.skills;
        if page.checking.is_some() || page.updating.as_deref() == Some(key) {
            return;
        }
        page.checking = Some(key.to_string());
        page.error = None;
        let (seq, key) = (page.seq, key.to_string());
        let Some(link) = self.skills_link() else {
            let page = &mut self.settings.skills;
            page.checking = None;
            let available = crate::mock::skills::has_update(&skill.name);
            page.patch(&skill.name, &skill.target, |s| {
                s.update_available = available;
                s.update_check_error = None;
            });
            return;
        };
        let request = skills::check_update(&skill.name, &skill.target);
        link.off_thread(
            move |base| skills::send(base, &request).and_then(|v| skills::parse_update_check(&v)),
            move |s: &mut State, _tx: &mut Tx, got: Result<bool, String>| {
                let page = &mut s.settings.skills;
                if page.seq != seq {
                    return;
                }
                if page.checking.as_deref() == Some(key.as_str()) {
                    page.checking = None;
                }
                match got {
                    Ok(available) => page.patch(&skill.name, &skill.target, |c| {
                        c.update_available = available;
                        c.update_check_error = None;
                    }),
                    Err(e) => {
                        page.patch(&skill.name, &skill.target, |c| c.update_check_error = Some(e.clone()));
                        page.error = Some(e);
                    }
                }
            },
        );
    }

    /// `handleUpdate`: updates one install, then reads the list again.
    pub fn update_skill(&mut self, key: &str) {
        let Some(skill) = self.settings.skills.skills.iter().find(|s| s.key() == key).cloned() else { return };
        let page = &mut self.settings.skills;
        if page.updating.is_some() {
            return;
        }
        page.updating = Some(key.to_string());
        page.error = None;
        let (seq, key) = (page.seq, key.to_string());
        let Some(link) = self.skills_link() else {
            let page = &mut self.settings.skills;
            page.updating = None;
            page.patch(&skill.name, &skill.target, |s| s.update_available = false);
            return;
        };
        let request = skills::update(&skill.name, &skill.target);
        link.off_thread(
            move |base| {
                let body = skills::send(base, &request)?;
                if body.is_null() { Err("No data returned".to_string()) } else { Ok(()) }
            },
            move |s: &mut State, _tx: &mut Tx, got: Result<(), String>| {
                if s.settings.skills.seq != seq {
                    return;
                }
                if s.settings.skills.updating.as_deref() == Some(key.as_str()) {
                    s.settings.skills.updating = None;
                }
                match got {
                    Ok(()) => s.reload_skills(),
                    Err(e) => s.settings.skills.error = Some(e),
                }
            },
        );
    }

    /// The Remove button: the first press asks to confirm (one at a time: another card's press moves it), the second removes.
    pub fn press_remove_skill(&mut self, key: &str) {
        let page = &mut self.settings.skills;
        if page.removing.is_some() {
            return;
        }
        if page.confirm_remove.as_deref() != Some(key) {
            page.confirm_remove = Some(key.to_string());
            return;
        }
        self.remove_skill(key);
    }

    pub fn cancel_remove_skill(&mut self) {
        self.settings.skills.confirm_remove = None;
    }

    /// `handleRemove`: `DELETE`, then the list again. A failure keeps the confirm state and shows under the list.
    fn remove_skill(&mut self, key: &str) {
        let Some(skill) = self.settings.skills.skills.iter().find(|s| s.key() == key).cloned() else { return };
        let page = &mut self.settings.skills;
        page.removing = Some(key.to_string());
        page.error = None;
        let seq = page.seq;
        let Some(link) = self.skills_link() else {
            let page = &mut self.settings.skills;
            page.removing = None;
            page.confirm_remove = None;
            page.skills.retain(|s| !(s.name == skill.name && s.target.is_same(&skill.target)));
            return;
        };
        let request = skills::remove(&skill.name, &skill.target);
        link.off_thread(
            move |base| skills::send(base, &request).map(drop),
            move |s: &mut State, _tx: &mut Tx, got: Result<(), String>| {
                if s.settings.skills.seq != seq {
                    return;
                }
                s.settings.skills.removing = None;
                match got {
                    Ok(()) => {
                        s.settings.skills.confirm_remove = None;
                        s.reload_skills();
                    }
                    Err(e) => s.settings.skills.error = Some(e),
                }
            },
        );
    }
}
