//! A conversation with pages shown in it, for the page card and full size (component 14, piece F): the same JSON a
//! `fleet_page_show` call leaves, read by the code a live conversation goes through.
use super::tools::{call, message};
use crate::State;
use crate::fleet::reducer;
use serde_json::json;

fn shown(id: &str, title: &str, source: &str, entry: &str, page: &str) -> serde_json::Value {
    call(
        &format!("p{id}"),
        "fleet_page_show",
        "completed",
        json!({"path": source, "placement": "conversation", "title": title}),
        json!({"title": title, "output": format!("Showing \"{title}\" in the conversation, above your reply."),
            "metadata": {"canvasId": null, "version": null, "page": {"id": page, "entry": entry}}}),
    )
}

/// One page shown, then the same file shown again after a fix (so only the second draws), then another turn's page.
pub fn of_pages() -> serde_json::Value {
    let said = |id: &str, body: &str| json!({"type": "text", "id": id, "text": body});
    let messages = vec![
        message("u1", "user", 1, vec![said("u1-t", "Chart how long the tests took this week.")]),
        message(
            "a1",
            "assistant",
            2,
            vec![
                shown("1", "Test times (draft)", "/work/demo/charts/times.html", "times.html", "pg_0123456789abcdef0123456789abcdef"),
                shown("2", "Test times by day", "/work/demo/charts/times.html", "times.html", "pg_0123456789abcdef0123456789abcde0"),
                said("a1-t", "The slow days are Tuesday and Thursday, when the integration suite runs."),
            ],
        ),
        message("u2", "user", 3, vec![said("u2-t", "Now the flaky ones.")]),
        message(
            "a2",
            "assistant",
            4,
            vec![
                shown(
                    "3",
                    "Flaky tests, ranked by how often they failed over the last thirty days of runs on the main branch",
                    "/work/demo/charts/flaky.html",
                    "flaky.html",
                    "pg_0123456789abcdef0123456789abcde1",
                ),
                said("a2-t", "Three tests account for most of the failures."),
            ],
        ),
    ];
    json!({"session": {}, "messages": messages, "delegations": [], "activityStatus": "idle"})
}

impl State {
    /// The conversation with its page cards, as the session of the scene.
    pub fn show_page_cards(&mut self, dark: bool) {
        self.dark = Some(dark);
        self.web.mock_base = Some("http://127.0.0.1:6411".into());
        self.set_open((0, 0));
        if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
            session.status = crate::mock::Status::Idle;
        }
        // Through the stream, so the fold plan knows which show of a page a later one replaced.
        self.stream = Some(reducer::create_state(&of_pages()));
        self.stream_of = None;
        self.sync_open();
    }
}
