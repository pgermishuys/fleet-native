//! The composer's logic, with nothing of Rune in it: what the session's status and harness allow, where a
//! draft goes, the message history, a draft per session, what the agent and model pickers mean by Default, the
//! effort remembered per model, and the model picker's sections. Ports `use-composer-actions.ts`,
//! `shell-commands.ts` (the draft half), `side-conversation.ts` (the parse), `use-input-history.ts`,
//! `draft-storage.ts`, `agent-model-choice.ts`, `effort-memory.ts`, `use-effort-memory.ts`,
//! `model-picker-groups.ts`, `stores/model-picker.ts`, `lib/format/numbers.ts` and the context ring's maths (`context-usage.ts`). The views and the requests
//! that use it come in the later pieces of component 5.
pub mod choice;
pub mod details;
pub mod drafts;
pub mod effort;
pub mod format;
pub mod groups;
pub mod history;
pub mod ring;
pub mod route;
pub mod selector;
pub mod status;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_choice;
#[cfg(test)]
mod tests_details;
#[cfg(test)]
mod tests_ring;
#[cfg(test)]
mod tests_selector;
