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

/// What the page tab's scenes show (component 14, piece C): the bar's source and warnings, and each state of the choice bar.
#[derive(Clone, Copy)]
pub enum TabCase {
    Waiting,
    Proposed,
    Refused,
    Answered,
    Skipped,
    /// A wait left behind by a Fleet restart, with the session idle.
    Stale,
    /// A request that Fleet refused.
    Failed,
    Warnings,
    Label,
    LongPath,
}

impl State {
    /// The page tab with the bar, drawn from a page canvas written as the server sends it.
    pub fn show_page_tab(&mut self, case: TabCase) {
        self.show_page_pick(if matches!(case, TabCase::Stale) { Case::Stale } else { Case::Waiting });
        self.web.mock_base = Some("http://127.0.0.1:6411".into());
        let at = "2026-10-07T01:50:00Z";
        let mut state = json!({"pageId": "p1", "entry": "index.html", "source": "/work/pocket-notes/layouts/index.html", "files": 3, "bytes": 4210, "shownAt": "2026-10-07T01:49:00Z"});
        let choice = |status: &str, extra: serde_json::Value| {
            let mut c = json!({"callId": "call_1", "status": status, "at": at});
            c.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            c
        };
        state["choice"] = match case {
            TabCase::Answered => choice("answered", json!({"value": "b", "summary": "Paged table"})),
            TabCase::Skipped => choice("not-answered", json!({"reason": "skipped"})),
            _ => choice("waiting", json!({})),
        };
        match case {
            TabCase::Warnings => {
                state["warnings"] = json!([
                    "fonts/inter.woff2 is outside the page's folder, so it was not copied",
                    "https://cdn.example.test/chart.js is on another site and may not load",
                    "images/logo.png was not found next to the page"
                ]);
            }
            TabCase::Label => state["label"] = json!("Walkthrough \u{b7} compared with main at 55650b2"),
            TabCase::LongPath => {
                state["source"] =
                    json!("/work/pocket-notes/docs/design/explorations/2026-autumn/layout-options/with-a-very-long-folder-name/index.html");
            }
            _ => {}
        }
        let mut canvas = page_canvas();
        canvas["state"] = state;
        let tabs = crate::fleet::canvases::read_server_canvas(&canvas).into_iter().collect();
        self.canvases.set_server_canvases("mock.0.0", tabs);
        self.pages.set("mock.0.0", read_page_canvas(&canvas).into_iter().collect());
        self.open_page("page_1");
        let tab = "canvas:page_1".to_string();
        let ui = self.web.choices.entry(tab).or_default();
        ui.call_id = "call_1".into();
        match case {
            TabCase::Proposed => ui.proposal = Some(("b".into(), "Paged table".into())),
            TabCase::Refused => ui.refusal = "That pick is too long to send (over 16,000 characters). Pick again.".into(),
            TabCase::Failed => {
                ui.proposal = Some(("b".into(), "Paged table".into()));
                ui.error = "The agent is no longer waiting for this pick.".into();
            }
            _ => {}
        }
    }
}

/// The page canvas the scenes' agent showed, as the server sends it.
fn page_canvas() -> serde_json::Value {
    json!({
        "canvasId": "page_1", "kind": "page", "title": "Layouts for pocket-notes", "version": 3,
        "state": {"pageId": "p1", "entry": "index.html", "choice": {"callId": "call_1", "status": "waiting", "at": "2026-10-07T01:50:00Z"}},
    })
}
