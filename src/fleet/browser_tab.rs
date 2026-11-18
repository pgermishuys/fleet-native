//! What a browser tab's canvas says (component 14, piece G; row 14.50). Ports `browserPage` of `lib/server-canvas.ts`: the
//! page the agent or the user asked for and the app that serves it, read from the canvas's `state` with anything missing or
//! of the wrong type empty.
use super::canvases::CanvasInstance;
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BrowserPage {
    /// The address on Fleet's machine; empty while the tab waits for an app to serve one.
    pub url: String,
    /// The app Fleet runs for the tab, when it has one.
    pub app_id: Option<String>,
    pub title: String,
}

/// `browserPage`: `url` when a string, else empty; `appId` only when a non-empty string.
pub fn browser_page(state: &Value) -> BrowserPage {
    BrowserPage {
        url: state.get("url").and_then(Value::as_str).unwrap_or_default().to_string(),
        app_id: state.get("appId").and_then(Value::as_str).filter(|id| !id.is_empty()).map(str::to_string),
        title: String::new(),
    }
}

impl CanvasInstance {
    /// A browser tab's page, with the tab's own title (`CanvasInstance.browser`); `None` for any other tab.
    pub fn browser(&self) -> Option<BrowserPage> {
        if self.kind != super::canvases::CanvasKind::Browser {
            return None;
        }
        let state = self.server.as_ref().map_or(&Value::Null, |k| &k.state);
        Some(BrowserPage { title: self.title.clone().unwrap_or_default(), ..browser_page(state) })
    }
}
