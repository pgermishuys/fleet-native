//! Scenes of an item's page (`github-pull*`, `github-issue`, `github-item-*`): a pull request with a session on it, with failing
//! checks and open threads to send, with the menu open, an issue, and the loading and failed states, drawn from invented data with
//! no server (`acme/rocket`, signed in as `mara-quinn`).
use super::rows_scene;
use crate::State;
use crate::fleet::send_to_agent::FIX;
use crate::plugins::github::item_page::Page;
use crate::plugins::github::items::ItemDetail;
use crate::plugins::github::{Kind, Where, mock, state};
use serde_json::json;

/// The invented session `sess-working` is on #218; the failing check and the open thread arrive on its link's metadata.
pub fn failing_link_for_tests(s: &mut State) {
    failing_link(s)
}

fn failing_link(s: &mut State) {
    let meta = json!({
        "ci": {"headSha": "a1b2c3d", "ciStatus": "failure", "checkRuns": [
            {"id": 13, "name": "test", "status": "completed", "conclusion": "failure", "htmlUrl": "https://github.com/acme/rocket/runs/13",
             "workflowName": "CI", "startedAt": "2026-11-19T14:00:00Z", "completedAt": "2026-11-19T14:09:00Z"},
            {"id": 11, "name": "build", "status": "completed", "conclusion": "success", "htmlUrl": "https://github.com/acme/rocket/runs/11",
             "workflowName": "CI", "startedAt": "2026-11-19T14:00:00Z", "completedAt": "2026-11-19T14:04:00Z"},
        ]},
        "reviewThreads": {"threads": [{"threadNodeId": "T1", "isResolved": false, "isOutdated": false, "path": "src/retry.rs", "line": 41,
            "comments": [{"id": "c1", "databaseId": 1, "body": "Please add a test.", "authorLogin": "kit-lee", "createdAt": "2026-11-18T11:00:00Z",
                          "url": "https://github.com/acme/rocket/pull/218#r1"}]}]},
    });
    s.smart_links.upsert_link(&json!({
        "id": "link-sess-working", "sessionId": "sess-working", "url": "https://github.com/acme/rocket/pull/218", "providerId": "github",
        "resourceType": "pull_request", "resourceId": "acme/rocket#218", "title": "Queue retries with backoff", "status": "open",
        "statusLabel": "Open", "metadataJson": meta.to_string(), "isDismissed": false, "isTerminal": false,
        "createdAt": "2026-11-18T00:00:00Z", "updatedAt": "2026-11-18T00:00:00Z", "relationship": "own", "enrichmentStatus": "resolved",
        "lastCheckedAt": null,
    }));
}

pub fn set(s: &mut State, which: &str, dark: bool) {
    s.dark = Some(dark);
    s.rail = crate::Rail::Plugin(crate::plugins::github::ID);
    rows_scene::link(s);
    let issue = which.starts_with("issue");
    let (kind, number, detail) = if issue { (Kind::Issue, 7, mock::issue_detail()) } else { (Kind::Pull, 218, mock::pull_detail()) };
    let g = state(&mut s.places);
    g.panel_asked = true;
    g.home_asked = true;
    g.auth.asked = true;
    g.auth.connected = true;
    g.bookmarks.has_loaded = true;
    g.at = Where::Item { owner: "acme".into(), repo: "rocket".into(), kind, number };
    let key = ("acme".to_string(), "rocket".to_string(), kind, number);
    g.item = match which {
        "item-loading" => Page::show(key, None, true, None),
        "item-error" => Page::show(key, None, false, Some("Connect GitHub in Settings to see this.")),
        _ => Page::show(key, Some(ItemDetail::read(&detail)), false, None),
    };
    if which.ends_with("-narrow") {
        // A scene has no window to narrow: the width a 760 px window leaves the page, for the one-column layout.
        state(&mut s.places).item.pinned_width = Some(700.0);
    }
    match which {
        "pull-fix" | "pull-fix-sent" | "pull-fix-failed" => failing_link(s),
        "pull-menu" => state(&mut s.places).item.menu = true,
        _ => {}
    }
    if which == "pull-fix-sent" {
        s.agent_sends.show_sent("sess-working", FIX);
    }
    if which == "pull-fix-failed" {
        s.agent_sends.show_error("sess-working", "Couldn't send to the agent (HTTP 502).");
    }
}
