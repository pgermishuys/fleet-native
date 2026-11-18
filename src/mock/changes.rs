//! Invented states for the Changes body's scenes and tests (component 12, piece D): what the body shows before there is a review.
use crate::State;
use crate::fleet::diffs::{Base, Refusal};
use serde_json::json;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// The first answer hasn't come.
    Loading,
    /// `/diffs` answered HTTP 500.
    Failed,
    /// Git won't read the folder.
    Refused,
    /// A folder off any branch, nothing changed.
    EmptySession,
    /// A branch compared with where it left main, nothing changed.
    EmptyBranch,
    /// An added file and an edited one, both drawn (piece E).
    Review,
    /// One file in each state its body has: loading, failed, binary, too large, no line changes, and a lockfile that starts shut.
    ReviewStates,
    /// One file whose diff has 1,200 changed lines: the first 600 draw and Show more draws the rest (piece F).
    ReviewLong,
}

pub const FIX: &str = "git config --global --add safe.directory /srv/demo-app";

impl State {
    /// The panel open on Changes with `case` held.
    pub fn show_changes_scene(&mut self, case: Case, dark: bool) {
        self.show_tabs(super::canvas_host::Case::Fresh, dark);
        self.content_open = true;
        self.right.review = false;
        let id = self.canvas_session();
        self.diffs.reset(&id);
        if matches!(case, Case::Review | Case::ReviewStates | Case::ReviewLong) {
            self.review_scene(case);
            return;
        }
        let d = &mut self.diffs;
        match case {
            Case::Loading => return,
            Case::Failed => d.error = Some("HTTP 500".into()),
            Case::Refused => {
                d.refusal =
                    Some(Refusal { message: "Git won't read this folder because another user owns it.".into(), fix_command: FIX.into() })
            }
            Case::EmptySession => d.base = Some(Base::Session),
            Case::EmptyBranch => d.base = Some(Base::Branch { branch: Some("main".into()), commit: Some("8a61fdd7c0ffee".into()) }),
            Case::Review | Case::ReviewStates | Case::ReviewLong => {}
        }
        d.has_loaded = true;
    }

    /// The review over invented files, their contents held as if they had come.
    fn review_scene(&mut self, case: Case) {
        use crate::fleet::diff_contents::Entry;
        use crate::fleet::diffs::{FileDiff, FileStatus};
        let id = self.canvas_session();
        self.right.review = true;
        let mut list = Vec::new();
        let mut held: Vec<(&str, Entry)> = Vec::new();
        let mut file = |path: &str, status: &str, add: u64, del: u64, before: String, after: String, flags: [bool; 2]| {
            list.push(json!({ "file": path, "status": status, "additions": add, "deletions": del, "isBinary": flags[0], "isTruncated": flags[1] }));
            FileDiff {
                file: path.into(),
                status: if status == "added" { FileStatus::Added } else { FileStatus::Modified },
                additions: add,
                deletions: del,
                before,
                after,
                binary: flags[0],
                truncated: flags[1],
            }
        };
        match case {
            Case::Review => {
                // The files the scripted model's `edit-file` and `write-file` make in a scratch Fleet, so the picture sets the same files
                // beside Fleet's own client.
                let before = include_str!("changes/paginate.before.txt").to_string();
                let after = include_str!("changes/paginate.after.txt").to_string();
                let paginate = file("src/paginate.py", "modified", 3, 3, before, after, [false; 2]);
                let page_size = file(
                    "src/page_size.py",
                    "added",
                    17,
                    0,
                    String::new(),
                    include_str!("changes/page_size.py.txt").to_string(),
                    [false; 2],
                );
                held.push(("src/paginate.py", Entry::Ready(paginate)));
                held.push(("src/page_size.py", Entry::Ready(page_size)));
            }
            Case::ReviewLong => {
                let row = |i: usize, changed: bool| format!("    \"item_{i}\": {}{}\n", i * 7, if changed { 1 } else { 0 });
                let before: String = (0..1200).map(|i| row(i, false)).collect();
                let after: String = (0..1200).map(|i| row(i, i % 2 == 1)).collect();
                let long = file("data/items.json", "modified", 600, 600, before, after, [false; 2]);
                held.push(("data/items.json", Entry::Ready(long)));
            }
            _ => {
                let one = |n: &str| (format!("{n} = 1\n"), format!("{n} = 2\n"));
                let (b, a) = one("alpha");
                let ready = file("src/alpha.rs", "modified", 1, 1, b, a, [false; 2]);
                held.push(("src/alpha.rs", Entry::Ready(ready)));
                file("src/beta_loading.rs", "modified", 1, 0, String::new(), String::new(), [false; 2]);
                file("assets/failed.rs", "modified", 4, 2, String::new(), String::new(), [false; 2]);
                held.push(("assets/failed.rs", Entry::Error("HTTP 502".into())));
                let binary = file("assets/logo.png", "modified", 0, 0, String::new(), String::new(), [true, false]);
                held.push(("assets/logo.png", Entry::Ready(binary)));
                let big = file("data/dump.sql", "modified", 400, 3, String::new(), String::new(), [false, true]);
                held.push(("data/dump.sql", Entry::Ready(big)));
                let same = file("docs/same.txt", "modified", 1, 1, "x\n".into(), "x\n".into(), [false; 2]);
                held.push(("docs/same.txt", Entry::Ready(same)));
                file("yarn.lock", "modified", 12, 12, String::new(), String::new(), [false; 2]);
            }
        }
        let base = json!({ "kind": "branch", "branch": "main", "commit": "8a61fdd7c0ffee" });
        let d = &mut self.diffs;
        d.apply(&json!({ "diffs": list, "available": true, "base": base }));
        d.has_loaded = true;
        for (path, entry) in held {
            self.diff_contents.put(&id, path, entry);
        }
        self.review.open(&id);
        let files = self.review_files();
        self.review.decide(&files);
    }

    /// The review of the scratch Fleet's two files with the first unchanged run of `src/paginate.py` opened, or with lines wrapped.
    pub fn show_review_hunks(&mut self, dark: bool, wrap: bool) {
        use crate::conv::diff::parse_diff_lines;
        use crate::conv::review::{ReviewItem, build_review};
        self.show_changes_scene(Case::Review, dark);
        self.sidebar.save_changes_wrap(wrap);
        if wrap {
            return;
        }
        let item = self.diff_contents.entry("src/paginate.py").and_then(|e| match e {
            crate::fleet::diff_contents::Entry::Ready(f) => Some(f.clone()),
            _ => None,
        });
        if let Some(f) = item {
            let items = build_review(&parse_diff_lines(&f.before, &f.after));
            if let Some(ReviewItem::Fold(fold)) = items.iter().find(|i| matches!(i, ReviewItem::Fold(_))) {
                self.review.open_fold("src/paginate.py", &fold.id);
            }
        }
    }
}
