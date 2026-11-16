//! The New session page's plain-Rust core (component 7): the request a session is created with, the plan line, and the
//! worktree naming preview. No Rune and no I/O in this folder; later pieces' views read these types.
pub mod defaults;
pub mod draft;
pub mod feed_calls;
pub mod feed_state;
pub mod feeds;
pub mod folder;
pub mod folder_menu;
pub mod harness_choice;
pub mod naming;
pub mod page;
pub mod pattern;
pub mod plan;
pub mod request;
pub mod starters;
pub mod strip;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_feeds;
#[cfg(test)]
mod tests_folder;
#[cfg(test)]
mod tests_state;
