//! Invented smart links for the mock rows, shaped as the server sends them (`metadataJson` a JSON string). They stand in
//! for a Fleet with a GitHub plugin: the `pr-badges-links` scene and the tests draw the session list's badges from these
//! instead of from a badge written on the row, which shows the whole path (link, facts, state, words) end to end.
use super::Project;
use serde_json::{Value, json};

fn run(name: &str, status: &str, conclusion: Option<&str>) -> Value {
    json!({"id": name.len(), "name": name, "status": status, "conclusion": conclusion, "htmlUrl": format!("https://ci.example.test/{name}"),
        "workflowName": "CI", "startedAt": null, "completedAt": null})
}

/// One invented pull request link of `session` (`own`: the session opened it). `runs` are its checks.
pub fn pull_request(session: &str, number: u32, status: &str, runs: Vec<Value>, review: Option<&str>) -> Value {
    let metadata = json!({
        "owner": "example", "repo": "demo", "number": number,
        "htmlUrl": format!("https://github.com/example/demo/pull/{number}"),
        "ci": {"headSha": "0123456789abcdef", "ciStatus": "none", "checkRuns": runs},
        "reviewDecision": review,
    });
    json!({
        "id": format!("link-{number}"), "sessionId": session, "url": format!("https://github.com/example/demo/pull/{number}"),
        "providerId": "github", "resourceType": "pull_request", "resourceId": format!("example/demo#{number}"),
        "title": format!("example/demo #{number}: Change {number}"), "status": status, "statusLabel": status,
        "metadataJson": metadata.to_string(), "isDismissed": false, "isTerminal": matches!(status, "merged" | "closed"),
        "createdAt": "2024-10-03T10:00:00Z", "updatedAt": "2024-10-04T11:00:00Z", "relationship": "own",
        "enrichmentStatus": "resolved", "lastCheckedAt": "2024-10-04T11:58:00Z",
    })
}

type Row<'a> = (&'a str, u32, &'a str, Vec<Value>, Option<&'a str>);

/// The links that give the `chips` project's rows the badges it writes on them: the same numbers, states and words.
pub fn chips_links(project: &Project) -> Vec<Value> {
    let ok = |name: &str| run(name, "completed", Some("success"));
    let table: [Row; 7] = [
        ("Add the export button", 187, "open", vec![ok("lint"), ok("build"), run("e2e", "in_progress", None)], None),
        ("Tidy the release notes", 184, "open", vec![ok("lint"), ok("build")], Some("APPROVED")),
        ("Port the importer", 181, "open", vec![ok("lint"), run("e2e", "completed", Some("failure"))], None),
        ("Draft the onboarding copy", 179, "draft", vec![run("lint", "queued", None)], None),
        ("Ship the invoice export", 170, "merged", vec![], None),
        ("Try the older parser", 166, "closed", vec![], None),
        (crate::mock::long::TITLE, 201, "open", vec![ok("lint"), ok("build")], Some("APPROVED")),
    ];
    table
        .into_iter()
        .filter_map(|(title, number, status, runs, review)| {
            let session = project.sessions.iter().find(|s| s.title == title)?;
            Some(pull_request(&session.id, number, status, runs, review))
        })
        .collect()
}
