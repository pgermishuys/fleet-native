//! The scenes of file links in messages (component 11, piece M): a reply that names files, with the ones the invented project has
//! drawn as links, and the file open at the line the text named.
use crate::State;
use crate::view::conversation::{Message, Part};

/// The line the reply names in `src/paginate.py` (the file the file scenes show).
pub const LINE: u32 = 24;

impl State {
    /// The open session's conversation is a question and a reply naming files; `opened` also shows the file at that line.
    pub fn show_message_file_links(&mut self, dark: bool, opened: bool) {
        if opened {
            self.show_file(super::file_tab::Case::Code, dark);
        } else {
            self.show_file_tabs(dark);
            self.content_open = false;
        }
        let (p, i) = self.open_at();
        let reply = "The page size is read in `src/paginate.py:24`, and the default is set in `src/ledger/page_size.py`. \
`src/ledger/legacy.py` is gone, so `paginate()` and `DEFAULT_PAGE_SIZE` stay code. The rules are written up in \
[the pagination doc](docs/pagination.md#L3).";
        let messages = vec![Message::user("u1", "Where is the page size read?"), Message::assistant("a1", vec![Part::text("a1-t", reply)])];
        self.projects[p].sessions[i].messages = messages;
        self.keep_file_links(0.0);
        if opened {
            let id = self.canvas_session();
            self.go_to_file.shown_line = Some((id, super::file_tab::PATH.to_string(), LINE));
        }
    }
}
