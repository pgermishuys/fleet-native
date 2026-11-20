//! Component 24, piece H: what the menus do (`desktop::menu` lists them). One `State::menu_run` serves the macOS menu bar and the
//! palette's entries, and the folders it opens and the checker it wakes are behind `MenuState`, so a test can see them asked for.
use crate::desktop::menu::{self, Item};
use crate::{State, Tx};
use rune::{App, MenuItem};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Where the folders are and how one is opened.
#[derive(Clone)]
pub struct MenuState {
    /// `logs/` in the app's data folder (`openLogs`).
    pub logs: Option<PathBuf>,
    /// Fleet's data folder (`openDataFolder`).
    pub data: Option<PathBuf>,
    pub open: Arc<dyn Fn(&Path) + Send + Sync>,
}

impl Default for MenuState {
    fn default() -> Self {
        let get = |k: &str| std::env::var(k).ok();
        MenuState {
            logs: crate::composer::drafts::app_dir(get)
                .map(|d| crate::desktop::log::log_file(&d).parent().map(Path::to_path_buf).unwrap_or(d)),
            data: crate::fleet::instance::data_dir(get),
            open: Arc::new(crate::desktop::start::open_folder),
        }
    }
}

impl State {
    /// A menu item or palette entry was chosen.
    pub fn menu_run(&mut self, item: Item, tx: &mut Tx) {
        match item {
            Item::Quit => self.quit_requested(tx),
            Item::CheckForUpdates => self.menu_check_for_updates(tx),
            Item::ReportProblem => {
                self.report_open(crate::fleet::report::Source::Help, None, "");
            }
            Item::FindInConversation => self.find_request(tx),
            Item::OpenLogs => self.menu_open(self.menu.logs.clone()),
            Item::OpenDataFolder => self.menu_open(self.menu.data.clone()),
            Item::ToggleFullScreen => tx.toggle_full_screen(),
        }
    }

    fn menu_open(&self, dir: Option<PathBuf>) {
        if let Some(dir) = dir {
            (self.menu.open)(&dir);
        }
    }

    /// Fleet says in a dialog what a check found (`updateCheckMessage`); here a found build opens the update card and the rest
    /// is a note, because the native updater answers in builds, not versions (checklist 24.108).
    fn menu_check_for_updates(&mut self, tx: &mut Tx) {
        if self.update.available.is_some() {
            self.update_open();
            return;
        }
        let words = if self.update.wake.is_none() {
            menu::NO_SELF_UPDATE
        } else if self.update.checking || self.update.busy() {
            menu::ALREADY_CHECKING
        } else {
            self.update_check_now();
            menu::CHECKING
        };
        self.toast(words.into(), None, 4.0, tx);
    }
}

/// macOS: Fleet's menu bar. Elsewhere Rune has none, and the palette carries the items (decision 9).
pub fn install(app: App<State>) -> App<State> {
    if !cfg!(target_os = "macos") {
        return app;
    }
    items().into_iter().fold(app.standard_menus(), |app, item| app.menu_item(item))
}

pub(crate) fn items() -> Vec<MenuItem<State>> {
    menu::macos().into_iter().map(to_item).collect()
}

fn to_item(e: menu::Entry) -> MenuItem<State> {
    let mut item = match e.item {
        Some(i) => MenuItem::new(e.title, action(i)).id(format!("fleet-{i:?}")),
        None => MenuItem::separator(),
    };
    if let Some(m) = e.menu {
        item = item.menu(m);
    }
    if let Some(a) = e.after {
        item = item.after(a);
    }
    if let Some(k) = e.key.and_then(|k| k.parse().ok()) {
        item = item.key_equivalent(k);
    }
    if e.key_left_to_app {
        item = item.keys_first();
    }
    item
}

fn action(item: Item) -> fn(&mut State, &mut Tx) {
    match item {
        Item::Quit => |s, tx| s.menu_run(Item::Quit, tx),
        Item::CheckForUpdates => |s, tx| s.menu_run(Item::CheckForUpdates, tx),
        Item::ReportProblem => |s, tx| s.menu_run(Item::ReportProblem, tx),
        Item::FindInConversation => |s, tx| s.menu_run(Item::FindInConversation, tx),
        Item::OpenLogs => |s, tx| s.menu_run(Item::OpenLogs, tx),
        Item::OpenDataFolder => |s, tx| s.menu_run(Item::OpenDataFolder, tx),
        Item::ToggleFullScreen => |s, tx| s.menu_run(Item::ToggleFullScreen, tx),
    }
}
