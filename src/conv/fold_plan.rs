//! What the stream draws of its messages once finished turns fold, as plain data the view reads: which messages are gone,
//! which one hosts each fold row, which tool calls and thinking blocks stay. It is `foldTurns` over `buildList` of the
//! reducer's messages, kept beside the view's own messages by id (the view's `Message` is drawn from the same ones).
use super::fold::{FoldOptions, fold_turns};
use super::message_list::{ListContext, ListMessage, build_list};
use crate::fleet::model::Msg;
use crate::fleet::prompts::SentPrompt;
use std::collections::{HashMap, HashSet};

/// What folding left of one message.
#[derive(Clone, Debug, PartialEq)]
pub struct Kept {
    /// The fold row this message hosts, with whether its turn is open.
    pub fold: Option<(super::message_list::FoldedTurn, bool)>,
    /// Nothing but the fold row is left of the message.
    pub fold_only: bool,
    /// The calls that stay drawn; every other call of the message is folded away.
    pub tools: HashSet<String>,
    /// Whether the message's thinking stays drawn.
    pub reasoning: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FoldPlan {
    /// Messages folded out of sight (by id). A prompt that is still on its way is never in here.
    pub hidden: HashSet<String>,
    /// Messages folding changed. Every other message draws as it is.
    pub kept: HashMap<String, Kept>,
    /// Where the messages that still draw sit among the session's messages; `None` when every one draws.
    pub visible: Option<Vec<usize>>,
}

impl FoldPlan {
    /// How many of `len` messages draw.
    pub fn count(&self, len: usize) -> usize {
        self.visible.as_ref().map_or(len, Vec::len)
    }

    /// The session message the `row`th drawn row is.
    pub fn index(&self, row: usize, len: usize) -> Option<usize> {
        match &self.visible {
            Some(v) => v.get(row).copied().filter(|&m| m < len),
            None => (row < len).then_some(row),
        }
    }

    /// Records which of the session's messages (by id, in order) draw.
    pub fn place<'a>(&mut self, ids: impl Iterator<Item = &'a str>) {
        self.visible = if self.hidden.is_empty() {
            None
        } else {
            Some(ids.enumerate().filter(|(_, id)| !self.hidden.contains(*id)).map(|(i, _)| i).collect())
        };
    }

    pub fn build(messages: &[Msg], sent: &[SentPrompt], context: &ListContext, options: &FoldOptions) -> FoldPlan {
        let list = build_list(messages, sent, context);
        let folded = fold_turns(&list, options);
        let shown: HashSet<&str> = folded.iter().map(|m| m.id.as_str()).collect();
        let mut plan = FoldPlan::default();
        for before in &list {
            if !shown.contains(before.id.as_str()) {
                plan.hidden.insert(before.id.clone());
            }
        }
        let originals: HashMap<&str, &ListMessage> = list.iter().map(|m| (m.id.as_str(), m)).collect();
        for after in &folded {
            let before = originals[after.id.as_str()];
            let changed = after.fold.is_some()
                || after.fold_only
                || after.tools.len() != before.tools.len()
                || after.reasoning.len() != before.reasoning.len();
            if changed {
                plan.kept.insert(
                    after.id.clone(),
                    Kept {
                        fold: after.fold.clone().map(|f| {
                            let open = options.open.contains(&f.id);
                            (f, open)
                        }),
                        fold_only: after.fold_only,
                        tools: after.tools.iter().map(|t| t.id.clone()).collect(),
                        reasoning: !after.reasoning.is_empty(),
                    },
                );
            }
        }
        // The messages the stream drops for having nothing to show (an empty assistant message) are gone from the list too.
        let listed: HashSet<&str> = list.iter().map(|m| m.id.as_str()).collect();
        for m in messages {
            if !listed.contains(m.id.as_str()) {
                plan.hidden.insert(m.id.clone());
            }
        }
        plan
    }
}
