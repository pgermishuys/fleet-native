//! The New session page's plain-Rust core (component 7): the request a session is created with, the plan line, and the
//! worktree naming preview. No Rune and no I/O in this folder; later pieces' views read these types.
pub mod naming;
pub mod pattern;
pub mod plan;
pub mod request;

#[cfg(test)]
mod tests;
