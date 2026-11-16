//! Invented session context for the header's chip and pill scenes and tests (component 8, piece G): every state of the pull
//! request pill, the chip row with where the session came from, an automation and "+2". Shaped as the server sends links
//! (`mock::links`), so the scenes draw the whole path from the wire to the pixels.
use super::links::pull_request;
use crate::State;
use crate::fleet::rows::Origin;
use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// Origin issue, the pull request (blocked) and two pinned links.
    Chips,
    Open,
    Blocked,
    Draft,
    Merged,
    Closed,
    /// GitHub hasn't answered: a neutral number only.
    Pending,
    /// An automation started the session; the pull request is blocked.
    Automation,
    /// An automation, the pull request and four pinned links: two chips and "+2".
    Overflow,
    /// The popover's pull request with a failing check, a thread, changes requested and a conflict.
    PopBlocked,
    /// Only a failing check and a running one.
    PopChecks,
    /// Only open threads.
    PopReview,
    PopMerged,
    /// Approved, with every check passed.
    PopReady,
}

/// What the popover's scene shows beyond the pull request.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pop {
    Plain,
    /// The footer's button and the failing check's button are mid-send.
    Sending,
    Sent,
    Error,
}

pub const SESSION: &str = "a-open";

fn run(name: &str, status: &str, conclusion: Option<&str>) -> Value {
    json!({"id": name.len(), "name": name, "status": status, "conclusion": conclusion, "htmlUrl": format!("https://ci.example.test/{name}"),
        "workflowName": "CI", "startedAt": null, "completedAt": null})
}

fn with(mut wire: Value, relationship: &str) -> Value {
    wire["relationship"] = relationship.into();
    wire
}

/// An invented issue link.
pub fn issue(number: u32, relationship: &str) -> Value {
    json!({
        "id": format!("link-{number}"), "sessionId": SESSION, "url": format!("https://github.com/example/demo/issues/{number}"),
        "providerId": "github", "resourceType": "issue", "resourceId": format!("example/demo#{number}"),
        "title": format!("example/demo #{number}: Thumbnails reload too often"), "status": "open", "statusLabel": "Open",
        "metadataJson": json!({"owner": "example", "repo": "demo", "number": number}).to_string(), "isDismissed": false,
        "isTerminal": false, "createdAt": "2024-10-03T10:00:00Z", "updatedAt": "2024-10-04T11:00:00Z", "relationship": relationship,
        "enrichmentStatus": "resolved", "lastCheckedAt": "2024-10-04T11:58:00Z",
    })
}

/// A pull request with a failing check and an unresolved thread.
pub fn failing(number: u32, relationship: &str) -> Value {
    let mut wire = with(
        pull_request(
            SESSION,
            number,
            "open",
            vec![run("lint", "completed", Some("success")), run("e2e", "completed", Some("failure"))],
            None,
        ),
        relationship,
    );
    let mut meta: Value = serde_json::from_str(wire["metadataJson"].as_str().unwrap()).unwrap();
    meta["reviewThreads"] = json!({"unresolvedCount": 1, "threads": [
        {"threadNodeId": "t1", "isResolved": false, "isOutdated": false, "path": "src/cache.rs", "line": 12, "comments": []}]});
    wire["metadataJson"] = meta.to_string().into();
    wire
}

fn thread(id: &str, path: &str, line: u32, author: &str, body: &str) -> Value {
    json!({"threadNodeId": id, "isResolved": false, "isOutdated": false, "path": path, "line": line, "comments": [
        {"id": format!("c-{id}"), "databaseId": 1, "body": body, "authorLogin": author, "createdAt": "2024-10-04T09:00:00Z",
         "url": format!("https://github.com/example/demo/pull/187#discussion_{id}")}]})
}

/// Fills in what the popover's header shows: branches, the diff stat, the reviewers and, as asked, the threads.
fn popover_pr(status: &str, runs: Vec<Value>, review: Option<&str>, threads: Vec<Value>, changes: bool, conflict: bool) -> Value {
    let mut wire = pull_request(SESSION, 187, status, runs, review);
    wire["title"] = "example/demo #187: Cache thumbnails between reloads".into();
    let mut meta: Value = serde_json::from_str(wire["metadataJson"].as_str().unwrap()).unwrap();
    meta["headRef"] = "fix/thumbnail-cache".into();
    meta["baseRef"] = "main".into();
    meta["additions"] = 412.into();
    meta["deletions"] = 88.into();
    meta["reviewers"] = json!([
        {"login": "mira", "state": if changes { "CHANGES_REQUESTED" } else { "APPROVED" }},
        {"login": "jonas", "state": if review == Some("APPROVED") { "APPROVED" } else { "REQUESTED" }},
    ]);
    if !threads.is_empty() {
        meta["reviewThreads"] = json!({"unresolvedCount": threads.len(), "threads": threads});
    }
    if conflict {
        meta["mergeable"] = false.into();
    }
    wire["metadataJson"] = meta.to_string().into();
    wire
}

fn links(case: Case) -> Vec<Value> {
    let pinned = |n| with(pull_request(SESSION, n, "open", vec![], None), "pinned");
    match case {
        Case::Chips => vec![issue(42, "origin"), failing(187, "own"), pinned(9), issue(7, "pinned")],
        Case::Open => {
            vec![pull_request(SESSION, 187, "open", vec![run("lint", "completed", Some("success")), run("e2e", "in_progress", None)], None)]
        }
        Case::Blocked | Case::Automation => vec![failing(187, "own")],
        Case::Draft => vec![pull_request(SESSION, 187, "draft", vec![run("lint", "queued", None)], None)],
        Case::Merged => vec![pull_request(SESSION, 187, "merged", vec![], None)],
        Case::Closed => vec![pull_request(SESSION, 187, "closed", vec![], None)],
        Case::Pending => {
            let mut wire = pull_request(SESSION, 187, "open", vec![], None);
            wire["enrichmentStatus"] = "pending".into();
            vec![wire]
        }
        Case::PopBlocked => vec![popover_pr(
            "open",
            vec![
                run("lint", "completed", Some("success")),
                run("build", "in_progress", None),
                run("e2e", "completed", Some("failure")),
                run("unit", "completed", Some("failure")),
            ],
            None,
            vec![
                thread(
                    "t1",
                    "src/cache.rs",
                    12,
                    "mira",
                    "This keeps every thumbnail for the whole session. Can the cache have a size limit?",
                ),
                thread("t2", "src/cache.rs", 48, "mira", "Please name this after what it holds."),
            ],
            true,
            true,
        )],
        Case::PopChecks => vec![popover_pr(
            "open",
            vec![run("lint", "completed", Some("success")), run("build", "in_progress", None), run("e2e", "completed", Some("failure"))],
            None,
            vec![],
            false,
            false,
        )],
        Case::PopReview => vec![popover_pr(
            "open",
            vec![run("lint", "completed", Some("success")), run("build", "completed", Some("success"))],
            None,
            vec![
                thread(
                    "t1",
                    "src/cache.rs",
                    12,
                    "mira",
                    "This keeps every thumbnail for the whole session. Can the cache have a size limit?",
                ),
                thread("t2", "src/cache.rs", 48, "jonas", "Please name this after what it holds."),
            ],
            false,
            false,
        )],
        Case::PopMerged => vec![popover_pr("merged", vec![], Some("APPROVED"), vec![], false, false)],
        Case::PopReady => vec![popover_pr(
            "open",
            vec![run("lint", "completed", Some("success")), run("build", "completed", Some("success"))],
            Some("APPROVED"),
            vec![],
            false,
            false,
        )],
        Case::Overflow => vec![failing(187, "own"), pinned(9), pinned(8), issue(7, "pinned"), issue(6, "pinned")],
    }
}

impl State {
    /// The popover open on `case`, with `pop` on its buttons.
    pub fn show_popover(&mut self, case: Case, dark: bool, pop: Pop) {
        self.show_chips(case, dark);
        self.ui.open = Some(crate::ui::Open::Popover("pr"));
        let id = self.session().id.clone();
        let key = crate::fleet::send_to_agent::check_key("e2e");
        match pop {
            Pop::Plain => {}
            Pop::Sending => {
                self.agent_sends.show_sending(&id, &key);
                self.agent_sends.show_sending(&id, crate::fleet::send_to_agent::FIX);
            }
            Pop::Sent => {
                self.agent_sends.show_sent(&id, &key);
                self.agent_sends.show_sent(&id, crate::fleet::send_to_agent::FIX);
            }
            Pop::Error => self.agent_sends.show_error(&id, &crate::fleet::send_to_agent::refused(502)),
        }
    }

    /// The open session with its links held, as if the server had sent them.
    pub fn show_chips(&mut self, case: Case, dark: bool) {
        self.show_header(super::header::Case::Fork, dark, true);
        let session = &mut self.projects[0].sessions[0];
        session.row.origin = matches!(case, Case::Automation | Case::Overflow).then(|| Origin {
            source_type: "automation".into(),
            title: Some("Nightly dependency sweep".into()),
            resource_url: None,
            resource_id: Some("auto-1".into()),
            provider_id: "builtin.automation".into(),
        });
        let id = session.id.clone();
        self.smart_links.set_links(&id, &links(case));
    }
}
