//! Agent's view's scenes (piece 10M): the agent's own tab as a picture with a ring and a caption on its latest step, a
//! failed step, the first picture still on its way, a tab that is gone, and a narrow panel. The scripted model never drives the
//! agent's browser, so none of these can be made live; the picture is the invented shop page, handed over as Fleet's
//! `.../frame` answer would carry it. Drawn in the panel's Browser body until component 14's canvas hosts it.
use super::failure::{NOW, chrono_free_iso};
use super::images::PAGE;
use crate::State;
use crate::conv::agent_tabs::AgentTab;
use crate::conv::browser::BrowserStep;
use crate::fleet::agent_tab::frame_key;
use crate::fleet::canvases::{CanvasInstance, CanvasKind};
use serde_json::{Value, json};

const TAB: &str = "tab_1";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// A click a moment ago: the ring on the element and its caption.
    Ring,
    /// The same, for a click that failed: the caption in red.
    Failed,
    /// The first picture hasn't come yet.
    Loading,
    /// The agent's tab is closed: no picture was ever had.
    Closed,
    /// A narrow panel: the size tag is left out.
    Narrow,
}

fn step(session: &str, seq: i64, kind: &str, ago_ms: i64, summary: &str, ok: bool, bounds: Option<Value>) -> Value {
    let mut v = json!({
        "sessionId": session, "seq": seq, "kind": kind, "summary": summary, "detail": "", "ok": ok, "tabId": TAB,
        "url": "http://localhost:5173/", "title": "Lantern & Loom", "at": chrono_free_iso(NOW - ago_ms),
    });
    if let Some(b) = bounds {
        v["box"] = b;
    }
    v
}

impl State {
    pub fn show_agent_tab(&mut self, case: Case, dark: bool) {
        self.show_context_tab_scene(super::context_tab::Case::Links, dark);
        self.now_ms = Some(NOW);
        let session = self.canvas_session();
        self.content_open = true;
        self.right.width = if case == Case::Narrow { 280.0 } else { 600.0 };
        // A browser canvas is a server canvas (component 14's); the scene stands one in the strip and makes it active.
        let mut tabs = self.open_canvases();
        tabs.canvases.push(CanvasInstance {
            id: "canvas:cv_shop".into(),
            kind: CanvasKind::Browser,
            payload: None,
            title: Some("Shop".into()),
            file: None,
            server: None,
        });
        tabs.active_id = "canvas:cv_shop".into();
        self.canvases.replace(&session, tabs);
        let moss = json!({"x": 464, "y": 250, "width": 361, "height": 250});
        // A tab with nothing recent on it: no ring and no caption over an empty stage.
        let quiet = matches!(case, Case::Loading | Case::Closed);
        let steps = match case {
            Case::Failed => vec![
                step(&session, 1, "tabs.open", 60_000, "Opened http://localhost:5173/", true, None),
                step(&session, 2, "click", 2_000, "Couldn't click “Buy now”: no element matches @e9 on this page", false, None),
            ],
            _ => vec![
                step(&session, 1, "tabs.open", 60_000, "Opened http://localhost:5173/", true, None),
                step(&session, 2, "click", if quiet { 30_000 } else { 2_000 }, "Clicked “Moss lamp”", true, Some(moss)),
            ],
        };
        for v in &steps {
            if let Some(s) = BrowserStep::read(v) {
                self.browser.apply(&session, s);
            }
        }
        self.browser.set_tabs(
            &session,
            vec![AgentTab::read(&json!({"id": TAB, "url": "http://localhost:5173/", "title": "Lantern & Loom"})).unwrap()],
            Some(TAB.into()),
        );
        self.watch_agent_tab(&session, TAB);
        let key = frame_key(&session, TAB, 0);
        match case {
            Case::Loading => {}
            Case::Closed => self.on_image_bytes(&key, Err("HTTP 404".into())),
            _ => self.supply_image(&key, PAGE),
        }
    }
}
