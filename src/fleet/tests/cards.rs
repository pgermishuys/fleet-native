//! What a tool call's card says of itself, read through the view's boil-down (`toToolCardItem` of
//! `activity-stream-utils.test.ts`): the title Fleet's own tools take from its answer, and a saved note shown as the
//! note itself.
use super::*;
use crate::conv::background::Finished;
use crate::fleet::model::{Msg, PartData};
use crate::view::conversation::Part;

/// The title and preview a call shows, from the card the view builds for it.
fn card(tool: &str, state: Value) -> (String, Option<String>) {
    let mut m = Msg::new("m1", "s1", Role::Assistant);
    m.parts = vec![PartData::Tool { id: "part-1".into(), tool: tool.into(), call_id: "call-1".into(), state }];
    let Part::Tool { detail, body, .. } = &view::message(&m, None, &Finished::new()).parts[0] else { panic!("a tool part") };
    (detail.clone(), body.preview.clone())
}

// activity-stream-utils.test.ts: "shows a browser tool's title and address, and the canvas it opened"
#[test]
fn shows_a_browser_tools_title_and_address_and_the_canvas_it_opened() {
    let done = json!({"status": "completed", "input": {"command": "npm run dev", "title": "Shop"}, "title": "Shop \u{b7} http://localhost:5173/",
        "output": "Running `npm run dev` (app_1) in /work/shop.", "metadata": {"canvasId": "cv_1", "version": 2}});
    let running = json!({"status": "running", "input": {"command": "npm run dev", "title": "Shop"}});
    assert_eq!(card("fleet_app_start", done.clone()).0, "Shop \u{b7} http://localhost:5173/");
    assert_eq!(card("fleet_app_start", running).0, "Shop \u{b7} npm run dev");
    // The canvas the call opened is kept on the list's tool.
    let mut m = Msg::new("m1", "s1", Role::Assistant);
    m.parts = vec![PartData::Tool { id: "part-1".into(), tool: "fleet_app_start".into(), call_id: "call-1".into(), state: done }];
    let list = crate::conv::message_list::build_list(
        &[m],
        &[],
        &crate::conv::message_list::ListContext {
            delegations: &[],
            finished: &Finished::new(),
            running: &[],
            older: &[],
            models: &[],
            thinking: crate::conv::gist::Thinking::Folded,
        },
    );
    assert!(list[0].tools[0].canvas);
}

// activity-stream-utils.test.ts: "shows a saved note as the note itself, whatever the harness says the call was"
#[test]
fn shows_a_saved_note_as_the_note_itself_whatever_the_harness_says_the_call_was() {
    let note = "WebFetch can't read github.com pages; use gh.";
    let done = json!({"status": "completed", "input": {"text": note, "list": "machine", "kind": "learned"}, "title": "Remembered for this machine",
        "output": format!("{note}\nSaved as note 1a2b3c4d for this machine. Sessions read it from their next request.")});
    let (title, preview) = card("fleet_memory_save", done);
    assert_eq!(title, note);
    assert_eq!(preview.as_deref(), Some(format!("\u{2514} {note} (2 lines)").as_str()));
    assert_eq!(card("fleet_memory_save", json!({"status": "running", "input": {"text": note}})).0, note);
}

/// What the row says on its right of a call, from the card the view builds for it.
fn row_state(tool: &str, state: Value) -> crate::view::conversation::ToolState {
    let mut m = Msg::new("m1", "s1", Role::Assistant);
    m.parts = vec![PartData::Tool { id: "part-1".into(), tool: tool.into(), call_id: "call-1".into(), state }];
    let Part::Tool { state, .. } = &view::message(&m, None, &Finished::new()).parts[0] else { panic!("a tool part") };
    *state
}

mod tool_card_status_indicator {
    use super::*;
    use crate::view::conversation::ToolState;

    // ToolCard.rows.test.ts: "shows +N and -N instead of the check when a completed call changed a file"
    #[test]
    fn shows_plus_n_and_minus_n_instead_of_the_check_when_a_completed_call_changed_a_file() {
        let lines = json!([{"type": "remove", "content": "-a"}, {"type": "add", "content": "+b"}, {"type": "add", "content": "+c"}, {"type": "context", "content": " d"}]);
        assert!(matches!(row_state("edit", json!({"status": "completed", "diffLines": lines})), ToolState::Diff(2, 1)));
    }

    // ToolCard.rows.test.ts: "shows the glyph, not the counts, while an edit is still running"
    #[test]
    fn shows_the_glyph_not_the_counts_while_an_edit_is_still_running() {
        assert!(matches!(
            row_state("edit", json!({"status": "running", "diffLines": [{"type": "add", "content": "+b"}]})),
            ToolState::Plain
        ));
    }

    // ToolCard.rows.test.ts: "shows the check when the diff has only context lines"
    #[test]
    fn shows_the_check_when_the_diff_has_only_context_lines() {
        let state = row_state("edit", json!({"status": "completed", "diffLines": [{"type": "context", "content": " same"}]}));
        assert!(matches!(state, ToolState::Plain | ToolState::Diff(0, 0)));
    }
}

// ToolCard.rows.test.ts: "a write shows +N −N on its line"
#[test]
fn a_write_shows_plus_n_minus_n_on_its_line() {
    let content: String = (1..=30).map(|n| format!("line {n}\n")).collect();
    let state = json!({"status": "completed", "input": {"filePath": "src/new.txt", "content": content}});
    assert!(matches!(row_state("write", state), crate::view::conversation::ToolState::Diff(30, 0)));
}
