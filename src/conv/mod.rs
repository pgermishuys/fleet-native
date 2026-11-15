//! What a conversation says about its parts, as plain logic with no Rune in it: the tool table
//! (`lib/tools/` in weave-fleet), a call's status and duration, and which rows fold together.
//! Views under `src/view/` draw from these; tests under `tests/` are Fleet's, under the same names.
pub mod card;
pub mod diff;
pub mod merge_reads;
pub mod output;
pub mod tools;

#[cfg(test)]
mod tests;
