//! Fleet's kit (`client/src/components/ui/`) and its command palette, in Rune. One file per
//! control; every colour comes from `theme.rs`. Controls are functions that return elements;
//! overlays (menus, selects, popovers, dialogs, the palette) are mounted once, at the root, from
//! `Ui::open`, anchored to the key of the element that opened them.
pub mod age;
pub mod button;
pub mod calendar;
pub mod chip;
pub mod code_box;
pub mod commands;
pub mod confirm;
pub mod controls;
pub mod dialog;
pub mod directory_picker;
pub mod empty_state;
pub mod error_note;
pub mod gallery;
pub mod menu;
pub mod menus;
pub(crate) mod overlay;
pub mod page_header;
pub mod palette;
pub mod scenes;
pub mod select;
#[cfg(test)]
mod tests;
pub mod text_menu;
pub mod toast;

use crate::State;
use crate::theme::{Theme, sans};
use rune::{El, HelpLook, TextEdit};

/// Fleet's rem: `font-size: 14px` on the root, so `h-9` is 31.5px.
pub const fn rem(n: f32) -> f32 {
    n * 14.0
}

/// The one overlay that is open (Fleet opens one menu at a time; a dialog sits over a palette
/// the same way a second `Open` would, so there is no stack here).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Open {
    Palette,
    /// A dropdown, by name (`menus::dropdown`).
    Menu(&'static str),
    /// A right-click menu on a session: (project, session).
    Context(usize, usize),
    /// A right-click menu on a project's header: its place in the list.
    ProjectMenu(usize),
    /// A right-click menu on a row, or on a project's header, of another machine (`State::machine_menu` says which).
    MachineContext,
    MachineProjectMenu,
    /// A right-click on a machine's heading (`State::machine_menu` holds its key and an empty id).
    MachineHeadingMenu,
    Select(&'static str),
    Popover(&'static str),
    Directory,
    /// Go to file (`view::go_to_file`); the session is `State::go_to_file.session`.
    GoToFile,
    Dialog,
    Alert,
    Sheet(dialog::SheetSide),
    /// The right-click menu on text (`ui::text_menu`): a field, selected text or a link.
    TextMenu,
}

pub struct Ui {
    pub open: Option<Open>,
    /// What the text menu is over, while `Open::TextMenu` is open.
    pub text_menu: Option<text_menu::TextMenu>,
    /// Where the right-click that opened a context menu landed.
    pub point: (f32, f32),
    pub palette: palette::Palette,
    pub dir: directory_picker::Picker,
    pub toast: Option<toast::Toast>,
    /// The composer's agent and model menus.
    pub sel: crate::fleet::selector::Menu,
    /// The gallery's own controls.
    pub g_switch: bool,
    pub g_tab: usize,
    pub g_tab2: usize,
    pub g_select: Option<String>,
    pub g_check: bool,
    pub g_input: TextEdit,
    pub g_area: TextEdit,
    pub g_collapsed: bool,
    /// The question being asked, if any (`ui::confirm`).
    pub confirm: Option<confirm::Confirm>,
    /// A context menu's footer: the hint of the row the pointer is on.
    pub hint: Option<String>,
    /// The row of the open menu whose submenu is open (its key).
    pub sub: Option<String>,
    /// The row of that submenu whose own submenu is open (Open in > an editor with key files).
    pub sub2: Option<String>,
}

impl Ui {
    /// A right-click menu opens at `at`, with no row highlighted and no submenu open.
    pub fn show_context(&mut self, open: Open, at: (f32, f32)) {
        self.point = at;
        self.hint = None;
        self.sub = None;
        self.sub2 = None;
        self.open = Some(open);
    }
}

impl Default for Ui {
    fn default() -> Self {
        Ui {
            open: None,
            text_menu: None,
            point: (0.0, 0.0),
            palette: palette::Palette::default(),
            dir: directory_picker::Picker::default(),
            toast: None,
            sel: Default::default(),
            g_switch: true,
            g_tab: 0,
            g_tab2: 0,
            g_select: Some("sonnet".into()),
            g_check: true,
            g_input: TextEdit::new(""),
            g_area: TextEdit::new(""),
            g_collapsed: false,
            confirm: None,
            hint: None,
            sub: None,
            sub2: None,
        }
    }
}

/// Fleet's tooltip (`tooltip/TooltipContent.vue`): the foreground colour as fill and the
/// background as ink, square, `px-3 py-1.5 text-xs`, 4px from the trigger, after reka's 700ms.
pub fn help_look(t: &Theme) -> HelpLook {
    HelpLook::system(t.dark)
        .fill(t.text)
        .ink(t.panel_bg)
        .ring(0.0, t.text)
        .radius(0.0)
        .padding(rem(0.75), rem(0.375))
        .font(sans(rem(0.75), 400.0, 1.33))
        .delay(0.7)
}

/// Every overlay the open state asks for, to mount above the window (`rune_kit::root`).
pub fn overlays(s: &State, t: &Theme) -> Vec<El> {
    let mut layer = match s.ui.open {
        None => Vec::new(),
        Some(Open::Palette) => palette::view(s, t),
        Some(Open::TextMenu) => text_menu::overlay(s, t),
        // A role's menu from the Run box draws the popover it opened from; from the dialog it is drawn with the dialog.
        Some(Open::Menu("wf-model" | "wf-effort")) => match s.workflows_nav.roles.menu {
            Some(menu) if menu.place == crate::fleet::workflows::roles::Place::RunBox => {
                crate::view::workflows::roles::run_box_menu(s, t, menu)
            }
            _ => Vec::new(),
        },
        Some(Open::Menu(name)) => menus::dropdown(s, t, name),
        Some(Open::Context(p, i)) => menus::session_context(s, t, p, i),
        Some(Open::ProjectMenu(p)) => menus::project_context(s, t, p),
        Some(Open::MachineContext) => menus::machine_session_context(s, t),
        Some(Open::MachineProjectMenu) => menus::machine_project_context(s, t),
        Some(Open::MachineHeadingMenu) => menus::machine_heading_context(s, t),
        Some(Open::Select(crate::view::mods::controls::OPEN)) => crate::view::mods::controls::select_list(s, t),
        Some(Open::Select("large-pastes")) => crate::view::settings::general::large_pastes_list(s, t),
        Some(Open::Select("dream-nightly-harness")) => crate::view::settings::dream_nightly::harness_list(s, t),
        Some(Open::Select("dream-nightly-model")) => crate::view::settings::dream_nightly::model_list(s, t),
        Some(Open::Select("dream-budget")) => crate::view::settings::dream_nightly::budget_list(s, t),
        Some(Open::Select("dreaming-harness")) => crate::view::settings::dreaming_panel::harness_list(s, t),
        Some(Open::Select("skills-target")) => crate::view::settings::install_picker::list(s, t, "skills-target"),
        Some(Open::Select("tools-target")) => crate::view::settings::install_picker::list(s, t, "tools-target"),
        Some(Open::Select("memory-repository")) => crate::view::settings::memory_notes::repository_list(s, t),
        Some(Open::Select("permission-chip")) => crate::view::settings::permissions::chip_list(s, t),
        Some(Open::Select("permission-machine")) => crate::view::settings::permissions_rows::machine_list(s, t),
        Some(Open::Select("permission-machine-add")) => crate::view::settings::permissions_rows::machine_add_list(s, t),
        Some(Open::Select("permission-add")) => crate::view::settings::permissions::add_list(s, t),
        Some(Open::Select("sign-in-field")) => crate::view::settings::harness_sign_in_flow::field_list(s, t),
        Some(Open::Select("update-default-rule")) => crate::view::settings::updates_strip::rule_list(s, t),
        Some(Open::Select("update-machine-rule")) => crate::view::settings::updates_version::list(s, t),
        Some(Open::Select("browser-port")) => crate::view::browser_app::port_list(s, t),
        Some(Open::Select("default-harness")) => crate::view::settings::harnesses::default_list(s, t),
        Some(Open::Select(crate::view::analytics_page::filters::PROJECT)) => crate::view::analytics_page::filters::list(s, t),
        Some(Open::Select(name)) => gallery::select_content(s, t, name),
        Some(Open::Popover("context")) => crate::view::ring_popover::overlay(s, t),
        Some(Open::Popover("allowed")) => crate::view::allowed::overlay(s, t),
        Some(Open::Popover("starters")) => crate::view::starters::menu(t),
        Some(Open::Popover("locations-browse")) => crate::view::settings::folders_browse::overlay(s, t),
        Some(Open::Popover("folder")) => crate::view::folder_menu::overlay(s, t),
        Some(Open::Popover("workspace")) => crate::view::workspace_picker::overlay(s, t),
        Some(Open::Popover("base")) => crate::view::base_picker::overlay(s, t),
        Some(Open::Popover("harness")) => crate::view::harness_picker::overlay(s, t),
        Some(Open::Popover("profile")) => crate::view::profile_picker::overlay(s, t),
        Some(Open::Popover("machine")) => crate::view::machine_picker::overlay(s, t),
        Some(Open::Popover("usage")) => crate::view::usage_popover::overlay(s, t),
        Some(Open::Popover("running")) => crate::view::running_counter::overlay(s, t),
        Some(Open::Popover("pr")) => crate::view::pr_popover::overlay(s, t),
        Some(Open::Popover(crate::fleet::analytics::calendar::POPOVER)) => crate::view::analytics_page::filters::calendar_overlay(s, t),
        Some(Open::Popover("analytics")) => crate::view::analytics::overlay(s, t),
        Some(Open::Popover("workflow-models")) => crate::view::workflows::roles::popover(s, t),
        Some(Open::Popover("workflow-project")) => crate::view::workflows::project_picker::overlay(s, t),
        Some(Open::Popover("more")) => crate::view::more_options::overlay(s, t),
        Some(Open::Popover(crate::fleet::automations::when::WHEN_POPOVER)) => crate::view::automations::when::overlay(s, t),
        Some(Open::Popover(crate::fleet::automations::strip::RUNS_IN_POPOVER)) => crate::view::automations::strip::runs_in_overlay(s, t),
        Some(Open::Popover(crate::fleet::automations::strip::MORE_POPOVER)) => crate::view::automations::strip::more_overlay(s, t),
        Some(Open::Popover(name)) => gallery::popover_content(s, t, name),
        Some(Open::Directory) => directory_picker::view(s, t, "g-dir"),
        Some(Open::GoToFile) => crate::view::go_to_file::overlay(s, t),
        Some(Open::Dialog) => gallery::dialog(s, t),
        Some(Open::Alert) => gallery::alert(s, t),
        Some(Open::Sheet(side)) => gallery::sheet(s, t, side),
    };
    let menu_open = s.ui.open.is_some();
    // Floating things a native web view could cover, by the key of the element: the host hides a page only where one is over it.
    let mut floats: Vec<&'static str> = Vec::new();
    // The autocomplete popup, 6 px above the composer's frame and as wide as it; it never takes focus from the text area.
    if s.ui.open.is_none()
        && let Some(popup) = if s.on_new_session_page() { s.new_popup() } else { s.auto.popup.as_ref() }
        && s.pickers.composer_w > 0.0
    {
        let panel = crate::view::autocomplete::popup(s, t, popup, s.pickers.composer_w);
        let anchor = if s.on_new_session_page() { "new-session-frame" } else { "composer-frame" };
        layer.push(panel.anchored_fit(anchor, rune::Side::Top, rune::Align::Start, 6.0));
        floats.push("autocomplete-popup");
    }
    // The completion popup under the code's caret; it never takes focus from the code. Its own function: this one's frame is large
    // in a debug build, and Windows' main thread has 1 MB of stack.
    if s.ui.open.is_none()
        && let Some(pop) = crate::view::completion::anchored(s, t)
    {
        layer.push(pop);
        floats.push("completion");
    }
    // The rail's tooltip, while nothing else is open over the window.
    if s.ui.open.is_none()
        && let Some(tip) = crate::view::rail::tip(s, t)
    {
        layer.push(tip);
        floats.push("rail-tip");
    }
    let before_dialogs = layer.len();
    if let Some(choice) = &s.machine_switch_scene {
        layer.extend(crate::view::machine_switch::overlay(s, t, crate::view::machine_switch::SCENE, choice));
    }
    if s.rail == crate::Rail::Settings {
        layer.extend(crate::view::settings::machine_header::overlay(s, t));
    }
    if s.rail == crate::Rail::Analytics {
        layer.extend(crate::view::analytics_page::menu::overlay(s, t));
    }
    if s.rail == crate::Rail::Automations {
        layer.extend(crate::view::automations::list::overlay(s, t));
    }
    if s.rail == crate::Rail::Workflows {
        layer.extend(crate::view::workflows::list::overlay(s, t));
        layer.extend(crate::view::workflows::roles::overlay(s, t));
    }
    if let Some(wizard) = crate::view::harness_setup::dialog(s, t) {
        layer.extend(wizard);
    }
    if let Some(delete) = crate::view::project_dialogs::delete(s, t) {
        layer.extend(delete);
    }
    if let Some(new) = crate::view::new_project::view(s, t) {
        layer.extend(new);
    }
    for overlay in s.places.overlays(&s.pickers.prefs) {
        layer.extend(overlay(s, t));
    }
    if let Some(ask) = confirm::view(s, t) {
        layer.extend(ask);
    }
    if let Some(weave) = crate::view::settings::harness_weave_dialog::view(s, t) {
        layer.extend(weave);
    }
    if let Some(edit) = crate::view::settings::skill_editor::view(s, t) {
        layer.extend(edit);
    }
    if let Some(history) = crate::view::settings::skill_history::view(s, t) {
        layer.extend(history);
    }
    if let Some(open) = crate::view::browser_open::view(s, t) {
        layer.extend(open);
    }
    if let Some(fork) = crate::view::fork::view(s, t) {
        layer.extend(fork);
    }
    if let Some(export) = crate::view::export::dialog(s, t) {
        layer.extend(export);
    }
    if let Some(report) = crate::view::report::dialog(s, t) {
        layer.extend(report);
    }
    if let Some(improve) = crate::view::improve_skill::dialog(s, t) {
        layer.extend(improve);
    }
    if let Some(update) = crate::view::update::dialog(s, t) {
        layer.extend(update);
    }
    if let Some(quit) = crate::view::quit_dialog::view(s, t) {
        layer.extend(quit);
    }
    if let Some(full) = crate::view::page_card::dialog(s, t) {
        layer.extend(full);
    }
    if let Some(ask) = crate::view::unsaved_dialog::view(s, t) {
        layer.extend(ask);
    }
    if let Some(card) = crate::view::annotation::overlay(s, t) {
        layer.push(card);
    }
    if let Some(lightbox) = &s.images.lightbox {
        layer.extend(crate::view::image::lightbox(s, t, lightbox));
    }
    let dialog_up = layer.len() > before_dialogs;
    // The message history list sits on the frame's top edge, as wide as it (row 8.3).
    if s.ui.open.is_none()
        && s.pickers.composer_w > 0.0
        && let Some(list) = crate::view::history::list(s, t, s.pickers.composer_w)
    {
        layer.push(list.anchored_fit("composer-frame", rune::Side::Top, rune::Align::Start, 0.0));
        floats.push("input-history");
    }
    if let Some(card) = crate::view::notice_card::view(s, t) {
        layer.push(card);
        floats.push("notice-card");
    }
    // One toast: what the archive waits on or failed at, else a plain message (`ArchiveUndoToast`).
    let queued = s.queue_toast();
    if let Some(toast) = queued.as_ref().or(s.ui.toast.as_ref()) {
        layer.push(
            rune::col()
                .key("toast-frame")
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .justify_end()
                .items_center()
                .pb(16.0)
                .child(toast::view(t, toast, s.clock)),
        );
        floats.push("toast");
    }
    // Menus, popovers, selects, the palette and every dialog dim or reach over the page whole; the floating ones only where they are.
    s.web.cover.set(&floats, menu_open || dialog_up);
    layer
}
