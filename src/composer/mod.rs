//! The composer's logic, with nothing of Rune in it: what the session's status and harness allow, where a
//! draft goes, the message history, a draft per session, what the agent and model pickers mean by Default, the
//! effort remembered per model, and the model picker's sections. Ports `use-composer-actions.ts`,
//! `shell-commands.ts` (the draft half), `side-conversation.ts` (the parse), `use-input-history.ts`,
//! `draft-storage.ts`, `agent-model-choice.ts`, `effort-memory.ts`, `use-effort-memory.ts`,
//! `model-picker-groups.ts`, `use-autocomplete.ts` (the triggers, items, keys and picks), `session-references.ts`,
//! `skill-mentions.ts`, `file-matches.ts`, `stores/model-picker.ts`, `lib/format/numbers.ts` and the context ring's maths (`context-usage.ts`). The views and the requests
//! that use it come in the later pieces of component 5.
pub mod choice;
pub mod details;
pub mod draftrefs;
pub mod drafts;
pub mod effort;
pub mod files;
pub mod format;
pub mod groups;
pub mod history;
pub mod refs;
pub mod ring;
pub mod route;
pub mod selector;
pub mod status;
pub mod triggers;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_autocomplete;
#[cfg(test)]
mod tests_choice;
#[cfg(test)]
mod tests_details;
#[cfg(test)]
mod tests_draftrefs;
#[cfg(test)]
mod tests_ring;
#[cfg(test)]
mod tests_selector;
