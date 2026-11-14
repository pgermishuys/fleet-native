//! Talking to a Fleet server (`docs/FLEET.md`). The pure parts (`model`, `events`, `reducer`, `signalr`,
//! `sessions`, `view`) have no Rune and no I/O, and are tested against frames captured from a
//! real server (`tests/fixtures/`). The impure parts (`http`, `link`) do the network work on their
//! own threads and hand results to the state.
pub mod delegations;
pub mod events;
pub mod http;
pub mod link;
pub mod model;
pub mod reducer;
pub mod sessions;
pub mod signalr;
pub mod state;
pub mod view;

#[cfg(test)]
mod tests;
