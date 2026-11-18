//! An invented project for the Files tab (component 11, piece D): the "Ledger Lite" the scratch Fleet's pictures use, so the
//! native tree and Fleet's own client show the same folders. It answers the browse endpoint for an invented session, and
//! draws the tab's scenes.
use crate::State;
use crate::fleet::canvases::CanvasKind;
use crate::fleet::file_browser::Entry;
use serde_json::json;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// `src/ledger` and `docs` open, two files changed.
    Tree,
    /// The same with a file clicked.
    Selected,
    /// The root is on its way.
    Loading,
    /// The folder has nothing in it.
    Empty,
    /// The root could not be read.
    Failed,
}

/// What the browse endpoint says for a folder of the invented project (the root with no path): folders first, then files, each in the server's order (case ignored, so `pyproject.toml` is before `README.md` and `__init__.py` last).
pub fn browse(path: Option<&str>) -> Result<Vec<Entry>, String> {
    let file = |dir: &str, name: &str| Entry::file(name, &format!("{dir}{name}"));
    let dir = |parent: &str, name: &str| Entry::dir(name, &format!("{parent}{name}"));
    Ok(match path.unwrap_or("") {
        "" => vec![dir("", "assets"), dir("", "docs"), dir("", "src"), dir("", "tests"), file("", "pyproject.toml"), file("", "README.md")],
        "assets" => vec![file("assets/", "logo.svg")],
        "docs" => vec![file("docs/", "pagination.md")],
        "src" => vec![dir("src/", "ledger")],
        "src/ledger" => vec![
            file("src/ledger/", "page_size.py"),
            file("src/ledger/", "paginate.py"),
            file("src/ledger/", "sample.csv"),
            file("src/ledger/", "totals.py"),
            file("src/ledger/", "__init__.py"),
        ],
        "tests" => vec![file("tests/", "test_paginate.py")],
        other => return Err(format!("Failed to browse directory: 404 Not Found ({other})")),
    })
}

impl State {
    /// The panel open on Files with `case` held.
    pub fn show_files_tab(&mut self, case: Case, dark: bool) {
        self.show_tabs(super::canvas_host::Case::Fresh, dark);
        self.content_open = true;
        let id = self.canvas_session();
        self.edit_canvases(|c, id| {
            c.open(id, CanvasKind::Files);
        });
        self.diffs.reset(&id);
        self.diffs.apply(&json!({
            "diffs": [
                {"file": "src/ledger/page_size.py", "status": "added", "additions": 5, "deletions": 0},
                {"file": "src/ledger/paginate.py", "status": "modified", "additions": 2, "deletions": 2},
            ],
            "available": true,
            "base": {"kind": "branch", "branch": "main", "commit": "8a61fdd7c0ffee"},
        }));
        self.diffs.has_loaded = true;
        self.file_browser = Default::default();
        let browser = &mut self.file_browser;
        browser.follow(&id);
        browser.start_root();
        match case {
            Case::Loading => return,
            Case::Failed => {
                browser.root_done(&id, Err("Failed to browse directory: 500 Internal Server Error".into()));
                return;
            }
            Case::Empty => {
                browser.root_done(&id, Ok(Vec::new()));
                return;
            }
            Case::Tree | Case::Selected => {}
        }
        browser.root_done(&id, browse(None));
        for dir in ["src", "src/ledger", "docs"] {
            browser.expand(dir);
            browser.dir_done(&id, dir, browse(Some(dir)));
        }
        if case == Case::Selected {
            browser.selected = Some("src/ledger/paginate.py".into());
        }
    }
}
