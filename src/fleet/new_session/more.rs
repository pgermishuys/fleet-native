//! What the "…" chip under the New session box shows and does (`NSD/MoreOptions.vue`; rows B8, A21.11): the project the session
//! is filed under, its title and its tags. Plain state changes: nothing here draws. The draft holds the three values; the
//! fields keep their own caret and selection while the menu is open.
use crate::State;
use crate::fleet::groups::ProjectInfo;
use rune::TextEdit;

pub const MORE_POPOVER: &str = "more";
pub const MORE_CHIP: &str = "new-session-more-chip";
pub const PROJECT_KEY: &str = "new-session-project";
pub const TITLE_KEY: &str = "session-title";
pub const TAGS_KEY: &str = "new-session-tags";

/// What the menu keeps while it is open.
#[derive(Default)]
pub struct MoreMenu {
    /// The Project field's list is open over the menu.
    pub select_open: bool,
    pub title: TextEdit,
    pub tags: TextEdit,
}

impl State {
    /// The projects the Project field offers: the user's own, never Scratch (`userProjects`).
    pub fn more_projects(&self) -> Vec<ProjectInfo> {
        self.new_session_projects(&self.target_key())
    }

    /// The name on the chip: the project the draft is filed under, unless that is Scratch (`selectedProjectName`).
    pub fn more_project_name(&self) -> Option<String> {
        let id = self.new_draft.draft()?.project_id.clone()?;
        self.more_projects().into_iter().find(|p| p.id == id).map(|p| p.name)
    }

    /// The chip's accessible name (B8.3).
    pub fn more_chip_label(&self) -> String {
        match self.more_project_name() {
            Some(name) => format!("More options (project {name}): project, title, tags"),
            None => "More options: project, title, tags".into(),
        }
    }

    /// The chip's tooltip (B8.4).
    pub fn more_chip_help(&self) -> String {
        match self.more_project_name() {
            Some(name) => format!("Project {name}: the sidebar group it's filed under"),
            None => "Project, title, tags".into(),
        }
    }

    pub fn more_menu_open(&self) -> bool {
        self.ui.open == Some(crate::ui::Open::Popover(MORE_POPOVER))
    }

    /// Opening shows what the draft holds, each field with its caret at the end.
    pub fn open_more_menu(&mut self) {
        let (title, tags) = self.new_draft.draft().map(|d| (d.title.clone(), d.tags.clone())).unwrap_or_default();
        self.pickers_ns.more = MoreMenu { select_open: false, title: TextEdit::new(title), tags: TextEdit::new(tags) };
        self.ui.open = Some(crate::ui::Open::Popover(MORE_POPOVER));
    }

    pub fn close_more_menu(&mut self) {
        self.pickers_ns.more.select_open = false;
        if self.more_menu_open() {
            self.ui.open = None;
        }
    }

    pub fn open_project_select(&mut self) {
        self.pickers_ns.more.select_open = true;
    }

    pub fn close_project_select(&mut self) {
        self.pickers_ns.more.select_open = false;
    }

    /// The Project field's value (`selectValue`): Scratch is no project (`None`). The list closes; the menu stays.
    pub fn set_project(&mut self, project: Option<String>) {
        if let Some(d) = self.new_draft.draft_mut() {
            d.project_id = project;
        }
        self.pickers_ns.more.select_open = false;
    }

    pub fn title_edited(&mut self) {
        let typed = self.pickers_ns.more.title.text.clone();
        if let Some(d) = self.new_draft.draft_mut() {
            d.title = typed;
        }
    }

    pub fn tags_edited(&mut self) {
        let typed = self.pickers_ns.more.tags.text.clone();
        if let Some(d) = self.new_draft.draft_mut() {
            d.tags = typed;
        }
    }
}

// ─── scenes ─────────────────────────────────────────────────────────────────────

/// A state of the "…" menu for the pictures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    /// The strip with a project picked: its name on the chip.
    Chip,
    /// The menu with nothing to pick a project from: Title and Tags only.
    Plain,
    /// The menu with the Project field, a title and tags typed.
    Filled,
    /// The Project list open over the menu.
    Select,
}

impl State {
    pub(crate) fn show_more_scene(&mut self, scene: Scene) {
        use crate::fleet::groups::ProjectKind;
        let project = |id: &str, name: &str, position| ProjectInfo { id: id.into(), name: name.into(), kind: ProjectKind::User, position };
        if scene != Scene::Plain {
            self.project_load.projects = vec![project("p-orchard", "Orchard", 1), project("p-lighthouse", "Lighthouse", 2)];
        }
        self.open_new_session_page(None);
        if let Some(d) = self.new_draft.draft_mut() {
            if scene != Scene::Plain {
                d.project_id = Some("p-lighthouse".into());
            }
            if matches!(scene, Scene::Filled | Scene::Select) {
                d.title = "Tidy the lamp room".into();
                d.tags = "review-requested, deploy".into();
            }
        }
        if scene != Scene::Chip {
            self.open_more_menu();
        }
        if scene == Scene::Select {
            self.open_project_select();
        }
    }
}
