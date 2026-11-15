//! The "conversation parts" scenes (piece 3D): reasoning, commands the user ran, a compaction and Fleet's recap, written
//! as the server sends them and boiled down by the code a live conversation goes through.
use super::tools::{conversation, message};
use crate::State;
use crate::mock::Status;
use serde_json::{Value, json};

fn text(id: &str, body: &str) -> Value {
    json!({"type": "text", "id": id, "text": body})
}

fn thinking(id: &str, body: &str, summary: Option<&str>) -> Value {
    json!({"type": "reasoning", "id": id, "text": body, "summary": summary})
}

/// A `!command` the user ran: a message of role `shell` holding one tool part.
fn shell(id: &str, created: i64, command: &str, state: &str, output: &str, meta: Value) -> Value {
    let mut call = json!({"status": state, "input": {"command": command}, "metadata": meta});
    if !output.is_empty() {
        call["output"] = json!(output);
    }
    message(id, "shell", created, vec![json!({"type": "tool", "id": format!("{id}-tool"), "tool": "bash", "callID": id, "state": call})])
}

const PAGING: &str = "The list endpoint loads every order and sorts them in memory. That is why it slows down as an account grows.\n\nA cursor on (created_at, id) can use the index that is already there. Offset paging would still scan the rows it skips.\n\nKeep the old response shape for one release, so the mobile app can move over without a forced update.";

/// A question, a finished thought and one with a summary, then the answer.
pub fn of_thinking() -> Vec<crate::view::conversation::Message> {
    conversation(vec![
        message("u1", "user", 1, vec![text("u1-t", "Why is the order list slow for big accounts?")]),
        message(
            "a1",
            "assistant",
            2,
            vec![
                thinking("th1", PAGING, None),
                thinking(
                    "th2",
                    "Checking the migration path first.\n\nThe old clients read `next_offset`, so it has to stay until they are gone.",
                    Some("**Choosing a paging scheme**\n\nCursor now, offset kept for one release."),
                ),
                text(
                    "a1-t",
                    "It sorts every order in memory on each request. I'd page by a cursor on `(created_at, id)` and keep `next_offset` for one release.",
                ),
            ],
        ),
    ])
}

/// A block the model is still writing: the folded line follows its newest sentence.
pub fn of_live_thinking() -> Vec<crate::view::conversation::Message> {
    conversation(vec![
        message("u1", "user", 1, vec![text("u1-t", "Why is the order list slow for big accounts?")]),
        message(
            "a1",
            "assistant",
            2,
            vec![thinking(
                "th1",
                "The endpoint loads every order. A cursor on (created_at, id) uses the existing index. Offset paging would sc",
                None,
            )],
        ),
    ])
}

fn long_output() -> String {
    (1..=40).map(|i| format!("src/orders/case_{i:02}.ts")).collect::<Vec<_>>().join("\n")
}

/// Commands the user ran: one that worked, one that failed, one still running, one that printed a lot, and one that printed nothing.
pub fn of_shell(ids: &[&str]) -> Vec<crate::view::conversation::Message> {
    let all = vec![
        shell(
            "s1",
            1,
            "git status --short",
            "completed",
            "On branch main\nnothing to commit, working tree clean\n",
            json!({"exit": 0, "status": "exited"}),
        ),
        shell(
            "s2",
            2,
            "bun test src/orders",
            "completed",
            "error: expect(received).toBe(expected)\n\nExpected: -12.34\nReceived: -12.35\n",
            json!({"exit": 3, "status": "exited"}),
        ),
        shell("s3", 3, "bun run build", "running", "", json!({})),
        shell("s4", 4, "git ls-files src/orders", "completed", &long_output(), json!({"exit": 0})),
        shell("s5", 5, "touch notes.md", "completed", "", json!({})),
        shell("s6", 6, "sleep 999", "completed", "waiting\n", json!({"status": "timeout", "truncated": true})),
    ];
    // The conversation scrolls to its end, so a picture names the commands it is about.
    conversation(all.into_iter().filter(|m| ids.iter().any(|id| m["info"]["id"] == *id)).collect())
}

/// A compaction with its sizes and a summary behind a toggle, one with only the sizes, one with nothing.
pub fn of_compaction() -> Vec<crate::view::conversation::Message> {
    let divider = |id: &str, extra: Value| {
        let mut part = json!({"type": "compaction", "id": id, "trigger": "auto"});
        part.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        message(&format!("m-{id}"), "assistant", 3, vec![part])
    };
    conversation(vec![
        message("u1", "user", 1, vec![text("u1-t", "Add cursor paging to the order list.")]),
        message("a1", "assistant", 2, vec![text("a1-t", "Done: the list takes a `cursor` and returns `next_cursor`.")]),
        divider(
            "c1",
            json!({"tokensBefore": 181_000, "tokensAfter": 34_000, "summary": "**Goal:** page the order list by a cursor.\n\n- Added `cursor` and `next_cursor` to the list endpoint.\n- Kept `next_offset` for one release.\n\n**Next:** update the mobile client."}),
        ),
        message("u2", "user", 4, vec![text("u2-t", "Now update the mobile client.")]),
        divider("c2", json!({"tokensBefore": 35_438})),
        divider("c3", json!({"trigger": null})),
    ])
}

impl State {
    /// Thinking, with every block opened.
    pub fn show_thinking_open(&mut self) {
        self.show_conversation(of_thinking());
        self.reasoning_open.extend(["th1".to_string(), "th2".to_string()]);
    }

    /// The model is still writing its thought: the session works, so the newest block is live.
    pub fn show_live_thinking(&mut self) {
        self.show_conversation(of_live_thinking());
        if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
            session.status = Status::Working;
        }
    }

    /// A compaction whose summary is shown.
    pub fn show_compaction_open(&mut self) {
        self.show_conversation(of_compaction());
        self.summary_open.insert("c1".into());
    }

    /// Fleet's recap under the conversation.
    pub fn show_recap(&mut self) {
        self.show_conversation(of_thinking());
        let snapshot = json!({"messages": [], "recap": {"text": "You're paging the order list by a cursor. Next, move the mobile app over before the offset goes."}});
        self.stream = Some(crate::fleet::reducer::create_state(&snapshot));
    }
}
