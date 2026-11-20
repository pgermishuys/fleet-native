//! What the folder menu keeps while it is open, and what its keys and rows do (`NSD/FolderPicker.vue`: the `open` watch,
//! `handleSearchKeydown`, `chooseOption`, `choose`; rows B2.19, B2.20, B2.42, B2.45-B2.53, B2.100-B2.103). The rows come from
//! `folder.rs`. The path view and creating a folder are `folder_path.rs`; the clone view and the clone itself are
//! `folder_clone.rs`.
use super::folder::{self, Act, FolderOption, Inputs};
use super::request::Folder;
use crate::{State, Tx};
use rune::TextEdit;
use std::collections::HashSet;

/// The key of the menu's panel, and of what is in it.
pub const SEARCH_KEY: &str = "new-session-folder-search";
pub const CHIP: &str = "new-session-folder-chip";
pub const POPOVER: &str = "folder";

/// Which of the menu's views shows (`view`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    List,
    Path,
    Clone,
}

/// A folder being checked, or added to the locations, before it is used (`openStatus`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Opening {
    #[default]
    Idle,
    Checking,
    Adding,
}

/// What a new folder in the path view starts as (`newKind`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NewKind {
    Empty,
    #[default]
    Git,
}

/// The server's warning about a folder that was made (`makeWarning`): the folder and the words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    pub path: String,
    pub text: String,
}

/// A create or clone that runs (`makeStatus`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Make {
    #[default]
    Idle,
    Creating,
    Cloning,
}

pub struct FolderMenu {
    pub view: View,
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
    /// Another folder was picked while the clone ran: it finishes without taking over (`isCloneSuperseded`).
    pub clone_superseded: bool,
    // The clone view (B2.160-B2.170): the repository box, the Location (`None` is "Another folder…") and the parent typed.
    pub clone_text: TextEdit,
    pub clone_location: Option<String>,
    pub clone_parent: TextEdit,
    /// The Location list under its select is showing.
    pub clone_root_list: bool,
    /// Where suggestions come from (`getRepositorySources`); none registered, none shown.
    pub sources: super::clone::RepositorySources,
    // The path view (B2.8-B2.10).
    pub path: TextEdit,
    pub path_highlight: usize,
    pub new_kind: NewKind,
    pub branch: TextEdit,
    pub opening: Opening,
    /// Why the last listing failed; stays until a later one succeeds (B2.133).
    pub listing_error: Option<String>,
    pub warning: Option<Warning>,
}

impl Default for FolderMenu {
    fn default() -> Self {
        FolderMenu {
            view: View::List,
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
            clone_superseded: false,
            clone_text: TextEdit::new(""),
            clone_location: None,
            clone_parent: TextEdit::new(""),
            clone_root_list: false,
            sources: Default::default(),
            path: TextEdit::new(""),
            path_highlight: 0,
            new_kind: NewKind::Git,
            branch: TextEdit::new(""),
            opening: Opening::Idle,
            listing_error: None,
            warning: None,
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
        let mut recent = defaults.recent_folders(&paths);
        let run_box = self.run_box_active();
        let run_folder = self.workflows_nav.folder().cloned();
        let draft = self.new_draft.draft();
        let current = if run_box { run_folder.as_ref() } else { draft.and_then(|d| d.folder.as_ref()) };
        let issue = !run_box && draft.is_some_and(|d| d.github_preset.is_some());
        if run_box {
            // A run is in a repository: no plain folders, no "No folder", no create or clone.
            recent.retain(|f| matches!(f, Folder::Repository(_)));
        }
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
            plain: if run_box { &[] } else { defaults.plain_folders() },
            current,
            query: &self.folder_menu.query.text,
            // Off in cloud mode (not in this app) and while a GitHub issue is attached (B2.86).
            allow_browse: !issue,
            allow_create: !issue && !run_box,
            allow_none: !issue && !run_box,
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
        // A clone that runs (or ended while the menu was shut) keeps the menu as it was (`keepStateOnOpen`).
        if self.folder_menu.make == Make::Cloning {
            return None;
        }
        self.folder_menu.query = TextEdit::new("");
        self.folder_menu.view = View::List;
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

    /// `resetMake`: forgets a failure and a warning. A create or clone that runs keeps running (typing never stops one).
    pub fn reset_make(&mut self) {
        self.folder_menu.error = None;
        self.folder_menu.existing = None;
        self.folder_menu.warning = None;
    }

    /// Typing (B2.42): the highlight goes back to the first row and an old failure is forgotten.
    pub fn folder_query_changed(&mut self) {
        self.folder_menu.highlight = 0;
        self.reset_make();
    }

    /// `choose`: the folder becomes the draft's, the menu shuts. Choosing while a clone runs lets the clone finish without
    /// taking over (B2.170).
    pub fn choose_folder(&mut self, folder: Folder) {
        if self.folder_menu.make == Make::Cloning {
            self.folder_menu.clone_superseded = true;
        }
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
            Act::Path(text) => self.show_path(text, tx),
            Act::CloneForm => self.show_clone(tx),
            Act::Create { target, root } => self.create_folder_at(target, true, Some(root), None),
            Act::Clone { source, target, root } => self.clone_into(source, target, Some(root)),
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
            (Some(name), Make::Cloning) if !self.folder_menu.clone_superseded => match self.folder_menu.clone_percent {
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
    /// The path view (piece G), on the scratch Fleet's project folder.
    Path(PathScene),
    /// A name that is a folder already: the list view's Create said no (409) and offers the folder that is there.
    Conflict,
    /// The clone view (piece H).
    Clone(CloneScene),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloneScene {
    /// A repository pasted in the search box and carried over: the preview of where it lands.
    Pasted,
    /// The box empty, a source registered: up to five suggestions (no live Fleet has a source without an account).
    Suggestions,
    /// The clone runs: git's words in the preview and on the chip, the fields shut.
    Running,
    /// The folder is there already (409): the failure and "Use that folder".
    Exists,
}

/// A source of repositories for the scene of suggestions.
struct SceneSource;

impl super::clone::RepositorySource for SceneSource {
    fn icon(&self) -> &'static str {
        "github"
    }
    fn refresh(&self) {}
    fn repositories(&self) -> Vec<super::clone::RemoteRepository> {
        ["ada/garden-notes", "ada/weave-website", "ada/recipe-box", "ada/demo", "ada/trail-maps", "ada/spare"]
            .iter()
            .map(|n| super::clone::RemoteRepository {
                id: n.to_string(),
                full_name: n.to_string(),
                name: n.rsplit('/').next().unwrap().into(),
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathScene {
    /// Just opened on the location: Open, then what is in it.
    Open,
    /// A name typed in a folder that is there: Create, its options and the preview.
    Create,
    /// Missing folders above the name too, an empty folder chosen.
    CreateDeep,
    /// A character no folder name can have: the note, and no Create.
    Invalid,
    /// A folder that is there: Open it, nothing to create.
    Exists,
    /// A folder outside the locations: the line that says Fleet adds it.
    Outside,
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
            Scene::Path(case) => {
                self.begin_folder_menu();
                self.show_path_scene(case, ROOT)
            }
            Scene::Clone(case) => {
                self.begin_folder_menu();
                self.folder_menu.query = TextEdit::new(if case == CloneScene::Suggestions { "" } else { "ada/recipe-box" });
                if case == CloneScene::Suggestions {
                    self.folder_menu.sources.register(std::sync::Arc::new(SceneSource));
                }
                self.enter_clone_view();
                if case == CloneScene::Running {
                    let m = &mut self.folder_menu;
                    (m.make, m.clone_name, m.clone_phase, m.clone_percent) =
                        (Make::Cloning, Some("recipe-box".into()), Some("Receiving objects".into()), Some(42));
                }
                if case == CloneScene::Exists {
                    self.folder_menu.error = Some(format!("{ROOT}/recipe-box already exists."));
                    self.folder_menu.existing = Some(format!("{ROOT}/recipe-box"));
                }
            }
            Scene::Conflict => {
                self.begin_folder_menu();
                self.folder_menu.query = TextEdit::new("clients");
                self.folder_menu.error = Some(format!("{ROOT}/clients already exists."));
                self.folder_menu.existing = Some(format!("{ROOT}/clients"));
            }
        }
        // `open_folder_menu` asks the machine; a scene has none, so the data above stays.
    }
}
