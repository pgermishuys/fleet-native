//! The "turn folding" scenes (pieces 3F and 3G): a turn that finished and one still running, written as the server
//! sends them and drawn the way the app draws a session from a server: through the reducer, the list and `fold_turns`.
use super::tools::{call, message};
use crate::State;
use crate::fleet::reducer;
use crate::mock::Status;
use serde_json::{Value, json};

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

/// `running` leaves the second turn working: its one call is still going.
fn conversation(running: bool) -> Value {
    let ok = "completed";
    json!({"session": {}, "delegations": [{
        "delegationId": "d1", "parentToolCallId": "call-t5", "childSessionId": "child1", "title": "Find the off-by-one", "status": "completed"
    }], "activityStatus": if running { "busy" } else { "idle" }, "messages": [
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
        stamped("b1", "assistant", 41_000, (!running).then_some(43_000), vec![call(
            "t7",
            "read",
            if running { "running" } else { ok },
            json!({"filePath": "src/orders/page.ts"}),
            json!({}),
        )]),
    ]})
}

impl State {
    /// A scene: the conversation a server would send, folded. `open` opens the first turn again; `running` leaves the
    /// second turn working (it stays open), else both turns are finished and fold.
    pub fn show_folded(&mut self, open: bool, running: bool) {
        self.open = (0, 0);
        let state = reducer::create_state(&conversation(running));
        if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
            session.status = if running { Status::Working } else { Status::Idle };
        }
        self.turns_open = open.then(|| "u1".to_string()).into_iter().collect();
        self.stream = Some(state);
        self.stream_of = None;
        self.sync_open();
    }
}
