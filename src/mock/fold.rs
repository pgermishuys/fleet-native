//! The "turn folding" scenes (piece 3F): a turn that finished and one still running, written as the server sends them,
//! run through the list and `fold_turns`, and drawn as plain rows with one text line where the fold row will go (piece
//! G draws the row itself). What the scene shows is what the logic decided: which rows went, which stayed.
use super::tools::{call, message};
use crate::conv::fold::{FoldOptions, fold_turns, summary_parts};
use crate::conv::gist::Thinking;
use crate::conv::message_list::{ListContext, ListMessage, build_list};
use crate::fleet::delegations::Delegation;
use crate::fleet::{reducer, view};
use crate::view::conversation::{Message, Part};
use serde_json::{Value, json};
use std::collections::HashSet;

fn text(id: &str, body: &str) -> Value {
    json!({"type": "text", "id": id, "text": body})
}

fn stamped(id: &str, role: &str, created: i64, completed: Option<i64>, parts: Vec<Value>) -> Value {
    let mut m = message(id, role, created, parts);
    if let Some(done) = completed {
        m["info"]["time"]["completed"] = json!(done);
    }
    m["info"]["agent"] = json!("build");
    m
}

fn conversation() -> Value {
    let ok = "completed";
    json!({"session": {}, "delegations": [{
        "delegationId": "d1", "parentToolCallId": "call-t5", "childSessionId": "child1", "title": "Find the off-by-one", "status": "completed"
    }], "activityStatus": "idle", "messages": [
        stamped("u1", "user", 1_000, None, vec![text("u1-t", "Run the tests, then have someone find why the last one fails.")]),
        stamped("a1", "assistant", 2_000, Some(9_000), vec![
            json!({"type": "reasoning", "id": "r1", "text": "Run them first, then split the failure off.", "summary": null}),
            call("t1", "bash", ok, json!({"command": "bun test"}), json!({"metadata": {"exit": 1}})),
            call("t2", "read", ok, json!({"filePath": "src/orders/total.ts"}), json!({})),
            call("t3", "read", ok, json!({"filePath": "src/orders/total.test.ts"}), json!({})),
        ]),
        stamped("a2", "assistant", 10_000, Some(31_000), vec![
            call("t5", "task", ok, json!({"subagent_type": "general", "description": "Find the off-by-one"}), json!({})),
            call("t6", "bash", "error", json!({"command": "bun test total"}), json!({"error": "1 test failed"})),
        ]),
        stamped("a3", "assistant", 32_000, Some(34_000), vec![text("a3-t", "The paging loop stops one row early: `i < n - 1` should be `i < n`.")]),
        stamped("u2", "user", 40_000, None, vec![text("u2-t", "Fix it.")]),
        stamped("b1", "assistant", 41_000, None, vec![call("t7", "read", ok, json!({"filePath": "src/orders/page.ts"}), json!({}))]),
    ]})
}

/// The conversation after folding: the first turn is finished, the second still runs. `open` opens the first again.
pub fn of_folded_turns(open: bool) -> Vec<Message> {
    let state = reducer::create_state(&conversation());
    let delegations: Vec<Delegation> = state.delegations.clone();
    let context = ListContext { delegations: &delegations, models: &[], thinking: Thinking::Folded };
    let list = build_list(&state.messages, &[], &context);
    let open: HashSet<String> = open.then(|| "u1".to_string()).into_iter().collect();
    let folded = fold_turns(&list, &FoldOptions { running: true, keep_last: false, directory: None, open: &open });
    let drawn = view::messages(&state.messages);
    drawn
        .into_iter()
        .filter_map(|message| {
            let found = folded.iter().find(|f| f.id == message.id);
            apply(message, found)
        })
        .collect()
}

/// A message as folding left it: only the calls it kept, no thinking when folded, and a line in the host's place.
fn apply(mut message: Message, folded: Option<&ListMessage>) -> Option<Message> {
    let folded = folded?;
    let kept: HashSet<&str> = folded.tools.iter().map(|t| t.id.as_str()).collect();
    message.parts.retain(|p| match p {
        Part::Tool { id, .. } => kept.contains(id.as_str()),
        Part::Reasoning { .. } => !folded.reasoning.is_empty(),
        _ => true,
    });
    if let Some(turn) = &folded.fold {
        let seconds = turn.summary.duration_ms.map_or(String::new(), |ms| format!("Worked for {}s \u{b7} ", ms / 1000));
        let line = format!("{seconds}{}", summary_parts(&turn.summary).join(" \u{b7} "));
        message.parts.insert(0, Part::text(format!("{}-fold", message.id), format!("**{line}**")));
    }
    // Nothing but the line is left: the fold row alone stands for the message.
    (!folded.fold_only || folded.fold.is_some()).then_some(message)
}
