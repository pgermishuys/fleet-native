//! The desktop shell (component 24, `docs/parity/24-desktop-shell.md`): what weave-fleet's `desktop/src` does around the
//! window. Piece B shows the start in the window (`start`, `view::start_page`). Piece A is the server process: finding the binary to start (`paths`), starting it in desktop mode, watching it
//! and stopping it (`server`), and its log (`log`). Piece C is the login shell's environment (`shell_env`). Piece E is closing, quitting and the tray (`close`, `tray`). Piece H is the menus (`menu`, and `crate::menu_bar` for what they do). Piece G is the window's place and the one-instance rule (`window`). Plain Rust with no Rune; every blocking call runs on a thread of its
//! own and talks back over a channel.
pub mod close;
pub mod log;
pub mod menu;
pub mod notify;
pub mod paths;
pub mod server;
pub mod shell_env;
pub mod start;
pub mod tray;
pub mod window;

#[cfg(test)]
mod close_tests;
#[cfg(test)]
mod menu_tests;
#[cfg(all(test, target_os = "linux"))]
mod notify_dbus_tests;
#[cfg(test)]
mod start_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_shell_env;
