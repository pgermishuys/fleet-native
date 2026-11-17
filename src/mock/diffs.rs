//! Invented changed files, as `GET /api/sessions/{id}/diffs` answers them: an added, a modified, a deleted and a lockfile entry
//! on a branch, and a session that changed nothing. For tests and scenes.
use serde_json::{Value, json};

/// A session on its own branch with four changes.
pub fn changed() -> Value {
    json!({
        "diffs": [
            {"file": "src/page_size.py", "status": "added", "additions": 17, "deletions": 0, "before": "", "after": ""},
            {"file": "src/paginate.py", "status": "modified", "additions": 3, "deletions": 3, "before": "", "after": ""},
            {"file": "docs/old-notes.md", "status": "deleted", "additions": 0, "deletions": 9, "before": "", "after": ""},
            {"file": "package-lock.json", "status": "modified", "additions": 1204, "deletions": 87, "before": "", "after": "", "isTruncated": true},
        ],
        "available": true,
        "base": {"kind": "branch", "branch": "main", "commit": "3f9a1c2d7e"},
    })
}

/// A session with nothing changed, compared with how the folder was when it started.
pub fn unchanged() -> Value {
    json!({"diffs": [], "available": true, "base": {"kind": "session"}})
}
