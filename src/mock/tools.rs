//! The "tool rows" scenes: one call of each kind, then each status, written the way the harness sends them (a tool
//! part with raw `state`) and boiled down by the same code a live conversation goes through, so a picture shows the
//! real path from the wire to the row.
use crate::State;
use crate::fleet::{reducer, view};
use serde_json::{Value, json};

fn call(id: &str, tool: &str, status: &str, input: Value, extra: Value) -> Value {
    let mut state = json!({"status": status, "input": input});
    if let (Some(state), Some(extra)) = (state.as_object_mut(), extra.as_object()) {
        state.extend(extra.clone());
    }
    json!({"type": "tool", "id": id, "tool": tool, "callID": format!("call-{id}"), "state": state})
}

fn message(id: &str, role: &str, created: i64, parts: Vec<Value>) -> Value {
    json!({"info": {"id": id, "role": role, "sessionID": "scene", "time": {"created": created}}, "parts": parts})
}

fn took(ms: i64) -> Value {
    json!({"time": {"start": 1_000, "end": 1_000 + ms}})
}

fn conversation(messages: Vec<Value>) -> Vec<crate::view::conversation::Message> {
    let state = reducer::create_state(&json!({"session": {}, "messages": messages, "delegations": [], "activityStatus": "idle"}));
    view::messages(&state.messages)
}

/// One call of every kind the checklist's pictures name: read, edit, write, bash, grep, glob, task, webfetch, todo, an
/// MCP call of another server, and a few of Fleet's own.
pub fn of_each_kind() -> Vec<crate::view::conversation::Message> {
    let ok = "completed";
    let calls = vec![
        call("c1", "read", ok, json!({"filePath": "src/orders/total.ts"}), json!({})),
        call(
            "c2",
            "edit",
            ok,
            json!({"filePath": "src/orders/total.ts"}),
            json!({"metadata": {"filediff": {"additions": 12, "deletions": 3}}}),
        ),
        call(
            "c3",
            "write",
            ok,
            json!({"filePath": "src/orders/total.test.ts"}),
            json!({"metadata": {"filediff": {"additions": 48, "deletions": 0}}}),
        ),
        call("c4", "bash", ok, json!({"command": "bun test src/orders", "description": "Run the order tests"}), took(34_000)),
        call("c5", "grep", ok, json!({"pattern": "applyDiscount"}), json!({})),
        call("c6", "glob", ok, json!({"pattern": "src/**/*.test.ts"}), json!({})),
        call("c7", "task", ok, json!({"description": "Check the rounding rules", "subagent_type": "general"}), json!({})),
        call("c8", "webfetch", ok, json!({"url": "https://example.test/docs/rounding"}), json!({})),
        call("c9", "todowrite", ok, json!({"todos": [{"content": "a"}, {"content": "b"}, {"content": "c"}]}), json!({})),
        call("c10", "mcp__acme_orders__lookup_order", ok, json!({"id": 7}), json!({})),
        call("c11", "fleet_page_show", ok, json!({"path": "/work/mockups/totals.html", "title": "Totals"}), json!({})),
        call("c12", "websearch", ok, json!({"query": "banker's rounding in currency totals"}), json!({})),
    ];
    conversation(vec![
        message("u1", "user", 1, vec![json!({"type": "text", "id": "u1-t", "text": "Fix the rounding in the order total."})]),
        message("a1", "assistant", 2, calls),
    ])
}

/// Each status a call can end in, and the reads that fold into one row.
pub fn of_each_status() -> Vec<crate::view::conversation::Message> {
    let read = |id: &str, file: &str, status: &str| call(id, "read", status, json!({"filePath": file}), json!({}));
    let calls = vec![
        read("r1", "src/orders/total.ts", "completed"),
        read("r2", "src/orders/tax.ts", "completed"),
        read("r3", "src/orders/discount.ts", "running"),
        call("s1", "bash", "running", json!({"command": "bun test", "description": "Run the order tests"}), json!({})),
        call("s2", "bash", "error", json!({"command": "bun run build"}), json!({"error": "exit 1"})),
        call("s3", "read", "error", json!({"filePath": "src/missing.ts"}), json!({})),
        call("s4", "bash", "completed", json!({"command": "ls"}), took(900)),
        call("s5", "bash", "completed", json!({"command": "bun run bench"}), took(125_000)),
        call(
            "s6",
            "edit",
            "completed",
            json!({"filePath": "docs/notes.md"}),
            json!({"metadata": {"filediff": {"additions": 0, "deletions": 0}}}),
        ),
        call("s7", "fleet_browser_screenshot", "completed", json!({"viewport": "phone"}), json!({"title": "Screenshot of /checkout"})),
        call("s8", "frobnicate", "pending", json!({}), json!({})),
    ];
    conversation(vec![
        message("u1", "user", 1, vec![json!({"type": "text", "id": "u1-t", "text": "Look through the order code."})]),
        message("a1", "assistant", 2, calls),
    ])
}

impl State {
    /// A scene: the first session shows `messages` instead of its own.
    pub fn show_conversation(&mut self, messages: Vec<crate::view::conversation::Message>) {
        self.open = (0, 0);
        if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
            session.status = crate::mock::Status::Idle;
            session.messages = messages;
        }
    }
}
