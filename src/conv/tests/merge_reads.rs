//! `lib/__tests__/merge-reads.test.ts`. The sixth case there ("hands back the same array for the same messages") is
//! about a JavaScript identity cache and has no counterpart here: these functions take and give owned values.
use crate::conv::merge_reads::merge_read_runs;
use crate::conv::tools::ToolStatus;
use crate::view::conversation::{Message, Part};

fn tool(id: &str, kind: &str, status: ToolStatus) -> Part {
    match Part::tool(id, kind, id, crate::view::conversation::ToolState::Plain) {
        Part::Tool { id, tool, call_id, detail, state, body, agent, .. } => {
            Part::Tool { id, tool, call_id, detail, status, state, body, agent }
        }
        other => other,
    }
}
fn read(id: &str) -> Part {
    tool(id, "read", ToolStatus::Completed)
}
fn msg(id: &str, parts: Vec<Part>) -> Message {
    Message::assistant(id, parts)
}
fn ids(m: &Message) -> Vec<&str> {
    m.parts.iter().map(|p| if let Part::Tool { id, .. } = p { id.as_str() } else { "other" }).collect()
}

#[test]
fn joins_reads_across_messages_into_the_first_messages_tools() {
    let out = merge_read_runs(vec![msg("a", vec![read("1"), read("2")]), msg("b", vec![read("3")]), msg("c", vec![read("4"), read("5")])]);
    assert_eq!(out.len(), 1);
    assert_eq!(ids(&out[0]), ["1", "2", "3", "4", "5"]);
}

#[test]
fn keeps_messages_apart_when_text_thinking_or_another_call_sits_between() {
    let with_text = merge_read_runs(vec![msg("a", vec![read("1")]), msg("b", vec![read("2"), Part::text("t", "Now this.")])]);
    assert_eq!(with_text.len(), 2);
    let text_first = merge_read_runs(vec![msg("a", vec![read("1")]), msg("b", vec![Part::text("t", "Now this."), read("2")])]);
    assert_eq!(text_first.len(), 2);
    let thinking = Part::Reasoning { id: "r".into(), text: "hm".into(), summary: None };
    assert_eq!(merge_read_runs(vec![msg("a", vec![read("1")]), msg("b", vec![thinking, read("2")])]).len(), 2);
    let after_edit = merge_read_runs(vec![msg("a", vec![read("1"), tool("x", "edit", ToolStatus::Completed)]), msg("b", vec![read("2")])]);
    assert_eq!(after_edit.len(), 2);
}

#[test]
fn moves_only_the_leading_reads_of_the_next_message_and_keeps_its_other_calls() {
    let out = merge_read_runs(vec![msg("a", vec![read("1")]), msg("b", vec![read("2"), tool("3", "edit", ToolStatus::Completed)])]);
    assert_eq!(out.iter().map(ids).collect::<Vec<_>>(), [vec!["1", "2"], vec!["3"]]);
}

#[test]
fn joins_a_read_that_is_still_running_but_not_one_that_failed() {
    let running = merge_read_runs(vec![msg("a", vec![read("1")]), msg("b", vec![tool("2", "read", ToolStatus::Running)])]);
    assert_eq!(running.len(), 1);
    let failed = merge_read_runs(vec![msg("a", vec![read("1")]), msg("b", vec![tool("2", "read", ToolStatus::Error)])]);
    assert_eq!(failed.len(), 2);
}
