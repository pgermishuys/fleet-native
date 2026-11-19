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
/// The file as the session left it: `SOURCE` is its git base.
const EDITED: &str = include_str!("changes/paginate.after.txt");

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
    /// An image, an image the folder lost, and an HTML file (piece I).
    Image,
    ImageMissing,
    Html,
    /// `Code` with lines 17 to 21 picked and the lines bar docked (piece K).
    Lines,
    /// `Lines` after Copy link: the tab's toast, lifted over the bar.
    LinesCopied,
    /// `Code` after Add to message: the reference in the draft, the pick gone.
    LinesAdded,
    /// The file changed against its git base: Diff (`Diff`), Edit with the stripe in the gutter (`Stripe`), and a file the session
    /// deleted (`Deleted`).
    Diff,
    Stripe,
    Deleted,
}

/// The invented project's logo (`assets/logo.svg`), 96 by 96 as its own size.
pub const IMAGE_PATH: &str = "assets/logo.svg";
pub const LOGO: &str = include_str!("file_logo.svg");
pub const HTML_PATH: &str = "docs/report.html";
pub const HTML: &str = include_str!("file_report.html");
pub const MARKDOWN: &str = include_str!("file_readme.md");

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
            Case::Markdown => "README.md",
            Case::Image | Case::ImageMissing => IMAGE_PATH,
            Case::Html => HTML_PATH,
            Case::Wrap | Case::WrapOff => WRAP_PATH,
            _ => PATH,
        };
        // The scenes of 11G were taken with Wrap off, as Fleet's pictures were; every file wraps unless it says otherwise.
        if matches!(
            case,
            Case::Code
                | Case::TooLarge
                | Case::Binary
                | Case::Missing
                | Case::Opening
                | Case::WrapOff
                | Case::Diff
                | Case::Stripe
                | Case::Deleted
        ) || matches!(case, Case::Lines | Case::LinesCopied | Case::LinesAdded)
        {
            self.sidebar.save_file_wrap(path, false);
        }
        self.diffs.reset(&id);
        self.diffs.apply(&json!({
            "diffs": [{"file": PATH, "status": if case == Case::Deleted { "deleted" } else { "modified" }, "additions": 3, "deletions": 3}],
            "available": true,
            "base": {"kind": "branch", "branch": "main", "commit": "8a61fdd7c0ffee"},
        }));
        self.diffs.has_loaded = true;
        self.edit_canvases(|c, id| {
            let view = match case {
                Case::Markdown | Case::Html => FileView::Rendered,
                Case::Diff => FileView::Diff,
                _ => FileView::Edit,
            };
            c.open_file(id, path, OpenFile { keep: true, view: Some(view) }, &|_| false);
        });
        match case {
            Case::Image => return self.supply_session_image(&id, path, LOGO.as_bytes()),
            Case::ImageMissing => {
                self.answer_session_image(&id, path, Ok((404, Vec::new())));
                return;
            }
            _ => {}
        }
        let machine = self.machine_key_of(&id).to_string();
        if matches!(case, Case::Diff | Case::Stripe | Case::Deleted) {
            self.give_base(&id, path);
        }
        let Some(request) = self.file_buffers.open(&machine, &id, path) else { return };
        self.read_for_scene(request, case);
        match case {
            Case::Lines | Case::LinesCopied => {
                let pick = crate::fleet::file_lines::LinePick { anchor: 17, head: 21 };
                self.file_buffers.set_pick(&machine, &id, path, Some(pick));
                if case == Case::LinesCopied {
                    self.file_lines.toast = Some(("Copied src/paginate.py:17-21".to_string(), self.clock + 60.0));
                }
            }
            Case::LinesAdded => self.append_draft_reference(&id, "@src/paginate.py:17-21"),
            _ => {}
        }
    }

    /// The git base the changes are compared with, as the server answered for the file.
    fn give_base(&mut self, id: &str, path: &str) {
        let machine = self.machine_key_of(id).to_string();
        let base = crate::fleet::diffs::Base::key(self.diffs.base.as_ref());
        if let crate::fleet::file_base::Found::Ask(request) = self.file_bases.fetch(&machine, id, &base, path) {
            self.file_bases.finish(&request, Ok((200, json!({"before": SOURCE, "after": EDITED}))));
        }
    }

    fn read_for_scene(&mut self, request: Request, case: Case) {
        let body = match case {
            Case::Opening => return,
            Case::Code | Case::Lines | Case::LinesCopied | Case::LinesAdded => Ok(answer(SOURCE)),
            Case::Diff | Case::Stripe => Ok(answer(EDITED)),
            Case::Deleted => Err("Failed to read file: 404 Not Found".to_string()),
            Case::Wrap | Case::WrapOff => Ok(answer(WRAP_SOURCE)),
            Case::Markdown => Ok(answer(MARKDOWN)),
            Case::Html => Ok(answer(HTML)),
            Case::Image | Case::ImageMissing => return,
            Case::TooLarge => Ok((200, json!({"content": null, "hash": "h1", "isBinary": false, "isTruncated": true}).to_string())),
            Case::Binary => Ok((200, json!({"content": null, "hash": "h1", "isBinary": true, "isTruncated": false}).to_string())),
            Case::Missing => Err("Failed to read file: 404 Not Found".to_string()),
        };
        self.file_buffers.finish(&request, body);
    }
}
