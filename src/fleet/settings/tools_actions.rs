//! Settings → Skills & tools → Tools' calls and what they change (`ToolsSection.vue`, `tools/InstalledToolsTab.vue`,
//! `tools/CatalogTab.vue`, `use-tools.ts`, `use-tool-catalog.ts`). Every call goes to the machine Settings shows. Nothing here
//! blocks: a thread sends and the answer comes back to the UI thread. With no Fleet to ask, the tab reads the invented tools
//! in `mock::settings_tools` and the buttons change them.
use super::install_target::InstallTarget;
use super::skills::{self, Inner, Tab};
use super::tool_custom::{self, Mode};
use super::tools::{self, CatalogEntry, InstalledTool, ToolsPage};
use crate::fleet::new_session::feeds::{self, Repository};
use crate::{State, Tx};
use serde_json::Value;

fn read_list(base: &str) -> Result<Vec<InstalledTool>, String> {
    tools::parse_tools(&skills::send(base, &tools::list())?)
}

fn read_repositories(base: &str) -> Vec<Repository> {
    let answer = crate::fleet::http::text_with("GET", base, "/api/repositories", None);
    feeds::parse_scan(answer).map(|scan| scan.repositories).unwrap_or_default()
}

/// What an install thread hands back: the install's own answer and, when it worked, the list read again after it.
pub(super) type Installed = Result<Result<Vec<InstalledTool>, String>, String>;

pub(super) fn install_then_read(base: &str, body: &Value) -> Installed {
    let answer = skills::send_with(base, &tools::install(), Some(body))?;
    if answer.is_null() {
        return Err("No data returned".into());
    }
    Ok(read_list(base))
}

impl State {
    /// Called from `keep_skills` while the page is open: the Tools tab reads when it becomes the one showing (and again when
    /// the page's machine moved), and forgets what it read when it is left, as Fleet's unmounted section does.
    pub(super) fn keep_tools(&mut self) -> bool {
        let skills = &self.settings.skills;
        let showing = skills.tab == Tab::Tools && skills.loaded_for.is_some();
        let seq = skills.seq;
        let page = &self.settings.tools;
        if !showing {
            let had = *page != ToolsPage::default();
            self.settings.tools = ToolsPage::default();
            return had;
        }
        if page.active && page.for_seq == seq {
            return false;
        }
        let target = if page.active { page.target.clone() } else { InstallTarget::Global };
        self.settings.tools = ToolsPage { active: true, for_seq: seq, target, ..Default::default() };
        self.load_tools_page();
        true
    }

    /// The tab's first read: the list and the repositories the picker offers, each on its own thread.
    fn load_tools_page(&mut self) {
        let seq = self.settings.skills.seq;
        let Some(link) = self.skills_link() else {
            // No Fleet: the invented tools, so the page, the scenes and the tests come out the same every time.
            let page = &mut self.settings.tools;
            page.tools = crate::mock::settings_tools::installed();
            page.repositories = crate::mock::skills::repositories();
            return;
        };
        self.settings.tools.repos_loading = true;
        self.read_tool_list(&link);
        link.off_thread(read_repositories, move |s: &mut State, _tx: &mut Tx, repos: Vec<Repository>| {
            let page = &mut s.settings.tools;
            if page.for_seq != seq {
                return;
            }
            page.repos_loading = false;
            page.repositories = repos.into_iter().map(|r| (r.path, r.name)).collect();
        });
    }

    /// `fetchTools`: the list again. The spinner replaces the list while it reads, as Fleet's does.
    fn read_tool_list(&mut self, link: &crate::fleet::link::Link) {
        let page = &mut self.settings.tools;
        page.loading = true;
        page.error = None;
        let (seq, read) = (page.for_seq, page.reads + 1);
        page.reads = read;
        link.off_thread(read_list, move |s: &mut State, _tx: &mut Tx, got: Result<Vec<InstalledTool>, String>| {
            let page = &mut s.settings.tools;
            if page.for_seq != seq || page.reads != read {
                return;
            }
            page.loading = false;
            match got {
                Ok(list) => page.tools = list,
                Err(e) => page.error = Some(e),
            }
        });
    }

    /// Reads the list again (the Installed tab opening).
    pub fn reload_tools(&mut self) {
        if let Some(link) = self.skills_link() {
            self.read_tool_list(&link);
        }
    }

    /// An inner tab of Tools. Installed reads the list each time it is opened; Catalog reads the catalog and the list.
    pub fn pick_tools_inner(&mut self, inner: Inner) {
        let was = self.settings.tools.inner;
        let page = &mut self.settings.tools;
        page.inner = inner;
        page.confirm_remove = None;
        if was == inner {
            return;
        }
        page.install_error = None;
        page.catalog_error = None;
        // A tab that is left forgets its words, as Fleet's unmounted one does.
        page.custom_error = None;
        page.mode = Mode::default();
        for field in [&mut page.native_url, &mut page.mcp_name, &mut page.mcp_command, &mut page.mcp_args, &mut page.mcp_env] {
            *field = rune::TextEdit::default();
        }
        if was == Inner::Catalog {
            page.catalog.clear();
            page.catalog_reads += 1;
            page.catalog_loading = false;
        }
        if inner == Inner::Catalog {
            self.load_tool_catalog();
        }
        if inner != Inner::Custom {
            self.reload_tools();
        }
    }

    /// `useToolCatalog`: reads the catalog (the tab reads it each time it opens).
    fn load_tool_catalog(&mut self) {
        let page = &mut self.settings.tools;
        let (seq, read) = (page.for_seq, page.catalog_reads + 1);
        page.catalog_reads = read;
        page.catalog_error = None;
        let Some(link) = self.skills_link() else {
            self.settings.tools.catalog = crate::mock::settings_tools::catalog();
            self.settings.tools.catalog_loading = false;
            return;
        };
        self.settings.tools.catalog_loading = true;
        link.off_thread(
            |base| tools::parse_catalog(&skills::send(base, &tools::catalog())?),
            move |s: &mut State, _tx: &mut Tx, got: Result<Vec<CatalogEntry>, String>| {
                let page = &mut s.settings.tools;
                if page.for_seq != seq || page.catalog_reads != read {
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

    /// The picker's choice, shared by Catalog and Custom and kept while you move between them.
    pub fn choose_tools_target(&mut self, key: &str) {
        self.settings.tools.target = InstallTarget::from_key(key);
    }

    /// The Install button of a catalog card (`handleInstall`).
    pub fn install_catalog_tool(&mut self, name: &str) {
        let page = &self.settings.tools;
        let Some(entry) = page.catalog.iter().find(|e| e.name == name).cloned() else { return };
        let (target, seq) = (page.target.clone(), page.for_seq);
        let page = &mut self.settings.tools;
        page.installing = true;
        page.installing_name = Some(name.to_string());
        page.install_error = None;
        let Some(link) = self.skills_link() else {
            let page = &mut self.settings.tools;
            if !tools::is_installed(&page.tools, name, &target) {
                page.tools.push(crate::mock::settings_tools::from_entry(&entry, target));
            }
            page.installing = false;
            page.installing_name = None;
            return;
        };
        let body = entry.install_body(&target);
        link.off_thread(
            move |base| install_then_read(base, &body),
            move |s: &mut State, _tx: &mut Tx, got: Installed| {
                let page = &mut s.settings.tools;
                if page.for_seq != seq {
                    return;
                }
                page.installing = false;
                page.installing_name = None;
                match got {
                    // A failed read after the install is the list's own error, not the install's.
                    Ok(Ok(list)) => page.tools = list,
                    Ok(Err(e)) => page.error = Some(e),
                    Err(e) => page.install_error = Some(e),
                }
            },
        );
    }

    /// The Remove button: the first press asks to confirm (one at a time: another card's press moves it), the second removes.
    pub fn press_remove_tool(&mut self, key: &str) {
        let page = &mut self.settings.tools;
        if page.removing.is_some() {
            return;
        }
        if page.confirm_remove.as_deref() != Some(key) {
            page.confirm_remove = Some(key.to_string());
            return;
        }
        self.remove_tool(key);
    }

    pub fn cancel_remove_tool(&mut self) {
        self.settings.tools.confirm_remove = None;
    }

    /// `handleRemove`: `DELETE`, then the list again. A failure keeps the confirm state and shows under the list.
    fn remove_tool(&mut self, key: &str) {
        let Some(tool) = self.settings.tools.tools.iter().find(|t| t.key() == key).cloned() else { return };
        let page = &mut self.settings.tools;
        page.removing = Some(key.to_string());
        page.error = None;
        let seq = page.for_seq;
        let Some(link) = self.skills_link() else {
            let page = &mut self.settings.tools;
            page.removing = None;
            page.confirm_remove = None;
            page.tools.retain(|t| !(t.name == tool.name && t.target.is_same(&tool.target)));
            return;
        };
        let request = tools::remove(&tool.name, &tool.target);
        link.off_thread(
            move |base| skills::send(base, &request).map(drop),
            move |s: &mut State, _tx: &mut Tx, got: Result<(), String>| {
                if s.settings.tools.for_seq != seq {
                    return;
                }
                s.settings.tools.removing = None;
                match got {
                    Ok(()) => {
                        s.settings.tools.confirm_remove = None;
                        s.reload_tools();
                    }
                    Err(e) => s.settings.tools.error = Some(e),
                }
            },
        );
    }

    /// `setMode`: the other form; what was typed stays, the error goes. Not while an install runs.
    pub fn set_tool_mode(&mut self, mode: Mode) {
        let page = &mut self.settings.tools;
        if page.installing {
            return;
        }
        page.mode = mode;
        page.custom_error = None;
    }

    /// `submitInstall`: "Install tool" or "Install MCP server", or Enter in a one-line field.
    pub fn install_custom_tool(&mut self) {
        let page = &mut self.settings.tools;
        if page.installing {
            return;
        }
        let made = match page.mode {
            Mode::Native => tool_custom::native_install(&page.native_url.text),
            Mode::Mcp => tool_custom::mcp_install(&page.mcp_name.text, &page.mcp_command.text, &page.mcp_args.text, &page.mcp_env.text),
        };
        let entry = match made {
            Ok(e) => e,
            Err(e) => {
                // Nothing is "installing" for a reason the form can give without asking.
                page.custom_error = Some(e.into());
                return;
            }
        };
        let (target, seq, mode) = (page.target.clone(), page.for_seq, page.mode);
        page.installing = true;
        page.custom_error = None;
        let Some(link) = self.skills_link() else {
            let page = &mut self.settings.tools;
            page.installing = false;
            if !tools::is_installed(&page.tools, &entry.name, &target) {
                page.tools.push(crate::mock::settings_tools::from_entry(&entry, target));
            }
            page.clear_custom_fields(mode);
            return;
        };
        let body = entry.install_body(&target);
        link.off_thread(
            move |base| install_then_read(base, &body),
            move |s: &mut State, _tx: &mut Tx, got: Installed| {
                let page = &mut s.settings.tools;
                if page.for_seq != seq {
                    return;
                }
                page.installing = false;
                match got {
                    Ok(list) => {
                        page.clear_custom_fields(mode);
                        // A failed read after the install is the list's own error, not the install's.
                        match list {
                            Ok(list) => page.tools = list,
                            Err(e) => page.error = Some(e),
                        }
                    }
                    Err(e) => page.custom_error = Some(e),
                }
            },
        );
    }
}
