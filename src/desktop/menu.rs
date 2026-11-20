//! Component 24, piece H: the menus of `desktop/src/menu.ts` (`buildAppMenu`). Rune draws a menu bar on macOS only, so macOS
//! gets Fleet's; Windows and Linux (where Electron hides the bar anyway) get the items that live nowhere else as palette
//! entries (`ui/commands.rs`, decision 9). Both call the same `State::menu_run`, so an item does one thing wherever it is.
//! The list is data so a test can read it: Rune's `MenuItem` can't be inspected.

/// What a menu item or a palette entry asks for (`MenuActions` in `menu.ts`, and Quit Fleet).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Quit,
    CheckForUpdates,
    ReportProblem,
    FindInConversation,
    OpenLogs,
    OpenDataFolder,
    ToggleFullScreen,
}

/// One line of the macOS menu bar. `menu: None` is the app menu ("Fleet"), whose first items are Rune's own (About, Services,
/// Hide, Quit); the item goes after About.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub menu: Option<&'static str>,
    pub title: &'static str,
    pub item: Option<Item>,
    /// Shown beside the item (`mod` is ⌘ here); `None` for none.
    pub key: Option<&'static str>,
    /// `registerAccelerator: false`: the key reaches the window first and the item runs only if nothing there takes it.
    pub key_left_to_app: bool,
    /// An AppKit action (`selectAll:`) the item follows, so it lands in the standard menu where Electron puts it.
    pub after: Option<&'static str>,
}

const fn line(menu: &'static str, title: &'static str, item: Item) -> Entry {
    Entry { menu: Some(menu), title, item: Some(item), key: None, key_left_to_app: false, after: None }
}

const fn bar() -> Entry {
    Entry { menu: None, title: "", item: None, key: None, key_left_to_app: false, after: None }
}

/// macOS's items, in order. The Edit items (Undo … Select All) and Window's (Minimize, Close) are Rune's `standard_menus`;
/// "Reload", "Force Reload" and "Toggle DevTools" have no page to act on (24.103), and zoom is the palette's and Appearance's (24.104).
pub fn macos() -> Vec<Entry> {
    vec![
        Entry { menu: None, title: "Check for Updates…", item: Some(Item::CheckForUpdates), ..bar() },
        // Edit → Find in Conversation: shown with ⌘F, the key itself handled by the conversation (MT "has Edit → Find in Conversation…").
        Entry { menu: Some("Edit"), after: Some("selectAll:"), ..bar() },
        Entry {
            key: Some("mod+f"),
            key_left_to_app: true,
            after: Some("selectAll:"),
            ..line("Edit", "Find in Conversation", Item::FindInConversation)
        },
        Entry { key: Some("ctrl+super+f"), ..line("View", "Toggle Full Screen", Item::ToggleFullScreen) },
        line("Help", "Report a Problem…", Item::ReportProblem),
        Entry { menu: Some("Help"), ..bar() },
        line("Help", "Open Logs Folder", Item::OpenLogs),
        line("Help", "Open Fleet Data Folder", Item::OpenDataFolder),
    ]
}

/// The palette's entries for what only a menu bar shows on macOS (`Item`, id, label, description): there the bar's items
/// are Fleet's; here they are not Fleet's place for them, and flagged so (decision 9).
pub fn palette() -> [(Item, &'static str, &'static str, &'static str); 4] {
    [
        (Item::Quit, "quit-fleet", "Quit Fleet", "Close Fleet and stop the Fleet it started."),
        (Item::CheckForUpdates, "check-for-updates", "Check for updates", "Look for a newer build of Fleet now."),
        (Item::OpenLogs, "open-logs-folder", "Open logs folder", "Show where Fleet writes its server log."),
        (Item::OpenDataFolder, "open-data-folder", "Open Fleet data folder", "Show the folder that holds Fleet's data."),
    ]
}

/// Fleet's words for a check that can't run or is already running (`updateCheckMessage`).
pub const NO_SELF_UPDATE: &str = "This build of Fleet doesn't update itself.";
pub const ALREADY_CHECKING: &str = "Fleet is already checking for updates.";
pub const CHECKING: &str = "Checking for updates…";
