//! The browser-steps scenes (piece 3O): a Code Mode call that drove the agent's browser, finished and still running.
//! The steps are handed over as Fleet's `agent-browser` answer would carry them.
use super::failure::NOW;
use super::images::PAGE;
use super::tools::{call, message};
use crate::State;
use crate::conv::browser::BrowserStep;
use crate::mock::Status;
use serde_json::{Value, json};

/// The session the scene's steps belong to.
const SESSION: &str = "scene";
/// Where the scene's screenshot step is kept.
const SHOT_PATH: &str = "/api/sessions/scene/screenshots/shot_2";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// A finished call: four steps, one of them a screenshot and one a failure.
    Done,
    /// A call still running, one step in.
    Running,
}

fn step(seq: i64, summary: &str, detail: &str, error: Option<&str>, shot: bool) -> Value {
    let mut v = json!({"sessionId": SESSION, "seq": seq, "kind": "click", "summary": summary, "detail": detail,
        "ok": error.is_none(), "callId": "call-c1"});
    if let Some(e) = error {
        v["error"] = json!(e);
    }
    if shot {
        v["title"] = json!("Shop · 1280×800");
        v["screenshot"] = json!({"id": "shot_2", "width": 1280, "height": 800});
    }
    v
}

impl State {
    pub fn show_browser(&mut self, case: Case) {
        self.open = (0, 0);
        let running = case == Case::Running;
        let text = |id: &str, body: &str| json!({"type": "text", "id": id, "text": body});
        let code = "const tab = await browser.open('http://localhost:5173/');\nawait tab.click('Add to cart');\nawait tab.screenshot();\nawait tab.click('Checkout');";
        let run = call("c1", "execute", if running { "running" } else { "completed" }, json!({"code": code}), json!({}));
        let mut parts = vec![run];
        if !running {
            parts.push(text("a1-t", "The cart took the item, but the Checkout button isn't on the page yet."));
        }
        let messages = vec![
            message("u1", "user", NOW - 120_000, vec![text("u1-t", "Add the first product to the cart and go to checkout.")]),
            message("a1", "assistant", NOW - 100_000, parts),
        ];
        let snapshot =
            json!({"session": {}, "messages": messages, "delegations": [], "activityStatus": if running { "busy" } else { "idle" }});
        self.stream = Some(crate::fleet::reducer::create_state(&snapshot));
        if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
            session.id = SESSION.into();
            session.status = if running { Status::Working } else { Status::Idle };
            session.row.activity = if running { "busy" } else { "idle" }.into();
        }
        let all = [
            step(1, "Opened http://localhost:5173/", "tabs.open · 420 ms", None, false),
            step(2, "Clicked “Add to cart”", "click @e2 · 170 ms", None, false),
            step(3, "Took a screenshot of the page", "screenshot · 310 ms", None, true),
            step(4, "Couldn't click “Checkout”", "click @e9 · 5.0 s", Some("No element matches @e9 on this page."), false),
        ];
        let shown = if running { &all[..1] } else { &all[..] };
        self.browser.set_from_server(SESSION, shown.iter().filter_map(BrowserStep::read).collect());
        self.now_ms = Some(NOW);
        self.stream_of = None;
        // A finished turn folds its calls away; the scene shows it open.
        self.turns_open = ["u1".to_string()].into_iter().collect();
        self.sync_open();
        if !running {
            self.supply_image(SHOT_PATH, PAGE);
        }
    }
}
