//! The desktop shell (component 24, `docs/parity/24-desktop-shell.md`): what weave-fleet's `desktop/src` does around the
//! window. Piece B shows the start in the window (`start`, `view::start_page`). Piece A is the server process: finding the binary to start (`paths`), starting it in desktop mode, watching it
//! and stopping it (`server`), and its log (`log`). Plain Rust with no Rune; every blocking call runs on a thread of its
//! own and talks back over a channel.
pub mod log;
pub mod paths;
pub mod server;
pub mod start;

#[cfg(test)]
mod start_tests;
#[cfg(test)]
mod tests;
