//! Component 18, piece N: the File view (rows 13.1-13.3 of docs/parity/18-workflows.md; `WorkflowFileEditor.vue`). Fleet has no test
//! of its own for the field (CodeMirror is stubbed in `WorkflowEditor.test.ts`), so these are the native ones: typing is checked after
//! 300 ms, a line the parser has an error on is marked, Ctrl/Cmd+S in the field saves, and text set from outside replaces the document.
use crate::fleet::workflows::editor::EditorView;
use crate::fleet::workflows::file_field::FIELD_SESSION;
use crate::tests::press_key;
use crate::tests_workflow_editor::{CHECK, FILES, Rig, check_json, rig};
use crate::view::code_view::CODE;
use rune::{Key, Mods};
use serde_json::json;

const NAME: &str = ".weave/workflows/build-it-our-way.yaml";

impl Rig {
    /// The File view of the mounted file, with the keyboard in its field.
    fn in_the_field(&mut self, comments: bool) {
        self.mount(comments);
        self.view(EditorView::File);
        self.frames();
        self.rt.update(|_, tx| tx.focus(CODE, false));
        self.frames();
    }

    fn type_text(&mut self, text: &str) {
        press_key(&mut self.rt, Key::Text(text.into()), Mods::default());
    }

    fn lines(&self) -> Vec<String> {
        let s = &self.rt.state;
        let m = s.workflows_nav.editor.machine().to_string();
        s.file_buffers.buffer(&m, FIELD_SESSION, NAME).map(|b| b.lines().to_vec()).unwrap_or_default()
    }
}

fn save_keys() -> Mods {
    if cfg!(target_os = "macos") { Mods { cmd: true, ..Mods::default() } } else { Mods { ctrl: true, cmd: true, ..Mods::default() } }
}

#[test]
fn the_file_view_shows_the_file_as_it_is_in_the_code_editor() {
    let mut r = rig();
    r.in_the_field(true);
    assert!(r.has("workflow-file-editor") && r.has(CODE));
    assert_eq!(r.lines(), vec!["# Kept by the platform team.", "name: Build it our way", ""]);
    assert_eq!(r.rt.state.workflow_field_target().map(|t| t.2), Some(NAME.to_string()));
}

#[test]
fn typing_checks_after_300_ms_and_not_before() {
    let mut r = rig();
    r.in_the_field(false);
    r.type_text("x");
    assert_eq!(r.ed().text, "xname: Build it our way\n", "what was typed is the editor's text at once");
    assert!(r.ed().is_dirty());
    assert_eq!(r.fleet.count("POST", CHECK), 0, "the check waits for the pause");
    r.settle();
    assert_eq!(r.fleet.count("POST", CHECK), 1);
    assert_eq!(r.fleet.bodies("POST", CHECK)[0]["text"], json!("xname: Build it our way\n"));
}

#[test]
fn a_line_with_an_error_is_marked_and_listed_under_the_field() {
    let mut r = rig();
    r.in_the_field(false);
    assert!(r.rt.state.workflow_error_lines(FIELD_SESSION).is_empty());
    r.fleet.on("POST", CHECK, |b| {
        let broken = json!([{"line": 1, "message": "Unknown field.", "step": null}, {"line": 1, "message": "Again.", "step": null}]);
        (200, check_json(None, b["text"].as_str().unwrap_or(""), broken, json!([])))
    });
    r.type_text("x");
    r.settled();
    assert_eq!(r.rt.state.workflow_error_lines(FIELD_SESSION), vec![1], "once, though two errors are on the line");
    assert_eq!(r.rt.state.workflow_error_lines("another-session"), Vec::<usize>::new());
    assert_eq!(r.words("workflow-problems"), "line 1 \u{b7} Unknown field. line 1 \u{b7} Again.");
    assert!(!r.save_enabled(), "Save waits until the file is fixed");
}

#[test]
fn ctrl_or_cmd_s_in_the_field_saves_the_text_as_typed() {
    let mut r = rig();
    r.in_the_field(false);
    r.type_text("x");
    r.settled();
    press_key(&mut r.rt, Key::Command('s'), save_keys());
    r.drain();
    let puts = r.fleet.bodies("PUT", FILES);
    assert_eq!(puts.len(), 1, "once, not once for the field and once for the window");
    assert_eq!(puts[0]["text"], json!("xname: Build it our way\n"));
    assert!(!r.ed().is_dirty());
}

#[test]
fn text_set_from_outside_replaces_the_document() {
    let mut r = rig();
    r.in_the_field(false);
    r.type_text("x");
    r.rt.update(|s, _| s.workflows_nav.editor.text = "name: Reloaded\nsteps: []\n".into());
    r.frames();
    assert_eq!(r.lines(), vec!["name: Reloaded", "steps: []", ""]);
    assert_eq!(r.ed().text, "name: Reloaded\nsteps: []\n", "and what the field held does not come back");
}

#[test]
fn the_buffer_goes_when_the_file_view_does() {
    let mut r = rig();
    r.in_the_field(false);
    assert!(!r.lines().is_empty());
    r.view(EditorView::Designer);
    r.frames();
    assert!(r.lines().is_empty());
    assert!(r.rt.state.workflow_field_target().is_none());
    // Back again: the document is the editor's text, not a stale one.
    r.view(EditorView::File);
    r.frames();
    assert_eq!(r.lines(), vec!["name: Build it our way", ""]);
}

#[test]
fn a_file_opened_again_is_not_drawn_with_the_colours_of_the_one_before() {
    let mut r = rig();
    r.in_the_field(false);
    let rev = |r: &Rig| {
        let s = &r.rt.state;
        s.file_buffers.buffer(s.workflows_nav.editor.machine(), FIELD_SESSION, NAME).map(|b| b.rev())
    };
    let before = rev(&r);
    r.view(EditorView::Designer);
    r.frames();
    r.view(EditorView::File);
    r.frames();
    assert!(before.is_some() && rev(&r).is_some());
    assert_ne!(rev(&r), before, "the colours are cached by revision, so a new document needs a new number");
}
