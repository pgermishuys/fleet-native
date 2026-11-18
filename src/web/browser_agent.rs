//! The browser tab's Your view / Agent's view (component 14, piece J; row 14.67): `BrowserCanvas.vue`'s `agentTab`, `view` and
//! the `watch(agentTab)` that puts the view back to the user's own when the agent's tab goes. The picture itself is component
//! 10's (`view::agent_tab`, `fleet::agent_tab`); this decides which tab is watched and when.
use crate::State;
use crate::conv::agent_tabs::AgentTab;
use crate::fleet::browser_tab::BrowserPage;
use crate::fleet::canvases::CanvasKind;

impl State {
    /// The agent's tab on the page this tab shows (`tabFor(sessionId, target.href ?? (url || app.url))`).
    pub fn browser_agent_tab(&self, tab: &str, page: &BrowserPage) -> Option<AgentTab> {
        let own = self.browser_ui(tab).and_then(|u| u.target.as_ref()).map(|u| u.as_str().to_string()).filter(|u| !u.is_empty());
        let url = own.unwrap_or_else(|| self.browser_page_of(page));
        self.browser.tab_for(&self.canvas_session(), Some(&url)).cloned()
    }

    /// The agent's tab, while the user is looking at it (Agent's view chosen and the agent still has the tab).
    pub fn browser_watching(&self, tab: &str, page: &BrowserPage) -> Option<AgentTab> {
        self.browser_ui(tab).filter(|u| u.agent)?;
        self.browser_agent_tab(tab, page)
    }

    /// The toggle: `view = 'you' | 'agent'`.
    pub fn browser_set_view(&mut self, tab: &str, agent: bool) {
        self.web.browsers.entry(tab.to_string()).or_default().agent = agent;
        self.keep_browser_agent();
    }

    /// Watches the agent's tab while the browser tab on screen is in Agent's view, and stops when it isn't. Puts a tab back to
    /// Your view when the agent has no tab on its page any more. True when something to draw changed.
    pub fn keep_browser_agent(&mut self) -> bool {
        if !self.web.agent_owned && !self.web.browsers.values().any(|u| u.agent) {
            return false;
        }
        let tabs = self.open_canvases();
        let mut changed = false;
        let mut want = None;
        for c in tabs.canvases.iter().filter(|c| c.kind == CanvasKind::Browser) {
            let Some(page) = c.browser() else { continue };
            if !self.browser_ui(&c.id).is_some_and(|u| u.agent) {
                continue;
            }
            let found = self.browser_agent_tab(&c.id, &page);
            if found.is_none() {
                self.web.browsers.entry(c.id.clone()).or_default().agent = false;
                changed = true;
            } else if tabs.active_id == c.id && self.rail == crate::Rail::Sessions && self.content_open && !self.browser_elsewhere() {
                want = found.map(|a| (self.canvas_session(), a.id));
            }
        }
        match want {
            Some((session, tab)) => {
                self.web.agent_owned = true;
                self.watch_agent_tab(&session, &tab);
            }
            None if self.web.agent_owned => {
                self.web.agent_owned = false;
                self.stop_watching_agent_tab();
                changed = true;
            }
            None => {}
        }
        changed
    }
}
