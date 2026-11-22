//! Mods (component 22, `docs/parity/22-mods.md`). Plain Rust (a field's text is Rune's `TextEdit`, nothing else of Rune's): the contract a mod's tree must keep, the check
//! that enforces it, and where a site finds the tree it draws. Ports weave-fleet's `client/src/lib/mods/` (`types.ts`,
//! `validate.ts`, `resolve.ts`, `points.ts`, `failures.ts`) as of its commit `c1a48bd`.
//!
//! A tree is kept as `serde_json::Value`, exactly as it arrives on the wire, so Fleet's tests can be read against ours
//! line by line. Nothing here draws; the views (pieces B onward) read what `resolve` hands them.
pub mod actions;
pub mod controls;
pub mod failures;
pub mod input_rate;
pub mod markdown;
pub mod panes;
pub mod points;
pub mod renders;
pub mod renders_state;
pub mod resolve;
pub mod rows;
pub mod types;
pub mod validate;
