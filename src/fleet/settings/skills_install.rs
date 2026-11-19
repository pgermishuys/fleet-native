//! Skills → Catalog and Custom: reading the catalog and installing, from a card or from a typed address (`CatalogTab.vue`,
//! `CustomInstallTab.vue`, `use-skill-catalog.ts`). Every call goes to the machine Settings shows, on a thread. With no Fleet to
//! ask, the page reads the invented catalog in `mock::skills` and an install adds to the invented list.
use super::install_target::InstallTarget;
use super::skills::{self, Inner, InstalledSkill, SkillsPage};
use super::skills_catalog::{self, CatalogEntry};
use crate::{State, Tx};
use rune::TextEdit;
use serde_json::Value;

/// What an install thread hands back: the install's own answer and, when it worked, the list read again after it.
type Installed = Result<Result<Vec<InstalledSkill>, String>, String>;

fn install_then_read(base: &str, body: &Value) -> Installed {
    let answer = skills::send_with(base, &skills_catalog::install(), Some(body))?;
    if answer.is_null() {
        return Err("No data returned".into());
    }
    Ok(skills::parse_skills(&skills::send(base, &skills::list())?))
}

impl State {
    /// `useSkillCatalog`: reads the catalog (the tab reads it each time it opens, as Fleet's unmounted one does).
    fn load_skill_catalog(&mut self) {
        let page = &mut self.settings.skills;
        let (seq, read) = (page.seq, page.catalog_reads + 1);
        page.catalog_reads = read;
        page.catalog_error = None;
        let Some(link) = self.skills_link() else {
            self.settings.skills.catalog = crate::mock::skills::catalog();
            self.settings.skills.catalog_loading = false;
            return;
        };
        self.settings.skills.catalog_loading = true;
        link.off_thread(
            |base| skills_catalog::parse_catalog(&skills::send(base, &skills_catalog::catalog())?),
            move |s: &mut State, _tx: &mut Tx, got: Result<Vec<CatalogEntry>, String>| {
                let page = &mut s.settings.skills;
                if page.seq != seq || page.catalog_reads != read {
                    return;
                }
                page.catalog_loading = false;
                match got {
                    Ok(list) => page.catalog = list,
                    Err(e) => page.catalog_error = Some(e),
                }
            },
        );
    }

    /// What opening or leaving an inner tab does: Catalog reads the catalog and the list (to know what is installed here),
    /// and a tab that is left forgets its words, as Fleet's unmounted one does.
    pub(super) fn enter_skills_inner(&mut self, was: Inner, inner: Inner) {
        if was == inner {
            return;
        }
        let page = &mut self.settings.skills;
        page.install_error = None;
        page.catalog_error = None;
        page.custom_error = None;
        page.custom = TextEdit::default();
        if was == Inner::Catalog {
            page.catalog.clear();
            page.catalog_reads += 1;
            page.catalog_loading = false;
        }
        if inner == Inner::Catalog {
            page.catalog_loading = true;
            self.load_skill_catalog();
            self.reload_skills();
        }
    }

    /// The Install button of a catalog card (`handleInstall`).
    pub fn install_catalog_skill(&mut self, name: &str) {
        let page = &self.settings.skills;
        let Some(entry) = page.catalog.iter().find(|e| e.name == name).cloned() else { return };
        let (target, seq) = (page.target.clone(), page.seq);
        let page = &mut self.settings.skills;
        page.installing = true;
        page.installing_name = Some(name.to_string());
        page.install_error = None;
        let Some(link) = self.skills_link() else {
            self.add_mock_install(&entry.name, &entry, &target);
            let page = &mut self.settings.skills;
            page.installing = false;
            page.installing_name = None;
            return;
        };
        let body = entry.install_body(&target);
        link.off_thread(
            move |base| install_then_read(base, &body),
            move |s: &mut State, _tx: &mut Tx, got: Installed| {
                let page = &mut s.settings.skills;
                if page.seq != seq {
                    return;
                }
                page.installing = false;
                page.installing_name = None;
                match got {
                    Ok(list) => apply_list(page, list),
                    Err(e) => page.install_error = Some(e),
                }
            },
        );
    }

    /// `submitInstall` of the Custom tab: "Install skill" or Enter in the field.
    pub fn install_custom_skill(&mut self) {
        let page = &mut self.settings.skills;
        if page.installing {
            return;
        }
        let url = page.custom.text.trim().to_string();
        if url.is_empty() {
            page.custom_error = Some("Skill URL or path is required.".into());
            return;
        }
        let target = page.target.clone();
        let body = match skills_catalog::custom_install_body(&url, &target) {
            Ok(b) => b,
            Err(e) => {
                // `isInstalling` is on while the address is read, and the reason is the form's error.
                page.custom_error = Some(e);
                return;
            }
        };
        page.installing = true;
        page.custom_error = None;
        let seq = page.seq;
        let Some(link) = self.skills_link() else {
            let name = body["name"].as_str().unwrap_or("").to_string();
            let page = &mut self.settings.skills;
            page.installing = false;
            page.custom = TextEdit::default();
            if !page.skills.iter().any(|s| s.name == name && s.target.is_same(&target)) {
                page.skills.push(crate::mock::skills::from_address(&name, &url, target));
            }
            return;
        };
        link.off_thread(
            move |base| install_then_read(base, &body),
            move |s: &mut State, _tx: &mut Tx, got: Installed| {
                let page = &mut s.settings.skills;
                if page.seq != seq {
                    return;
                }
                page.installing = false;
                match got {
                    Ok(list) => {
                        page.custom = TextEdit::default();
                        apply_list(page, list);
                    }
                    Err(e) => page.custom_error = Some(e),
                }
            },
        );
    }

    fn add_mock_install(&mut self, name: &str, entry: &CatalogEntry, target: &InstallTarget) {
        let page = &mut self.settings.skills;
        if !skills_catalog::is_installed(&page.skills, name, target) {
            page.skills.push(crate::mock::skills::from_entry(entry, target.clone()));
        }
    }
}

/// The list read after an install: a failed read is the list's own error, not the install's.
fn apply_list(page: &mut SkillsPage, list: Result<Vec<InstalledSkill>, String>) {
    match list {
        Ok(list) => page.skills = list,
        Err(e) => page.error = Some(e),
    }
}
