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
    /// "pag" typed in the search box, answered.
    Search,
    /// A query nothing matches.
    SearchNone,
    /// Contents mode, "page" searched inside the files, answered.
    Find,
    /// Contents mode, a query nothing matches.
    FindNone,
    /// Contents mode, the search stopped at its cap in a folder that isn't a git repository.
    FindLimit,
    /// Contents mode, a regex the server refused.
    FindBad,
    /// Contents mode, a machine whose Fleet is too old to search inside files.
    FindOld,
    /// Contents mode, the field empty: the tree and the hint.
    FindIdle,
}

/// A matching line the way the server answers it: every case-insensitive `needle` marked.
fn line(number: u32, words: &str, needle: &str) -> crate::fleet::find_text::TextMatch {
    let (low, key) = (words.to_lowercase(), needle.to_lowercase());
    // The server counts UTF-16 units, and the cut line starts with a "…" of one unit and three bytes.
    let units = |bytes: usize| low[..bytes].encode_utf16().count();
    let ranges = low
        .match_indices(&key)
        .map(|(i, m)| crate::fleet::find_text::TextRange { start: units(i), end: units(i) + m.encode_utf16().count() })
        .collect();
    crate::fleet::find_text::TextMatch { line: number, column: 1, text: words.to_string(), ranges }
}

fn file(path: &str, total: u32, matches: Vec<crate::fleet::find_text::TextMatch>) -> crate::fleet::find_text::TextFile {
    crate::fleet::find_text::TextFile { path: path.into(), total, matches }
}

/// What `find/text` answers for "page" in the invented project (ASCII only, so byte and UTF-16 offsets agree).
pub fn find_answer(engine: crate::fleet::find_text::Engine) -> crate::fleet::find_text::TextSearchResult {
    let paginate = [
        (1, "\"\"\"Split ledger rows into pages.\"\"\""),
        (4, "DEFAULT_PAGE_SIZE = 50"),
        (7, "def paginate(rows, page_size=DEFAULT_PAGE_SIZE):"),
        (8, "    \"\"\"Yield one page of rows at a time.\"\"\""),
        (9, "\u{2026}e(0, len(rows), page_size):"),
        (10, "        yield rows[start : start + page_size]"),
        (14, "def page_count(rows, size=DEFAULT_PAGE_SIZE):"),
    ];
    crate::fleet::find_text::TextSearchResult {
        files: vec![
            file("docs/pagination.md", 1, vec![line(3, "Rows are returned one page at a time.", "page")]),
            file(
                "src/ledger/page_size.py",
                2,
                vec![line(1, "\"\"\"The page size a ledger report uses.\"\"\"", "page"), line(3, "PAGE_SIZE = 50", "page")],
            ),
            file("src/ledger/paginate.py", 7, paginate.iter().map(|(n, w)| line(*n, w, "page")).collect()),
        ],
        matches: 10,
        stopped: None,
        engine,
    }
}

/// Every file of the invented project, folders left out (what the search lists once they are filtered).
const ALL_FILES: [&str; 11] = [
    "assets/logo.svg",
    "docs/pagination.md",
    "pyproject.toml",
    "README.md",
    "src/ledger/page_size.py",
    "src/ledger/paginate.py",
    "src/ledger/sample.csv",
    "src/ledger/totals.py",
    "src/ledger/__init__.py",
    "tests/test_paginate.py",
    "src/ledger/",
];

/// Every path of the invented project's file list.
pub fn all_files() -> Vec<&'static str> {
    ALL_FILES.to_vec()
}

/// Whether `path` is a file of the invented project (what the resolve endpoint answers for it).
pub fn is_file(path: &str) -> bool {
    (ALL_FILES.contains(&path) && !path.ends_with('/')) || path == super::file_tab::PATH
}

/// What the find endpoint answers for the invented project: files and the folder, ranked as Fleet's server does.
pub fn find(query: &str) -> Vec<String> {
    let all: Vec<String> = ALL_FILES.iter().map(|p| p.to_string()).collect();
    crate::composer::files::narrow_file_matches(&all, query.trim())
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
            Case::Tree | Case::Selected | Case::Search | Case::SearchNone => {}
            Case::Find | Case::FindNone | Case::FindLimit | Case::FindBad | Case::FindOld | Case::FindIdle => {}
        }
        browser.root_done(&id, browse(None));
        for dir in ["src", "src/ledger", "docs"] {
            browser.expand(dir);
            browser.dir_done(&id, dir, browse(Some(dir)));
        }
        if case == Case::Selected {
            browser.selected = Some("src/ledger/paginate.py".into());
        }
        use crate::fleet::find_text::{Engine, FailureKind, FindTextFailure, Stopped, TextSearchResult};
        if case == Case::FindIdle || matches!(case, Case::Find | Case::FindNone | Case::FindLimit | Case::FindBad | Case::FindOld) {
            browser.search_in = crate::fleet::file_browser::SearchIn::Contents;
        }
        let none = |engine| TextSearchResult { files: Vec::new(), matches: 0, stopped: None, engine };
        let (typed, found): (&str, Result<TextSearchResult, FindTextFailure>) = match case {
            Case::Search => ("pag", Ok(none(Engine::Git))),
            Case::SearchNone => ("zzq", Ok(none(Engine::Git))),
            Case::Find => ("page", Ok(find_answer(Engine::Git))),
            Case::FindNone => ("zzq", Ok(none(Engine::Git))),
            Case::FindLimit => ("page", Ok(TextSearchResult { stopped: Some(Stopped::Limit), matches: 200, ..find_answer(Engine::Walk) })),
            Case::FindBad => {
                ("page(", Err(FindTextFailure { kind: FailureKind::Pattern, message: "Unmatched ( or \\( (git extended regex)".into() }))
            }
            Case::FindOld => ("page", Err(FindTextFailure { kind: FailureKind::Unsupported, message: String::new() })),
            _ => return,
        };
        if matches!(case, Case::Find | Case::FindNone | Case::FindLimit | Case::FindBad | Case::FindOld) {
            self.find_text.regex = case == Case::FindBad;
            browser.field = rune::TextEdit::new(typed);
            self.find_text.show(&id, typed, found, 100.0);
            return;
        }
        browser.field = rune::TextEdit::new(typed);
        let scope = crate::fleet::finder::Scope::Session(id);
        // The first look starts the 80 ms wait; a later one sends, and the invented project answers.
        browser.search.want(Some(&scope), Some(typed), 0.0);
        if let crate::fleet::finder::Step::Send { generation, query } = browser.search.want(Some(&scope), Some(typed), 1.0) {
            browser.search.answered(generation, &query, Ok(find(&query)));
        }
    }

    /// The strip with file tabs: two kept (one filled pin each) and the preview, whose name is italic, in front.
    pub fn show_file_tabs(&mut self, dark: bool) {
        self.show_files_tab(Case::Selected, dark);
        // Widened, so the whole strip shows.
        self.right.widened = true;
        use crate::fleet::canvases::OpenFile;
        self.edit_canvases(|c, id| {
            let keep = OpenFile { keep: true, view: None };
            c.open_file(id, "README.md", keep.clone(), &|_| false);
            c.open_file(id, "src/ledger/paginate.py", keep, &|_| false);
            c.open_file(id, "src/ledger/totals.py", OpenFile::default(), &|_| false);
        });
    }
}
