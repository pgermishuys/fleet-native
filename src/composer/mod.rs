//! The composer's logic, with nothing of Rune in it: what the session's status and harness allow, where a
//! draft goes, the message history, and a draft per session. Ports `use-composer-actions.ts`,
//! `shell-commands.ts` (the draft half), `side-conversation.ts` (the parse), `use-input-history.ts` and
//! `draft-storage.ts`. The views and the requests that use it come in the later pieces of component 5.
pub mod drafts;
pub mod history;
pub mod route;
pub mod status;

#[cfg(test)]
mod tests;
