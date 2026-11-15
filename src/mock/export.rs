//! The "Fix pagination" / `run-tests` session as the scripted model writes it, for the export scenes: the same messages the
//! scratch Fleet holds after "run-tests", with invented times so the file reads the same every time.
use crate::State;
use crate::conv::export::ExportToolMode;
use crate::conv::export_dialog::{Load, Pane};
use crate::conv::export_history::HistoryError;
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

/// The same session with a command that printed a key-looking line, for the dialog's warning about keys. Invented.
fn session_with_key() -> Vec<Msg> {
    let mut all = session();
    all.push(message(
        "m6",
        Role::Assistant,
        Some("fake-model"),
        vec![PartData::Tool {
            id: "p6".into(),
            tool: "bash".into(),
            call_id: "c2".into(),
            state: json!({"status": "completed", "input": {"command": "env", "description": "Show the environment"}, "output": "HOME=/work\nAPI_KEY=abcdef1234567890\n"}),
        }],
    ));
    all
}

/// The states the dialog's scenes and pictures show.
#[derive(Clone, Copy)]
pub enum Case {
    Ready,
    /// Tool output on: the warning about keys and the line that looks like one.
    Output,
    Preview,
    Loading,
    Failed,
}

/// The dialog open over the sessions view in `case`, with the export's time fixed so the file reads the same every time.
pub fn show(s: &mut State, case: Case) {
    s.now_ms = Some(AT + 60_000);
    s.show_export(Load::Loading(0));
    let generation = s.export.dialog.as_ref().map_or(0, |d| d.generation);
    match case {
        Case::Loading => s.export_progress(generation, 120),
        Case::Failed => s.export_done(
            generation,
            Err(HistoryError::Failed("Fleet could not load the conversation (error 502 after 200 messages).".into())),
        ),
        Case::Ready | Case::Preview => s.export_done(generation, Ok(session())),
        Case::Output => {
            s.export_done(generation, Ok(session_with_key()));
            s.export_set_tools(ExportToolMode::Output);
        }
    }
    if matches!(case, Case::Preview) {
        s.export_set_pane(Pane::Preview);
    }
}
