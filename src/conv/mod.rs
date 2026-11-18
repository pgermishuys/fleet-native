//! What a conversation says about its parts, as plain logic with no Rune in it: the tool table
//! (`lib/tools/` in weave-fleet), a call's status and duration, and which rows fold together.
//! Views under `src/view/` draw from these; tests under `tests/` are Fleet's, under the same names.
pub mod agent_tabs;
pub mod attached;
pub mod background;
pub mod browser;
pub mod bubble;
pub mod card;
pub mod compaction;
pub mod diff;
pub mod export;
pub mod export_dialog;
pub mod export_history;
pub mod find;
pub mod fold;
pub mod fold_plan;
pub mod gist;
pub mod image_sniff;
pub mod merge_reads;
pub mod message_list;
pub mod output;
pub mod pin;
pub mod references;
pub mod retry;
pub mod review;
pub mod shell;
pub mod time;
pub mod tools;
pub mod turn_error;
pub mod work_clock;
pub mod work_output;
pub mod working;

#[cfg(test)]
mod tests;
