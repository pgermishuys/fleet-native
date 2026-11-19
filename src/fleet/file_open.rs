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
        self.read_file_for_tab(session, path);
        self.sync_review();
        self.reveal_active(tx);
    }

    /// An image is only ever shown, so it is read as bytes and has no buffer; any other file is read as text.
    fn read_file_for_tab(&mut self, session: &str, path: &str) {
        if super::canvases::is_image_path(path) {
            self.read_session_image(session, path);
        } else {
            self.open_file_buffer(session, path);
        }
    }

    /// The tab that came to the front is an image: the agent may have changed it while it was behind (`onActivated`).
    pub fn reread_active_image(&mut self) {
        let tabs = self.open_canvases();
        let Some(path) = tabs.active().and_then(|c| c.file.as_ref()).map(|f| f.path.clone()).filter(|p| super::canvases::is_image_path(p))
        else {
            return;
        };
        let session = self.canvas_session();
        self.read_session_image(&session, &path);
    }

    /// A press on the view switch (`setFileView`): the open file tab shows `view`.
    pub fn set_active_file_view(&mut self, path: &str, view: super::canvases::FileView) {
        let session = self.canvas_session();
        self.canvases.set_file_view(&session, path, view);
    }

    /// The Wrap button and Alt+Z (`setWrap`): the file's choice, kept in the settings file when it differs from the default.
    pub fn set_file_wrap(&mut self, path: &str, wrap: bool) {
        self.sidebar.save_file_wrap(path, wrap);
    }

    /// Alt+Z (`toggleWrap`, in the editor's keymap): turns wrap on or off for the open file while its code shows. Fleet binds it
    /// in the editor, so a key typed in the composer or another field is left to that field.
    pub fn file_wrap_key(&mut self, ev: &rune::KeyEvent, tx: &rune::Tx) -> bool {
        let pressed = ev.mods.alt && !ev.mods.ctrl && !ev.mods.cmd && !ev.mods.shift;
        let z = matches!(&ev.key, rune::Key::Text(t) if t.eq_ignore_ascii_case("z") || t == "\u{3a9}" || t == "\u{b8}");
        if !pressed || !z || tx.focused().is_some_and(|k| !(k.starts_with("file-") || k.starts_with("code-"))) {
            return false;
        }
        let tabs = self.open_canvases();
        let Some(file) = tabs.active().and_then(|c| c.file.clone()) else { return false };
        let session = self.canvas_session();
        let machine = self.machine_key_of(&session).to_string();
        let ready = self.file_buffers.info(&machine, &session, &file.path).is_some_and(|i| i.status == super::file_buffers::Status::Ready);
        if !ready || file.view == super::canvases::FileView::Rendered {
            return false;
        }
        let on = self.sidebar.file_wrap(&file.path);
        self.set_file_wrap(&file.path, !on);
        true
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
