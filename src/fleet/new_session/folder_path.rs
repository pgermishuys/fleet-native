//! What the folder menu's path view keeps and does (`NSD/FolderPicker.vue`: `showPath`, `enterFolder`, `goUp`,
//! `createFromPath`, `runPathRow`, `handlePathKeydown`, `loadListing`, and the making of a folder: `create`, `useMade`,
//! `openFolder`, `useExisting`; rows B2.110-B2.145). What the rows are is `path.rs`; every call to the machine runs on a
//! thread (`feed_calls.rs`) and hands its answer back.
use super::feeds::{Inspection, Listing, NewFolder, parse_created};
use super::folder::{root_containing, separator_of};
use super::folder_menu::{Make, NewKind, Opening, View, Warning};
use super::path::{self, PathAction, PathCreate, PathInputs, PathRow};
use super::request::Folder;
use crate::fleet::http;
use crate::{State, Tx};
use rune::{Key, KeyEvent, TextEdit};

pub const PATH_KEY: &str = "new-session-path-input";
pub const BRANCH_KEY: &str = "new-session-path-branch";

/// The key of path row `index`: Create and Open have Fleet's test ids, the others a number.
pub fn path_row_key(index: usize, row: &PathRow) -> String {
    match row.action {
        PathAction::Create => "new-session-path-create".into(),
        PathAction::Open => "new-session-path-open".into(),
        PathAction::Enter => format!("new-session-path-enter-{index}"),
    }
}

/// `folderForInspection`: a git checkout is a repository, so it can get a worktree.
pub fn folder_for_inspection(i: &Inspection) -> Folder {
    if i.is_git_repo { Folder::Repository(i.path.clone()) } else { Folder::Directory(i.path.clone()) }
}

impl State {
    /// How this machine writes paths, judged from one of its folders (`separator`).
    pub fn folder_separator(&self) -> char {
        let key = self.target_key();
        let current = self.new_draft.draft().and_then(|d| match d.folder.as_ref() {
            Some(Folder::Repository(p) | Folder::Directory(p)) => Some(p.clone()),
            _ => None,
        });
        let first = self.feeds.repos.get(&key).and_then(|f| f.list.first()).map(|r| r.path.clone());
        let from = self.feeds.roots.first().cloned().or(current).or(first);
        separator_of(from.as_deref())
    }

    /// The folder whose contents the box lists (`folderToList`).
    pub fn folder_to_list(&self) -> String {
        path::folder_to_list(&self.folder_menu.path.text, self.folder_root().as_deref(), self.folder_separator())
    }

    /// The listing for exactly the folder typed: an answer for another folder is not used (`currentListing`).
    pub fn current_listing(&self) -> Option<&Listing> {
        let want = self.folder_to_list();
        self.feeds.listing.value.as_ref().filter(|_| self.feeds.listing.path.as_deref() == Some(want.as_str()))
    }

    fn with_path<T>(&self, f: impl FnOnce(&PathInputs) -> T) -> T {
        let input = PathInputs {
            text: &self.folder_menu.path.text,
            listing: self.current_listing(),
            allow_create: self.folder_allow_create(),
            home: self.feeds.home_folder(),
        };
        f(&input)
    }

    pub fn path_rows(&self) -> Vec<PathRow> {
        self.with_path(path::path_rows)
    }

    pub fn path_create(&self) -> Option<PathCreate> {
        self.with_path(path::path_create)
    }

    pub fn path_note(&self) -> Option<String> {
        self.with_path(|i| path::path_note(i, self.folder_menu.listing_error.as_deref()))
    }

    pub fn shown_path(&self, p: &str) -> String {
        path::shown_path(p, self.feeds.home_folder())
    }

    pub fn highlighted_path_row(&self) -> Option<PathRow> {
        self.path_rows().into_iter().nth(self.folder_menu.path_highlight)
    }

    /// Whether the locations have been read (a failed read counts: none is known then; so does an answer with none, which makes
    /// every Open or Create row add one: `roots.value !== null`).
    pub(crate) fn roots_known(&self) -> bool {
        self.feeds.roots_read || !self.feeds.roots.is_empty() || self.feeds.roots_error.is_some()
    }

    /// The line that says Fleet adds the folder to the locations (B2.136).
    pub fn path_adds_location(&self) -> bool {
        let roots = self.roots_known().then_some(self.feeds.roots.as_slice());
        path::adds_location(self.highlighted_path_row().as_ref(), roots)
    }

    /// The folder the box lists is asked for when it changes, and again each time the view is entered (the `watch` on
    /// `[folderToList, view]`).
    fn refresh_listing(&mut self) {
        if self.folder_menu.view != View::Path {
            return;
        }
        let want = self.folder_to_list();
        if self.feeds.listing.path.as_deref() != Some(want.as_str()) {
            let key = self.target_key();
            self.list_folder(&key, Some(&want));
        }
    }

    /// `showPath`: the box opens on the location (or on the text the search box held), git selected, what is there listed.
    pub fn show_path(&mut self, text: Option<String>, tx: &mut Tx) {
        self.reset_make();
        let text =
            text.unwrap_or_else(|| path::initial_text(self.folder_root().as_deref(), self.feeds.home_folder(), self.folder_separator()));
        self.folder_menu.path = TextEdit::new(text);
        self.folder_menu.path_highlight = 0;
        self.folder_menu.new_kind = NewKind::Git;
        self.folder_menu.branch = TextEdit::new(self.feeds.first_branch());
        self.folder_menu.view = View::Path;
        self.feeds.listing.cancel();
        self.refresh_listing();
        tx.focus(PATH_KEY, false);
    }

    /// The Back row: the list again, the search box focused (`showList`).
    pub fn show_folder_list(&mut self, tx: &mut Tx) {
        self.reset_make();
        self.folder_menu.view = View::List;
        tx.focus(super::folder_menu::SEARCH_KEY, false);
    }

    /// Typing in the box (`watch(pathText)`).
    pub fn path_changed(&mut self) {
        self.folder_menu.path_highlight = 0;
        self.reset_make();
        self.refresh_listing();
    }

    /// Goes into a folder: the box shows what's in it (`enterFolder`).
    pub fn enter_folder(&mut self, folder: &str, tx: &mut Tx) {
        let text = path::entered_text(folder, self.feeds.home_folder());
        self.folder_menu.path.replace_text(text);
        self.folder_menu.path_highlight = 0;
        self.refresh_listing();
        tx.focus(PATH_KEY, false);
    }

    /// The Up button (`goUp`).
    pub fn go_up(&mut self, tx: &mut Tx) {
        let here = self.current_listing().map_or_else(|| self.folder_to_list(), |l| l.path.clone());
        if let Some(parent) = path::parent_of(&here) {
            self.enter_folder(&parent, tx);
        }
    }

    /// ArrowDown and ArrowUp wrap (B2.129).
    pub fn path_step(&mut self, by: isize, tx: &mut Tx) {
        let rows = self.path_rows();
        if rows.is_empty() {
            return;
        }
        let at = (self.folder_menu.path_highlight + rows.len()).saturating_add_signed(by) % rows.len();
        self.folder_menu.path_highlight = at;
        tx.scroll_into_view(path_row_key(at, &rows[at]));
    }

    /// `createFromPath`.
    pub fn create_from_path(&mut self) {
        let Some(plan) = self.path_create().filter(|p| p.invalid.is_none()) else { return };
        let root = root_containing(&plan.target, &self.feeds.roots).cloned();
        let branch = Some(self.folder_menu.branch.text.trim().to_string()).filter(|b| !b.is_empty());
        let git = self.folder_menu.new_kind == NewKind::Git;
        self.create_folder_at(plan.target, git, root, branch);
    }

    /// `runPathRow`.
    pub fn run_path_row(&mut self, row: &PathRow, tx: &mut Tx) {
        match row.action {
            PathAction::Create => self.create_from_path(),
            PathAction::Open => self.open_folder(&row.path),
            PathAction::Enter if row.is_git_repo => self.open_folder(&row.path),
            PathAction::Enter => self.enter_folder(&row.path, tx),
        }
    }

    /// `handlePathKeydown`: true when the key was used (and must not reach the box).
    pub fn path_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        match ev.key {
            Key::Down => self.path_step(1, tx),
            Key::Up => self.path_step(-1, tx),
            Key::Enter => {
                if let Some(row) = self.highlighted_path_row()
                    && self.folder_menu.make == Make::Idle
                {
                    self.run_path_row(&row, tx);
                }
            }
            Key::Tab if !ev.mods.shift => match self.highlighted_path_row() {
                Some(row) if row.action == PathAction::Enter => self.enter_folder(&row.path, tx),
                _ => return false,
            },
            _ => return false,
        }
        true
    }

    // ─── making and opening a folder ────────────────────────────────────────────

    /// `create`: one at a time. `root` is the location the new folder is in (remembered for the next one).
    pub fn create_folder_at(&mut self, target: String, git: bool, root: Option<String>, branch: Option<String>) {
        if self.folder_menu.make != Make::Idle {
            return;
        }
        self.reset_make();
        self.folder_menu.make = Make::Creating;
        let key = self.target_key();
        let asked = target.clone();
        let branch = branch.filter(|_| git);
        let started = self.feed_call(
            &key,
            move |base| parse_created(&asked, http::create_directory(base, &asked, git, branch.as_deref())),
            move |s, _, result| {
                match result {
                    Ok(made) => s.use_made(made, root),
                    Err(failure) => {
                        s.folder_menu.error = Some(failure.message);
                        s.folder_menu.existing = failure.existing;
                    }
                }
                s.folder_menu.make = Make::Idle;
            },
        );
        if !started {
            self.folder_menu.make = Make::Idle;
            self.folder_menu.error = Some("Couldn't create that folder.".into());
        }
    }

    /// `useMade`: the folder is the draft's; a warning keeps the menu open so it can be read.
    pub(crate) fn use_made(&mut self, made: NewFolder, root: Option<String>) {
        let key = self.target_key();
        self.folder_menu.created.insert(made.path.clone());
        self.read_repositories(&key, true);
        if let Some(root) = root {
            self.remember_new_folder_root(&key, &root);
        }
        let folder = if made.is_git_repo { Folder::Repository(made.path.clone()) } else { Folder::Directory(made.path.clone()) };
        match made.warning {
            Some(text) => {
                self.set_folder(folder, true);
                self.folder_menu.warning = Some(Warning { path: made.path, text });
            }
            None => self.choose_folder(folder),
        }
    }

    /// `openFolder`: uses a folder that's there. A git checkout becomes a repository, anything else a plain folder; one
    /// outside the locations is added to them first, so it's listed next time.
    pub fn open_folder(&mut self, folder: &str) {
        if self.folder_menu.opening != Opening::Idle {
            return;
        }
        self.reset_make();
        self.folder_menu.opening = Opening::Checking;
        let key = self.target_key();
        let machine = key.clone();
        let started = self.inspect_folder(
            &key,
            folder,
            Box::new(move |s, _, result| match result {
                Err(e) => s.stop_opening(Some(e)),
                Ok(i) if !i.exists => s.stop_opening(Some("There's no folder at that path.".into())),
                Ok(i) if i.is_within_roots => s.finish_opening(i),
                Ok(i) => {
                    s.folder_menu.opening = Opening::Adding;
                    let added = i.clone();
                    let started = s.add_workspace_root(
                        &machine,
                        &i.path,
                        Box::new(move |s, _, r| match r {
                            Ok(()) => s.finish_opening(added),
                            Err(e) => s.stop_opening(Some(e)),
                        }),
                    );
                    if !started {
                        s.stop_opening(Some("Couldn't add that folder.".into()));
                    }
                }
            }),
        );
        if !started {
            self.stop_opening(Some("Couldn't open that folder.".into()));
        }
    }

    fn stop_opening(&mut self, error: Option<String>) {
        self.folder_menu.opening = Opening::Idle;
        self.folder_menu.error = error;
        self.folder_menu.existing = None;
    }

    fn finish_opening(&mut self, i: Inspection) {
        let key = self.target_key();
        if !i.is_within_roots {
            self.read_repositories(&key, true);
        }
        self.folder_menu.opening = Opening::Idle;
        self.choose_folder(folder_for_inspection(&i));
    }

    /// "Use that folder" after a 409: opens the folder that was already there (`useExisting`).
    pub fn use_existing_folder(&mut self) {
        if let Some(path) = self.folder_menu.existing.clone() {
            self.open_folder(&path);
        }
    }
}

/// Scenes of the path view for the pictures: what the scratch Fleet's project folder holds, with the listing seeded as the
/// machine would answer, so a picture of Fleet's own client shows the same rows.
impl State {
    pub(crate) fn show_path_scene(&mut self, case: super::folder_menu::PathScene, root: &str) {
        use super::feeds::Entry;
        use super::folder_menu::PathScene::*;
        let entry = |dir: &str, name: &str, git: bool| Entry { name: name.into(), path: format!("{dir}/{name}"), is_git_repo: git };
        let (text, dir, entries): (String, String, Vec<Entry>) = match case {
            Open => (
                format!("{root}/"),
                root.into(),
                vec![entry(root, "clients", false), entry(root, "demo", true), entry(root, "notes", false)],
            ),
            Create => {
                let clients = format!("{root}/clients");
                (format!("{clients}/acme-portal"), clients.clone(), vec![entry(&clients, "northwind-api", false)])
            }
            CreateDeep => {
                let clients = format!("{root}/clients");
                (format!("{clients}/acme/site"), format!("{clients}/acme"), Vec::new())
            }
            Invalid => (
                format!("{root}/notes?"),
                root.into(),
                vec![entry(root, "clients", false), entry(root, "demo", true), entry(root, "notes", false)],
            ),
            Exists => (
                format!("{root}/clients"),
                root.into(),
                vec![entry(root, "clients", false), entry(root, "demo", true), entry(root, "notes", false)],
            ),
            Outside => ("/opt/agent-playbook/".into(), "/opt/agent-playbook".into(), vec![entry("/opt/agent-playbook", "docs", false)]),
        };
        self.folder_menu.view = View::Path;
        self.folder_menu.path = TextEdit::new(text);
        self.folder_menu.branch = TextEdit::new(self.feeds.first_branch());
        self.folder_menu.new_kind = if case == CreateDeep { NewKind::Empty } else { NewKind::Git };
        self.folder_menu.path_highlight = 0;
        // The listing is for the folder the box lists, which for a text with no trailing separator is its parent.
        let asked = self.folder_to_list();
        let exists = case != CreateDeep;
        let listing = Listing { path: dir.clone(), exists, nearest_existing: (!exists).then(|| format!("{root}/clients")), entries };
        let seq = self.feeds.listing.begin(Some(&asked), false).expect("a folder to list");
        self.feeds.listing.finish(seq, Ok(listing));
        if case == Outside {
            self.feeds.roots_error = None;
        }
    }
}
