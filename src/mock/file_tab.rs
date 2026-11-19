//! An invented file for the file tab's scenes and tests (component 11, piece G): `src/paginate.py` of the scratch Fleet's demo project,
//! so the native tab and Fleet's own client show the same lines, in each state the tab has.
use crate::State;
use crate::fleet::canvases::{FileView, OpenFile};
use crate::fleet::file_buffers::Request;
use serde_json::json;

pub const PATH: &str = "src/paginate.py";
/// The file with long lines (line 17 is a docstring that wraps twice).
pub const WRAP_PATH: &str = "src/pager.py";
const WRAP_SOURCE: &str = include_str!("file_wrap.txt");
const SOURCE: &str = include_str!("changes/paginate.before.txt");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// The file read and drawn.
    Code,
    /// The read is on its way.
    Opening,
    /// The server says the file is over 512 KB.
    TooLarge,
    /// The file isn't text.
    Binary,
    /// The file isn't in the folder any more.
    Missing,
    /// A Markdown file, which has a Rendered view.
    Markdown,
    /// A file with long lines, wrapped (the default) and with Wrap turned off (`WrapOff`).
    Wrap,
    WrapOff,
}

/// What the read endpoint answers for `text`.
pub fn answer(text: &str) -> (u16, String) {
    (200, json!({"content": text, "hash": "h1", "isBinary": false, "isTruncated": false}).to_string())
}

impl State {
    /// The panel open on a file tab held in `case`, widened so the whole bar shows.
    pub fn show_file(&mut self, case: Case, dark: bool) {
        self.show_tabs(super::canvas_host::Case::Fresh, dark);
        self.content_open = true;
        self.right.widened = true;
        let id = self.canvas_session();
        let path = match case {
            Case::Markdown => "docs/pagination.md",
            Case::Wrap | Case::WrapOff => WRAP_PATH,
            _ => PATH,
        };
        // The scenes of 11G were taken with Wrap off, as Fleet's pictures were; every file wraps unless it says otherwise.
        if matches!(case, Case::Code | Case::TooLarge | Case::Binary | Case::Missing | Case::Opening | Case::WrapOff) {
            self.sidebar.save_file_wrap(path, false);
        }
        self.diffs.reset(&id);
        self.diffs.apply(&json!({
            "diffs": [{"file": PATH, "status": "modified", "additions": 3, "deletions": 3}],
            "available": true,
            "base": {"kind": "branch", "branch": "main", "commit": "8a61fdd7c0ffee"},
        }));
        self.diffs.has_loaded = true;
        self.edit_canvases(|c, id| {
            let view = if case == Case::Markdown { FileView::Rendered } else { FileView::Edit };
            c.open_file(id, path, OpenFile { keep: true, view: Some(view) }, &|_| false);
        });
        let machine = self.machine_key_of(&id).to_string();
        let Some(request) = self.file_buffers.open(&machine, &id, path) else { return };
        self.read_for_scene(request, case);
    }

    fn read_for_scene(&mut self, request: Request, case: Case) {
        let body = match case {
            Case::Opening => return,
            Case::Code => Ok(answer(SOURCE)),
            Case::Wrap | Case::WrapOff => Ok(answer(WRAP_SOURCE)),
            Case::Markdown => {
                Ok(answer("# Pagination\n\nPages are `size` rows long.\n\n- first page is 0\n- last page is `last_page(total)`\n"))
            }
            Case::TooLarge => Ok((200, json!({"content": null, "hash": "h1", "isBinary": false, "isTruncated": true}).to_string())),
            Case::Binary => Ok((200, json!({"content": null, "hash": "h1", "isBinary": true, "isTruncated": false}).to_string())),
            Case::Missing => Err("Failed to read file: 404 Not Found".to_string()),
        };
        self.file_buffers.finish(&request, body);
    }
}
