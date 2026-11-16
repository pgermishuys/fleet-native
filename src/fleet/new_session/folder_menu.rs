//! What the folder menu keeps while it is open, and what its keys and rows do (`NSD/FolderPicker.vue`: the `open` watch,
//! `handleSearchKeydown`, `chooseOption`, `choose`; rows B2.19, B2.20, B2.42, B2.45-B2.53, B2.100-B2.103). The rows come from
//! `folder.rs`. The path view (piece G), the clone view and the actual create and clone (piece H) are not here: their rows say so.
use super::folder::{self, Act, FolderOption, Inputs};
use super::request::Folder;
use crate::{State, Tx};
use rune::TextEdit;
use std::collections::HashSet;

/// The key of the menu's panel, and of what is in it.
pub const SEARCH_KEY: &str = "new-session-folder-search";
pub const CHIP: &str = "new-session-folder-chip";
pub const POPOVER: &str = "folder";

/// A create or clone that runs (`makeStatus`). Nothing starts one before pieces G and H; the rows already read it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Make {
    #[default]
    Idle,
    Creating,
    Cloning,
}

pub struct FolderMenu {
    pub query: TextEdit,
    pub highlight: usize,
    /// The Location picked in this opening; resets on each open (B2.95).
    pub picked_root: Option<String>,
    /// The Location list under its select is showing.
    pub root_list: bool,
    pub make: Make,
    /// The server's words for a failed create or clone, and the folder that was already there (the 409).
    pub error: Option<String>,
    pub existing: Option<String>,
    /// Folders made or cloned from this menu this run: the chip marks them `new` (B2.14).
    pub created: HashSet<String>,
    /// The clone that runs: the repository's name, git's last words (`Receiving objects`) and its percent (set by piece H).
    pub clone_name: Option<String>,
    pub clone_phase: Option<String>,
    pub clone_percent: Option<u8>,
}

impl Default for FolderMenu {
    fn default() -> Self {
        FolderMenu {
            query: TextEdit::new(""),
            highlight: 0,
            picked_root: None,
            root_list: false,
            make: Make::Idle,
            error: None,
            existing: None,
            created: HashSet::new(),
            clone_name: None,
            clone_phase: None,
            clone_percent: None,
        }
    }
}

/// The key of row `index` in the list: the New and Other rows have Fleet's test ids, the rest a number.
pub fn row_key(index: usize, option: &FolderOption) -> String {
    match option.group {
        folder::Group::New | folder::Group::Other => format!("new-session-folder-{}", option.id),
        _ => format!("new-session-folder-row-{index}"),
    }
}

impl State {
    /// The folder menu is showing.
    pub fn folder_menu_open(&self) -> bool {
        self.ui.open == Some(crate::ui::Open::Popover(POPOVER))
    }

    fn with_inputs<T>(&self, f: impl FnOnce(&Inputs) -> T) -> T {
        let key = self.target_key();
        let repos = self.feeds.repos(&key).list;
        let defaults = self.sidebar.new_session_defaults(&key);
        let paths: Vec<String> = repos.iter().map(|r| r.path.clone()).collect();
        let recent = defaults.recent_folders(&paths);
        let draft = self.new_draft.draft();
        let current = draft.and_then(|d| d.folder.as_ref());
        let issue = draft.is_some_and(|d| d.github_preset.is_some());
        let current_path = match current {
            Some(Folder::Repository(p) | Folder::Directory(p)) => Some(p.as_str()),
            _ => None,
        };
        let root = self
            .folder_menu
            .picked_root
            .as_ref()
            .or_else(|| folder::default_new_folder_root(&self.feeds.roots, defaults.last_new_folder_root(), current_path))
            .cloned();
        let input = Inputs {
            repositories: &repos,
            recent: &recent,
            plain: defaults.plain_folders(),
            current,
            query: &self.folder_menu.query.text,
            // Off in cloud mode (not in this app) and while a GitHub issue is attached (B2.86).
            allow_browse: !issue,
            allow_create: !issue,
            allow_none: !issue,
            root: root.as_deref(),
            first_branch: self.feeds.first_branch(),
        };
        f(&input)
    }

    /// The rows now showing.
    pub fn folder_rows(&self) -> Vec<FolderOption> {
        self.with_inputs(folder::rows)
    }

    pub fn folder_empty_note(&self) -> Option<String> {
        self.with_inputs(|i| folder::empty_note(i, &folder::rows(i)))
    }

    pub fn folder_invalid_char(&self) -> Option<char> {
        self.with_inputs(folder::invalid_in_query)
    }

    pub fn folder_allow_create(&self) -> bool {
        self.with_inputs(|i| i.allow_create)
    }

    /// Where a Create or Clone would go: the Location select's value.
    pub fn folder_root(&self) -> Option<String> {
        self.with_inputs(|i| i.root.map(str::to_string))
    }

    /// The Location row shows when a Create or Clone is offered and more than one location is known (B2.90).
    pub fn folder_shows_location(&self) -> bool {
        self.feeds.roots.len() > 1 && folder::offers_to_make(&self.folder_rows())
    }

    /// The `open` watch (B2.19): everything starts over, the locations and defaults are asked for, and the highlight goes to
    /// the chosen row.
    pub fn open_folder_menu(&mut self, tx: &mut Tx) {
        let Some(selected) = self.begin_folder_menu() else { return };
        tx.focus(SEARCH_KEY, false);
        tx.scroll_into_view(format!("new-session-folder-row-{selected}"));
    }

    /// Everything of opening that isn't the keyboard's: the row to bring into view, or `None` when a clone's state is kept.
    pub(crate) fn begin_folder_menu(&mut self) -> Option<usize> {
        self.ui.open = Some(crate::ui::Open::Popover(POPOVER));
        if self.folder_menu.make == Make::Cloning {
            return None;
        }
        self.folder_menu.query = TextEdit::new("");
        self.folder_menu.picked_root = None;
        self.folder_menu.root_list = false;
        self.reset_make();
        let key = self.target_key();
        self.read_roots(&key);
        if self.feeds.defaults.is_none() {
            self.read_directory_defaults(&key);
        }
        let selected = self.folder_rows().iter().position(|r| r.selected).unwrap_or(0);
        self.folder_menu.highlight = selected;
        Some(selected)
    }

    /// Closing aborts a folder listing on its way (B2.20).
    pub fn close_folder_menu(&mut self) {
        if self.folder_menu_open() {
            self.ui.open = None;
        }
        self.feeds.listing.cancel();
    }

    pub fn reset_make(&mut self) {
        self.folder_menu.make = Make::Idle;
        self.folder_menu.error = None;
        self.folder_menu.existing = None;
    }

    /// Typing (B2.42): the highlight goes back to the first row and an old failure is forgotten.
    pub fn folder_query_changed(&mut self) {
        self.folder_menu.highlight = 0;
        self.reset_make();
    }

    /// `choose`: the folder becomes the draft's, the menu shuts.
    pub fn choose_folder(&mut self, folder: Folder) {
        self.set_folder(folder, true);
        self.close_folder_menu();
    }

    /// ArrowDown and ArrowUp wrap, and bring the row into view (B2.45, B2.46); nothing with no rows (B2.48).
    pub fn folder_step(&mut self, by: isize, tx: &mut Tx) {
        let n = self.folder_rows().len();
        if n == 0 {
            return;
        }
        let at = (self.folder_menu.highlight + n).saturating_add_signed(by) % n;
        self.folder_menu.highlight = at;
        tx.scroll_into_view(self.folder_row_key(at));
    }

    fn folder_row_key(&self, at: usize) -> String {
        self.folder_rows().get(at).map(|o| row_key(at, o)).unwrap_or_default()
    }

    /// `chooseOption`: a New row does nothing while a create or clone already runs; every other row still works.
    pub fn run_folder_row(&mut self, at: usize, tx: &mut Tx) {
        let Some(option) = self.folder_rows().into_iter().nth(at) else { return };
        if option.group == folder::Group::New && self.folder_menu.make != Make::Idle {
            return;
        }
        match option.act {
            Act::Choose(folder) => self.choose_folder(folder),
            Act::Path(_) => self.toast("The path view isn't in the native app yet (component 7, piece G).".into(), None, 4.0, tx),
            Act::CloneForm => self.toast("The clone view isn't in the native app yet (component 7, piece H).".into(), None, 4.0, tx),
            Act::Create { .. } => self.toast("Creating a folder isn't in the native app yet (component 7, piece G).".into(), None, 4.0, tx),
            Act::Clone { .. } => self.toast("Cloning isn't in the native app yet (component 7, piece H).".into(), None, 4.0, tx),
        }
    }

    /// The Location select picks a root (B2.94).
    pub fn pick_folder_root(&mut self, root: String) {
        self.folder_menu.picked_root = Some(root);
        self.folder_menu.root_list = false;
    }

    /// `cloneStatusText`: git's phase and percent, or `Starting…` before its first line.
    pub fn clone_status_text(&self) -> String {
        match (&self.folder_menu.clone_phase, self.folder_menu.clone_percent) {
            (Some(phase), Some(p)) => format!("{phase} {p}%"),
            (Some(phase), None) => phase.clone(),
            _ => "Starting…".into(),
        }
    }

    /// The chip's words (`chipLabel`): while a clone for this folder runs it says so (B2.10).
    pub fn folder_chip_label(&self, folder: Option<&Folder>) -> String {
        match (&self.folder_menu.clone_name, self.folder_menu.make) {
            (Some(name), Make::Cloning) => match self.folder_menu.clone_percent {
                Some(p) => format!("Cloning {name}… {p}%"),
                None => format!("Cloning {name}…"),
            },
            _ => self.folder_label(folder),
        }
    }

    /// The `new` tag after the chip's label (B2.14).
    pub fn folder_is_new(&self, folder: Option<&Folder>) -> bool {
        match folder {
            Some(Folder::Repository(p) | Folder::Directory(p)) => self.folder_menu.created.contains(p),
            _ => false,
        }
    }
}

/// Which state a scene of the menu draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    /// Open on the scratch Fleet's one repository.
    Open,
    /// A new folder's name typed (Create offered); `second_root` adds a second location so the Location row shows.
    Typed { second_root: bool },
    /// A name that matches nothing, with no location known: the note.
    Nothing,
    /// A repository with a very long name chosen, the menu shut.
    LongName,
}

/// Scenes for the pictures: the same data the scratch Fleet of `docs/FLEET.md` serves (one repository under one location),
/// so a picture of Fleet's own client shows the same rows.
impl State {
    pub(crate) fn show_folder_menu(&mut self, scene: Scene) {
        use super::feeds::{Repository, Scan};
        const ROOT: &str = "/tmp/fleet-native-scratch/proj";
        let long = "customer-portal-redesign-phase-two-migration-of-the-billing-and-invoicing-services-to-the-new-platform";
        let repo = |name: &str| Repository { name: name.into(), path: format!("{ROOT}/{name}"), parent_root: ROOT.into() };
        let mut repos = vec![repo("demo")];
        if scene == Scene::LongName {
            repos.push(repo(long));
        }
        self.feeds.repos.insert("home".into(), super::feed_state::RepoFeed::seeded(Scan { repositories: repos, scanned_at: Some(0) }));
        self.feeds.roots = if scene == Scene::Nothing { vec![] } else { vec![ROOT.into()] };
        if scene == (Scene::Typed { second_root: true }) {
            self.feeds.roots.push("/tmp/fleet-native-scratch/work".into());
        }
        self.feeds.defaults =
            Some(super::feeds::DirectoryDefaults { first_branch: "main".into(), home: Some("/tmp/fleet-native-scratch/home".into()) });
        self.open_new_session_page(None);
        match scene {
            Scene::Open => drop(self.begin_folder_menu()),
            Scene::Typed { .. } => {
                self.begin_folder_menu();
                self.folder_menu.query = TextEdit::new("Recipe Box");
            }
            Scene::Nothing => {
                self.begin_folder_menu();
                self.folder_menu.query = TextEdit::new("zzz");
            }
            Scene::LongName => self.set_folder(Folder::Repository(format!("{ROOT}/{long}")), true),
        }
        // `open_folder_menu` asks the machine; a scene has none, so the data above stays.
    }
}
