//! The invented GitHub of this component's scenes and tests: answers shaped as Fleet's `/api/integrations/github/*` routes send
//! them (owners `acme` and `lumen-labs`; people `mara-quinn` (signed in), `devon-park`, `kit-lee`), so nothing needs a server.
//! Each function is one answer as JSON; read it with the types in `items`.
use serde_json::{Value, json};

fn pull(number: u64, title: &str, extra: Value) -> Value {
    let mut item = json!({
        "kind": "pull", "owner": "acme", "repo": "rocket", "number": number, "title": title,
        "url": format!("https://github.com/acme/rocket/pull/{number}"), "state": "open", "isDraft": false,
        "author": "devon-park", "authorAvatarUrl": null, "createdAt": "2026-11-16T09:12:00Z", "updatedAt": "2026-11-19T14:30:00Z",
        "comments": 2, "labels": [{"name": "backend", "color": "1d76db"}], "headRef": format!("feat/pr-{number}"), "baseRef": "main",
        "additions": 42, "deletions": 7, "checks": "success", "reviewDecision": null, "mergeable": "MERGEABLE",
        "reviewers": [{"login": "kit-lee", "state": "REQUESTED"}], "assignees": [],
    });
    if let (Some(item), Some(extra)) = (item.as_object_mut(), extra.as_object()) {
        item.extend(extra.clone());
    }
    item
}

fn issue(number: u64, title: &str) -> Value {
    json!({
        "kind": "issue", "owner": "acme", "repo": "rocket", "number": number, "title": title,
        "url": format!("https://github.com/acme/rocket/issues/{number}"), "state": "open", "isDraft": false,
        "author": "kit-lee", "authorAvatarUrl": null, "createdAt": "2026-11-10T08:00:00Z", "updatedAt": "2026-11-18T10:00:00Z",
        "comments": 4, "labels": [{"name": "bug", "color": "d73a4a"}], "headRef": null, "baseRef": null, "additions": null,
        "deletions": null, "checks": null, "reviewDecision": null, "mergeable": null, "reviewers": [], "assignees": ["mara-quinn"],
    })
}

/// A search answer: a page of two, with more to come.
pub fn search_page() -> Value {
    json!({
        "items": [pull(218, "Queue retries with backoff", json!({})), issue(7, "Launch countdown skips a second")],
        "totalCount": 31, "endCursor": "Y3Vyc29yOjI5", "hasNextPage": true,
    })
}

/// One pull request's page: checks (one failing, one skipped), a review, a comment and a merge-state change, an HTML comment
/// in the body.
pub fn pull_detail() -> Value {
    json!({
        "summary": pull(218, "Queue retries with backoff", json!({
            "checks": "failure", "reviewDecision": "CHANGES_REQUESTED", "mergeable": "CONFLICTING",
            "reviewers": [{"login": "kit-lee", "avatarUrl": "https://avatars.example/kit-lee", "state": "CHANGES_REQUESTED"}],
        })),
        "body": "Retries a failed launch.\n<!-- bot: release notes -->\nCloses #7.",
        "changedFiles": 5, "mergedAt": null, "mergedBy": null,
        "checks": [
            {"name": "build", "state": "success", "url": "https://github.com/acme/rocket/actions/runs/1", "workflowName": "CI", "checkRunId": 11,
             "startedAt": "2026-11-19T14:00:00Z", "completedAt": "2026-11-19T14:04:00Z"},
            {"name": "lint", "state": "skipped", "url": null, "workflowName": "CI", "checkRunId": 12, "startedAt": null, "completedAt": null},
            {"name": "test", "state": "failure", "url": "https://github.com/acme/rocket/actions/runs/2", "workflowName": "CI", "checkRunId": 13,
             "startedAt": "2026-11-19T14:00:00Z", "completedAt": "2026-11-19T14:09:00Z"},
        ],
        "unresolvedThreads": 1,
        "timeline": [
            {"kind": "comment", "author": "devon-park", "authorAvatarUrl": null, "createdAt": "2026-11-16T09:20:00Z", "body": "Ready for a look.", "state": null, "url": null, "reference": null},
            {"kind": "review", "author": "kit-lee", "authorAvatarUrl": null, "createdAt": "2026-11-18T11:00:00Z", "body": "Please add a test.", "state": "CHANGES_REQUESTED", "url": null, "reference": null},
            {"kind": "referenced", "author": "mara-quinn", "authorAvatarUrl": null, "createdAt": "2026-11-19T08:00:00Z", "body": null, "state": "open", "url": "https://github.com/acme/billing/issues/3",
             "reference": {"kind": "issue", "repository": "acme/billing", "number": 3}},
            {"kind": "pinned-by-a-newer-server", "author": null, "createdAt": "2026-11-19T09:00:00Z"},
        ],
    })
}

/// What the signed-in user has to do: one review asked, three of their own (a failing check, requested changes, nothing wrong).
pub fn work() -> Value {
    json!({
        "login": "mara-quinn", "avatarUrl": null,
        "reviewRequested": [pull(220, "Fuel gauge rounding", json!({"repo": "rocket"}))],
        "authored": [
            pull(221, "Retry the launch hook", json!({"author": "mara-quinn", "checks": "failure"})),
            pull(222, "Trim the manifest", json!({"author": "mara-quinn", "reviewDecision": "CHANGES_REQUESTED"})),
            pull(223, "Rename the crew screen", json!({"author": "mara-quinn"})),
        ],
        "assigned": [issue(7, "Launch countdown skips a second")],
        "repos": [
            {"fullName": "acme/rocket", "openPullRequests": 4, "openIssues": 9},
            {"fullName": "lumen-labs/atlas", "openPullRequests": 0, "openIssues": 2},
        ],
    })
}

/// Your repositories as `GET .../repos` sends them: the three the panel scenes follow, and six to pick from (a private one, one with
/// no language, one with no stars).
pub fn repos() -> Value {
    let repo = |id: u64, owner: &str, name: &str, private: bool, language: Value, stars: u64| {
        json!({"id": id, "full_name": format!("{owner}/{name}"), "name": name, "owner": {"login": owner}, "private": private,
               "language": language, "stargazers_count": stars})
    };
    json!([
        repo(1, "acme", "rocket", false, json!("Rust"), 120),
        repo(2, "acme", "launchpad", true, json!("Rust"), 12),
        repo(3, "acme", "crew-screen", false, json!("TypeScript"), 48),
        repo(4, "acme", "billing", true, json!("Go"), 0),
        repo(5, "lumen-labs", "atlas", false, json!("Python"), 9),
        repo(6, "lumen-labs", "beacon", false, json!("Python"), 3),
        repo(7, "lumen-labs", "lens", false, json!("Python"), 31),
        repo(8, "lumen-labs", "observatory", false, Value::Null, 5),
        repo(9, "mara-quinn", "dotfiles", true, json!("Shell"), 0),
    ])
}
