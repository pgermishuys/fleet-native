//! Scenes of an item's row and its session chip (`github-rows*`): one row of every kind and state, a session chip in each tone,
//! drawn from invented data with no server. The scene's page is the plain list the rows sit in until Your work (piece G) and a
//! repository's page (piece I) draw theirs.
use crate::State;
use crate::mock::Status;
use crate::plugins::github::items::ItemSummary;
use crate::plugins::github::{Where, mock, state};
use serde_json::json;

/// The sessions linked to the rows: (id, title, status, the item they are on, minutes ago, archived).
const SESSIONS: [(&str, &str, Status, &str, i64, bool); 5] = [
    ("sess-working", "Fix the retry backoff", Status::Working, "acme/rocket#218", 1, false),
    ("sess-waiting", "Round the fuel gauge", Status::NeedsInput, "acme/rocket#220", 3, false),
    ("sess-waiting-2", "Review gauge tests", Status::Idle, "acme/rocket#220", 30, false),
    ("sess-idle", "Hook retries", Status::Idle, "acme/rocket#221", 120, false),
    // Archived: never on a chip.
    ("sess-archived", "Old attempt", Status::Working, "acme/rocket#222", 600, true),
];

pub fn scene(s: &mut State, dark: bool) {
    s.dark = Some(dark);
    s.rail = crate::Rail::Plugin(crate::plugins::github::ID);
    s.now_ms = Some(1_795_176_000_000); // 20 Nov 2026, noon UTC: the invented items are a day or two old
    let now = s.now_ms.unwrap_or_default();
    let wires: Vec<_> = SESSIONS
        .iter()
        .map(|(id, title, _, on, _, _)| {
            json!({
                "id": format!("link-{id}"), "sessionId": id, "url": format!("https://github.com/{}", on.replace('#', "/pull/")),
                "providerId": "github", "resourceType": "pull_request", "resourceId": on, "title": title, "status": "open",
                "statusLabel": "Open", "metadataJson": null, "isDismissed": false, "isTerminal": false,
                "createdAt": "2026-11-18T00:00:00Z", "updatedAt": "2026-11-18T00:00:00Z", "relationship": "own",
                "enrichmentStatus": "resolved", "lastCheckedAt": null,
            })
        })
        .collect();
    s.smart_links.header_links_loaded(&wires);
    link_sessions(s, now);
    let g = state(&mut s.places);
    g.panel_asked = true;
    g.auth.connected = true;
    g.bookmarks.has_loaded = true;
    g.at = Where::Home;
    g.demo_rows = mock::rows()
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| (ItemSummary::read(&r["item"]), r["reason"].as_str().map(str::to_string)))
        .collect();
}

/// Makes the list's first sessions the ones the links name, with their status and age.
fn link_sessions(s: &mut State, now: i64) {
    let Some(project) = s.projects.first_mut() else { return };
    for (n, (id, title, status, _, minutes, archived)) in SESSIONS.iter().enumerate() {
        let Some(session) = project.sessions.get_mut(n) else { return };
        session.id = id.to_string();
        session.title = title.to_string();
        session.status = *status;
        session.ago_secs = None;
        session.row.updated = now - minutes * 60_000;
        session.row.archived = *archived;
    }
}
