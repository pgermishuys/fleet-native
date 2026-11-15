//! The background strip's scenes (piece 3K): the sub-agent the scripted model's `background-work` keyword leaves running,
//! a mix of every kind, and work that just ended. Written as the server sends it and drawn through the reducer, with the
//! clock fixed so a picture reads the same every time.
use super::failure::{NOW, chrono_free_iso};
use super::tools::message;
use crate::State;
use crate::fleet::reducer;
use crate::fleet::work::to_items;
use crate::mock::Status;
use serde_json::{Value, json};

/// Which state a scene shows.
#[derive(Clone, Copy)]
pub enum Case {
    /// The one sub-agent `background-work` starts, 20 seconds in.
    Running,
    /// A shell, a sub-agent and a monitor, and the sub-agent's Pi-style cousin that runs inside its call.
    Mixed,
    /// A shell that just finished next to one that runs, a failed one and a stopped one.
    Finished,
    /// The same as `Mixed`, the header folded to one line.
    Collapsed,
}

fn ago(secs: i64) -> String {
    chrono_free_iso(NOW - secs * 1000)
}

fn item(id: &str, kind: &str, title: &str, label: &str, started: i64, extra: Value) -> Value {
    let mut v = json!({
        "id": id, "sessionId": "scene", "workId": format!("w-{id}"), "kind": kind, "title": title, "label": label,
        "status": "running", "background": true, "startedAt": ago(started),
    });
    v.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap_or_default());
    v
}

fn ended(mut v: Value, since: i64, status: &str, reason: &str, detail: Option<&str>) -> Value {
    v["status"] = json!(status);
    v["endedReason"] = json!(reason);
    v["endedAt"] = json!(ago(since));
    if let Some(d) = detail {
        v["detail"] = json!(d);
    }
    v
}

impl State {
    pub fn show_work(&mut self, case: Case) {
        self.open = (0, 0);
        let text = |id: &str, body: &str| json!({"type": "text", "id": id, "text": body});
        let messages = vec![
            message("u1", "user", NOW - 120_000, vec![text("u1-t", "background-work")]),
            message(
                "a1",
                "assistant",
                NOW - 100_000,
                vec![text("a1-t", "I started the cache migration in the background. I'll report when it is done.")],
            ),
        ];
        let work = match case {
            Case::Running => vec![item(
                "a91d",
                "subagent",
                "general",
                "Migrate the cache",
                20,
                json!({"childSessionId": "child-1", "toolCallId": "call-1", "canStop": true}),
            )],
            Case::Mixed | Case::Collapsed => vec![
                item(
                    "s1",
                    "shell",
                    "shell",
                    "bun run test:e2e",
                    192,
                    json!({"toolCallId": "call-2", "canStop": true, "canReadOutput": true}),
                ),
                item("a1", "subagent", "code-reviewer", "Review the diff", 92, json!({"childSessionId": "child-1", "canStop": true})),
                item("m1", "monitor", "Monitor", "Watch fleet.log for errors", 50, json!({"toolCallId": "call-3", "canReadOutput": true})),
                item("p1", "subagent", "reviewer", "Review the diff", 5, json!({"background": false, "detail": "step 1 of 2"})),
            ],
            Case::Finished => vec![
                item("s1", "shell", "shell", "bun run test:e2e", 192, json!({"canStop": true, "canReadOutput": true})),
                ended(
                    item("s2", "shell", "shell", "bun run lint", 40, json!({"canReadOutput": true})),
                    8,
                    "completed",
                    "completed",
                    Some("exit 0"),
                ),
                ended(item("s3", "shell", "shell", "make build-all", 90, json!({})), 12, "error", "error", Some("exit 2")),
                ended(item("t1", "task", "task", "Index the docs", 80, json!({})), 15, "cancelled", "cancelled", None),
                ended(item("t2", "task", "task", "Warm the cache", 70, json!({})), 20, "error", "lost", None),
            ],
        };
        let mut stream = reducer::create_state(&json!({"session": {}, "messages": messages, "delegations": [], "activityStatus": "idle"}));
        stream.running_work = to_items(Some(&Value::Array(work)));
        if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
            session.status = Status::Idle;
            session.row.activity = "idle".into();
        }
        self.now_ms = Some(NOW);
        self.work.collapsed = matches!(case, Case::Collapsed);
        self.stream = Some(stream);
        self.stream_of = None;
        self.sync_open();
    }
}
