//! What the agent asks and how it is worded (component 4, `docs/parity/04-questions-and-permissions.md`). Plain Rust,
//! no Rune: the views in `view::permission` read it, and the stores of later pieces fill it.
pub mod focus;
pub mod permission;
pub mod wording;

#[cfg(test)]
mod tests;
