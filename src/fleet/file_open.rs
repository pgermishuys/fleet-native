//! Opening a file in its own tab (`stores/canvases.ts` `openFile`, with what the buffers and the strip do about it): the tab
//! opens as the preview or, kept, beside the others and comes to the front; the file is read; the preview it replaced loses its
//! buffer. The Files tab's rows, its search results, Go to file and a message's file links all come through here.
use super::canvases::OpenFile;
use crate::State;
use rune::Tx;

impl State {
    /// Opens `path` of `session` as a tab (a preview unless `options.keep`), reading it the first time.
    pub fn open_file(&mut self, session: &str, path: &str, options: OpenFile, tx: &mut Tx) {
        let machine = self.machine_key_of(session).to_string();
        let buffers = &self.file_buffers;
        let opened = self.canvases.open_file(session, path, options, &|p| buffers.is_dirty(&machine, session, p));
        if let Some(old) = opened.dropped {
            self.close_file_buffer(session, &old);
        }
        self.open_file_buffer(session, path);
        self.sync_review();
        self.reveal_active(tx);
    }

    /// A press on the view switch (`setFileView`): the open file tab shows `view`.
    pub fn set_active_file_view(&mut self, path: &str, view: super::canvases::FileView) {
        let session = self.canvas_session();
        self.canvases.set_file_view(&session, path, view);
    }

    /// A double-click on a preview tab keeps it (`keepPreview`); a kept tab does nothing.
    pub fn keep_preview_tab(&mut self, id: &str) {
        let tabs = self.open_canvases();
        let Some(path) =
            tabs.canvases.iter().find(|c| c.id == id).and_then(|c| c.file.as_ref()).filter(|f| f.preview).map(|f| f.path.clone())
        else {
            return;
        };
        let session = self.canvas_session();
        self.canvases.keep_file(&session, &path);
    }

    /// The pin on a file tab (`togglePin`): a preview is kept; a kept tab becomes the preview, and the preview it replaces closes
    /// with its buffer unless that one has unsaved changes.
    pub fn toggle_file_pin(&mut self, id: &str, tx: &mut Tx) {
        let tabs = self.open_canvases();
        let Some(file) = tabs.canvases.iter().find(|c| c.id == id).and_then(|c| c.file.clone()) else { return };
        let session = self.canvas_session();
        if file.preview {
            self.canvases.keep_file(&session, &file.path);
            return;
        }
        let machine = self.machine_key_of(&session).to_string();
        let buffers = &self.file_buffers;
        if let Some(old) = self.canvases.unpin_file(&session, &file.path, &|p| buffers.is_dirty(&machine, &session, p)) {
            self.close_file_buffer(&session, &old);
        }
        self.sync_review();
        self.reveal_active(tx);
    }
}
