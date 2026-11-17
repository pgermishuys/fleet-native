//! Invented conversations for the Turns tab's scenes and tests (component 10, piece J): five rounds like the scripted model's
//! keywords make (a test run, an edit, a new file, a sub-agent, a plain question), written as the server sends them and drawn
//! through the reducer, so a picture shows the real path from the wire to the row.
use super::tools::{call, message};
use crate::State;
use crate::fleet::canvases::CanvasKind;
use crate::fleet::reducer;
use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// Five rounds; the newest is open.
    Main,
    /// The edit round open, with its file's diff.
    Diff,
    /// "Only turns that changed files" on.
    Filter,
    /// The filter on over rounds that wrote nothing.
    FilterEmpty,
    /// No messages.
    Empty,
    /// The history is still loading.
    Loading,
    /// Older rounds exist and aren't loaded.
    Older,
    /// The panel at its least width.
    Narrow,
}

fn text(id: &str, body: &str) -> Value {
    json!({"type": "text", "id": id, "text": body})
}

fn user(id: &str, created: i64, body: &str) -> Value {
    message(id, "user", created, vec![text(&format!("{id}-t"), body)])
}

fn reply(id: &str, created: i64, done: i64, tokens: (i64, i64), cost: f64, parts: Vec<Value>) -> Value {
    let mut m = message(id, "assistant", created, parts);
    m["info"]["time"]["completed"] = json!(done);
    m["info"]["agent"] = json!("build");
    m["info"]["modelID"] = json!("claude-opus-5");
    m["info"]["tokens"] = json!({"input": tokens.0, "output": tokens.1, "reasoning": 0});
    m["info"]["cost"] = json!(cost);
    m
}

fn line(kind: &str, content: &str, old: u32, new: u32) -> Value {
    json!({"type": kind, "content": content, "oldLineNumber": old, "newLineNumber": new})
}

fn edit_call() -> Value {
    let mut lines = vec![line("context", "def page(items, size):", 11, 11), line("context", "    start = 0", 12, 12)];
    lines.push(json!({"type": "remove", "content": "    while start < len(items) - 1:", "oldLineNumber": 13}));
    lines.push(json!({"type": "add", "content": "    while start < len(items):", "newLineNumber": 13}));
    lines.push(line("context", "        yield items[start:start + size]", 14, 14));
    lines.push(line("context", "        start += size", 15, 15));
    call("e1", "edit", "completed", json!({"filePath": "src/paginate.py"}), json!({"diffLines": lines}))
}

fn write_call() -> Value {
    let body: Vec<Value> = ["PAGE_SIZE = 25", "", "def clamp(n):", "    return max(1, min(n, 100))"]
        .iter()
        .enumerate()
        .map(|(i, l)| json!({"type": "add", "content": l, "newLineNumber": i + 1}))
        .collect();
    call("w1", "write", "completed", json!({"filePath": "src/page_size.py"}), json!({"diffLines": body}))
}

fn rounds(reads_only: bool) -> Vec<Value> {
    let ok = "completed";
    let mut m = vec![
        user("u1", 1_000, "run-tests"),
        reply(
            "a1",
            2_000,
            14_000,
            (3_200, 410),
            0.04,
            vec![
                call("t1", "bash", ok, json!({"command": "bun test", "description": "Run the tests"}), json!({"metadata": {"exit": 0}})),
                call("t2", "read", ok, json!({"filePath": "src/paginate.py"}), json!({})),
            ],
        ),
        user("u2", 20_000, "Fix the off-by-one in the paging loop."),
        reply(
            "a2",
            21_000,
            52_000,
            (5_800, 920),
            0.11,
            if reads_only {
                vec![call("t3", "read", ok, json!({"filePath": "src/paginate.py"}), json!({}))]
            } else {
                vec![
                    call("t3", "read", ok, json!({"filePath": "src/paginate.py"}), json!({})),
                    edit_call(),
                    call("t4", "bash", "error", json!({"command": "bun test paginate"}), json!({"error": "1 test failed"})),
                ]
            },
        ),
    ];
    if !reads_only {
        m.extend([user("u3", 60_000, "Add a page size helper."), reply("a3", 61_000, 75_000, (2_100, 380), 0.06, vec![write_call()])]);
    }
    m.extend([
        user("u4", 90_000, "Have someone check the rounding rules."),
        reply(
            "a4",
            91_000,
            140_000,
            (4_000, 600),
            0.09,
            vec![call("t9", "task", ok, json!({"subagent_type": "general", "description": "Check the rounding rules"}), json!({}))],
        ),
        user("u5", 150_000, "What does the paging loop do now?"),
        reply("a5", 151_000, 156_000, (1_900, 240), 0.02, vec![text("a5-t", "It yields pages of `size` items until the list runs out.")]),
    ]);
    m
}

fn conversation(case: Case) -> Value {
    let messages = match case {
        Case::Empty | Case::Loading => vec![],
        c => rounds(c == Case::FilterEmpty),
    };
    json!({"session": {}, "delegations": [{
        "delegationId": "d1", "parentToolCallId": "call-t9", "childSessionId": "child1", "title": "Check the rounding rules", "status": "completed"
    }], "activityStatus": "idle", "messages": messages})
}

impl State {
    pub fn show_turns_tab(&mut self, case: Case, dark: bool) {
        self.show_context_tab_scene(crate::mock::context_tab::Case::Links, dark);
        self.sidebar_open = true;
        let mut state = reducer::create_state(&conversation(case));
        state.has_more = case == Case::Older;
        state.cursor = (case == Case::Older).then(|| "older".to_string());
        self.stream = Some(state);
        self.stream_of = None;
        self.loading = case == Case::Loading;
        self.turns_tab = Default::default();
        self.turns_tab.changed_only = matches!(case, Case::Filter | Case::FilterEmpty);
        self.sync_open();
        self.edit_canvases(|c, id| {
            c.introduce(id, CanvasKind::Turns);
            c.open(id, CanvasKind::Turns);
        });
        if case == Case::Diff {
            self.turns_tab.set_open("u2", true);
            self.turns_tab.set_open("u5", false);
            self.turns_tab.open_file = Some("u2|src/paginate.py".into());
        }
        if case == Case::Narrow {
            self.right.width = 280.0;
        }
        self.set_content_open(true);
        self.keep_turns_tab();
    }
}
