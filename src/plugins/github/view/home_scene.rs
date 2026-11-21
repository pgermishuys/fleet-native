//! Scenes of Your work and its connect section (`github-home*`, `github-connect*`): each state the page can be in, drawn from invented
//! data with no server (owners `acme`, `lumen-labs`; signed in as `mara-quinn`).
use super::panel_scene::{self, Which};
use crate::State;
use crate::mock::Status;
use crate::plugins::github::auth::Device;
use crate::plugins::github::items::Work;
use crate::plugins::github::{mock, state};
use serde_json::json;

/// The sessions on the page: (id, title, status, the pull request it is on, minutes ago). The first is on a pull request that needs
/// the user, the second on one that is in no list but In Fleet.
const SESSIONS: [(&str, &str, Status, &str, i64); 2] = [
    ("sess-retry", "Retry the launch hook", Status::Idle, "acme/rocket#221", 25),
    ("sess-audit", "Paginate the audit log", Status::Idle, "acme/rocket#215", 2),
];

pub fn set(s: &mut State, which: &str, dark: bool) {
    panel_scene::scene(s, dark, if which.starts_with("connect") { Which::Off } else { Which::Followed });
    s.now_ms = Some(1_795_176_000_000); // 20 Nov 2026, noon UTC
    // Two followed repositories, as the stand-in's fixture has.
    state(&mut s.places).bookmarks.list.pop();
    match which {
        "home" => home(s),
        "home-loading" => {
            let g = state(&mut s.places);
            g.work.work = None;
            g.work.loading = true;
        }
        "home-error" => {
            let g = state(&mut s.places);
            g.work.work = None;
            g.work.error = Some("GitHub rate limit exceeded. Try again in a few minutes.".into());
        }
        "home-empty" => {
            home(s);
            let g = state(&mut s.places);
            g.work.work = Some(Work { login: Some("mara-quinn".into()), ..Work::default() });
            s.smart_links.header_links_loaded(&[]);
        }
        "connect" => {}
        "connect-waiting" => {
            let at = s.clock + 14.0 * 60.0;
            state(&mut s.places).auth.device = Device::Awaiting {
                user_code: "WDJB-MJHT".into(),
                verification_uri: "https://github.com/login/device".into(),
                device_code: "scene".into(),
                expires_at: at,
                interval: 5,
            };
        }
        "connect-expired" => state(&mut s.places).auth.device = Device::Expired,
        other => panic!("no Your work scene called {other}"),
    }
}

/// Your work loaded just now, with a session on two of its pull requests.
fn home(s: &mut State) {
    let now = s.now_ms.unwrap_or_default();
    let wires: Vec<_> = SESSIONS
        .iter()
        .map(|(id, title, _, on, _)| {
            let number: u64 = on.rsplit('#').next().and_then(|n| n.parse().ok()).unwrap_or_default();
            json!({
                "id": format!("link-{id}"), "sessionId": id, "url": format!("https://github.com/acme/rocket/pull/{number}"),
                "providerId": "github", "resourceType": "pull_request", "resourceId": on, "title": format!("acme/rocket #{number}: {title}"),
                "status": "open", "statusLabel": "Open",
                "metadataJson": json!({"owner": "acme", "repo": "rocket", "number": number, "author": "kit-lee", "headRef": "audit-log-pages",
                    "additions": 96, "deletions": 12, "updatedAt": "2026-11-20T09:00:00Z"}).to_string(),
                "isDismissed": false, "isTerminal": false, "createdAt": "2026-11-19T00:00:00Z", "updatedAt": "2026-11-20T09:00:00Z",
                "relationship": "own", "enrichmentStatus": "resolved", "lastCheckedAt": null,
            })
        })
        .collect();
    s.smart_links.header_links_loaded(&wires);
    if let Some(project) = s.projects.first_mut() {
        for (n, (id, title, status, _, minutes)) in SESSIONS.iter().enumerate() {
            let Some(session) = project.sessions.get_mut(n) else { break };
            session.id = id.to_string();
            session.title = title.to_string();
            session.status = *status;
            session.ago_secs = None;
            session.row.updated = now - minutes * 60_000;
            session.row.archived = false;
        }
    }
    let clock = s.clock;
    let g = state(&mut s.places);
    g.work.work = Some(Work::read(&mock::work()));
    g.work.loaded_at = Some(clock);
}
