//! The sub-agent rows' and background notices' scenes (piece 3M): what the scripted model's `spawn-agent` and
//! `background-work` keywords leave in a conversation, a child that asks, and the notices that say background work
//! finished. Written as the server sends it and drawn through the reducer.
use super::failure::NOW;
use super::tools::{call, message};
use crate::State;
use crate::mock::Status;
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub enum Case {
    /// `spawn-agent`: a sub-agent that ran and finished.
    Done,
    /// `background-work`: the call returned and the sub-agent runs on.
    Background,
    /// A child stopped on a question.
    Asks,
    /// Every state of the row, one under another.
    Every,
    /// The notices: finished, failed, stopped, and a shell stopped by Fleet whose error body is not shown.
    Notices,
}

fn text(id: &str, body: &str) -> Value {
    json!({"type": "text", "id": id, "text": body})
}

fn task(id: &str, status: &str, description: &str, kind: &str, extra: Value) -> Value {
    call(id, "task", status, json!({"description": description, "subagent_type": kind}), extra)
}

fn delegation(call_id: &str, child: &str, title: &str, status: &str, extra: Value) -> Value {
    let mut d = json!({
        "delegationId": format!("d-{call_id}"), "parentToolCallId": format!("call-{call_id}"), "childSessionId": child,
        "title": title, "status": status,
    });
    d.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap_or_default());
    d
}

impl State {
    pub fn show_agents(&mut self, case: Case) {
        self.open = (0, 0);
        let mut messages = vec![message("u1", "user", NOW - 120_000, vec![text("u1-t", "Check the rounding rules in the background.")])];
        let mut delegations = vec![];
        match case {
            Case::Done => {
                messages.push(message(
                    "a1",
                    "assistant",
                    NOW - 100_000,
                    vec![
                        task("c1", "completed", "Find the off-by-one in pagination", "general", json!({})),
                        text("a1-t", "Found it: the last page is cut one row short."),
                    ],
                ));
                delegations.push(delegation("c1", "child-1", "Find the off-by-one in pagination", "completed", json!({})));
            }
            Case::Background => {
                let running = json!({"background": true, "metadata": {"sessionID": "child-1"}});
                messages.push(message(
                    "a1",
                    "assistant",
                    NOW - 100_000,
                    vec![
                        task("c1", "running", "Migrate the cache", "general", running),
                        text("a1-t", "I started the cache migration in the background."),
                    ],
                ));
                delegations.push(delegation("c1", "child-1", "Migrate the cache", "running", json!({"background": true})));
            }
            Case::Asks => {
                messages.push(message(
                    "a1",
                    "assistant",
                    NOW - 100_000,
                    vec![task("c1", "running", "Pick the rounding mode", "general", json!({}))],
                ));
                delegations.push(delegation(
                    "c1",
                    "child-1",
                    "Pick the rounding mode",
                    "running",
                    json!({"childActivityStatus": "waiting_input"}),
                ));
            }
            Case::Every => {
                let rows = [
                    ("c1", "completed", "Find the off-by-one in pagination", json!({}), json!({})),
                    ("c2", "running", "Map every archive entry point", json!({}), json!({})),
                    (
                        "c3",
                        "running",
                        "Migrate the cache",
                        json!({"background": true, "metadata": {"sessionID": "child-3"}}),
                        json!({"background": true}),
                    ),
                    ("c4", "running", "Pick the rounding mode", json!({}), json!({"childActivityStatus": "waiting_input"})),
                    ("c5", "error", "Rebuild the search index", json!({}), json!({})),
                    ("c6", "completed", "Summarise the diff", json!({}), json!({})),
                ];
                let mut parts = vec![];
                for (n, (id, status, what, extra, d)) in rows.into_iter().enumerate() {
                    parts.push(task(
                        id,
                        if status == "error" { "error" } else { status },
                        what,
                        if n == 5 { "" } else { "explore" },
                        extra,
                    ));
                    let mut delegation = delegation(id, &format!("child-{n}"), what, status, d);
                    if n == 5 {
                        delegation["status"] = json!("cancelled");
                    }
                    delegations.push(delegation);
                }
                messages.push(message("a1", "assistant", NOW - 100_000, parts));
            }
            Case::Notices => {
                let shell = |state: &str, body: &str| {
                    format!("<shell id=\"sh_1\" state=\"{state}\" command=\"bun run test:e2e\">\n{body}\n</shell>")
                };
                let helper = |n: u8, state: &str, body: &str| {
                    format!("<subagent sessionID=\"ses_{n}\" state=\"{state}\" description=\"Review the diff\">\n{body}\n</subagent>")
                };
                let started = |id: &str, handle: Value| {
                    call(id, "shell", "running", json!({"command": "bun run test:e2e"}), json!({"background": true, "metadata": handle}))
                };
                messages.push(message("a1", "assistant", NOW - 100_000, vec![started("c1", json!({"shellID": "sh_1"}))]));
                messages.push(message(
                    "n1",
                    "user",
                    NOW - 90_000,
                    vec![text("n1-t", &shell("completed", "12 passed\n\nCommand exited with code 0."))],
                ));
                messages.push(message(
                    "n2",
                    "user",
                    NOW - 80_000,
                    vec![text("n2-t", &helper(1, "completed", "The diff is small and safe to merge."))],
                ));
                messages.push(message("n3", "user", NOW - 70_000, vec![text("n3-t", &helper(2, "error", "It ran out of context."))]));
                messages.push(message("n4", "user", NOW - 60_000, vec![text("n4-t", &helper(3, "cancelled", ""))]));
            }
        }
        let snapshot = json!({"session": {}, "messages": messages, "delegations": delegations, "activityStatus": "idle"});
        let stream = crate::fleet::reducer::create_state(&snapshot);
        if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
            session.status = Status::Idle;
            session.row.activity = "idle".into();
        }
        self.now_ms = Some(NOW);
        self.stream = Some(stream);
        self.stream_of = None;
        self.sync_open();
    }
}
