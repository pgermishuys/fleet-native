//! The File view's text field (component 18, piece N; `WorkflowFileEditor.vue`). Fleet puts the file in CodeMirror; here it is
//! component 11's editor over a buffer that is not a session's file: `file_buffers` holds it under a session name of its own, so
//! the code view, its keys, caret, history and highlighting work on it as they do on a file. This module keeps that buffer and the
//! workflow editor's `text` in step: what is typed goes to `Editor::edit_text` (which arms the 300 ms check), and text set from
//! outside (a reload, a save, a switch from the designer) replaces the document.
use crate::State;
use crate::fleet::code_editing::FileKey;
use crate::fleet::workflows::editor::EditorView;

/// The "session" the field's buffer lives under. No session has this id, so nothing else (the tabs, Find, the changes lists)
/// ever finds it.
pub const FIELD_SESSION: &str = "workflow-file-field";

#[derive(Debug, Default)]
pub struct Field {
    /// The buffer that is open: it goes when the File view does.
    open: Option<FileKey>,
    /// The text both sides last agreed on.
    synced: String,
}

impl State {
    /// The field's buffer while the File view of the open workflow is on screen.
    pub fn workflow_field_target(&self) -> Option<FileKey> {
        let ed = &self.workflows_nav.editor;
        let shown = self.rail == crate::Rail::Workflows && self.workflow_editor_shown() && ed.view == EditorView::File;
        let file = ed.file.as_ref().filter(|_| shown)?;
        Some((ed.machine().to_string(), FIELD_SESSION.to_string(), file.file.clone()))
    }

    /// The 1-based lines the parser has errors on (`errorLines`), for the view's marks.
    pub fn workflow_error_lines(&self, session: &str) -> Vec<usize> {
        if session != FIELD_SESSION {
            return Vec::new();
        }
        let mut lines: Vec<usize> = self.workflows_nav.editor.errors().iter().filter(|e| e.line > 0).map(|e| e.line as usize).collect();
        lines.sort_unstable();
        lines.dedup();
        lines
    }

    /// Once a frame (`poll`): opens the buffer when the File view appears, takes what was typed to the editor's state, puts text
    /// that came from outside into the buffer, and drops the buffer when the view goes. True when something changed.
    pub fn keep_workflow_field(&mut self) -> bool {
        let want = self.workflow_field_target();
        let field = &mut self.workflows_nav.frame.field;
        if field.open != want
            && let Some((m, s, p)) = field.open.take()
        {
            self.file_buffers.remove(&m, &s, &p);
            self.code_edit.file = self.code_edit.file.take().filter(|f| *f != (m, s, p));
        }
        let Some((m, s, p)) = want else { return false };
        let field = &mut self.workflows_nav.frame.field;
        if field.open.is_none() {
            let text = self.workflows_nav.editor.text.clone();
            self.file_buffers.load_text(&m, &s, &p, &text);
            let field = &mut self.workflows_nav.frame.field;
            field.synced = text;
            field.open = Some((m, s, p));
            return true;
        }
        let outside = self.workflows_nav.editor.text.clone();
        if outside != field.synced {
            // The editor's text changed under the field (a reload, a save): the document is replaced.
            self.file_buffers.load_text(&m, &s, &p, &outside);
            self.workflows_nav.frame.field.synced = outside;
            self.code_edit.stamp += 1;
            return true;
        }
        let Some(typed) = self.file_buffers.take_text(&m, &s, &p) else { return false };
        if typed == self.workflows_nav.frame.field.synced {
            return false;
        }
        self.workflows_nav.frame.field.synced = typed.clone();
        self.workflows_nav.editor.edit_text(&typed);
        true
    }
}
