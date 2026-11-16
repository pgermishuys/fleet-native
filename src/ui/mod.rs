//! Fleet's kit (`client/src/components/ui/`) and its command palette, in Rune. One file per
//! control; every colour comes from `theme.rs`. Controls are functions that return elements;
//! overlays (menus, selects, popovers, dialogs, the palette) are mounted once, at the root, from
//! `Ui::open`, anchored to the key of the element that opened them.
pub mod age;
pub mod button;
pub mod chip;
pub mod commands;
pub mod confirm;
pub mod controls;
pub mod dialog;
pub mod directory_picker;
pub mod gallery;
pub mod menu;
pub mod menus;
pub(crate) mod overlay;
pub mod palette;
pub mod scenes;
pub mod select;
#[cfg(test)]
mod tests;
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
    Dialog,
    Alert,
    Sheet(dialog::SheetSide),
}

pub struct Ui {
    pub open: Option<Open>,
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
}

impl Default for Ui {
    fn default() -> Self {
        Ui {
            open: None,
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
        Some(Open::Menu(name)) => menus::dropdown(s, t, name),
        Some(Open::Context(p, i)) => menus::session_context(s, t, p, i),
        Some(Open::ProjectMenu(p)) => menus::project_context(s, t, p),
        Some(Open::MachineContext) => menus::machine_session_context(s, t),
        Some(Open::MachineProjectMenu) => menus::machine_project_context(s, t),
        Some(Open::MachineHeadingMenu) => menus::machine_heading_context(s, t),
        Some(Open::Select(name)) => gallery::select_content(s, t, name),
        Some(Open::Popover("context")) => crate::view::ring_popover::overlay(s, t),
        Some(Open::Popover("allowed")) => crate::view::allowed::overlay(s, t),
        Some(Open::Popover("starters")) => crate::view::starters::menu(t),
        Some(Open::Popover("folder")) => crate::view::folder_menu::overlay(s, t),
        Some(Open::Popover(name)) => gallery::popover_content(s, t, name),
        Some(Open::Directory) => directory_picker::view(s, t, "g-dir"),
        Some(Open::Dialog) => gallery::dialog(s, t),
        Some(Open::Alert) => gallery::alert(s, t),
        Some(Open::Sheet(side)) => gallery::sheet(s, t, side),
    };
    // The autocomplete popup, 6 px above the composer's frame and as wide as it; it never takes focus from the text area.
    if s.ui.open.is_none()
        && let Some(popup) = &s.auto.popup
        && s.pickers.composer_w > 0.0
    {
        let panel = crate::view::autocomplete::popup(s, t, popup, s.pickers.composer_w);
        layer.push(panel.anchored_fit("composer-frame", rune::Side::Top, rune::Align::Start, 6.0));
    }
    if let Some(delete) = crate::view::project_dialogs::delete(s, t) {
        layer.extend(delete);
    }
    if let Some(ask) = confirm::view(s, t) {
        layer.extend(ask);
    }
    if let Some(export) = crate::view::export::dialog(s, t) {
        layer.extend(export);
    }
    if let Some(update) = crate::view::update::dialog(s, t) {
        layer.extend(update);
    }
    if let Some(lightbox) = &s.images.lightbox {
        layer.extend(crate::view::image::lightbox(s, t, lightbox));
    }
    // The message history list sits on the frame's top edge, as wide as it (row 8.3).
    if s.ui.open.is_none()
        && s.pickers.composer_w > 0.0
        && let Some(list) = crate::view::history::list(s, t, s.pickers.composer_w)
    {
        layer.push(list.anchored_fit("composer-frame", rune::Side::Top, rune::Align::Start, 0.0));
    }
    if let Some(toast) = &s.ui.toast {
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
    }
    layer
}
