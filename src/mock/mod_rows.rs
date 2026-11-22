//! Invented mod trees on tool rows (component 22, piece E): a shell row whose line holds pills and whose opened body holds buttons above
//! Fleet's own output, the same trees Fleet's own client draws from `scripts/mod-standin/deploy-notes` and the kept samples. Nothing here
//! is from a real mod, session or machine.
use super::failure::NOW;
use super::tools::{call, message, took};
use crate::State;
use crate::conv::card::Toggle;
use crate::fleet::mods::points::{ModTreeView, ToolRowSite, ToolRowView};
use crate::fleet::mods::types::Author;
use crate::mock::Status;
use serde_json::{Value, json};

fn pill(tone: &str, label: &str, extra: Value) -> Value {
    let mut props = json!({"tone": tone, "label": label});
    props.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap_or_default());
    json!({"type": "Pill", "props": props})
}

/// The line's tree: what `deploy-notes` draws on a shell row.
pub fn line_tree() -> Value {
    json!({"type": "Box", "props": {"gap": 1, "alignItems": "center"}, "children": [
        pill("good", "42 passed", json!({"icon": "check"})),
        pill("neutral", "0 skipped", json!({})),
        {"type": "Icon", "props": {"name": "zap", "color": "accent", "label": "Fast"}},
    ]})
}

/// A line that fails: the call's error preview is dropped while it draws.
pub fn failing_line() -> Value {
    json!({"type": "Box", "props": {"gap": 1}, "children": [pill("bad", "1 failed", json!({"icon": "x"}))]})
}

/// A pill long enough that the line's cap and the title's floor both matter.
pub fn long_line() -> Value {
    json!({"type": "Box", "props": {"gap": 1}, "children": [
        pill("warn", "2 checks waiting on the staging deploy and the smoke test", json!({"icon": "clock"})),
        pill("neutral", "retry in 30 s", json!({})),
    ]})
}

/// A line with a control: pressing it goes to its mod and doesn't open the row.
pub fn button_line() -> Value {
    json!({"type": "Button", "props": {"key": "rerun", "label": "Run again"}, "handles": {"onPress": "h-rerun"}})
}

/// The body's tree: what `press-counter` and `release-board` draw above Fleet's own body.
pub fn body_tree() -> Value {
    json!({"type": "Box", "props": {"flexDirection": "column", "gap": 2}, "children": [
        {"type": "Box", "props": {"flexDirection": "row", "gap": 2, "alignItems": "center"}, "children": [
            {"type": "Button", "props": {"key": "count", "label": "Count a press", "tone": "primary"}, "handles": {"onPress": "h-count"}},
            {"type": "Text", "props": {"dimColor": true}, "children": ["Pressed 0 times"]},
        ]},
        {"type": "Button", "props": {"key": "board", "label": "Open board", "tone": "primary"}, "handles": {"onPress": "h-board"}},
        {"type": "Fleet"},
    ]})
}

pub fn draft() -> Vec<Author> {
    vec![Author { name: "deploy-notes".into(), draft: true }]
}

impl State {
    /// A finished turn of these calls (as the server's snapshot says them), open, in the first invented session.
    pub fn show_calls(&mut self, calls: Vec<Value>) {
        self.show_turn(vec![calls]);
    }

    /// The same, one assistant message to each list of calls.
    pub fn show_turn(&mut self, messages_of_calls: Vec<Vec<Value>>) {
        self.set_open((0, 0));
        let mut messages =
            vec![message("u1", "user", NOW - 120_000, vec![json!({"type": "text", "id": "u1-t", "text": "Run the tests."})])];
        for (i, calls) in messages_of_calls.into_iter().enumerate() {
            messages.push(message(&format!("a{}", i + 1), "assistant", NOW - 100_000 + i as i64, calls));
        }
        let snapshot = json!({"session": {}, "messages": messages, "delegations": [], "activityStatus": "idle"});
        // The turn is finished and folds; its calls show when it is open (as Fleet's own picture has it).
        self.turns_open = std::iter::once("u1".to_string()).collect();
        self.stream = Some(crate::fleet::reducer::create_state(&snapshot));
        if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
            session.status = Status::Idle;
            session.row.activity = "idle".into();
        }
        self.now_ms = Some(NOW);
        self.stream_of = None;
        self.sync_open();
    }
}

#[derive(Clone, Copy)]
pub enum Case {
    /// Every kind of line: pills, a count beside `+N −N`, a failed call without its preview, a running call, a long pill, a button.
    Rows,
    /// The opened body with the tree above Fleet's own.
    Body,
}

impl State {
    pub fn show_mod_rows(&mut self, case: Case) {
        let output = "README.md\nsrc\ntests\npackage.json\ntests passed";
        let tests = call("c1", "bash", "completed", json!({"command": "bun test", "description": "Run the tests"}), {
            let mut x = took(1_200);
            x["output"] = json!(output);
            x
        });
        let mut calls = vec![tests];
        if let Case::Rows = case {
            calls.push(call(
                "c2",
                "edit",
                "completed",
                json!({"filePath": "src/paginate.py"}),
                json!({"metadata": {"filediff": {"additions": 6, "deletions": 2}}}),
            ));
            let mut failed =
                call("c3", "bash", "error", json!({"command": "bun run build", "description": "Build"}), json!({"error": "exit 1"}));
            failed["state"]["output"] = json!("error: build failed");
            calls.push(failed);
            calls.push(call("c4", "bash", "running", json!({"command": "bun run deploy:staging", "description": "Deploy"}), json!({})));
            calls.push(call(
                "c5",
                "bash",
                "completed",
                json!({"command": "bun run lint", "description": "Lint the whole repository"}),
                took(900),
            ));
            calls.push(call("c6", "bash", "completed", json!({"command": "bun run check", "description": "Check"}), took(700)));
        }
        self.show_calls(calls);
        self.mod_renders.views.clear();
        let session = self.mod_row_session();
        let view = |site, call: &str, tree: Value| ToolRowView {
            site,
            session_id: session.clone(),
            call_id: format!("call-{call}"),
            tool: "bash".into(),
            view: ModTreeView::new(tree, draft()),
        };
        let mut views = vec![view(ToolRowSite::ToolUse, "c1", line_tree())];
        match case {
            Case::Rows => {
                views.push(view(ToolRowSite::ToolUse, "c2", json!({"type": "Text", "props": {"dimColor": true}, "children": ["3 files"]})));
                views.push(view(ToolRowSite::ToolUse, "c3", failing_line()));
                views.push(view(ToolRowSite::ToolUse, "c4", long_line()));
                views.push(view(ToolRowSite::ToolUse, "c5", Value::Null));
                views.push(view(ToolRowSite::ToolUse, "c6", button_line()));
            }
            Case::Body => {
                views.push(view(ToolRowSite::ToolResult, "c1", body_tree()));
                self.tool_open.insert("c1".into(), Toggle { open: true, under: true });
            }
        }
        self.mod_renders.views.contribute("scene", views);
        self.sync_row_controls();
    }
}
