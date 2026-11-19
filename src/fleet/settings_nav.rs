//! Ports `composables/use-settings-nav.ts`: which Settings page is showing, and the ids other parts of the app use to send the
//! user to one. Fleet keeps this in module state and in the address (`/settings`, `/settings/plugins/<id>`); the native app has
//! no address, so `State.settings` holds it, and it is kept while the app runs, as Fleet's module state is.
use crate::State;

/// A page of the menu; `Plugin` is a plugin's own page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    General,
    Appearance,
    Harnesses,
    Permissions,
    Skills,
    Memory,
    Folders,
    Machines,
    Labs,
    Updates,
    Credentials,
    Plugins,
    Plugin(String),
}

impl Page {
    /// The page's id in Fleet (`SettingsPageId`).
    pub fn id(&self) -> String {
        match self {
            Page::General => "general".into(),
            Page::Appearance => "appearance".into(),
            Page::Harnesses => "harnesses".into(),
            Page::Permissions => "permissions".into(),
            Page::Skills => "skills".into(),
            Page::Memory => "memory".into(),
            Page::Folders => "folders".into(),
            Page::Machines => "machines".into(),
            Page::Labs => "labs".into(),
            Page::Updates => "updates".into(),
            Page::Credentials => "credentials".into(),
            Page::Plugins => "plugins".into(),
            Page::Plugin(id) => format!("plugin:{id}"),
        }
    }

    /// The title over the page and, for the menu's pages, the entry's words (`PAGES`).
    pub fn title(&self) -> &'static str {
        match self {
            Page::General => "General",
            Page::Appearance => "Appearance",
            Page::Harnesses => "Harnesses",
            Page::Permissions => "What agents can do",
            Page::Skills => "Skills & tools",
            Page::Memory => "Memory",
            Page::Folders => "Folders",
            Page::Machines => "Machines",
            Page::Labs => "Labs",
            Page::Updates => "Updates & about",
            Page::Credentials => "API keys",
            Page::Plugins => "Plugins",
            Page::Plugin(id) => super::settings::plugins::find(id).map_or("Plugin", |p| p.name),
        }
    }

    /// The line under the title, word for word.
    pub fn line(&self) -> &'static str {
        match self {
            Page::General => "Everyday behaviour.",
            Page::Appearance => "How Fleet looks.",
            Page::Harnesses => "The runtimes Fleet drives sessions with. Everything about one harness is on its tab.",
            Page::Permissions => "Edits, commands, messages and other machines: one setting each, with its exceptions.",
            Page::Skills => "What agents can use: skills, tools and, with Mods on, mods.",
            Page::Memory => "What Fleet remembers about each repository.",
            Page::Folders => "Where your repositories are, how worktrees are named, and the disk they take.",
            Page::Machines => "The machines you work on, phones and devices, and the access token.",
            Page::Labs => "Experiments. They can change or go away, and each is off until you turn it on.",
            Page::Updates => "Every version and update: Fleet, harnesses and machines, and what's new.",
            Page::Credentials => "Credentials for model providers.",
            Page::Plugins => {
                "Add-ons that put things in Fleet: rail icons, settings pages and pickers. Turn one off and everything it adds goes with it."
            }
            Page::Plugin(id) => super::settings::plugins::find(id).map_or("", |p| p.description),
        }
    }

    /// The menu entry that lights for this page: a plugin's page lights Plugins.
    pub fn entry(&self) -> Page {
        match self {
            Page::Plugin(_) => Page::Plugins,
            other => other.clone(),
        }
    }
}

/// Where an id lands: a page and the group or tab on it to open at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub page: Page,
    pub anchor: Option<String>,
}

/// `resolveSettingsTarget`: current ids as they are, the ids from before the pages were grouped on the page that holds the
/// setting now, and `integration:<id>` on that plugin's page.
pub fn resolve(id: &str, anchor: Option<&str>) -> Target {
    let own = |page: Page, fixed: Option<&str>| Target { page, anchor: anchor.or(fixed).map(str::to_string) };
    if let Some(plugin) = id.strip_prefix("integration:").or_else(|| id.strip_prefix("plugin:")) {
        return own(Page::Plugin(plugin.to_string()), None);
    }
    match id {
        "features" => own(Page::General, None),
        "workspace" => own(Page::Folders, None),
        "storage" => own(Page::Folders, Some("storage")),
        "browser" => own(Page::Permissions, Some("browser")),
        "tools" => own(Page::Skills, Some("tools")),
        "mods" => own(Page::Skills, Some("mods")),
        "system" => own(Page::Updates, None),
        "weave" => own(Page::Harnesses, Some("weave")),
        "workflows" => own(Page::Labs, Some("workflows")),
        "appearance" => own(Page::Appearance, None),
        "harnesses" => own(Page::Harnesses, None),
        "permissions" => own(Page::Permissions, None),
        "skills" => own(Page::Skills, None),
        "memory" => own(Page::Memory, None),
        "folders" => own(Page::Folders, None),
        "machines" => own(Page::Machines, None),
        "labs" => own(Page::Labs, None),
        "updates" => own(Page::Updates, None),
        "credentials" => own(Page::Credentials, None),
        "plugins" => own(Page::Plugins, None),
        _ => own(Page::General, None),
    }
}

/// What Settings shows and what asked for it (`activeSection`, `activeAnchor`, `activeHarness`, `memoryRepository`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SettingsNav {
    pub page: Page,
    /// The group or tab to open at; the page lets go of it once it has gone there.
    pub anchor: Option<String>,
    /// The harness tab Harnesses shows; none is the default harness's.
    pub harness: Option<String>,
    /// A repository Memory should show when it opens.
    pub repository: Option<String>,
    /// A kept anchor has been scrolled to, so it isn't again (`fleet::settings::anchor`).
    pub scrolled: bool,
    /// The rows whose More text is open (`SettingRow`'s `moreOpen`), by row id.
    pub more_open: std::collections::BTreeSet<String>,
    /// The kit's own page (the `settings-kit` scenes and tests), shown in place of the page when set.
    pub demo_open: bool,
    /// Preference keys whose save is under way: their switch shows the spinner and doesn't press (`saving`).
    pub saving: std::collections::BTreeSet<String>,
    /// Plugins in the menu folded (`Some(true)`) or unfolded (`Some(false)`) by hand; `None` follows the open page.
    pub plugins_folded: Option<bool>,
    /// Scene and test only: the Plugins page as it is with no plugin installed.
    pub plugins_none: bool,
    pub demo: crate::view::settings::kit_scene::Demo,
    /// What Harnesses keeps: the machine it asked, the checks under way, the switch being saved.
    pub harnesses: crate::fleet::settings::harnesses::HarnessesPage,
    /// What What agents can do keeps: the machine it asked and the exception list that is open.
    pub permissions: crate::fleet::settings::permissions::PermissionsPage,
    /// The agent browser's settings, one copy per machine (`use-agent-browser-settings.ts`).
    pub browser: std::collections::HashMap<String, crate::fleet::settings::browser::Copy>,
    /// The machine Settings shows (`use-settings-machine.ts`): the one you're on until the menu's switch picks another.
    pub choice: crate::fleet::machine_choice::MachineChoice,
    /// What Memory read from the machine shown, while the page is open.
    pub memory: crate::fleet::settings::memory::MemoryPage,
    /// What Skills & tools read from the machine shown: the skills, the picker's repositories and the Mods switch.
    pub skills: crate::fleet::settings::skills::SkillsPage,
    /// GitHub's connection page: the status, the device flow, the token box.
    pub github: crate::fleet::settings::github_auth::Github,
    /// What the Tools tab of that page read from the machine shown, while the tab is the one showing.
    pub tools: crate::fleet::settings::tools::ToolsPage,
    /// What Folders read from the machine shown: the locations, the add box and the folder browser.
    pub locations: crate::fleet::settings::locations::LocationsPage,
    /// What Folders' Worktrees card read from the machine shown: the user's layer, the fields and the sample.
    pub naming: crate::fleet::settings::naming_draft::NamingPage,
    /// What Folders' Storage card read: the checkouts, their sizes, the preview and the report of a clean.
    pub storage: crate::fleet::settings::storage::StoragePage,
    /// Folder sizes measured lately, per machine; they outlive the page so a return to Folders reuses them.
    pub storage_sizes: crate::fleet::settings::storage::SizeCache,
    /// Memory's search, add and edit forms and its folds.
    /// The Bun Mods run on and the install, read from the machine shown while Plugins is open.
    pub mods_runtime: crate::fleet::settings::mods_runtime::ModsRuntime,
    pub memory_ui: crate::fleet::settings::memory_notes::MemoryUi,
    /// What API keys read from the machine shown, and the forms' secrets, while the page is open.
    pub credentials: crate::fleet::settings::credentials::CredentialsPage,
    /// What the Built-in skills page read from the machine shown, while it is open.
    pub built_in: crate::fleet::settings::built_in_skills::BuiltInPage,
    /// The skill Editor and History dialogs; each opens from Settings' Edit and History, the Editor also from Improve.
    pub editor: crate::fleet::settings::skill_editor::SkillEditor,
    pub history: crate::fleet::settings::skill_history::SkillHistory,
}

impl SettingsNav {
    /// `SettingRow`'s More / Less.
    pub fn toggle_more(&mut self, row: &str) {
        if !self.more_open.remove(row) {
            self.more_open.insert(row.to_string());
        }
    }

    /// `setActiveSection`: an old id lands on its page; the old Weave id leaves the harness tab to be chosen.
    pub fn set(&mut self, id: &str, anchor: Option<&str>) {
        let target = resolve(id, anchor);
        self.page = target.page;
        self.anchor = target.anchor;
        self.scrolled = false;
        self.demo_open = false;
        if id == "weave" {
            self.harness = None;
        }
    }
}

impl State {
    /// `resetSettingsMachine` where Fleet makes the Settings route: coming to Settings from another rail forgets a machine
    /// picked on an earlier visit, so it opens on the machine you're on. Moving between Settings' pages doesn't.
    pub fn enter_settings(&mut self) {
        if self.rail != crate::Rail::Settings {
            self.settings.choice.reset();
        }
        self.rail = crate::Rail::Settings;
    }

    /// Opens Settings on a page: the rail, and the page by its id or an old id. Every entry point goes through here.
    pub fn open_settings(&mut self, id: &str, anchor: Option<&str>) {
        self.enter_settings();
        self.settings.set(id, anchor);
        if matches!(self.settings.page, Page::General | Page::Labs | Page::Plugins | Page::Plugin(_)) {
            self.refresh_preferences();
        }
    }

    /// `openHarness`: Harnesses on that harness's tab, optionally at one of its groups (`weave`, `may-do`, `profiles`).
    pub fn open_harness(&mut self, harness: &str, anchor: Option<&str>) {
        self.settings.harness = Some(harness.to_string());
        self.open_settings("harnesses", anchor);
    }

    /// `openMemory`: Memory with the repository selected.
    pub fn open_memory(&mut self, repository: &str) {
        self.settings.repository = Some(repository.to_string());
        self.open_settings("memory", None);
    }

    /// `openAddMachine`: Machines at the Add a machine row, with the form asked for.
    pub fn open_add_machine(&mut self) {
        self.add_machine_asked = true;
        self.open_settings("machines", Some("add-machine"));
    }
}
