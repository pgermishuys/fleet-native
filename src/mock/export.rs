//! The "Fix pagination" / `run-tests` session as the scripted model writes it, for the export scene: the same messages the
//! scratch Fleet holds after "run-tests", with invented times so the file reads the same every time.
use crate::conv::export::{DEFAULT_EXPORT_OPTIONS, ExportContext, ExportOptions, ExportToolMode, build_markdown};
use crate::fleet::model::{Msg, PartData};
use crate::view::conversation::Role;
use serde_json::json;

/// 15 November 2026, 14:19 UTC.
const AT: i64 = 1_794_752_340_000;

fn message(id: &str, role: Role, model: Option<&str>, parts: Vec<PartData>) -> Msg {
    let mut m = Msg::new(id, "s1", role);
    m.created_at = Some(AT);
    m.model_id = model.map(str::to_string);
    m.parts = parts;
    m
}

fn text(id: &str, text: &str) -> PartData {
    PartData::Text { id: id.into(), text: text.into() }
}

pub fn session() -> Vec<Msg> {
    let call = json!({
        "status": "completed",
        "input": {"command": "printf 'README.md\\nsrc\\ntests\\npackage.json\\n' && echo tests passed", "description": "Run the tests"},
        "output": "README.md\nsrc\ntests\npackage.json\ntests passed\n",
    });
    let plan = "Here's the plan.\n\n1. **Read** the failing test and the code it covers.\n2. **Fix** the off-by-one in `paginate()`.\n3. **Run** the suite again.\n\nSay `run-tests` and I'll run them now. ";
    vec![
        message("m1", Role::User, None, vec![text("p1", "The pagination test fails on the last page. Can you look?")]),
        message("m2", Role::Assistant, Some("fake-model"), vec![text("p2", plan)]),
        message("m3", Role::User, None, vec![text("p3", "run-tests")]),
        message(
            "m4",
            Role::Assistant,
            Some("fake-model"),
            vec![PartData::Tool { id: "p4".into(), tool: "bash".into(), call_id: "c1".into(), state: call }],
        ),
        message(
            "m5",
            Role::Assistant,
            Some("fake-model"),
            vec![text("p5", "Ran it, run 1 of the suite. **All 42 tests pass** and the build is clean. ")],
        ),
    ]
}

/// The Markdown of that session, with the tools as lines or with their output.
pub fn markdown(with_output: bool) -> String {
    let options =
        ExportOptions { tools: if with_output { ExportToolMode::Output } else { ExportToolMode::Line }, ..DEFAULT_EXPORT_OPTIONS };
    let context = ExportContext {
        title: "Fix pagination".into(),
        harness: Some("opencode".into()),
        model: Some("fake-model".into()),
        directory: Some("/work/proj/demo".into()),
        exported_at: AT + 60_000,
        offset_secs: 0,
    };
    build_markdown(&session(), &options, &context).markdown
}
