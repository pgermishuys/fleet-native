//! Fleet's tests for the tool table and the tool rows, under the same names (`lib/tools/__tests__/registry.test.ts`,
//! `lib/__tests__/tool-labels.test.ts`, `tool-classification.test.ts`, `tool-row.test.ts`, `merge-reads.test.ts`,
//! `components/session/__tests__/ToolCard.rows.test.ts`, `MessageToolList.test.ts`, `reasoning-gist.test.ts`, `compaction.test.ts`, `shell-commands.test.ts`, `attached-files.test.ts`, `file-references.test.ts`, `format-utils.ts`, `turn-fold.test.ts`, `turn-fold-row-stats.test.ts`, `merge-messages.test.ts`,
//! `ActivityStream.messages.test.ts` and `ActivityStream.thinking.test.ts`, the list ones).
mod attached;
mod bubble;
mod card;
mod compaction;
mod diff;
mod fold;
mod gist;
mod merge_reads;
mod message_list;
mod output;
mod references;
mod registry;
mod shell;
mod time;
mod tool_row;
