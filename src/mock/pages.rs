//! The page-pick card's scenes (component 4, piece F): a page canvas written as a server would send it (invented), read by the
//! same reader the live path uses.
use super::Status;
use crate::State;
use crate::fleet::pages::read_page_canvas;
use crate::fleet::reducer;
use serde_json::json;

#[derive(Clone, Copy)]
pub enum Case {
    /// The agent waits for a pick: the card.
    Waiting,
    /// A Fleet restart left "waiting" behind and the session is idle: no card.
    Stale,
}

impl State {
    /// The open session has a page waiting on a pick under a short conversation.
    pub fn show_page_pick(&mut self, case: Case) {
        self.set_open((0, 0));
        let text = |id: &str, body: &str| json!({"type": "text", "id": id, "text": body});
        let message = |id: &str, role: &str, at: i64, parts: Vec<serde_json::Value>| json!({"info": {"id": id, "role": role, "sessionID": "mock", "time": {"created": at}}, "parts": parts});
        let activity = if matches!(case, Case::Waiting) { "waiting_input" } else { "idle" };
        let messages = vec![
            message("u1", "user", 1_000, vec![text("u1-t", "show-page")]),
            message("a1", "assistant", 2_000, vec![text("a1-t", "Here are two layouts. Pick the one you like.")]),
        ];
        let state = reducer::create_state(&json!({"session": {}, "messages": messages, "delegations": [], "activityStatus": activity}));
        if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
            session.status = if matches!(case, Case::Waiting) { Status::NeedsInput } else { Status::Idle };
        }
        self.pages.set("mock.0.0", read_page_canvas(&page_canvas()).into_iter().collect());
        self.now_ms = Some(super::failure::NOW);
        self.stream = Some(state);
        self.stream_of = None;
        self.sync_open();
    }
}

impl State {
    /// The same page opened in the panel (component 14, piece A), with a Fleet to open it from in the system's browser.
    pub fn show_page_fallback(&mut self) {
        self.show_page_pick(Case::Waiting);
        self.web.mock_base = Some("http://127.0.0.1:6411".into());
        // The server would list the canvas, so it is a tab (component 10, piece C).
        let tabs = crate::fleet::canvases::read_server_canvas(&page_canvas()).into_iter().collect();
        self.canvases.set_server_canvases("mock.0.0", tabs);
        self.open_page("page_1");
    }
}

/// The page canvas the scenes' agent showed, as the server sends it.
fn page_canvas() -> serde_json::Value {
    json!({
        "canvasId": "page_1", "kind": "page", "title": "Layouts for pocket-notes", "version": 3,
        "state": {"pageId": "p1", "entry": "index.html", "choice": {"callId": "call_1", "status": "waiting", "at": "2026-10-07T01:50:00Z"}},
    })
}
