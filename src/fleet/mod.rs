//! Talking to a Fleet server (`docs/FLEET.md`). The pure parts (`model`, `events`, `reducer`, `signalr`,
//! `sessions`, `view`) have no Rune and no I/O, and are tested against frames captured from a
//! real server (`tests/fixtures/`). The impure parts (`http`, `link`) do the network work on their
//! own threads and hand results to the state.
pub mod attach;
pub mod autocomplete;
pub mod browser;
pub mod catalog;
pub mod command;
pub mod compact;
pub mod context;
pub mod delegations;
pub mod events;
pub mod export;
pub mod find;
pub mod finder;
pub mod groups;
pub mod history;
pub mod http;
pub mod images;
pub mod instance;
pub mod interrupted;
pub mod link;
pub mod model;
pub mod modes;
pub mod prefs;
pub mod presence;
pub mod prompts;
pub mod queue;
pub mod reducer;
pub mod references;
pub mod refresh;
pub mod retry;
pub mod rows;
pub mod scroll;
pub mod selector;
pub mod sessions;
pub mod signalr;
pub mod state;
pub mod topics;
pub mod uploads;
pub mod usage;
pub mod view;
pub mod work;
pub mod work_actions;

#[cfg(test)]
mod tests;
