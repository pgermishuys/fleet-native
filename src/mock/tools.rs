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

/// The test run a card opens on: a few passing cases, one failing, timings dimmed, in terminal colours.
fn test_run() -> String {
    let (esc, ok, bad) = ("\x1b", "\x1b[32m✓\x1b[0m", "\x1b[31m✗\x1b[0m");
    let mut out = format!("{esc}[1mbun test{esc}[0m v1.1\n\nsrc/orders/total.test.ts:\n");
    for (name, ms) in [("adds line items", 2), ("applies a percentage discount", 1), ("rounds half to even", 3)] {
        out += &format!("{ok} {name} {esc}[2m[{ms}.00ms]{esc}[0m\n");
    }
    out += &format!("{bad} {esc}[1;31mrounds a negative total{esc}[0m {esc}[2m[4.00ms]{esc}[0m\n");
    out += &format!(
        "{esc}[31merror{esc}[0m: expect(received).toBe(expected)\n\nExpected: {esc}[32m-12.34{esc}[0m\nReceived: {esc}[31m-12.35{esc}[0m\n\n"
    );
    out += &format!(" {esc}[32m3 pass{esc}[0m\n {esc}[31m1 fail{esc}[0m\nRan 4 tests across 1 file. {esc}[2m[31.00ms]{esc}[0m\n");
    out
}

/// A suite of `n` cases, one line each, for the long-output card.
fn suite(n: usize) -> String {
    (1..=n).map(|i| format!("\x1b[32m✓\x1b[0m orders/case_{i:03} \x1b[2m[{}ms]\x1b[0m", i % 7 + 1)).collect::<Vec<_>>().join("\n")
}

/// Cards that have something under their row: a test run (`t1`), a failed build (`t2`), a long output (`t3`), a call that
/// printed nothing (`t4`) and one with a summary (`t5`). Only the named calls, so a card is on screen without scrolling.
pub fn of_output_only(ids: &[&str]) -> Vec<crate::view::conversation::Message> {
    let calls = vec![
        call("t1", "bash", "completed", json!({"command": "bun test src/orders"}), {
            let mut x = took(31);
            x["output"] = json!(test_run());
            x["metadata"] = json!({"exit": 1});
            x
        }),
        call(
            "t2",
            "bash",
            "error",
            json!({"command": "bun run build"}),
            json!({"error": "\u{1b}[31merror\u{1b}[0m TS2322: Type 'string' is not assignable to type 'number'.\n  src/orders/total.ts:41:7\nBuild failed with 1 error."}),
        ),
        call("t3", "bash", "completed", json!({"command": "bun test --all"}), {
            let mut x = took(5_200);
            x["output"] = json!(suite(200));
            x["metadata"] = json!({"exit": 0});
            x
        }),
        call("t4", "webfetch", "completed", json!({"url": "https://example.test/docs/rounding"}), json!({})),
        call(
            "t5",
            "read",
            "completed",
            json!({"filePath": "src/orders/total.ts"}),
            json!({"summary": "Read 120 lines.", "output": "export const total = (items) => items.reduce(add, 0);"}),
        ),
    ];
    let calls = calls.into_iter().filter(|c| ids.contains(&c["id"].as_str().unwrap_or(""))).collect();
    conversation(vec![
        message("u1", "user", 1, vec![json!({"type": "text", "id": "u1-t", "text": "Run the order tests and the build."})]),
        message("a1", "assistant", 2, calls),
    ])
}

/// A unified diff of `src/orders/total.ts` with a long unchanged stretch between its two changes.
fn rounding_patch() -> String {
    let body = |from: usize, to: usize| (from..=to).map(|n| format!(" const step{n} = items[{n}];")).collect::<Vec<_>>().join("\n");
    format!(
        "--- a/src/orders/total.ts\n+++ b/src/orders/total.ts\n@@ -10,29 +10,30 @@ export function total(items: Item[]) {{\n{}\n-  const sum = items.reduce(add, 0);\n+  const sum = items.reduce(add, 0);\n+  const rounded = roundHalfToEven(sum);\n{}\n-  return sum;\n+  return rounded;\n{}\n",
        body(10, 12),
        body(13, 30),
        body(31, 33)
    )
}

/// A file written whole: 30 lines, so the card shows 12 and "Show all 30 lines".
fn new_test_file() -> String {
    let mut lines =
        vec!["import { expect, test } from \"bun:test\";".to_string(), "import { total } from \"./total\";".to_string(), String::new()];
    for n in 1..=9 {
        lines.push(format!("test(\"rounds case {n}\", () => {{"));
        lines.push(format!("  expect(total([{n}.005])).toBe({n}.0);"));
        lines.push("});".to_string());
    }
    lines.join("\n")
}

/// Calls with a diff: an edit whose harness kept a unified diff (`e1`), and a write of a new file (`w1`).
pub fn of_diffs(ids: &[&str]) -> Vec<crate::view::conversation::Message> {
    let calls = vec![
        call("e1", "edit", "completed", json!({"filePath": "src/orders/total.ts"}), json!({"metadata": {"diff": rounding_patch()}})),
        call(
            "x1",
            "frobnicate",
            "completed",
            json!({}),
            json!({"diffLines": [{"type": "remove", "content": "a", "oldLineNumber": 1}, {"type": "add", "content": "b", "newLineNumber": 1}]}),
        ),
        call("w1", "write", "completed", json!({"filePath": "src/orders/total.test.ts", "content": new_test_file()}), json!({})),
    ];
    let calls = calls.into_iter().filter(|c| ids.contains(&c["id"].as_str().unwrap_or(""))).collect();
    conversation(vec![
        message("u1", "user", 1, vec![json!({"type": "text", "id": "u1-t", "text": "Round the order total half to even, with a test."})]),
        message("a1", "assistant", 2, calls),
    ])
}

impl State {
    /// Opens the cards of `of_output` the picture shows, and widens the long one when asked.
    pub fn open_cards(&mut self, ids: &[&str], wide: &[&str]) {
        use crate::conv::card::Toggle;
        for id in ids {
            self.tool_open.insert((*id).to_string(), Toggle { open: true, under: true });
        }
        self.tool_wide.extend(wide.iter().map(|s| s.to_string()));
    }

    /// A scene: the first session shows `messages` instead of its own.
    pub fn show_conversation(&mut self, messages: Vec<crate::view::conversation::Message>) {
        self.open = (0, 0);
        if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
            session.status = crate::mock::Status::Idle;
            session.messages = messages;
        }
    }
}
