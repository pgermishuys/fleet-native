//! What the folder menu's clone view keeps and does (`NSD/FolderPicker.vue`: `showClone`, `clone`, `submitClone`, `useMade`
//! for a clone, `isCloneSuperseded`, `keepStateOnOpen`; rows B2.161-B2.170 and the rows B2.5, B2.10, B2.74, B2.78, B2.79 and
//! B2.103 that a running clone shows). The text rules are `clone.rs`; the call runs on a thread and hands git's progress back
//! line by line.
use super::clone::{self, ClonePlan, CloneProgress, Suggestion, follow};
use super::feeds::MakeFailure;
use super::folder::CloneSource;
use super::folder_menu::{Make, POPOVER, View};
use crate::fleet::http;
use crate::{State, Tx};
use rune::TextEdit;

pub const CLONE_KEY: &str = "new-session-clone-repository";
pub const PARENT_KEY: &str = "new-session-clone-parent";

impl State {
    /// `showClone`: the box holds what was pasted into the search box (when it read as a repository), the Location is the new
    /// folder's, and every source is asked for its list again.
    pub fn show_clone(&mut self, tx: &mut Tx) {
        self.enter_clone_view();
        tx.focus(CLONE_KEY, false);
    }

    pub(crate) fn enter_clone_view(&mut self) {
        self.reset_make();
        let typed = self.folder_menu.query.text.trim().to_string();
        let pasted = self.folder_allow_create() && super::folder::parse_clone_source(&typed).is_some();
        self.folder_menu.clone_text = TextEdit::new(if pasted { typed } else { String::new() });
        self.folder_menu.clone_location = self.folder_root();
        self.folder_menu.clone_parent = TextEdit::new("");
        self.folder_menu.clone_root_list = false;
        self.repository_sources().refresh();
        self.folder_menu.view = View::Clone;
    }

    pub fn clone_plan(&self) -> ClonePlan {
        let m = &self.folder_menu;
        clone::plan(&m.clone_text.text, m.clone_location.as_deref(), &m.clone_parent.text)
    }

    /// What the Clone view suggests from: the sources registered directly and those of the plugins that are on.
    fn repository_sources(&self) -> clone::RepositorySources {
        let mut all = self.folder_menu.sources.clone();
        self.places.repository_sources(&self.pickers.prefs).into_iter().for_each(|s| all.register(s));
        all
    }

    /// `cloneSuggestions`: repositories the sources offer that are not here yet.
    pub fn clone_suggestions(&self) -> Vec<Suggestion> {
        let local: Vec<String> = self.feeds.repos(&self.target_key()).list.into_iter().map(|r| r.name).collect();
        clone::suggestions(&self.repository_sources(), &local, &self.folder_menu.clone_text.text)
    }

    /// `canSubmitClone`.
    pub fn can_submit_clone(&self) -> bool {
        self.folder_menu.make == Make::Idle && self.clone_plan().target.is_some()
    }

    /// A clone is running and has not been given up on: starting a session waits (`update:busy`).
    pub fn folder_clone_busy(&self) -> bool {
        self.folder_menu.make == Make::Cloning && !self.folder_menu.clone_superseded
    }

    /// The Location select picks a location, or `None` for "Another folder…" (B2.164).
    pub fn pick_clone_location(&mut self, location: Option<String>) {
        self.folder_menu.clone_location = location;
        self.folder_menu.clone_root_list = false;
    }

    /// A suggestion puts its name in the box; an old failure stays, as in Fleet (only typing clears it).
    pub fn pick_clone_suggestion(&mut self, full_name: String, tx: &mut Tx) {
        self.folder_menu.clone_text = TextEdit::new(full_name);
        tx.focus(CLONE_KEY, false);
    }

    /// Typing in either box forgets the last failure.
    pub fn clone_fields_changed(&mut self) {
        self.reset_make();
    }

    /// `submitClone`.
    pub fn submit_clone(&mut self) {
        let plan = self.clone_plan();
        if !self.can_submit_clone() {
            return;
        }
        let (Some(source), Some(target)) = (plan.source, plan.target) else { return };
        let root = if plan.other { None } else { self.folder_menu.clone_location.clone() };
        self.clone_into(source, target, root);
    }

    /// `clone`: one create or clone at a time. `root` is the location the folder is in, `None` for one outside them.
    pub fn clone_into(&mut self, source: CloneSource, target: String, root: Option<String>) {
        if self.folder_menu.make != Make::Idle {
            return;
        }
        self.reset_make();
        let m = &mut self.folder_menu;
        m.make = Make::Cloning;
        m.clone_phase = None;
        m.clone_percent = None;
        m.clone_name = Some(source.name.clone());
        m.clone_superseded = false;
        let key = self.target_key();
        let (repository, path) = (source.repository, target);
        let started = self.feed_stream(
            &key,
            move |base, post| {
                let (status, body) = http::clone_directory(base, &repository, &path)
                    .map_err(|_| MakeFailure { message: "Couldn't clone that repository.".into(), existing: None })?;
                follow(status, body, &path, |p| post.update(move |s, _| s.on_clone_progress(p)))
            },
            move |s, tx, result| s.finish_clone(result, root, tx),
        );
        if !started {
            self.folder_menu.make = Make::Idle;
            self.folder_menu.clone_name = None;
            self.folder_menu.error = Some("Couldn't clone that repository.".into());
        }
    }

    fn on_clone_progress(&mut self, p: CloneProgress) {
        if self.folder_menu.make == Make::Cloning {
            self.folder_menu.clone_phase = Some(p.phase);
            self.folder_menu.clone_percent = Some(p.percent);
        }
    }

    fn finish_clone(&mut self, result: Result<super::feeds::NewFolder, MakeFailure>, root: Option<String>, tx: &mut Tx) {
        let superseded = self.folder_menu.clone_superseded;
        let m = &mut self.folder_menu;
        m.make = Make::Idle;
        m.clone_name = None;
        m.clone_phase = None;
        m.clone_percent = None;
        match result {
            // Another folder was picked meanwhile: the clone is marked `new` and listed, and the choice stays (B2.170).
            Ok(made) if superseded => {
                m.created.insert(made.path);
                let key = self.target_key();
                self.read_repositories(&key, true);
            }
            Ok(made) => self.use_made(made, root),
            Err(failure) => {
                m.error = Some(failure.message);
                m.existing = failure.existing;
                // Shut while it ran: reopen as it was, to show how it ended (B2.103).
                if !superseded && !self.folder_menu_open() {
                    self.ui.open = Some(crate::ui::Open::Popover(POPOVER));
                    tx.focus(CLONE_KEY, false);
                }
            }
        }
    }
}
