//! Scenes and fixtures for the browser tab's Your view / Agent's view and the "Open a browser tab" dialog (component 14, piece
//! J). The scripted model never drives the agent's browser, so the agent's tab is made here: the invented shop page, handed
//! over as Fleet's `.../frame` answer would carry it.
use super::browser_tab::{BrowserCase, PAGE as SITE};
use super::failure::{NOW, chrono_free_iso};
use super::images::PAGE;
use crate::State;
use crate::conv::agent_tabs::AgentTab;
use crate::conv::browser::BrowserStep;
use crate::fleet::agent_tab::frame_key;
use crate::web::browser_open::BrowserOpen;
use serde_json::json;

pub const AGENT_TAB: &str = "tab_1";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AgentCase {
    /// The user's own view, with the banner offering the agent's.
    You,
    /// Agent's view: the live picture and its banner.
    Watching,
    /// A narrow panel: "You" and "Agent" in the toggle.
    Narrow,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpenCase {
    /// Nothing typed yet, with the commands used here before.
    Empty,
    /// The command that last served a page, filled in, and the commands used here before.
    Command,
    /// An address typed.
    Address,
    /// Fleet refused.
    Error,
    /// The request is on its way.
    Busy,
}

fn step(seq: i64, kind: &str, ago_ms: i64, summary: &str, bounds: Option<serde_json::Value>) -> Option<BrowserStep> {
    let mut v = json!({
        "sessionId": "mock.0.0", "seq": seq, "kind": kind, "summary": summary, "detail": "", "ok": true, "tabId": AGENT_TAB,
        "url": SITE, "title": "Lantern & Loom", "at": chrono_free_iso(NOW - ago_ms),
    });
    if let Some(b) = bounds {
        v["box"] = b;
    }
    BrowserStep::read(&v)
}

impl State {
    /// The browser tab on the invented site, with the agent trying the same site in its own tab.
    pub fn show_browser_agent(&mut self, case: AgentCase, dark: bool) {
        self.dark = Some(dark);
        self.show_browser_tab(BrowserCase::Shown);
        self.now_ms = Some(NOW);
        self.right.width = if case == AgentCase::Narrow { 440.0 } else { 600.0 };
        let session = self.canvas_session();
        let moss = json!({"x": 464, "y": 250, "width": 361, "height": 250});
        for s in [
            step(1, "tabs.open", 60_000, "Opened http://localhost:5173/", None),
            step(2, "click", 2_000, "Clicked \u{201c}Moss lamp\u{201d}", Some(moss)),
        ]
        .into_iter()
        .flatten()
        {
            self.browser.apply(&session, s);
        }
        let tab = AgentTab::read(&json!({"id": AGENT_TAB, "url": SITE, "title": "Lantern & Loom"})).expect("a tab with an id");
        self.browser.set_tabs(&session, vec![tab], Some(AGENT_TAB.into()));
        if case == AgentCase::Watching {
            self.browser_set_view(super::browser_tab::TAB, true);
            self.supply_image(&frame_key(&session, AGENT_TAB, 0), PAGE);
        }
    }

    /// The + menu's Browser dialog over the browser tab.
    pub fn show_browser_open(&mut self, case: OpenCase, dark: bool) {
        self.dark = Some(dark);
        self.show_browser_tab(BrowserCase::Shown);
        let known = ["npm run dev", "npm run preview"].map(String::from).to_vec();
        let (value, suggestions) = match case {
            OpenCase::Empty => ("", known),
            OpenCase::Address => ("5173", known),
            OpenCase::Error => ("99999", known),
            _ => ("npm run dev", known),
        };
        self.web.open = Some(BrowserOpen {
            session: self.canvas_session(),
            value: rune::TextEdit::new(value),
            suggestions,
            touched: false,
            busy: case == OpenCase::Busy,
            error: (case == OpenCase::Error)
                .then(|| "\"url\" must be an http or https address on this machine, e.g. http://localhost:5173/.".into()),
        });
    }
}
