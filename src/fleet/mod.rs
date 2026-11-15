//! Talking to a Fleet server (`docs/FLEET.md`). The pure parts (`model`, `events`, `reducer`, `signalr`,
//! `sessions`, `view`) have no Rune and no I/O, and are tested against frames captured from a
//! real server (`tests/fixtures/`). The impure parts (`http`, `link`) do the network work on their
//! own threads and hand results to the state.
pub mod catalog;
pub mod context;
pub mod delegations;
pub mod events;
pub mod history;
pub mod http;
pub mod instance;
pub mod link;
pub mod model;
pub mod prefs;
pub mod presence;
pub mod prompts;
pub mod reducer;
pub mod refresh;
pub mod retry;
pub mod rows;
pub mod selector;
pub mod sessions;
pub mod signalr;
pub mod state;
pub mod topics;
pub mod view;
pub mod work;

#[cfg(test)]
mod tests;
