//! Reads that follow each other read as one row ("Read 8 files") even when the agent made them in separate
//! steps, which the harness delivers as separate messages (`lib/merge-reads.ts`). Only the tool rows are
//! joined: a message with any text, thinking, image or question of its own between two reads keeps them apart,
//! since it is something the reader should see in between.
use super::tools::{ToolRowItem, is_groupable_read};
use crate::view::conversation::{Message, Part, Role};

impl ToolRowItem for Part {
    fn id(&self) -> &str {
        match self {
            Part::Tool { id, .. } => id,
            _ => "",
        }
    }
    fn kind(&self) -> &str {
        match self {
            Part::Tool { tool, .. } => tool,
            _ => "",
        }
    }
    fn status(&self) -> super::tools::ToolStatus {
        match self {
            Part::Tool { status, .. } => *status,
            _ => super::tools::ToolStatus::Other,
        }
    }
}

fn is_done_read(part: &Part) -> bool {
    matches!(part, Part::Tool { .. }) && is_groupable_read(part)
}

/// Nothing drawn here: text, thinking or a summary with only blanks in it.
fn is_blank(part: &Part) -> bool {
    match part {
        Part::Text { text, .. } => text.trim().is_empty(),
        Part::Reasoning { text, summary, .. } => text.trim().is_empty() && summary.as_deref().is_none_or(|s| s.trim().is_empty()),
        _ => false,
    }
}

/// The message shows nothing above its calls: no text, thinking, image, or anything else drawn before them.
fn starts_with_calls(message: &Message) -> bool {
    message.role == Role::Assistant
        && message.parts.iter().any(|p| matches!(p, Part::Tool { .. }))
        && message.parts.iter().all(|p| matches!(p, Part::Tool { .. }) || is_blank(p))
}

/// The message shows nothing after its calls either: its last drawn part is a call.
fn ends_with_calls(message: &Message) -> bool {
    message.role == Role::Assistant && message.parts.iter().rev().find(|p| !is_blank(p)).is_some_and(|p| matches!(p, Part::Tool { .. }))
}

/// Joins the leading reads of each message onto the reads that end the one before it. The rest of the message,
/// if any, stays a message of its own.
pub fn merge_read_runs(messages: Vec<Message>) -> Vec<Message> {
    let mut out: Vec<Message> = Vec::with_capacity(messages.len());
    for mut message in messages {
        let joins = out.last().is_some_and(|previous| {
            starts_with_calls(&message)
                && ends_with_calls(previous)
                && previous.parts.iter().rev().find(|p| !is_blank(p)).is_some_and(is_done_read)
                && message.parts.iter().find(|p| !is_blank(p)).is_some_and(is_done_read)
        });
        if !joins {
            out.push(message);
            continue;
        }
        // Take the run of reads off the front of this message.
        let calls: Vec<Part> = std::mem::take(&mut message.parts);
        let mut calls = calls.into_iter().filter(|p| !is_blank(p)).peekable();
        let previous = out.last_mut().expect("joins is only true with a previous message");
        while let Some(read) = calls.next_if(is_done_read) {
            previous.parts.push(read);
        }
        message.parts = calls.collect();
        if !message.parts.is_empty() {
            out.push(message);
        }
    }
    out
}
