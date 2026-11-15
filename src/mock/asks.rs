//! The permission card's scenes (component 4, piece A): an ask of each kind, written as a server would send it
//! (invented) and read by the same reader the live path uses, then shown under a short conversation.
use super::tools::message;
use crate::State;
use crate::asks::permission::{PermissionAsk, to_permission_ask};
use crate::fleet::reducer;
use crate::mock::Status;
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub enum Case {
    /// A shell command, with the folder it runs in.
    Command,
    /// An edit with a diff, headers and all.
    Edit,
    /// A write that changes nothing: the diff is headers only.
    EditEmpty,
    /// The harness asks to step outside the folder (`external_directory`).
    Path,
    /// A sub-agent asked.
    Subagent,
    /// Fleet's own ask to send work to another machine.
    Handoff,
    /// A hand-off to a session on a Windows machine, with a folder and no stated level.
    HandoffFolder,
    /// A very long command, a very long diff and a very long pattern.
    Long,
}

const DIFF: &str = "Index: src/paginate.py\n===================================================================\n--- src/paginate.py\n+++ src/paginate.py\n@@ -10,7 +10,9 @@ def page(items, size):\n     start = 0\n     out = []\n-    while start < len(items):\n-        out.append(items[start:start + size])\n+    while start < len(items):\n+        chunk = items[start:start + size]\n+        out.append(chunk)\n+        # the last page may be short\n         start += size\n     return out\n \n";

fn ask(case: Case) -> Value {
    let base = json!({"id": "per_1", "sessionId": "mock.0.0", "askedAt": "2026-10-07T01:51:00Z"});
    let extra = match case {
        Case::Command => {
            json!({"kind": "shell", "tool": "bash", "title": "ls -la && echo tests passed", "always": ["ls *", "echo *"]})
        }
        Case::Edit => json!({"kind": "edit", "tool": "edit", "title": "src/paginate.py", "detail": DIFF, "always": ["*"]}),
        Case::EditEmpty => {
            json!({"kind": "edit", "tool": "write", "title": "src/page_size.py", "detail": "Index: src/page_size.py\n=====\n--- src/page_size.py\n+++ src/page_size.py\n", "always": ["*"]})
        }
        Case::Path => {
            json!({"kind": "other", "tool": "external_directory", "title": "/home/demo/src/demo-app/notes/report.md", "always": ["/home/demo/src/demo-app/notes/*"]})
        }
        Case::Subagent => {
            json!({"kind": "shell", "tool": "bash", "title": "git log --oneline -n 20", "directory": "/work/shop", "always": ["git log *"], "subagent": "explorer"})
        }
        Case::Handoff => json!({
            "kind": "other", "tool": "fleet_session_start", "title": "Start a session on taron", "always": [], "handoff":
            {"machine": "taron", "os": "windows", "task": "Run the installer tests and report what fails.", "level": "ask"}
        }),
        Case::HandoffFolder => json!({
            "kind": "other", "tool": "fleet_session_message", "always": [], "subagent": "explorer", "handoff":
            {"machine": "harbor", "os": "macos", "session": "Cursor paging", "folder": "/work/shop", "task": "Rebase on main, then rerun the order tests."}
        }),
        Case::Long => json!({
            "kind": "shell", "tool": "bash", "title": "find /work/shop/services/orders -name '*.py' -newer /work/shop/.last-build -not -path '*/node_modules/*' -print0 | xargs -0 grep -n 'page_size' | sort | uniq -c | sort -rn | head -n 40",
            "directory": "/work/shop/services/orders/internal/pagination/cursors",
            "always": ["find /work/shop/services/orders -name '*.py' -newer /work/shop/.last-build *", "xargs -0 grep -n *"]
        }),
    };
    let mut out = base;
    out.as_object_mut().expect("an object").extend(extra.as_object().expect("an object").clone());
    out
}

/// The ask as the reader makes it: what a scene and a test show.
pub fn read(case: Case) -> PermissionAsk {
    to_permission_ask(&ask(case)).expect("the scene's ask can be read")
}

impl State {
    /// The open session waits on `case`'s ask, under a short conversation.
    pub fn show_ask(&mut self, case: Case) {
        self.set_open((0, 0));
        let text = |id: &str, body: &str| json!({"type": "text", "id": id, "text": body});
        // The two the scripted model can raise live carry its keywords, so the picture sits beside Fleet's own.
        let (prompt, reply) = match case {
            Case::Command => ("run-tests", None),
            Case::Path => ("ask-permission", None),
            _ => ("Add cursor paging to the order list.", Some("I'll read the paging helper, then make the change.")),
        };
        let mut messages = vec![message("u1", "user", 1_000, vec![text("u1-t", prompt)])];
        if let Some(reply) = reply {
            messages.push(message("a1", "assistant", 2_000, vec![text("a1-t", reply)]));
        }
        let state =
            reducer::create_state(&json!({"session": {}, "messages": messages, "delegations": [], "activityStatus": "waiting_input"}));
        if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
            session.status = Status::NeedsInput;
        }
        self.asks.set("mock.0.0", vec![read(case)]);
        self.now_ms = Some(super::failure::NOW);
        self.stream = Some(state);
        self.stream_of = None;
        self.sync_open();
    }
}

impl State {
    /// The command ask after an answer that Fleet refused, or while one is on its way.
    pub fn show_ask_act(&mut self, busy: bool) {
        self.show_ask(Case::Command);
        let error = (!busy).then(|| "The harness is restarting.".to_string());
        self.ask_acts.insert("per_1".into(), crate::fleet::retry::Act { busy, error });
    }
}
