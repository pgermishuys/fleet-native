//! The agent's browser tabs and steps, kept current (`use-agent-browser.ts`): loaded with the session and after a reconnect from
//! `GET /api/sessions/{id}/agent-browser`, and added to by `browser.step` events. Steps aren't replayed after a
//! disconnect, which is why a reconnect loads them again.
use super::events::{Event, Kind};
use crate::State;
use crate::conv::agent_tabs::AgentTab;
use crate::conv::browser::BrowserStep;
use serde_json::Value;

impl State {
    /// Asks Fleet for the session's tabs and steps, off the UI thread.
    pub fn load_browser(&mut self, session: &str) {
        let Some(link) = self.link_for(session) else { return };
        let id = session.to_string();
        let path = format!("/api/sessions/{}/agent-browser", crate::conv::image_sniff::encode_component(session));
        link.off_thread(
            move |base| super::http::get(base, &path),
            move |s, _, result| {
                // A Fleet that can't say (an older one, an error) leaves the steps as the events made them.
                if let Ok(body) = result {
                    s.on_browser_loaded(&id, &body);
                }
            },
        );
    }

    /// What `GET .../agent-browser` said: the steps merged with those that arrived meanwhile, the tabs and focus replaced.
    /// Missing ones read as empty, as Fleet reads `body.tabs ?? []`.
    pub fn on_browser_loaded(&mut self, id: &str, body: &Value) {
        let steps = body.get("steps").and_then(Value::as_array).map(|a| a.iter().filter_map(BrowserStep::read).collect());
        self.browser.set_from_server(id, steps.unwrap_or_default());
        let tabs = body.get("tabs").and_then(Value::as_array).map(|a| a.iter().filter_map(AgentTab::read).collect());
        let focused = body.get("focusedTabId").and_then(Value::as_str).map(str::to_string);
        self.browser.set_tabs(id, tabs.unwrap_or_default(), focused);
        self.sync_open();
    }

    /// A `browser.step` of the open session. True when it was one.
    pub fn apply_browser_event(&mut self, id: &str, event: &Event) -> bool {
        let Event::Other(Kind::BrowserStep, props) = event else { return false };
        if props.get("sessionId").and_then(Value::as_str).is_some_and(|s| s != id) {
            return true;
        }
        if let Some(step) = BrowserStep::read(props) {
            self.browser.apply(id, step);
        }
        true
    }
}
