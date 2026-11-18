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
}
